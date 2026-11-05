//! Watching and crash recovery for loose pages (`session/loose.rs`): one page file opened
//! outside any notebook folder.
//!
//! Each open loose page gets a [`FolderWatcher`] on its own folder that is not recursive and
//! reports only that file, so outside edits are noticed without watching a busy folder such as
//! Downloads. The recovery journal already lives in the settings folder, not next to the
//! file, so a loose page is journaled like any other; what is added here is the offer: the
//! first time a loose page is seen, the journal is checked for text saved for its file.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use notecraft_storage::watch::FolderWatcher;
use notecraft_storage::Journal;

use super::Wake;
use crate::session::Session;

/// The watchers of the open loose pages and the files already checked for recovery.
#[derive(Default)]
pub(super) struct LooseWatch {
    pub(super) watchers: BTreeMap<PathBuf, FolderWatcher>,
    offered: BTreeSet<PathBuf>,
}

impl LooseWatch {
    /// Start a watcher for each open loose page, drop the ones for closed pages, and offer
    /// the journal's unsaved text for pages seen for the first time.
    pub(super) fn sync(
        &mut self,
        session: &mut Session,
        journal: Option<&Journal>,
        debounce: Duration,
        wake: &Wake,
    ) {
        let files = session.loose_files();
        self.watchers.retain(|file, _| files.contains(file));
        for file in files {
            if !self.watchers.contains_key(&file) {
                let wake = std::sync::Arc::clone(wake);
                match FolderWatcher::start_file(&file, debounce, move || wake()) {
                    Ok(watcher) => {
                        self.watchers.insert(file.clone(), watcher);
                    }
                    Err(err) => {
                        tracing::warn!(file = %file.display(), "cannot watch the page: {err}");
                    }
                }
            }
            if self.offered.insert(file) {
                if let Some(journal) = journal {
                    offer(session, journal);
                }
            }
        }
    }

    /// Tell the session about files changed outside NoteCraft.
    pub(super) fn drain(&self, session: &mut Session) {
        for watcher in self.watchers.values() {
            for path in watcher.poll() {
                session.note_outside_change(&path);
            }
        }
    }
}

fn offer(session: &mut Session, journal: &Journal) {
    let entries = journal.load_all();
    if entries.is_empty() {
        return;
    }
    for path in session.offer_recovery(entries) {
        journal.remove(&path);
    }
}
