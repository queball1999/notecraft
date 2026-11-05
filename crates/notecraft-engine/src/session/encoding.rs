//! Pages that are not saved as UTF-8 text: they open read-only, decoded for display, and
//! `page.convert_encoding` rewrites them as UTF-8 after keeping a backup of the original bytes
//! (docs/markdown.md section 1).

use std::path::{Path, PathBuf};

use notecraft_storage::{
    decode_page_bytes, keep_original_bytes, read_page_bytes, write_page_text, PageEncoding,
    NOT_UTF8_NOTICE,
};

use super::saving::{hash_text, Resolved};
use super::{PageNotice, Session};

/// What `page.convert_encoding` did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Utf8Outcome {
    /// The page shown is not a page that needs converting.
    NotApplicable,
    /// The page was rewritten as UTF-8 and is editable now. Holds the path of the backup of
    /// the original bytes.
    Converted { backup: PathBuf },
    /// Nothing was changed; holds a short reason (also shown on the page).
    Failed(String),
}

impl Session {
    /// True when the page shown is read-only only because its file is not UTF-8 text, so
    /// `page.convert_encoding` can offer to convert it.
    pub fn can_convert_to_utf8(&self) -> bool {
        self.selection
            .page
            .as_deref()
            .is_some_and(|id| self.saving.legacy.contains_key(id) && self.page_text(id).is_some())
    }

    /// How the page shown was decoded, when it is read-only only because its file is not
    /// UTF-8 text (the bar names it, so the user can tell whether the guess was right before
    /// converting).
    pub fn current_page_legacy_encoding(&self) -> Option<PageEncoding> {
        if !self.can_convert_to_utf8() {
            return None;
        }
        let id = self.selection.page.as_deref()?;
        self.saving.legacy.get(id).copied()
    }

    /// `page.convert_encoding`: keep the original bytes of the page shown, rewrite its file as
    /// UTF-8 with the same text (line endings kept), and make the page editable. The file is
    /// read again first; if it no longer holds the text the page shows, nothing is written.
    /// The page's undo steps are dropped.
    pub fn convert_current_page_to_utf8(&mut self) -> Utf8Outcome {
        let Some(page_id) = self.selection.page.clone() else {
            return Utf8Outcome::NotApplicable;
        };
        if !self.can_convert_to_utf8() {
            return Utf8Outcome::NotApplicable;
        }
        match self.convert_to_utf8(&page_id) {
            Ok(backup) => Utf8Outcome::Converted { backup },
            Err(reason) => {
                tracing::warn!(page = %page_id, "cannot convert the page to UTF-8: {reason}");
                self.page_notices.insert(
                    page_id,
                    PageNotice {
                        text: format!("{NOT_UTF8_NOTICE} It could not be converted: {reason}."),
                        read_only: true,
                    },
                );
                self.mark_changed();
                Utf8Outcome::Failed(reason)
            }
        }
    }

    fn convert_to_utf8(&mut self, page_id: &str) -> Result<PathBuf, String> {
        let Resolved::File(path) = self.resolve_page_file(page_id) else {
            return Err("the page has no file".to_owned());
        };
        let bytes = match read_page_bytes(&path) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Err("the file is too large".to_owned()),
            Err(err) => return Err(format!("the file cannot be read ({err})")),
        };
        let (text, encoding) = decode_page_bytes(&bytes);
        if encoding == PageEncoding::Utf8 || self.page_text(page_id) != Some(text.as_str()) {
            return Err("the file changed outside NoteCraft".to_owned());
        }
        let location = self.page_location(page_id);
        let notebook = location
            .as_ref()
            .map(|(_, root, rel)| (root.as_path(), Path::new(rel.as_str())));
        let backup = keep_original_bytes(&path, notebook, &bytes)
            .map_err(|err| format!("the original could not be kept ({err})"))?;
        write_page_text(&path, &text)
            .map_err(|err| format!("the file cannot be written ({err})"))?;
        self.saving.legacy.remove(page_id);
        self.saving
            .disk_hash
            .insert(page_id.to_owned(), hash_text(&text));
        self.page_notices.remove(page_id);
        self.history.clear_page(page_id);
        // Notices that come from the page's own content (an unreadable canvas block) return.
        self.load_canvas(page_id, &text, false);
        self.mark_changed();
        Ok(backup)
    }
}
