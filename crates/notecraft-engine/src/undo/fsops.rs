//! Reversible file system operations for structure editing (docs/architecture.md section 5).
//!
//! The disk is the truth for a notebook's tree. Each operation changes the disk first and marks
//! its notebook dirty; the session reads the folder again once the whole step has applied (or
//! been rolled back), so the tree in memory never drifts from the files. Paths are relative to
//! the notebook folder, with `/` separators, and are checked to stay inside it.
//!
//! Nothing here deletes user content. Taking something away is a [`FsOp::Move`] into
//! `.notecraft/trash`, so its inverse is just the move back.

use std::path::{Component, Path, PathBuf};

use notecraft_storage::structure::{
    copy_file_new, create_dir_new, create_file_new, rename_new, write_text, TRASH_DIR,
};

use crate::session::Session;

use super::Op;

/// What a [`FsOp::Create`] makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Content {
    /// A folder.
    Dir,
    /// A file holding this text.
    Text(String),
    /// A copy of the file at this notebook-relative path.
    CopyOf(String),
}

/// One reversible change to the files of a notebook.
#[derive(Clone, Debug, PartialEq)]
pub enum FsOp {
    /// Make `rel`. If the entry the inverse of this op moved to `slot` is still there (this
    /// is a redo), it is moved back instead, so the content comes back exactly as it was.
    /// The inverse moves `rel` to `slot`, a path in the trash folder.
    Create {
        notebook: String,
        rel: String,
        content: Content,
        slot: String,
    },
    /// Rename or move a file or folder; ids of pages and sections under it follow. Refused if
    /// the target exists.
    Move {
        notebook: String,
        from: String,
        to: String,
    },
    /// Replace a settings file: `before` is its text now (`None` when it is absent), `after`
    /// the text to write (`None` removes it). The inverse swaps them.
    WriteText {
        notebook: String,
        rel: String,
        before: Option<String>,
        after: Option<String>,
    },
    /// Replace `removed` at byte offset `at` of the page file `rel` with `inserted`: one
    /// changed link when a rename updates links (build_plan/04 PR 2.3). A page that is open
    /// is edited in memory (and saved with the usual checks); any other page is rewritten on
    /// disk, only if its text still holds `removed` there. Refused for a read-only page.
    PatchText {
        notebook: String,
        rel: String,
        at: usize,
        removed: String,
        inserted: String,
    },
    /// Take a notebook out of the open list without touching its folder.
    Unlist {
        notebook: String,
        root: PathBuf,
        index: usize,
    },
    /// Put an unlisted notebook back, read again from `root`.
    Relist {
        notebook: String,
        root: PathBuf,
        index: usize,
    },
}

impl FsOp {
    pub(super) fn inverse(&self) -> FsOp {
        match self {
            FsOp::Create {
                notebook,
                rel,
                slot,
                ..
            } => FsOp::Move {
                notebook: notebook.clone(),
                from: rel.clone(),
                to: slot.clone(),
            },
            FsOp::Move { notebook, from, to } => FsOp::Move {
                notebook: notebook.clone(),
                from: to.clone(),
                to: from.clone(),
            },
            FsOp::WriteText {
                notebook,
                rel,
                before,
                after,
            } => FsOp::WriteText {
                notebook: notebook.clone(),
                rel: rel.clone(),
                before: after.clone(),
                after: before.clone(),
            },
            FsOp::PatchText {
                notebook,
                rel,
                at,
                removed,
                inserted,
            } => FsOp::PatchText {
                notebook: notebook.clone(),
                rel: rel.clone(),
                at: *at,
                removed: inserted.clone(),
                inserted: removed.clone(),
            },
            FsOp::Unlist {
                notebook,
                root,
                index,
            } => FsOp::Relist {
                notebook: notebook.clone(),
                root: root.clone(),
                index: *index,
            },
            FsOp::Relist {
                notebook,
                root,
                index,
            } => FsOp::Unlist {
                notebook: notebook.clone(),
                root: root.clone(),
                index: *index,
            },
        }
    }

    pub(super) fn bytes(&self) -> usize {
        const BASE: usize = 64;
        match self {
            FsOp::Create {
                rel,
                content,
                slot,
                notebook,
            } => {
                let extra = match content {
                    Content::Dir => 0,
                    Content::Text(text) | Content::CopyOf(text) => text.len(),
                };
                BASE + notebook.len() + rel.len() + slot.len() + extra
            }
            FsOp::Move { notebook, from, to } => BASE + notebook.len() + from.len() + to.len(),
            FsOp::WriteText {
                notebook,
                rel,
                before,
                after,
            } => {
                let size = |text: &Option<String>| text.as_ref().map_or(0, String::len);
                BASE + notebook.len() + rel.len() + size(before) + size(after)
            }
            FsOp::PatchText {
                notebook,
                rel,
                removed,
                inserted,
                ..
            } => BASE + notebook.len() + rel.len() + removed.len() + inserted.len(),
            FsOp::Unlist { notebook, .. } | FsOp::Relist { notebook, .. } => BASE + notebook.len(),
        }
    }

    /// Apply the operation. False, changing nothing, when it does not fit the files as they
    /// are now (a target exists, a source is gone, a path leaves the notebook).
    pub(crate) fn apply(&self, session: &mut Session) -> bool {
        let applied = match self {
            FsOp::Create {
                notebook,
                rel,
                content,
                slot,
            } => create(session, notebook, rel, content, slot),
            FsOp::Move { notebook, from, to } => relocate(session, notebook, from, to),
            FsOp::WriteText {
                notebook,
                rel,
                after,
                ..
            } => write(session, notebook, rel, after.as_deref()),
            FsOp::PatchText {
                notebook,
                rel,
                at,
                removed,
                inserted,
            } => patch(session, notebook, rel, *at, removed, inserted),
            FsOp::Unlist {
                notebook, index, ..
            } => session.unlist_notebook(notebook, *index),
            FsOp::Relist {
                notebook,
                root,
                index,
            } => session.relist_notebook(notebook, root, *index),
        };
        if let Err(reason) = &applied {
            tracing::warn!("structure step refused: {reason}");
        }
        applied.is_ok()
    }
}

impl From<FsOp> for Op {
    fn from(op: FsOp) -> Self {
        Op::Fs(op)
    }
}

/// `root` joined with `rel`, if `rel` is a plain relative path with no `..`.
fn resolve(root: &Path, rel: &str) -> Option<PathBuf> {
    let path = Path::new(rel);
    let plain = !rel.is_empty() && path.components().all(|c| matches!(c, Component::Normal(_)));
    plain.then(|| root.join(path))
}

fn root_of(session: &Session, notebook: &str) -> Result<PathBuf, String> {
    session
        .notebook_root(notebook)
        .map(Path::to_path_buf)
        .ok_or_else(|| "the notebook has no folder".to_owned())
}

fn path_of(root: &Path, rel: &str) -> Result<PathBuf, String> {
    resolve(root, rel).ok_or_else(|| format!("{rel:?} is not a path inside the notebook"))
}

fn create(
    session: &mut Session,
    notebook: &str,
    rel: &str,
    content: &Content,
    slot: &str,
) -> Result<(), String> {
    let root = root_of(session, notebook)?;
    let target = path_of(&root, rel)?;
    let slot_path = path_of(&root, slot)?;
    let done = if slot_path.exists() {
        // A redo: the content comes back from the trash, and so does what is stored per id.
        let moved = rename_new(&slot_path, &target);
        if moved.is_ok() {
            session.rekey_ids(notebook, slot, rel);
        }
        moved
    } else {
        match content {
            Content::Dir => create_dir_new(&target),
            Content::Text(text) => create_file_new(&target, text),
            Content::CopyOf(source) => copy_file_new(&path_of(&root, source)?, &target),
        }
    };
    done.map_err(|err| err.to_string())?;
    session.mark_dirty(notebook);
    Ok(())
}

fn relocate(session: &mut Session, notebook: &str, from: &str, to: &str) -> Result<(), String> {
    let root = root_of(session, notebook)?;
    let (source, target) = (path_of(&root, from)?, path_of(&root, to)?);
    if target.starts_with(&source) {
        return Err("a folder cannot move into itself".to_owned());
    }
    rename_new(&source, &target).map_err(|err| err.to_string())?;
    session.rekey_ids(notebook, from, to);
    session.mark_dirty(notebook);
    // A trash slot emptied by a move back does not stay behind as a bare folder.
    remove_empty_trash_parent(&root, from);
    Ok(())
}

fn write(
    session: &mut Session,
    notebook: &str,
    rel: &str,
    text: Option<&str>,
) -> Result<(), String> {
    let root = root_of(session, notebook)?;
    let path = path_of(&root, rel)?;
    write_text(&path, text).map_err(|err| err.to_string())?;
    session.mark_dirty(notebook);
    if text.is_none() {
        remove_empty_trash_parent(&root, rel);
    }
    Ok(())
}

/// `text` with `removed` at `at` replaced by `inserted`; `None` when the text there differs.
fn spliced(text: &str, at: usize, removed: &str, inserted: &str) -> Option<String> {
    let end = at.checked_add(removed.len())?;
    if text.get(at..end) != Some(removed) {
        return None;
    }
    let mut next = String::with_capacity(text.len() + inserted.len());
    next.push_str(text.get(..at)?);
    next.push_str(inserted);
    next.push_str(text.get(end..)?);
    Some(next)
}

fn patch(
    session: &mut Session,
    notebook: &str,
    rel: &str,
    at: usize,
    removed: &str,
    inserted: &str,
) -> Result<(), String> {
    let root = root_of(session, notebook)?;
    let path = path_of(&root, rel)?;
    let id = format!("{notebook}/{rel}");
    if let Some(text) = session.page_text(&id) {
        // An open page: edit its text; it saves like any other edit.
        if session.page_is_read_only(&id) || session.page_has_conflict(&id) {
            return Err(format!("{rel} cannot be changed now"));
        }
        let next = spliced(text, at, removed, inserted)
            .ok_or_else(|| format!("{rel} changed; its link was not updated"))?;
        session.replace_page_text(&id, next);
        session.mark_page_dirty(&id);
        return Ok(());
    }
    let page = notecraft_storage::read_page_file(&path).map_err(|err| err.to_string())?;
    if page.read_only() {
        return Err(format!("{rel} is read-only"));
    }
    let next = spliced(&page.text, at, removed, inserted)
        .ok_or_else(|| format!("{rel} changed; its link was not updated"))?;
    notecraft_storage::write_page_text(&path, &next).map_err(|err| err.to_string())?;
    Ok(())
}

/// If `rel` is inside the trash folder, remove its folder when that is empty now.
fn remove_empty_trash_parent(root: &Path, rel: &str) {
    let Some((parent, _)) = rel.rsplit_once('/') else {
        return;
    };
    let inside = parent
        .strip_prefix(TRASH_DIR)
        .is_some_and(|rest| rest.starts_with('/'));
    if !inside {
        return;
    }
    if let Some(dir) = resolve(root, parent) {
        // Fails, harmlessly, while something is still in it.
        let _ = std::fs::remove_dir(dir);
    }
}
