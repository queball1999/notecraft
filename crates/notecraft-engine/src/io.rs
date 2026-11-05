//! The session's background work: timed auto-save, the recovery journal, reading pages off
//! the UI thread, and watching notebook folders for outside edits (PR 1.7).
//!
//! [`SessionIo`] is owned by the front end, next to the session. The front end calls
//! [`SessionIo::pump`] once per frame with the clock and whether the window has focus, and
//! [`SessionIo::flush`] on quit. Nothing here knows about egui. The `wake` function given to
//! [`SessionIo::new`] is called from worker threads when they have something for `pump`
//! (a front end uses it to request a repaint).
//!
//! What triggers a save: [`SAVE_AFTER_IDLE`] (1.5 s) after the last edit, at most
//! [`SAVE_MAX_WAIT`] (30 s) after the first unsaved edit while typing goes on, the window
//! losing focus, showing a different page, and quit. A save that fails is remembered per page
//! ([`SessionIo::save_error`]) and reported once ([`SessionIo::take_new_save_failures`]). The journal holds a copy of each unsaved page, refreshed every
//! [`JOURNAL_EVERY`] (2 s) while the text changes, so a crash loses at most a few seconds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use notecraft_storage::reader::PageReader;
use notecraft_storage::watch::{FolderWatcher, DEBOUNCE};
use notecraft_storage::Journal;

use crate::session::{ReadPurpose, SaveOutcome, Session};

/// Quiet time after the last edit before the page is saved.
pub const SAVE_AFTER_IDLE: Duration = Duration::from_millis(1500);
/// Longest an unsaved edit waits while typing goes on without a pause, and the time between
/// retries of a save that failed.
pub const SAVE_MAX_WAIT: Duration = Duration::from_secs(30);
/// Time between refreshes of the recovery journal.
pub const JOURNAL_EVERY: Duration = Duration::from_secs(2);

type Wake = Arc<dyn Fn() + Send + Sync>;

/// Timers, threads, and the journal behind a [`Session`].
pub struct SessionIo {
    journal: Option<Journal>,
    reader: Option<PageReader<(String, ReadPurpose)>>,
    watchers: BTreeMap<PathBuf, FolderWatcher>,
    /// Background refreshes of each notebook's index (the navigation reads it).
    index_sync: index_sync::IndexSync,
    /// Folders already checked for recovery entries.
    offered: Vec<PathBuf>,
    /// Watchers and recovery offers for loose pages (`io/loose.rs`).
    loose: loose::LooseWatch,
    wake: Wake,
    debounce: Duration,
    save_after_idle: Duration,
    save_max_wait: Duration,
    /// When the session first had unsaved changes (or the last save attempt left some).
    dirty_since: Option<Instant>,
    /// When the unsaved text last changed; `None` after a save attempt left it unsaved.
    last_edit: Option<Instant>,
    /// The session revision seen by the last pump, to notice edits.
    seen_revision: u64,
    /// Page files just written by this session (their watcher events are not outside edits).
    own_writes: own_writes::OwnWrites,
    /// The reason the last save of each page failed.
    save_errors: BTreeMap<String, String>,
    /// Failures not yet handed to the front end.
    new_failures: Vec<(String, String)>,
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
            index_sync: index_sync::IndexSync::default(),
            offered: Vec::new(),
            loose: loose::LooseWatch::default(),
            wake,
            debounce: DEBOUNCE,
            save_after_idle: SAVE_AFTER_IDLE,
            save_max_wait: SAVE_MAX_WAIT,
            dirty_since: None,
            last_edit: None,
            seen_revision: 0,
            own_writes: own_writes::OwnWrites::default(),
            save_errors: BTreeMap::new(),
            new_failures: Vec::new(),
            last_journal: None,
            journaled: BTreeMap::new(),
            last_page: None,
            was_focused: true,
        }
    }

    /// Change the auto-save delays (tests): the quiet time after an edit, and the cap.
    pub fn with_save_delays(mut self, after_idle: Duration, max_wait: Duration) -> Self {
        self.save_after_idle = after_idle;
        self.save_max_wait = max_wait;
        self
    }

    /// Why the last save of page `page_id` failed, if it did and the page is still unsaved.
    pub fn save_error(&self, page_id: &str) -> Option<&str> {
        self.save_errors.get(page_id).map(String::as_str)
    }

    /// Pages whose save failed for the first time since the last call: page id and reason.
    /// The front end raises one notice for each.
    pub fn take_new_save_failures(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.new_failures)
    }

    fn record_saves(
        &mut self,
        session: &Session,
        now: Instant,
        outcomes: Vec<(String, SaveOutcome)>,
    ) {
        for (id, outcome) in outcomes {
            if outcome == SaveOutcome::Saved {
                if let Some(path) = session.page_file_path(&id) {
                    self.own_writes.record(&id, &path, now);
                }
            }
            match outcome {
                SaveOutcome::Failed(reason) => {
                    if self
                        .save_errors
                        .insert(id.clone(), reason.clone())
                        .is_none()
                    {
                        self.new_failures.push((id, reason));
                    }
                }
                // A conflict or a read-only page has its own notice; only failures show here.
                _ => {
                    self.save_errors.remove(&id);
                }
            }
        }
    }

    /// Save every unsaved page and note what failed.
    fn save_now(&mut self, session: &mut Session, now: Instant) {
        let outcomes = session.save_dirty();
        self.record_saves(session, now, outcomes);
        // What is still unsaved (failed or in conflict) is retried after the cap.
        self.last_edit = None;
        self.dirty_since = session.has_unsaved_changes().then_some(now);
        self.seen_revision = session.revision();
    }

    /// Track edits to decide when the timed save falls due.
    fn watch_edits(&mut self, session: &Session, now: Instant) {
        if !session.has_unsaved_changes() {
            self.dirty_since = None;
            self.last_edit = None;
            self.seen_revision = session.revision();
            return;
        }
        if self.dirty_since.is_none() {
            self.dirty_since = Some(now);
            self.last_edit = Some(now);
        } else if session.revision() != self.seen_revision {
            self.last_edit = Some(now);
        }
        self.seen_revision = session.revision();
    }

    /// When the timed save falls due, if anything is unsaved.
    fn save_due(&self) -> Option<Instant> {
        let cap = self.dirty_since? + self.save_max_wait;
        Some(match self.last_edit {
            Some(edit) => cap.min(edit + self.save_after_idle),
            None => cap,
        })
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
        self.loose
            .sync(session, self.journal.as_ref(), self.debounce, &self.wake);

        // Saves on a page switch and on focus loss.
        let page = session.selection().page.clone();
        let switched = self.last_page.is_some() && self.last_page != page;
        self.last_page = page;
        let lost_focus = self.was_focused && !focused;
        self.was_focused = focused;
        if switched || lost_focus {
            self.save_now(session, now);
        }

        self.drain_watchers(session, now);
        self.index_sync.pump(session, &self.wake);
        session.load_current_page();
        self.run_reads(session);

        self.watch_edits(session, now);
        if self.save_due().is_some_and(|due| now >= due) {
            self.save_now(session, now);
        }
        let dirty: Vec<String> = self.save_errors.keys().cloned().collect();
        for id in dirty {
            if !session.is_page_dirty(&id) {
                self.save_errors.remove(&id);
            }
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
    /// quit. Returns the ids of the pages that stayed unsaved and were kept in the journal for
    /// recovery at the next start.
    pub fn flush(&mut self, session: &mut Session) -> Vec<String> {
        let outcomes = session.save_dirty();
        self.record_saves(session, Instant::now(), outcomes);
        self.write_journal(session);
        self.clean_journal(session);
        session
            .journal_candidates()
            .into_iter()
            .filter(|(_, path, revision, _)| self.journaled.get(path) == Some(revision))
            .map(|(id, ..)| id)
            .collect()
    }

    fn next_wake(&self, session: &Session, now: Instant) -> Option<Duration> {
        let mut due = self.save_due();
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
        // Drop what can never be offered back (damaged files, duplicates) before reading.
        let pruned = journal.prune();
        if pruned > 0 {
            tracing::info!(pruned, "pruned the recovery journal");
        }
        let entries = journal.load_all();
        if entries.is_empty() {
            return;
        }
        for path in session.offer_recovery(entries) {
            journal.remove(&path);
        }
    }

    fn drain_watchers(&mut self, session: &mut Session, now: Instant) {
        self.loose.drain(session);
        for (root, watcher) in &self.watchers {
            for path in watcher.poll() {
                let id = session.page_id_for_path(&path);
                if !self.own_writes.is_echo(id.as_deref(), &path, now) {
                    self.index_sync.note_change(root, &path);
                }
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

mod index_sync;
mod loose;
#[cfg(test)]
mod loose_tests;
mod own_writes;
#[cfg(test)]
mod tests;
