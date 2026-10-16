//! Loading `.notecraft` pages and saving pages with their ink (PR 1.9).
//!
//! A `.notecraft` page is held in the session as its whole file text (frontmatter, containers,
//! and the canvas block); the canvas block is read once on load and its strokes go into the
//! page's ink layer. [`Session::save_page`] writes the text and the strokes back with minimal
//! diffs (only the `"ink"` value of the block changes) through `craft-atomic`.
//!
//! Plugging in auto-save (PR 1.7): the auto-save worker calls `save_page(id)` for each page
//! that `mark_page_dirty` reported, instead of writing the text itself, so ink and text go out
//! in one write. A `.md` page that gets its first stroke is promoted to `.notecraft` by that
//! same call; the result says the page id changed, so the caller refreshes what it holds.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use notecraft_canvas::ink::InkLayer;
use notecraft_markdown::{apply_edits, remove_container_markers};
use notecraft_model::SectionNode;
use notecraft_storage::{
    change_page_kind, keep_unreadable_copy, read_canvas, strip_canvas_block, write_ink,
    write_page_text, PageKind, WriteOutcome,
};

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

/// What [`Session::save_page`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    /// The file already held exactly this page.
    Unchanged,
    /// The file was written.
    Written,
    /// A `.md` page was promoted to `.notecraft` because it has ink. The page id changed.
    Promoted { new_id: String },
}

/// Why a page was not saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveError {
    /// The page is not from a notebook folder (a mock page), or its text was never read.
    NotSaveable,
    /// The page was cut short or its canvas data was not understood; it is not saved over.
    ReadOnly,
    /// The file could not be written. Carries a plain-text reason.
    Io(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSaveable => f.write_str("this page cannot be saved"),
            Self::ReadOnly => f.write_str("this page is read-only"),
            Self::Io(why) => write!(f, "the page could not be written: {why}"),
        }
    }
}

impl std::error::Error for SaveError {}

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
    /// Read the canvas block of a `.notecraft` page and put its strokes in the page's ink
    /// layer. A block that cannot be used gives a read-only notice, keeps a copy of the file
    /// under `.notecraft/history/`, and leaves the page in flow layout with its text.
    pub(super) fn load_canvas(&mut self, page_id: &str, text: &str, root: PathBuf, rel: &Path) {
        let read = read_canvas(text);
        if let Some(problem) = read.problem {
            let notice = PageNotice {
                text: problem.message(),
                read_only: problem.blocks_saving(),
            };
            // Keep the first notice (a size warning) and add the canvas one after it.
            let merged = match self.page_notices.remove(page_id) {
                Some(old) => PageNotice {
                    text: format!("{} {}", old.text, notice.text),
                    read_only: old.read_only || notice.read_only,
                },
                None => notice,
            };
            self.page_notices.insert(page_id.to_owned(), merged);
            snapshot_unreadable(&root, rel, text);
        }
        if read.strokes.is_empty() {
            return;
        }
        let layer = InkLayer::from_strokes(read.strokes);
        self.ink_mut()
            .layers
            .insert(page_id.to_owned(), Arc::new(layer));
    }

    /// The notebook folder and the path inside it for page `page_id`.
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

    /// Write page `page_id` to its file: its text and its strokes. Use it for every save
    /// (auto-save, a save command, closing). Never panics; see [`SaveError`].
    pub fn save_page(&mut self, page_id: &str) -> Result<SaveOutcome, SaveError> {
        let (notebook, root, rel) = self.page_location(page_id).ok_or(SaveError::NotSaveable)?;
        let kind = kind_of(Path::new(&rel)).ok_or(SaveError::NotSaveable)?;
        if self.page_notices.get(page_id).is_some_and(|n| n.read_only) {
            return Err(SaveError::ReadOnly);
        }
        let text = self
            .page_text(page_id)
            .ok_or(SaveError::NotSaveable)?
            .to_owned();
        let strokes: Vec<_> = self
            .ink()
            .layer(page_id)
            .map(|layer| layer.strokes().to_vec())
            .unwrap_or_default();
        let io_err = |err: std::io::Error| SaveError::Io(err.to_string());
        let path = root.join(&rel);
        match kind {
            PageKind::Markdown if strokes.is_empty() => {
                match write_page_text(&path, &text).map_err(io_err)? {
                    WriteOutcome::Unchanged => Ok(SaveOutcome::Unchanged),
                    WriteOutcome::Written => Ok(SaveOutcome::Written),
                }
            }
            PageKind::Markdown => {
                // First ink on a markdown page: write the canvas block, move to `.notecraft`.
                let with_ink = write_ink(&text, &strokes).map_err(|_| SaveError::ReadOnly)?;
                let new_rel = change_page_kind(&root, &rel, &with_ink).map_err(io_err)?;
                let new_id = format!("{notebook}/{new_rel}");
                self.rekey_page(page_id, &new_id);
                self.set_page_source(&new_id, with_ink);
                Ok(SaveOutcome::Promoted { new_id })
            }
            PageKind::Canvas => {
                let out = write_ink(&text, &strokes).map_err(|_| SaveError::ReadOnly)?;
                let outcome = write_page_text(&path, &out).map_err(io_err)?;
                if out != text {
                    self.set_page_source(page_id, out);
                }
                Ok(match outcome {
                    WriteOutcome::Unchanged => SaveOutcome::Unchanged,
                    WriteOutcome::Written => SaveOutcome::Written,
                })
            }
        }
    }

    /// `page.convert_to_markdown`: turn the page shown into a plain `.md` page. Without
    /// `confirm`, a page that would lose ink or positions only reports the warning.
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
        if self
            .page_notices
            .get(&page_id)
            .is_some_and(|notice| notice.read_only)
        {
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
        match change_page_kind(&root, &rel, &markdown) {
            Ok(new_rel) => {
                let new_id = format!("{notebook}/{new_rel}");
                self.rekey_page(&page_id, &new_id);
                self.ink_mut().layers.remove(&new_id);
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

    /// Move everything the session keeps about page `old` to id `new` (its file moved to
    /// a new file name). The page title is the file name without its extension, so it does
    /// not change. The undo history is cleared: its steps name the old id.
    fn rekey_page(&mut self, old: &str, new: &str) {
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
        self.history.clear();
        self.mark_changed();
    }
}

/// Keep a copy of a `.notecraft` file whose canvas block could not be read (docs/markdown.md
/// section 3.4). Best effort: a failure is only logged.
fn snapshot_unreadable(root: &Path, rel: &Path, text: &str) {
    if let Err(err) = keep_unreadable_copy(root, rel, text) {
        tracing::warn!("cannot keep a copy of the unreadable page: {err}");
    }
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
