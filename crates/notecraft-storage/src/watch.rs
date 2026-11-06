//! Watching a notebook folder for changes made by other programs (docs/architecture.md
//! section 3.3).
//!
//! A `notify` watcher feeds raw events to a small thread, which waits until the folder has
//! been quiet for the debounce time ([`DEBOUNCE`], 300 ms) and then hands over the set of
//! changed paths. NoteCraft's own writes also arrive here; the caller tells them apart by
//! comparing file contents.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use notify::{RecursiveMode, Watcher};

/// How long a folder must be quiet before changes are reported.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Most paths held in one batch; more are dropped (the next event reports them again).
const MAX_BATCH: usize = 4096;

/// How often a single file is polled where the native watcher cannot see edits to one file
/// (the kqueue systems: a kqueue watch on a folder only reports entries added or removed).
#[cfg(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
const FILE_POLL: Duration = Duration::from_millis(500);

/// A running watcher. Dropping it stops the thread.
pub struct FolderWatcher {
    root: PathBuf,
    /// The `notify` watcher, when a native one is used.
    _watcher: Option<Box<dyn Watcher + Send>>,
    /// Set on drop to end a polling thread.
    stop: Arc<AtomicBool>,
    changes: Receiver<Vec<PathBuf>>,
}

impl Drop for FolderWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl FolderWatcher {
    /// Watch `root` and everything under it. `wake` is called (from the watcher thread)
    /// whenever a batch is ready, so a UI can repaint. `debounce` is normally [`DEBOUNCE`].
    pub fn start(
        root: &Path,
        debounce: Duration,
        wake: impl Fn() + Send + 'static,
    ) -> notify::Result<Self> {
        Self::start_with(
            root,
            RecursiveMode::Recursive,
            None,
            debounce,
            WatcherKind::Native,
            wake,
        )
    }

    /// Watch the one file `file` (a loose page). Only its folder is watched, without
    /// descending into subfolders, and only events for that file name are reported, so a
    /// busy folder such as Downloads costs next to nothing. On the kqueue systems (FreeBSD,
    /// OpenBSD, NetBSD, DragonFly), where a folder watch never reports edits to a file in
    /// it, the file itself is polled instead ([`FILE_POLL`]), which also follows a save that
    /// replaces the file.
    pub fn start_file(
        file: &Path,
        debounce: Duration,
        wake: impl Fn() + Send + 'static,
    ) -> notify::Result<Self> {
        let (Some(folder), Some(name)) = (file.parent(), file.file_name()) else {
            return Err(notify::Error::generic("the file has no folder"));
        };
        #[cfg(any(
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ))]
        let (target, kind) = (file, WatcherKind::Poll(FILE_POLL));
        #[cfg(not(any(
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        )))]
        let (target, kind) = (folder, WatcherKind::Native);
        let mut started = Self::start_with(
            target,
            RecursiveMode::NonRecursive,
            Some(name.to_owned()),
            debounce,
            kind,
            wake,
        )?;
        started.root = folder.to_path_buf();
        Ok(started)
    }

    fn start_with(
        root: &Path,
        mode: RecursiveMode,
        only: Option<std::ffi::OsString>,
        debounce: Duration,
        kind: WatcherKind,
        wake: impl Fn() + Send + 'static,
    ) -> notify::Result<Self> {
        let (raw_tx, raw_rx) = mpsc::channel::<PathBuf>();
        let (batch_tx, batch_rx) = mpsc::channel::<Vec<PathBuf>>();
        let stop = Arc::new(AtomicBool::new(false));
        let watcher: Option<Box<dyn Watcher + Send>> = match kind {
            WatcherKind::Native => {
                let handler = move |event: notify::Result<notify::Event>| match event {
                    Ok(event) => {
                        if event.kind.is_access() {
                            return;
                        }
                        for path in event.paths {
                            if only
                                .as_ref()
                                .is_some_and(|name| path.file_name() != Some(name.as_os_str()))
                            {
                                continue;
                            }
                            let _ = raw_tx.send(path);
                        }
                    }
                    Err(err) => tracing::warn!("file watcher error: {err}"),
                };
                let mut watcher = notify::recommended_watcher(handler)?;
                watcher.watch(root, mode)?;
                Some(Box::new(watcher))
            }
            WatcherKind::Poll(every) => {
                let file = root.to_path_buf();
                let stop = Arc::clone(&stop);
                std::thread::Builder::new()
                    .name("notecraft-poll".to_owned())
                    .spawn(move || poll_loop(&file, every, &stop, &raw_tx))
                    .map_err(notify::Error::io)?;
                None
            }
        };
        std::thread::Builder::new()
            .name("notecraft-watch".to_owned())
            .spawn(move || debounce_loop(&raw_rx, &batch_tx, debounce, &wake))
            .map_err(notify::Error::io)?;
        Ok(Self {
            root: root.to_path_buf(),
            _watcher: watcher,
            stop,
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

/// How a watcher learns about changes.
enum WatcherKind {
    /// The platform's own `notify` backend (inotify, FSEvents, ReadDirectoryChanges, kqueue).
    Native,
    /// Polling one file's metadata at this interval (a small thread of our own: `notify`'s
    /// poller compares modified times in whole seconds, so it misses quick edits).
    #[cfg_attr(
        not(any(
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        )),
        allow(dead_code)
    )]
    Poll(Duration),
}

/// What a poll compares: size, modified time at full precision, and (on Unix) the file's
/// inode, so an edit in place and a save that replaces the file are both seen. `None` when
/// the file cannot be read (gone, or not allowed).
fn file_stamp(file: &Path) -> Option<(u64, Option<SystemTime>, u64)> {
    let meta = std::fs::metadata(file).ok()?;
    #[cfg(unix)]
    let inode = std::os::unix::fs::MetadataExt::ino(&meta);
    #[cfg(not(unix))]
    let inode = 0;
    Some((meta.len(), meta.modified().ok(), inode))
}

/// Polls `file` every `every` until `stop` is set, sending its path when its stamp changes
/// (including when it appears or disappears).
fn poll_loop(file: &Path, every: Duration, stop: &AtomicBool, out: &Sender<PathBuf>) {
    let mut last = file_stamp(file);
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(every);
        let now = file_stamp(file);
        if now != last {
            last = now;
            if out.send(file.to_path_buf()).is_err() {
                return;
            }
        }
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
    fn a_file_watcher_reports_only_its_file_and_ignores_subfolders() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let file = root.join("page.md");
        std::fs::write(&file, "x").unwrap();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let watcher = FolderWatcher::start_file(&file, Duration::from_millis(100), || {}).unwrap();
        std::fs::write(root.join("other.md"), "y").unwrap();
        std::fs::write(root.join("sub").join("page.md"), "z").unwrap();
        std::fs::write(&file, "changed").unwrap();
        let start = Instant::now();
        let mut seen = Vec::new();
        while seen.is_empty() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(50));
            seen = watcher.poll();
        }
        std::thread::sleep(Duration::from_millis(300));
        seen.extend(watcher.poll());
        assert!(
            seen.iter().any(|p| p.file_name() == file.file_name()),
            "{seen:?}"
        );
        assert!(
            seen.iter().all(|p| p.parent() == file.parent()),
            "a file in a subfolder is not reported: {seen:?}"
        );
        assert!(
            seen.iter().all(|p| p.file_name() == file.file_name()),
            "{seen:?}"
        );
    }

    /// The polling backend the kqueue systems use for one file, run on every system: an edit
    /// in place and a save that replaces the file are both reported.
    #[test]
    fn a_polled_file_reports_edits_in_place_and_replacements() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let file = root.join("page.md");
        std::fs::write(&file, "x").unwrap();
        let watcher = FolderWatcher::start_with(
            &file,
            RecursiveMode::NonRecursive,
            file.file_name().map(std::ffi::OsStr::to_os_string),
            Duration::from_millis(100),
            WatcherKind::Poll(Duration::from_millis(100)),
            || {},
        )
        .unwrap();
        let wait = |what: &str| {
            let start = Instant::now();
            loop {
                let seen = watcher.poll();
                if !seen.is_empty() {
                    assert!(seen.iter().all(|p| p == &file), "{what}: {seen:?}");
                    return;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "timed out: {what}"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
        };
        // Poll times are coarse on some file systems: change the size, not only the time.
        std::thread::sleep(Duration::from_millis(250));
        std::fs::write(&file, "edited in place").unwrap();
        wait("edit in place");
        let temp = root.join("page.md.tmp");
        std::fs::write(&temp, "replaced by a rename, longer").unwrap();
        std::fs::rename(&temp, &file).unwrap();
        wait("replacement");
    }

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
