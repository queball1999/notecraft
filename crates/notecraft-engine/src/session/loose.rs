//! The loose page view: a single page file opened on its own, outside any notebook folder
//! (docs/architecture.md, "Opening a page file"). OneNote has no such view; it is a kept
//! extra (docs/post-parity.md).
//!
//! A loose page is modeled as a one-section, one-page notebook whose folder is the file's
//! folder, so reading, saving, text editing, and attachments take the same paths as any other
//! page. Three things differ, all enforced by the session:
//!
//! - nothing is written next to the file except the file itself: no `.notecraft/` folder, no
//!   index, no watcher (the loose root is left out of [`Session::notebook_roots`] and
//!   [`Session::notebook_folders`]);
//! - structure edits (new, rename, move, delete of pages and sections) are refused, because
//!   the folder is not a notebook;
//! - the notebook id is `loose-` and 16 hex digits of a hash of the file path, so it is the
//!   same on every open of the same file.

use std::path::Path;

use notecraft_model::{Notebook, PageRef, Section, SectionNode};

use super::{Selection, Session};
use crate::mock;

/// Notebook swatch for a loose page.
const LOOSE_COLOR: u32 = 0x78909C;

impl Session {
    /// Whether notebook `id` is a loose page view.
    pub fn is_loose_notebook(&self, id: &str) -> bool {
        self.loose.contains(id)
    }

    /// The id of the open notebook (not a loose page) whose folder is `root`.
    pub fn notebook_id_for_root(&self, root: &Path) -> Option<String> {
        let canonical = root.canonicalize().ok();
        self.notebook_roots.iter().find_map(|(id, folder)| {
            if self.loose.contains(id) {
                return None;
            }
            let same = folder == root
                || canonical
                    .as_ref()
                    .is_some_and(|c| folder.canonicalize().ok().as_ref() == Some(c));
            same.then(|| id.clone())
        })
    }

    /// Show page `page`, switching to its notebook and section. Returns whether any open
    /// notebook has the page.
    pub fn select_page_by_id(&mut self, page: &str) -> bool {
        let found = self.notebooks.iter().find_map(|nb| {
            nb.section_of_page(page)
                .map(|s| (nb.id.clone(), s.id.clone()))
        });
        let Some((notebook, section)) = found else {
            return false;
        };
        self.selection = Selection {
            notebook: Some(notebook),
            section: Some(section),
            page: Some(page.to_owned()),
        };
        true
    }

    /// Open the page file `file` as a loose page and show it, or show it again if it is
    /// already open that way. Returns the page id, or `None` when `file` has no folder or
    /// no usable name. The file is not read here; [`Session::load_current_page`] does that.
    pub fn open_loose_page(&mut self, file: &Path) -> Option<String> {
        let folder = file.parent()?;
        let name = file.file_name()?.to_str()?;
        let stem = file.file_stem()?.to_str()?;
        let notebook_id = loose_id(file);
        let page_id = format!("{notebook_id}/{name}");
        if !self.notebooks.iter().any(|nb| nb.id == notebook_id) {
            let folder_name = folder
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Page")
                .to_owned();
            self.notebooks.retain(|nb| !mock::is_mock_id(&nb.id));
            self.notebooks.push(Notebook {
                id: notebook_id.clone(),
                name: stem.to_owned(),
                color: LOOSE_COLOR,
                sections: vec![SectionNode::Section(Section {
                    id: format!("{notebook_id}/{folder_name}"),
                    name: folder_name,
                    color: LOOSE_COLOR,
                    pages: vec![PageRef::new(page_id.clone(), stem, 0)],
                })],
            });
            self.notebook_roots
                .insert(notebook_id.clone(), folder.to_path_buf());
            self.loose.insert(notebook_id);
        }
        self.select_page_by_id(&page_id).then_some(page_id)
    }
}

/// The notebook id of the loose page at `file`.
fn loose_id(file: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in file.to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("loose-{hash:016x}")
}
