//! The session's background work: timed auto-save, the recovery journal, reading pages off
//! the UI thread, and watching notebook folders for outside edits (PR 1.7).
//!
//! [`SessionIo`] is owned by the front end, next to the session. The front end calls
//! [`SessionIo::pump`] once per frame with the clock and whether the window has focus, and
//! [`SessionIo::flush`] on quit. Nothing here knows about egui. The `wake` function given to
//! [`SessionIo::new`] is called from worker threads when they have something for `pump`
//! (a front end uses it to request a repaint).
//!
//! What triggers a save: every [`AUTOSAVE_EVERY`] (30 s), the window losing focus, showing a
//! different page, and quit. The journal holds a copy of each unsaved page, refreshed every
//! [`JOURNAL_EVERY`] (2 s) while the text changes, so a crash loses at most a few seconds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use notecraft_storage::reader::PageReader;
use notecraft_storage::watch::{FolderWatcher, DEBOUNCE};
use notecraft_storage::Journal;

use crate::session::{ReadPurpose, Session};

/// Time between timed saves.
pub const AUTOSAVE_EVERY: Duration = Duration::from_secs(30);
/// Time between refreshes of the recovery journal.
pub const JOURNAL_EVERY: Duration = Duration::from_secs(2);

type Wake = Arc<dyn Fn() + Send + Sync>;

/// Timers, threads, and the journal behind a [`Session`].
pub struct SessionIo {
    journal: Option<Journal>,
    reader: Option<PageReader<(String, ReadPurpose)>>,
    watchers: BTreeMap<PathBuf, FolderWatcher>,
    /// Folders already checked for recovery entries.
    offered: Vec<PathBuf>,
    wake: Wake,
    debounce: Duration,
    autosave_every: Duration,
    last_autosave: Option<Instant>,
    last_journal: Option<Instant>,
    /// The text revision last written to the journal, by page file.
    journaled: BTreeMap<PathBuf, u64>,
    last_page: Option<String>,
    was_focused: bool,
}

impl std::fmt::Debug for SessionIo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionIo")
            .field("watched", &self.watchers.len())
            .finish_non_exhaustive()
    }
}

impl SessionIo {
    /// `journal_dir` is the recovery folder inside the settings folder; `None` (no settings
    /// folder) turns the journal off. `wake` is called from worker threads.
    pub fn new(journal_dir: Option<PathBuf>, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let wake: Wake = Arc::new(wake);
        let reader_wake = Arc::clone(&wake);
        let reader = match PageReader::start(move || reader_wake()) {
            Ok(reader) => Some(reader),
            Err(err) => {
                tracing::warn!("cannot start the page reader; reading pages inline: {err}");
                None
            }
        };
        Self {
            journal: journal_dir.map(Journal::new),
            reader,
            watchers: BTreeMap::new(),
            offered: Vec::new(),
            wake,
            debounce: DEBOUNCE,
            autosave_every: AUTOSAVE_EVERY,
            last_autosave: None,
            last_journal: None,
            journaled: BTreeMap::new(),
            last_page: None,
            was_focused: true,
        }
    }

    /// Change the auto-save interval (tests).
    pub fn with_autosave_every(mut self, every: Duration) -> Self {
        self.autosave_every = every;
        self
    }

    /// Change the watcher debounce time (tests; the default is 300 ms).
    pub fn with_debounce(mut self, debounce: Duration) -> Self {
        self.debounce = debounce;
        self
    }

    /// Do the background work that is due. Call once per frame. Returns when the next timed
    /// work falls due, if any, so the caller can ask for a repaint then.
    pub fn pump(&mut self, session: &mut Session, now: Instant, focused: bool) -> Option<Duration> {
        session.set_background_reads(self.reader.is_some());
        self.sync_notebooks(session);

        // Saves on a page switch and on focus loss.
        let page = session.selection().page.clone();
        let switched = self.last_page.is_some() && self.last_page != page;
        self.last_page = page;
        let lost_focus = self.was_focused && !focused;
        self.was_focused = focused;
        if switched || lost_focus {
            session.save_dirty();
        }

        self.drain_watchers(session);
        session.load_current_page();
        self.run_reads(session);

        let last = *self.last_autosave.get_or_insert(now);
        if now.saturating_duration_since(last) >= self.autosave_every {
            session.save_dirty();
            self.last_autosave = Some(now);
        }
        if session.has_unsaved_changes() {
            let last = *self.last_journal.get_or_insert(now);
            if now.saturating_duration_since(last) >= JOURNAL_EVERY {
                self.write_journal(session);
                self.last_journal = Some(now);
            }
        }
        self.clean_journal(session);
        self.next_wake(session, now)
    }

    /// Save everything now and make sure what could not be saved is in the journal. Call on
    /// quit.
    pub fn flush(&mut self, session: &mut Session) {
        session.save_dirty();
        self.write_journal(session);
        self.clean_journal(session);
    }

    fn next_wake(&self, session: &Session, now: Instant) -> Option<Duration> {
        let mut due = self.last_autosave.map(|last| last + self.autosave_every);
        if session.has_unsaved_changes() {
            let journal = self.last_journal.map(|last| last + JOURNAL_EVERY);
            due = match (due, journal) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
        }
        due.map(|at| at.saturating_duration_since(now))
    }

    /// Start a watcher for each open notebook folder, drop the ones for closed folders, and
    /// offer the journal's unsaved text for folders seen for the first time.
    fn sync_notebooks(&mut self, session: &mut Session) {
        let folders: Vec<PathBuf> = session.notebook_folders().map(Path::to_path_buf).collect();
        self.watchers.retain(|root, _| folders.contains(root));
        for folder in folders {
            if !self.watchers.contains_key(&folder) {
                let wake = Arc::clone(&self.wake);
                match FolderWatcher::start(&folder, self.debounce, move || wake()) {
                    Ok(watcher) => {
                        self.watchers.insert(folder.clone(), watcher);
                    }
                    Err(err) => {
                        tracing::warn!(folder = %folder.display(), "cannot watch the folder: {err}");
                    }
                }
            }
            if !self.offered.contains(&folder) {
                self.offered.push(folder);
                self.offer_recovery(session);
            }
        }
    }

    fn offer_recovery(&mut self, session: &mut Session) {
        let Some(journal) = &self.journal else {
            return;
        };
        let entries = journal.load_all();
        if entries.is_empty() {
            return;
        }
        for path in session.offer_recovery(entries) {
            journal.remove(&path);
        }
    }

    fn drain_watchers(&mut self, session: &mut Session) {
        for watcher in self.watchers.values() {
            for path in watcher.poll() {
                session.note_outside_change(&path);
            }
        }
    }

    fn run_reads(&mut self, session: &mut Session) {
        let Some(reader) = &self.reader else {
            return;
        };
        for request in session.take_read_requests() {
            reader.request((request.page_id, request.purpose), request.path);
        }
        for done in reader.poll() {
            let (page_id, purpose) = done.tag;
            session.finish_read(&page_id, purpose, done.result);
        }
    }

    fn write_journal(&mut self, session: &Session) {
        let Some(journal) = &self.journal else {
            return;
        };
        for (_page_id, path, revision, text) in session.journal_candidates() {
            if self.journaled.get(&path) == Some(&revision) {
                continue;
            }
            match journal.write(&path, &text) {
                Ok(()) => {
                    self.journaled.insert(path, revision);
                }
                Err(err) => tracing::warn!("cannot write the recovery journal: {err}"),
            }
        }
    }

    fn clean_journal(&mut self, session: &mut Session) {
        for path in session.take_journal_clean() {
            self.journaled.remove(&path);
            if let Some(journal) = &self.journal {
                journal.remove(&path);
            }
        }
    }
}

#[cfg(test)]
mod tests;
