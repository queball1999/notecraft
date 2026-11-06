//! Watching a notebook folder for changes made by other programs (docs/architecture.md
//! section 3.3).
//!
//! A `notify` watcher feeds raw events to a small thread, which waits until the folder has
//! been quiet for the debounce time ([`DEBOUNCE`], 300 ms) and then hands over the set of
//! changed paths. NoteCraft's own writes also arrive here; the caller tells them apart by
//! comparing file contents.

use std::collections::{BTreeMap, BTreeSet};
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

/// How often a file or a notebook folder is polled on the kqueue systems (FreeBSD, OpenBSD,
/// NetBSD, DragonFly). There `notify` is not used: a kqueue watch on a folder only reports
/// entries added or removed, never edits to a file in it, and its watch on a file can be
/// lost when a save replaces the file (temp file renamed over it), depending on the order
/// it handles the delete and the folder change.
#[cfg(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
const POLL_EVERY: Duration = Duration::from_millis(500);

/// Most entries one poll of a folder tree records; the rest are not watched.
const MAX_POLL_ENTRIES: usize = 100_000;

/// Deepest folder level one poll descends to.
const MAX_POLL_DEPTH: usize = 64;

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
    /// On the kqueue systems the folder tree is polled instead ([`POLL_EVERY`]; the notebook's
    /// own `.notecraft` folder is skipped, as callers ignore it).
    pub fn start(
        root: &Path,
        debounce: Duration,
        wake: impl Fn() + Send + 'static,
    ) -> notify::Result<Self> {
        #[cfg(any(
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ))]
        let kind = WatcherKind::Poll(POLL_EVERY);
        #[cfg(not(any(
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        )))]
        let kind = WatcherKind::Native;
        Self::start_with(root, RecursiveMode::Recursive, None, debounce, kind, wake)
    }

    /// Watch the one file `file` (a loose page). Only its folder is watched, without
    /// descending into subfolders, and only events for that file name are reported, so a
    /// busy folder such as Downloads costs next to nothing. On the kqueue systems (FreeBSD,
    /// OpenBSD, NetBSD, DragonFly), where a folder watch never reports edits to a file in
    /// it, the file itself is polled instead ([`POLL_EVERY`]), which also follows a save that
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
        let (target, kind) = (file, WatcherKind::Poll(POLL_EVERY));
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
                let target = root.to_path_buf();
                let recursive = mode == RecursiveMode::Recursive;
                // The first snapshot is taken before returning, so a change made right after
                // `start` is seen.
                let first = snapshot(&target, recursive);
                let stop = Arc::clone(&stop);
                std::thread::Builder::new()
                    .name("notecraft-poll".to_owned())
                    .spawn(move || poll_loop(&target, recursive, first, every, &stop, &raw_tx))
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
    /// Polling metadata (one file, or a folder tree) at this interval, on a small thread of
    /// our own: `notify`'s poller compares modified times in whole seconds, so it misses
    /// quick edits.
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

/// Size, modified time, and inode of one path.
type Stamp = (u64, Option<SystemTime>, u64);

/// What a poll compares: size, modified time at full precision, and (on Unix) the file's
/// inode, so an edit in place and a save that replaces the file are both seen. `None` when
/// the file cannot be read (gone, or not allowed).
fn file_stamp(file: &Path) -> Option<Stamp> {
    let meta = std::fs::symlink_metadata(file).ok()?;
    #[cfg(unix)]
    let inode = std::os::unix::fs::MetadataExt::ino(&meta);
    #[cfg(not(unix))]
    let inode = 0;
    Some((meta.len(), meta.modified().ok(), inode))
}

/// The stamps of `target`: just the path itself, or, with `recursive`, every file and folder
/// under it (symbolic links are not followed; the notebook's own `.notecraft` folder at the
/// top is skipped; at most [`MAX_POLL_ENTRIES`] entries and [`MAX_POLL_DEPTH`] levels).
fn snapshot(target: &Path, recursive: bool) -> BTreeMap<PathBuf, Stamp> {
    let mut out = BTreeMap::new();
    if let Some(stamp) = file_stamp(target) {
        out.insert(target.to_path_buf(), stamp);
    }
    if !recursive {
        return out;
    }
    let mut folders = vec![(target.to_path_buf(), 0usize)];
    while let Some((folder, depth)) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            if out.len() >= MAX_POLL_ENTRIES {
                return out;
            }
            let path = entry.path();
            if depth == 0 && entry.file_name() == crate::notebook_dir::NOTEBOOK_DIR_NAME {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if let Some(stamp) = file_stamp(&path) {
                out.insert(path.clone(), stamp);
            }
            if kind.is_dir() && depth + 1 < MAX_POLL_DEPTH {
                folders.push((path, depth + 1));
            }
        }
    }
    out
}

/// Polls `target` every `every` until `stop` is set, sending each path whose stamp changed,
/// appeared, or disappeared since the last poll.
fn poll_loop(
    target: &Path,
    recursive: bool,
    first: BTreeMap<PathBuf, Stamp>,
    every: Duration,
    stop: &AtomicBool,
    out: &Sender<PathBuf>,
) {
    let mut last = first;
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(every);
        let now = snapshot(target, recursive);
        let changed = now
            .iter()
            .filter(|(path, stamp)| last.get(*path) != Some(*stamp))
            .map(|(path, _)| path)
            .chain(last.keys().filter(|path| !now.contains_key(*path)));
        for path in changed {
            if out.send(path.clone()).is_err() {
                return;
            }
        }
        last = now;
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

    /// The polling backend the kqueue systems use for a notebook folder, run on every system.
    #[test]
    fn a_polled_folder_reports_new_edited_replaced_nested_and_removed_files() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("Sec/Group")).unwrap();
        std::fs::create_dir_all(root.join(".notecraft")).unwrap();
        let page = root.join("Sec/page.md");
        std::fs::write(&page, "x").unwrap();
        let watcher = FolderWatcher::start_with(
            &root,
            RecursiveMode::Recursive,
            None,
            Duration::from_millis(100),
            WatcherKind::Poll(Duration::from_millis(100)),
            || {},
        )
        .unwrap();
        let wait_for = |what: &str, want: &Path| {
            let start = Instant::now();
            let mut seen = Vec::new();
            while !seen.iter().any(|p: &PathBuf| p == want) {
                assert!(
                    start.elapsed() < Duration::from_secs(10),
                    "timed out: {what}: {seen:?}"
                );
                std::thread::sleep(Duration::from_millis(50));
                seen.extend(watcher.poll());
            }
            assert!(
                seen.iter().all(|p| !p.starts_with(root.join(".notecraft"))),
                "{what}: .notecraft is not reported: {seen:?}"
            );
        };
        // Written right after start: the first snapshot was taken before `start` returned.
        let new = root.join("Sec/new.md");
        std::fs::write(&new, "new").unwrap();
        std::fs::write(root.join(".notecraft/index.sqlite"), "ignored").unwrap();
        wait_for("new file", &new);
        std::fs::write(&page, "edited in place").unwrap();
        wait_for("edit in place", &page);
        let temp = root.join("Sec/page.md.tmp");
        std::fs::write(&temp, "replaced by a rename").unwrap();
        std::fs::rename(&temp, &page).unwrap();
        wait_for("replacement", &page);
        let nested = root.join("Sec/Group/deep.md");
        std::fs::write(&nested, "deep").unwrap();
        wait_for("nested file", &nested);
        std::fs::remove_file(&new).unwrap();
        wait_for("removed file", &new);
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
