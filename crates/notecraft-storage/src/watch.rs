//! Watching a notebook folder for changes made by other programs (docs/architecture.md
//! section 3.3).
//!
//! A `notify` watcher feeds raw events to a small thread, which waits until the folder has
//! been quiet for the debounce time ([`DEBOUNCE`], 300 ms) and then hands over the set of
//! changed paths. NoteCraft's own writes also arrive here; the caller tells them apart by
//! comparing file contents.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// How long a folder must be quiet before changes are reported.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Most paths held in one batch; more are dropped (the next event reports them again).
const MAX_BATCH: usize = 4096;

/// A running watcher. Dropping it stops the thread.
pub struct FolderWatcher {
    root: PathBuf,
    _watcher: RecommendedWatcher,
    changes: Receiver<Vec<PathBuf>>,
}

impl FolderWatcher {
    /// Watch `root` and everything under it. `wake` is called (from the watcher thread)
    /// whenever a batch is ready, so a UI can repaint. `debounce` is normally [`DEBOUNCE`].
    pub fn start(
        root: &Path,
        debounce: Duration,
        wake: impl Fn() + Send + 'static,
    ) -> notify::Result<Self> {
        let (raw_tx, raw_rx) = mpsc::channel::<PathBuf>();
        let (batch_tx, batch_rx) = mpsc::channel::<Vec<PathBuf>>();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| match event {
                Ok(event) => {
                    if event.kind.is_access() {
                        return;
                    }
                    for path in event.paths {
                        let _ = raw_tx.send(path);
                    }
                }
                Err(err) => tracing::warn!("file watcher error: {err}"),
            })?;
        watcher.watch(root, RecursiveMode::Recursive)?;
        std::thread::Builder::new()
            .name("notecraft-watch".to_owned())
            .spawn(move || debounce_loop(&raw_rx, &batch_tx, debounce, &wake))
            .map_err(notify::Error::io)?;
        Ok(Self {
            root: root.to_path_buf(),
            _watcher: watcher,
            changes: batch_rx,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Paths changed since the last call, each once, in path order. Never blocks.
    pub fn poll(&self) -> Vec<PathBuf> {
        let mut all = BTreeSet::new();
        while let Ok(batch) = self.changes.try_recv() {
            all.extend(batch);
        }
        all.into_iter().collect()
    }
}

fn debounce_loop(
    raw: &Receiver<PathBuf>,
    out: &Sender<Vec<PathBuf>>,
    debounce: Duration,
    wake: &dyn Fn(),
) {
    let mut pending: BTreeSet<PathBuf> = BTreeSet::new();
    loop {
        let next = if pending.is_empty() {
            raw.recv().map_err(|_| RecvTimeoutError::Disconnected)
        } else {
            raw.recv_timeout(debounce)
        };
        match next {
            Ok(path) => {
                if pending.len() < MAX_BATCH {
                    pending.insert(path);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                let batch: Vec<PathBuf> = std::mem::take(&mut pending).into_iter().collect();
                if out.send(batch).is_err() {
                    return;
                }
                wake();
            }
            Err(RecvTimeoutError::Disconnected) => {
                if !pending.is_empty() {
                    let _ = out.send(std::mem::take(&mut pending).into_iter().collect());
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn a_burst_of_writes_is_reported_after_the_quiet_time() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let watcher = FolderWatcher::start(&root, Duration::from_millis(150), || {}).unwrap();
        let file = root.join("a.md");
        for i in 0..5 {
            std::fs::write(&file, format!("v{i}")).unwrap();
        }
        let start = Instant::now();
        let mut seen = Vec::new();
        while seen.is_empty() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(50));
            seen = watcher.poll();
        }
        assert!(
            seen.iter().any(|p| p.file_name() == file.file_name()),
            "{seen:?}"
        );
    }
}
