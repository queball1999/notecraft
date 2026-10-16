//! `.notecraft` pages in the session: loading their strokes, the file text a save writes,
//! promotion of a `.md` page on its first stroke, and conversion back to markdown (PR 1.9).
//!
//! A `.notecraft` page is held in the session as its whole file text (frontmatter, containers,
//! and the canvas block); the canvas block is read once on load and its strokes go into the
//! page's ink layer. There is one save path, [`Session::save_page`] in `saving.rs`: it asks
//! this module for the text the file should hold ([`Session::file_text`], the page text with
//! the strokes written into the canvas block by `notecraft_storage::write_ink`, changing only
//! the `"ink"` value), checks the file on disk, and writes it atomically. A `.md` page that
//! has strokes is promoted: the file is written as `.notecraft` and the `.md` file removed,
//! and everything the session keeps for the page follows the new id.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use notecraft_canvas::ink::InkLayer;
use notecraft_markdown::{apply_edits, remove_container_markers};
use notecraft_model::SectionNode;
use notecraft_storage::{
    change_page_kind, keep_unreadable_copy, read_canvas, strip_canvas_block, write_ink, PageKind,
};

use super::saving::{hash_text, SaveOutcome};
use super::{PageNotice, Session};

/// The kind of page file at `rel`, by extension; `None` for other files.
pub(super) fn kind_of(rel: &Path) -> Option<PageKind> {
    let ext = rel.extension()?.to_str()?;
    if ext.eq_ignore_ascii_case("md") {
        Some(PageKind::Markdown)
    } else if ext.eq_ignore_ascii_case("notecraft") {
        Some(PageKind::Canvas)
    } else {
        None
    }
}

/// What `page.convert_to_markdown` found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConvertOutcome {
    /// The page is already markdown, or none is shown.
    NotApplicable,
    /// The page has ink or positions that would be lost; ask first.
    NeedsConfirm { warning: String },
    /// The page is now a `.md` file with this page id.
    Converted { new_id: String },
}

/// The warning shown before a page is converted to markdown.
pub const CONVERT_WARNING: &str = "Converting this page to markdown removes its ink and the positions of its notes. The text is kept. This cannot be undone.";

impl Session {
    /// The notebook id, folder, and path inside it for page `page_id`.
    fn page_location(&self, page_id: &str) -> Option<(String, PathBuf, String)> {
        let (notebook, root) = self
            .notebook_roots
            .iter()
            .filter(|(id, _)| {
                page_id
                    .strip_prefix(id.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
            })
            .max_by_key(|(id, _)| id.len())?;
        let rel = page_id.get(notebook.len() + 1..)?;
        Some((notebook.clone(), root.clone(), rel.to_owned()))
    }

    fn kind_of_page(&self, page_id: &str) -> Option<PageKind> {
        let (_, _, rel) = self.page_location(page_id)?;
        kind_of(Path::new(&rel))
    }

    /// Read the canvas block of a `.notecraft` page's `text` and make its strokes the page's
    /// ink layer (replacing what was there). A block that cannot be used gives a read-only
    /// notice, keeps a copy of the file under `.notecraft/history/`, and leaves the page in
    /// flow layout with its text. Markdown pages and cut-short reads are left alone.
    pub(super) fn load_canvas(&mut self, page_id: &str, text: &str, truncated: bool) {
        if self.kind_of_page(page_id) == Some(PageKind::Canvas) {
            self.load_canvas_block(page_id, text, truncated);
        }
    }

    /// Like [`Self::load_canvas`] for text from the recovery journal, which holds the whole
    /// file as it would be saved, canvas block included, even for a `.md` page that had its
    /// first stroke.
    pub(super) fn load_canvas_from_journal(&mut self, page_id: &str, text: &str) {
        if self.kind_of_page(page_id).is_some() {
            self.load_canvas_block(page_id, text, false);
        }
    }

    fn load_canvas_block(&mut self, page_id: &str, text: &str, truncated: bool) {
        if truncated {
            return;
        }
        let read = read_canvas(text);
        if let Some(problem) = read.problem {
            let notice = PageNotice {
                text: problem.message(),
                read_only: problem.blocks_saving(),
            };
            // Keep an earlier notice (a size warning) and add the canvas one after it.
            let merged = match self.page_notices.remove(page_id) {
                Some(old) => PageNotice {
                    text: format!("{} {}", old.text, notice.text),
                    read_only: old.read_only || notice.read_only,
                },
                None => notice,
            };
            self.page_notices.insert(page_id.to_owned(), merged);
            if let Some((_, root, rel)) = self.page_location(page_id) {
                if let Err(err) = keep_unreadable_copy(&root, Path::new(&rel), text) {
                    tracing::warn!("cannot keep a copy of the unreadable page: {err}");
                }
            }
        }
        if read.strokes.is_empty() {
            self.ink_mut().layers.remove(page_id);
        } else {
            let layer = InkLayer::from_strokes(read.strokes);
            self.ink_mut()
                .layers
                .insert(page_id.to_owned(), Arc::new(layer));
        }
    }

    /// The text page `page_id`'s file should hold: the page text with the strokes written into
    /// its canvas block. `None` for a page with no text; `Some(Err(()))` when the canvas block
    /// is one that must not be rewritten.
    pub(super) fn file_text(&self, page_id: &str) -> Option<Result<String, ()>> {
        let text = self.page_text(page_id)?;
        let kind = self.kind_of_page(page_id)?;
        let strokes = self.ink().layer(page_id).map_or(&[][..], InkLayer::strokes);
        if kind == PageKind::Markdown && strokes.is_empty() {
            return Some(Ok(text.to_owned()));
        }
        Some(write_ink(text, strokes).map_err(|_| ()))
    }

    /// True for a `.md` page that has strokes, which must be saved as `.notecraft`.
    pub(super) fn needs_promotion(&self, page_id: &str) -> bool {
        self.kind_of_page(page_id) == Some(PageKind::Markdown)
            && self.ink().layer(page_id).is_some_and(|l| !l.is_empty())
    }

    /// After a save wrote `text` (the page text with the canvas block), make the session text
    /// match it, unless it already does (so a plain markdown page keeps its revision).
    pub(super) fn adopt_saved_text(&mut self, page_id: &str, text: String) {
        if self.page_text(page_id) != Some(text.as_str()) {
            self.set_page_source(page_id, text);
        }
    }

    /// Save `text` as the `.notecraft` page that replaces the `.md` page `page_id`.
    pub(super) fn promote_and_save(&mut self, page_id: &str, text: String) -> SaveOutcome {
        let Some((notebook, root, rel)) = self.page_location(page_id) else {
            return SaveOutcome::NoFile;
        };
        let old_path = root.join(&rel);
        match change_page_kind(&root, &rel, &text) {
            Ok(new_rel) => {
                let new_id = format!("{notebook}/{new_rel}");
                self.saving.journal_clean.push(old_path);
                self.rekey_page(page_id, &new_id);
                self.saving
                    .disk_hash
                    .insert(new_id.clone(), hash_text(&text));
                self.saving.dirty.remove(&new_id);
                self.adopt_saved_text(&new_id, text);
                self.mark_changed();
                SaveOutcome::Promoted { new_id }
            }
            Err(err) => {
                tracing::warn!(page = %page_id, "cannot promote page: {err}");
                SaveOutcome::Failed(err.to_string())
            }
        }
    }

    /// `page.convert_to_markdown`: turn the page shown into a plain `.md` page. Without
    /// `confirm`, only the warning to show first is reported.
    pub fn convert_current_page_to_markdown(&mut self, confirm: bool) -> ConvertOutcome {
        let Some(page_id) = self.selection.page.clone() else {
            return ConvertOutcome::NotApplicable;
        };
        let Some((notebook, root, rel)) = self.page_location(&page_id) else {
            return ConvertOutcome::NotApplicable;
        };
        if kind_of(Path::new(&rel)) != Some(PageKind::Canvas) {
            return ConvertOutcome::NotApplicable;
        }
        let Some(text) = self.page_text(&page_id).map(str::to_owned) else {
            return ConvertOutcome::NotApplicable;
        };
        let blocked = self
            .page_notices
            .get(&page_id)
            .is_some_and(|notice| notice.read_only)
            || self.saving.conflicts.contains_key(&page_id);
        if blocked {
            return ConvertOutcome::NotApplicable;
        }
        if !confirm {
            return ConvertOutcome::NeedsConfirm {
                warning: CONVERT_WARNING.to_owned(),
            };
        }
        let block_start = notecraft_storage::find_canvas_block(&text)
            .map_or(text.len(), |block| block.block.start);
        let markdown = apply_edits(&text, &remove_container_markers(&text, block_start))
            .unwrap_or_else(|_| text.clone());
        let markdown = strip_canvas_block(&markdown);
        let old_path = root.join(&rel);
        match change_page_kind(&root, &rel, &markdown) {
            Ok(new_rel) => {
                let new_id = format!("{notebook}/{new_rel}");
                self.saving.journal_clean.push(old_path);
                self.rekey_page(&page_id, &new_id);
                self.ink_mut().layers.remove(&new_id);
                self.saving
                    .disk_hash
                    .insert(new_id.clone(), hash_text(&markdown));
                self.saving.dirty.remove(&new_id);
                self.set_page_source(&new_id, markdown);
                self.mark_changed();
                ConvertOutcome::Converted { new_id }
            }
            Err(err) => {
                tracing::warn!(page = %page_id, "cannot convert page: {err}");
                ConvertOutcome::NotApplicable
            }
        }
    }

    /// Move everything the session keeps about page `old` to id `new` (its file got a new
    /// extension): the page list, selection, text, notice, style, strokes, and the save state
    /// (dirty mark, disk hash, conflict, pending recovery, queued reads). The page title is
    /// the file name without its extension, so it does not change. The undo history is
    /// cleared: its steps name the old id.
    pub(super) fn rekey_page(&mut self, old: &str, new: &str) {
        fn walk(nodes: &mut [SectionNode], old: &str, new: &str, depth: u8) {
            if depth > 16 {
                return;
            }
            for node in nodes {
                match node {
                    SectionNode::Section(section) => {
                        for page in &mut section.pages {
                            if page.id == old {
                                page.id = new.to_owned();
                            }
                        }
                    }
                    SectionNode::Group(group) => walk(&mut group.children, old, new, depth + 1),
                }
            }
        }
        for notebook in self.notebooks_mut() {
            walk(&mut notebook.sections, old, new, 0);
        }
        if self.selection.page.as_deref() == Some(old) {
            self.selection.page = Some(new.to_owned());
        }
        if let Some(value) = self.page_sources.remove(old) {
            self.page_sources.insert(new.to_owned(), value);
        }
        if let Some(value) = self.page_notices.remove(old) {
            self.page_notices.insert(new.to_owned(), value);
        }
        if let Some(value) = self.page_styles.remove(old) {
            self.page_styles.insert(new.to_owned(), value);
        }
        if let Some(value) = self.ink.layers.remove(old) {
            self.ink.layers.insert(new.to_owned(), value);
        }
        self.saving.rekey(old, new);
        self.history.clear_page(old);
        self.mark_changed();
    }
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
