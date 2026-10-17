//! Saving, crash recovery, and outside edits: the session side of PR 1.7.
//!
//! The session keeps, per page, whether it has unsaved changes (the dirty set), a hash of
//! the text last known to be on disk, and any outside version that clashed with unsaved
//! work. It does not own threads, timers, or the recovery journal; [`crate::io::SessionIo`]
//! drives those and calls the methods here.
//!
//! The small API an editing path needs is [`Session::mark_page_dirty`]. Everything that goes
//! through [`Session::commit`], undo, or redo (page text, strokes) sets it already, so a
//! caller that edits through those needs to do nothing more. Saving never writes a page whose
//! notice is read-only, and never writes over a page file that changed outside NoteCraft
//! without the user choosing which version to keep.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};

use notecraft_storage::{read_page_file, write_page_text, PageReadError, PageText, WriteOutcome};

use super::Session;

/// Why a page file is being read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadPurpose {
    /// The page was shown and has no text yet.
    Load,
    /// A file watcher reported that the page file changed.
    Outside,
}

/// A page file read the IO worker should do (see [`Session::take_read_requests`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadRequest {
    pub page_id: String,
    pub path: PathBuf,
    pub purpose: ReadPurpose,
}

/// Unsaved text found in the recovery journal, waiting for the user to restore or discard it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryItem {
    pub page_id: String,
    pub path: PathBuf,
    pub text: String,
    disk_hash: u64,
}

/// What [`Session::save_page`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    /// The file was written.
    Saved,
    /// The file already held this text.
    Unchanged,
    /// The page has no unsaved changes.
    NotDirty,
    /// The page is read-only (cut short or unreadable); it is never written.
    ReadOnly,
    /// The file changed outside NoteCraft; nothing was written until the user chooses.
    Conflict,
    /// The page has no file (a sample page, or its path is not valid).
    NoFile,
    /// A `.md` page got its first stroke and was saved as a `.notecraft` page. The page id
    /// changed: the session already follows it, but a caller holding the old id must too.
    Promoted { new_id: String },
    /// The write failed; the page stays unsaved. Holds a short reason.
    Failed(String),
}

/// How the shown notebooks stand, for the status bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveState {
    /// Pages with changes not yet on disk.
    pub unsaved: usize,
    /// Pages whose file changed outside NoteCraft while they had unsaved changes.
    pub conflicts: usize,
}

pub(super) enum Resolved {
    /// Not backed by a markdown file in a notebook folder.
    NotAFile,
    /// A path that is not a plain relative path inside the notebook.
    Invalid,
    File(PathBuf),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SavingState {
    /// Read pages on the IO worker instead of inline.
    pub background: bool,
    /// Pages whose first read is queued or out.
    pub loading: BTreeSet<String>,
    /// Pages with changes not yet written.
    pub dirty: BTreeSet<String>,
    /// Hash of the text last known to be in each page's file.
    pub disk_hash: BTreeMap<String, u64>,
    /// The outside version of a page that also has unsaved changes.
    pub conflicts: BTreeMap<String, String>,
    pub read_requests: Vec<ReadRequest>,
    pub recovery: Vec<RecoveryItem>,
    /// Page files whose journal entry is no longer needed.
    pub journal_clean: Vec<PathBuf>,
}

impl SavingState {
    /// Page `old` is now page `new`: move its save state.
    pub(super) fn rekey(&mut self, old: &str, new: &str) {
        if self.dirty.remove(old) {
            self.dirty.insert(new.to_owned());
        }
        if self.loading.remove(old) {
            self.loading.insert(new.to_owned());
        }
        if let Some(value) = self.disk_hash.remove(old) {
            self.disk_hash.insert(new.to_owned(), value);
        }
        if let Some(value) = self.conflicts.remove(old) {
            self.conflicts.insert(new.to_owned(), value);
        }
        for request in &mut self.read_requests {
            if request.page_id == old {
                request.page_id = new.to_owned();
            }
        }
        for item in &mut self.recovery {
            if item.page_id == old {
                item.page_id = new.to_owned();
            }
        }
    }

    pub(super) fn forget_notebook(&mut self, prefix: &str) {
        self.loading.retain(|id| !id.starts_with(prefix));
        self.dirty.retain(|id| !id.starts_with(prefix));
        self.disk_hash.retain(|id, _| !id.starts_with(prefix));
        self.conflicts.retain(|id, _| !id.starts_with(prefix));
        self.read_requests
            .retain(|r| !r.page_id.starts_with(prefix));
        self.recovery.retain(|r| !r.page_id.starts_with(prefix));
    }
}

pub(super) fn hash_text(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

impl Session {
    /// Turn background reads on or off. With them on, [`Session::load_current_page`] and
    /// outside changes queue [`ReadRequest`]s instead of reading inline; the owner of the IO
    /// worker drains them with [`Session::take_read_requests`] and returns each answer
    /// through [`Session::finish_read`].
    pub fn set_background_reads(&mut self, on: bool) {
        self.saving.background = on;
    }

    /// Reads waiting for the IO worker, oldest first.
    pub fn take_read_requests(&mut self) -> Vec<ReadRequest> {
        std::mem::take(&mut self.saving.read_requests)
    }

    pub(super) fn queue_read(&mut self, page_id: String, path: PathBuf, purpose: ReadPurpose) {
        if purpose == ReadPurpose::Load {
            self.saving.loading.insert(page_id.clone());
        }
        let request = ReadRequest {
            page_id,
            path,
            purpose,
        };
        if !self.saving.read_requests.contains(&request) {
            self.saving.read_requests.push(request);
        }
    }

    /// Whether the page shown now is still being read.
    pub fn is_loading_current_page(&self) -> bool {
        self.selection
            .page
            .as_ref()
            .is_some_and(|id| self.saving.loading.contains(id))
    }

    /// Hand back the answer to a [`ReadRequest`].
    pub fn finish_read(
        &mut self,
        page_id: &str,
        purpose: ReadPurpose,
        result: Result<PageText, PageReadError>,
    ) {
        match purpose {
            ReadPurpose::Load => self.apply_load_result(page_id, result),
            ReadPurpose::Outside => self.apply_outside_result(page_id, result),
        }
    }

    /// The file of page `page_id`, if the page comes from a notebook folder.
    pub(super) fn resolve_page_file(&self, page_id: &str) -> Resolved {
        // The longest notebook id that prefixes the page id wins.
        let found = self
            .notebook_roots
            .iter()
            .filter_map(|(nb, root)| {
                let rel = page_id.strip_prefix(nb.as_str())?.strip_prefix('/')?;
                Some((nb.len(), root, rel))
            })
            .max_by_key(|(len, _, _)| *len);
        let Some((_, root, rel)) = found else {
            return Resolved::NotAFile;
        };
        let rel = Path::new(rel);
        if super::save::kind_of(rel).is_none() {
            return Resolved::NotAFile;
        }
        if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            return Resolved::Invalid;
        }
        Resolved::File(root.join(rel))
    }

    /// The `.md` or `.notecraft` file of page `page_id`, when it has one.
    pub fn page_file_path(&self, page_id: &str) -> Option<PathBuf> {
        match self.resolve_page_file(page_id) {
            Resolved::File(path) => Some(path),
            _ => None,
        }
    }

    /// The id of the page whose file is `path`, if `path` is inside an open notebook folder.
    pub fn page_id_for_path(&self, path: &Path) -> Option<String> {
        self.notebook_roots.iter().find_map(|(nb, root)| {
            let rel = path.strip_prefix(root).ok().or_else(|| {
                let canonical = root.canonicalize().ok()?;
                path.strip_prefix(canonical).ok()
            })?;
            let mut id = nb.clone();
            for part in rel.components() {
                let Component::Normal(part) = part else {
                    return None;
                };
                id.push('/');
                id.push_str(part.to_str()?);
            }
            Some(id)
        })
    }

    /// The folders of the notebooks read from disk.
    pub fn notebook_folders(&self) -> impl Iterator<Item = &Path> {
        self.notebook_roots.values().map(PathBuf::as_path)
    }

    // ----- dirty state -----

    /// Record that page `page_id` has changes not yet on disk. This is the one call an
    /// editing path makes (PR 1.5 text editing, anything else that changes page content
    /// outside [`Session::commit`]). Changes made through `commit`, undo, and redo (page
    /// text and strokes) already call it. Pages without a file are ignored.
    pub fn mark_page_dirty(&mut self, page_id: &str) {
        if self.page_file_path(page_id).is_some() {
            self.saving.dirty.insert(page_id.to_owned());
        }
    }

    pub fn is_page_dirty(&self, page_id: &str) -> bool {
        self.saving.dirty.contains(page_id)
    }

    /// Whether any page has unsaved changes.
    pub fn has_unsaved_changes(&self) -> bool {
        !self.saving.dirty.is_empty()
    }

    /// Unsaved and conflicted page counts.
    pub fn save_state(&self) -> SaveState {
        SaveState {
            unsaved: self.saving.dirty.len(),
            conflicts: self.saving.conflicts.len(),
        }
    }

    /// The unsaved pages that the recovery journal should hold: id, file, a revision (a hash
    /// of the text, which changes whenever the text or the strokes do), and the text to
    /// journal. The text is the whole file as it would be saved, canvas block with strokes
    /// included, so recovery brings the strokes back too. Read-only pages are left out.
    pub fn journal_candidates(&self) -> Vec<(String, PathBuf, u64, String)> {
        self.saving
            .dirty
            .iter()
            .filter(|id| !self.is_read_only(id))
            .filter_map(|id| {
                let path = self.page_file_path(id)?;
                let text = self.file_text(id)?.ok()?;
                Some((id.clone(), path, hash_text(&text), text))
            })
            .collect()
    }

    /// Page files whose recovery journal entry can be deleted now (saved, discarded, or
    /// replaced by the outside version).
    pub fn take_journal_clean(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.saving.journal_clean)
    }

    fn is_read_only(&self, page_id: &str) -> bool {
        self.page_notices
            .get(page_id)
            .is_some_and(|notice| notice.read_only)
    }

    fn note_clean(&mut self, page_id: &str) {
        if let Some(path) = self.page_file_path(page_id) {
            self.saving.journal_clean.push(path);
        }
    }

    // ----- saving -----

    /// Write page `page_id` if it has unsaved changes. Never writes a read-only page, and
    /// never writes over a file that changed outside NoteCraft since it was read: that
    /// becomes a conflict for the user to settle. The text and the page's strokes go out in
    /// one atomic write (the strokes live in the page's canvas block, PR 1.9). A `.md` page
    /// that has strokes is written as a `.notecraft` page and the `.md` file is removed
    /// ([`SaveOutcome::Promoted`]).
    pub fn save_page(&mut self, page_id: &str) -> SaveOutcome {
        if !self.saving.dirty.contains(page_id) {
            return SaveOutcome::NotDirty;
        }
        if self.is_read_only(page_id) {
            return SaveOutcome::ReadOnly;
        }
        if self.saving.conflicts.contains_key(page_id) {
            return SaveOutcome::Conflict;
        }
        let Some(path) = self.page_file_path(page_id) else {
            self.saving.dirty.remove(page_id);
            return SaveOutcome::NoFile;
        };
        let Some(text) = self.file_text(page_id) else {
            self.saving.dirty.remove(page_id);
            return SaveOutcome::NoFile;
        };
        // A canvas block that cannot be rewritten safely is never written over.
        let Ok(text) = text else {
            return SaveOutcome::ReadOnly;
        };
        let promote = self.needs_promotion(page_id);
        match read_page_file(&path) {
            Ok(disk) => {
                let known = self.saving.disk_hash.get(page_id).copied();
                if known.is_some_and(|h| h != hash_text(&disk.text)) && disk.text != text {
                    tracing::info!(page = %page_id, "page changed on disk; not saving over it");
                    self.saving.conflicts.insert(page_id.to_owned(), disk.text);
                    self.mark_changed();
                    return SaveOutcome::Conflict;
                }
            }
            Err(PageReadError::NotFound) => {}
            Err(err) => {
                tracing::warn!(page = %page_id, "cannot check the page before saving: {err}");
                return SaveOutcome::Failed("the page file cannot be read".to_owned());
            }
        }
        if promote {
            return self.promote_and_save(page_id, text);
        }
        match write_page_text(&path, &text) {
            Ok(outcome) => {
                self.saving
                    .disk_hash
                    .insert(page_id.to_owned(), hash_text(&text));
                self.saving.dirty.remove(page_id);
                self.note_clean(page_id);
                // The session text now carries the canvas block the file holds.
                self.adopt_saved_text(page_id, text);
                self.mark_changed();
                match outcome {
                    WriteOutcome::Written => SaveOutcome::Saved,
                    WriteOutcome::Unchanged => SaveOutcome::Unchanged,
                }
            }
            Err(err) => {
                tracing::warn!(page = %page_id, "cannot save page: {err}");
                SaveOutcome::Failed(err.to_string())
            }
        }
    }

    /// Save every unsaved page. Returns what happened to each, except pages that needed
    /// nothing.
    pub fn save_dirty(&mut self) -> Vec<(String, SaveOutcome)> {
        let ids: Vec<String> = self.saving.dirty.iter().cloned().collect();
        ids.into_iter()
            .map(|id| {
                let outcome = self.save_page(&id);
                (id, outcome)
            })
            .filter(|(_, outcome)| *outcome != SaveOutcome::NotDirty)
            .collect()
    }

    // ----- outside edits -----

    /// A watcher reported a change at `path`. If it is a page file NoteCraft knows, queue a
    /// read of it (inline when background reads are off). Returns whether it was a page.
    pub fn note_outside_change(&mut self, path: &Path) -> bool {
        let Some(page_id) = self.page_id_for_path(path) else {
            return false;
        };
        let Resolved::File(file) = self.resolve_page_file(&page_id) else {
            return false;
        };
        if !self.page_sources.contains_key(&page_id) {
            // Not read yet, or it failed to read: let the next show read it fresh.
            if self.page_notices.remove(&page_id).is_some() {
                self.mark_changed();
            }
            return true;
        }
        if self.saving.background {
            self.queue_read(page_id, file, ReadPurpose::Outside);
        } else {
            let result = read_page_file(&file);
            self.apply_outside_result(&page_id, result);
        }
        true
    }

    pub(super) fn apply_outside_result(
        &mut self,
        page_id: &str,
        result: Result<PageText, PageReadError>,
    ) {
        let page = match result {
            Ok(page) => page,
            Err(err) => {
                tracing::info!(page = %page_id, "page file not readable after a change: {err}");
                return;
            }
        };
        if page.truncated || !self.page_sources.contains_key(page_id) {
            return;
        }
        let theirs = hash_text(&page.text);
        let known = self.saving.disk_hash.get(page_id).copied();
        if known == Some(theirs) {
            // Our own write, or the file is back to what we last saw.
            if self.saving.conflicts.remove(page_id).is_some() {
                self.mark_changed();
            }
            return;
        }
        let dirty = self.saving.dirty.contains(page_id);
        let ours = match self.file_text(page_id) {
            Some(Ok(text)) => text,
            _ => self.page_text(page_id).unwrap_or_default().to_owned(),
        };
        if dirty && ours != page.text {
            // Keep the user's text; the conflict bar offers the outside version.
            if self.saving.conflicts.get(page_id) != Some(&page.text) {
                self.saving.conflicts.insert(page_id.to_owned(), page.text);
                self.mark_changed();
            }
            return;
        }
        self.saving.disk_hash.insert(page_id.to_owned(), theirs);
        if dirty {
            // The outside version is what the user has: nothing left to save.
            self.saving.dirty.remove(page_id);
            self.saving.conflicts.remove(page_id);
            self.note_clean(page_id);
        } else {
            self.reload_page(page_id, page);
        }
        self.mark_changed();
    }

    /// Replace the text of a clean page with a newer version from disk. The undo steps of
    /// the page refer to the old text, so they are dropped (logged, no toast).
    fn reload_page(&mut self, page_id: &str, page: PageText) {
        tracing::info!(page = %page_id, "page changed outside NoteCraft; reloaded it");
        self.history.clear_page(page_id);
        match page.warning() {
            Some(text) => {
                self.page_notices.insert(
                    page_id.to_owned(),
                    super::PageNotice {
                        text,
                        read_only: false,
                    },
                );
            }
            None => {
                self.page_notices.remove(page_id);
            }
        }
        self.load_canvas(page_id, &page.text, page.truncated);
        self.set_page_source(page_id, page.text);
    }

    /// The outside version of the page shown now, when it clashed with unsaved changes.
    pub fn current_conflict(&self) -> Option<&str> {
        let id = self.selection.page.as_deref()?;
        self.saving.conflicts.get(id).map(String::as_str)
    }

    /// Settle the conflict on the page shown now. `keep_mine` keeps the user's text (it is
    /// saved over the outside version on the next save); otherwise the outside version
    /// replaces it and the page's undo steps are dropped. Returns false when there is no
    /// conflict to settle.
    pub fn resolve_current_conflict(&mut self, keep_mine: bool) -> bool {
        let Some(id) = self.selection.page.clone() else {
            return false;
        };
        let Some(theirs) = self.saving.conflicts.remove(&id) else {
            return false;
        };
        self.saving.disk_hash.insert(id.clone(), hash_text(&theirs));
        if keep_mine {
            self.saving.dirty.insert(id);
        } else {
            self.saving.dirty.remove(&id);
            self.note_clean(&id);
            self.history.clear_page(&id);
            self.load_canvas(&id, &theirs, false);
            self.set_page_source(&id, theirs);
        }
        self.mark_changed();
        true
    }

    // ----- crash recovery -----

    /// Offer the journal entries that belong to open notebooks and differ from the file on
    /// disk. Returns the files whose entries hold nothing new, so the caller can delete them.
    /// Entries for notebooks that are not open stay offered later (call again after opening).
    pub fn offer_recovery(
        &mut self,
        entries: Vec<notecraft_storage::RecoveryEntry>,
    ) -> Vec<PathBuf> {
        let mut stale = Vec::new();
        for entry in entries {
            let Some(page_id) = self.page_id_for_path(&entry.path) else {
                continue;
            };
            let Resolved::File(path) = self.resolve_page_file(&page_id) else {
                continue;
            };
            let disk = match read_page_file(&path) {
                Ok(disk) => disk,
                Err(PageReadError::NotFound) => continue,
                Err(err) => {
                    tracing::warn!(page = %page_id, "cannot compare a recovery entry: {err}");
                    continue;
                }
            };
            if disk.text == entry.text {
                stale.push(entry.path);
                continue;
            }
            self.saving.recovery.retain(|item| item.page_id != page_id);
            self.saving.recovery.push(RecoveryItem {
                page_id,
                path: entry.path,
                text: entry.text,
                disk_hash: hash_text(&disk.text),
            });
            self.mark_changed();
        }
        stale
    }

    /// Unsaved text from a previous run waiting for the user's answer.
    pub fn pending_recovery(&self) -> &[RecoveryItem] {
        &self.saving.recovery
    }

    /// Put the recovered text back as unsaved changes (the next save writes it). Returns how
    /// many pages were restored.
    pub fn restore_recovery(&mut self) -> usize {
        let items = std::mem::take(&mut self.saving.recovery);
        let mut restored = 0;
        for item in items {
            if self.is_read_only(&item.page_id) {
                continue;
            }
            self.saving
                .disk_hash
                .insert(item.page_id.clone(), item.disk_hash);
            // The journal holds the whole file text, so the strokes come back with the text.
            self.load_canvas_from_journal(&item.page_id, &item.text);
            self.set_page_source(&item.page_id, item.text);
            self.saving.dirty.insert(item.page_id);
            restored += 1;
        }
        self.mark_changed();
        restored
    }

    /// Throw the recovered text away. Returns how many pages were discarded.
    pub fn discard_recovery(&mut self) -> usize {
        let items = std::mem::take(&mut self.saving.recovery);
        let count = items.len();
        for item in items {
            self.saving.journal_clean.push(item.path);
        }
        self.mark_changed();
        count
    }
}
