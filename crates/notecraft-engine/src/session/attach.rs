//! Pictures and file attachments on the page shown now (docs/markdown.md section 2.4).
//!
//! An insert copies the file into the section's `attachments/` folder (or links it in place
//! when it is very large), then adds one paragraph at the end of the page text: `![name](link)`
//! for a picture, `[name](link)` for a file. The text change is one undo step. Undo takes the
//! paragraph back out; the copied file stays in `attachments/`, so Redo works and nothing the
//! user might have wanted is deleted. Clearing unused attachments is a later task.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use notecraft_markdown::{image, Editor};
use notecraft_storage::attachments::{
    decode_link_path, is_picture_name, store_attachment_with, AttachError, StoredAttachment,
    LINK_THRESHOLD_BYTES,
};
use notecraft_storage::find_canvas_block;

use super::Session;
use crate::undo::{Op, Scope, Transaction};

/// Whether an insert is a picture drawn on the page or a file shown as an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachKind {
    Picture,
    File,
}

/// What an insert did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attached {
    pub kind: AttachKind,
    /// The link written into the page, such as `attachments/3f9a2c-budget.xlsx`.
    pub link: String,
    /// The original file name.
    pub name: String,
    /// The file was linked in place, not copied (it is over the size limit).
    pub linked: bool,
}

/// Why an insert did nothing. The text is shown to the user as a short plain line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertError(pub String);

impl fmt::Display for InsertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InsertError {}

fn fail(text: &str) -> InsertError {
    InsertError(text.to_owned())
}

impl From<AttachError> for InsertError {
    fn from(err: AttachError) -> Self {
        Self(match err {
            AttachError::NotAFile => "that file could not be found.".to_owned(),
            other => format!("{other}."),
        })
    }
}

/// Characters that would end or break the link text of `[text](target)`.
fn link_text(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '[' => '(',
            ']' => ')',
            '\\' | '`' | '*' | '<' | '>' | '~' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let trimmed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    trimmed.chars().take(200).collect()
}

/// The file name without its extension, for the alt text of a picture.
fn stem_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(at) if at > 0 => name.get(..at).unwrap_or(name),
        _ => name,
    }
}

impl Session {
    /// The file of page `page_id` on disk, for a page of a notebook read from a folder.
    fn page_file(&self, page_id: &str) -> Option<PathBuf> {
        let notebook = self.selection.notebook.as_deref()?;
        let rest = page_id.strip_prefix(notebook)?.strip_prefix('/')?;
        let rel = Path::new(rest);
        if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            return None;
        }
        Some(self.notebook_roots.get(notebook)?.join(rel))
    }

    /// The folder of the page shown now (its section folder), where `attachments/` lives.
    fn page_folder(&self) -> Option<PathBuf> {
        let page = self.selection.page.as_deref()?;
        self.page_file(page)?.parent().map(Path::to_path_buf)
    }

    /// Copy `source` into the attachments folder next to the page shown now and add a link to
    /// it at the end of the page, as one undo step. A picture must be a PNG, JPEG, WebP, or SVG
    /// file. Nothing changes on failure.
    ///
    /// The engine call `insert.picture` and `insert.file_attachment` make; it is not a command
    /// itself.
    pub fn insert_attachment(
        &mut self,
        source: &Path,
        kind: AttachKind,
    ) -> Result<Attached, InsertError> {
        self.insert_attachment_with(source, kind, LINK_THRESHOLD_BYTES)
    }

    /// [`Session::insert_attachment`], linking in place only files over `link_over` bytes.
    /// A pasted picture passes `u64::MAX`: its source is a temporary file that is deleted
    /// right after, so it must always be copied.
    pub fn insert_attachment_with(
        &mut self,
        source: &Path,
        kind: AttachKind,
        link_over: u64,
    ) -> Result<Attached, InsertError> {
        let page = self
            .selection
            .page
            .clone()
            .ok_or_else(|| fail("open a page first."))?;
        let Some(folder) = self.page_folder() else {
            return Err(fail("open a notebook folder first."));
        };
        if self
            .page_notices
            .get(&page)
            .is_some_and(|notice| notice.read_only)
        {
            return Err(fail("this page is read-only."));
        }
        let Some(text) = self.page_text(&page) else {
            return Err(fail("this page cannot take pictures or files yet."));
        };
        let name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if kind == AttachKind::Picture && !is_picture_name(&name) {
            return Err(fail("pictures must be PNG, JPEG, WebP, or SVG files."));
        }
        let stored = store_attachment_with(&folder, source, link_over)?;
        let (at, inserted) = append_paragraph(text, &paragraph(kind, &stored));
        let label = match kind {
            AttachKind::Picture => "Insert picture",
            AttachKind::File => "Insert file",
        };
        let op = Op::ReplaceText {
            page: page.clone(),
            at,
            removed: String::new(),
            inserted,
        };
        if !self.commit(Transaction::new(Scope::Page(page), label).with(op)) {
            return Err(fail("the page could not be changed."));
        }
        Ok(Attached {
            kind,
            link: stored.link,
            name: stored.display_name,
            linked: stored.linked,
        })
    }

    /// Record the display width of a picture on the page shown now, as one undo step: the
    /// `nth` image (from 0, in page order) whose link is `src` gets `<!-- notecraft:width=N -->`
    /// right after it, or loses it when `width` is `None` (back to its natural size). Only
    /// that comment changes, so the page diff is a few bytes. A width outside the allowed
    /// range is clamped. Nothing happens, and no undo step is made, if the width is already
    /// that.
    ///
    /// The engine call `picture.resize` makes; it is not a command itself.
    pub fn resize_picture(
        &mut self,
        src: &str,
        nth: usize,
        width: Option<u16>,
    ) -> Result<(), InsertError> {
        let page = self
            .selection
            .page
            .clone()
            .ok_or_else(|| fail("open a page first."))?;
        if self
            .page_notices
            .get(&page)
            .is_some_and(|notice| notice.read_only)
        {
            return Err(fail("this page is read-only."));
        }
        let Some(text) = self.page_text(&page) else {
            return Err(fail("this page cannot be changed yet."));
        };
        let width = width.map(|w| w.clamp(image::MIN_WIDTH, image::MAX_WIDTH));
        let shown_end = find_canvas_block(text).map_or(text.len(), |b| b.block.start);
        let shown = text.get(..shown_end).unwrap_or(text);
        let Some(edit) = image::set_image_width(shown, src, nth, width) else {
            return Err(fail("that picture is not on the page."));
        };
        let removed = shown
            .get(edit.span.start..edit.span.end)
            .unwrap_or_default()
            .to_owned();
        if removed == edit.text {
            return Ok(());
        }
        let op = Op::ReplaceText {
            page: page.clone(),
            at: edit.span.start,
            removed,
            inserted: edit.text,
        };
        if !self.commit(Transaction::new(Scope::Page(page), "Resize picture").with(op)) {
            return Err(fail("the page could not be changed."));
        }
        Ok(())
    }

    /// Where the link `src` of the page shown now points on disk: a path under the page's
    /// folder, or the absolute path of a linked file. `None` for a web address, an anchor, a
    /// network share or device path, a path that climbs out of the folder, or when no notebook
    /// folder is open. The file may
    /// not exist.
    pub fn attachment_path(&self, src: &str) -> Option<PathBuf> {
        let decoded = decode_link_path(src.trim());
        if decoded.is_empty() || decoded.starts_with('#') {
            return None;
        }
        // A network share or device path (`\\server\share`, `//server/share`, `\\?\`, `\\.\`):
        // reading it can reach another computer and hand it the user's sign-in, and pictures
        // load as soon as the page shows. Only local files are followed.
        let mut lead = decoded.chars();
        if matches!(
            (lead.next(), lead.next()),
            (Some('/' | '\\'), Some('/' | '\\'))
        ) {
            return None;
        }
        // A scheme such as `https:` or `mailto:` (a drive letter is one letter, not a scheme).
        if let Some((scheme, _)) = decoded.split_once(':') {
            if scheme.len() > 1 && scheme.chars().all(|c| c.is_ascii_alphanumeric()) {
                return None;
            }
        }
        let path = Path::new(&decoded);
        if path.is_absolute() {
            return Some(path.to_path_buf());
        }
        if !path.components().all(|c| matches!(c, Component::Normal(_))) {
            return None;
        }
        Some(self.page_folder()?.join(path))
    }
}

/// The markdown for one inserted object.
fn paragraph(kind: AttachKind, stored: &StoredAttachment) -> String {
    match kind {
        AttachKind::Picture => {
            let alt = link_text(stem_of(&stored.display_name));
            format!("![{alt}]({})", stored.link)
        }
        AttachKind::File => {
            let name = link_text(&stored.display_name);
            let name = if name.is_empty() { "Attachment" } else { &name };
            format!("[{name}]({})", stored.link)
        }
    }
}

/// The insertion that adds `paragraph` as a new paragraph at the end of `text`: the byte
/// offset and the text to put there, written with the page's own line endings.
fn append_paragraph(text: &str, paragraph: &str) -> (usize, String) {
    let trailing = text
        .bytes()
        .rev()
        .take_while(|b| *b == b'\n' || *b == b'\r')
        .filter(|b| *b == b'\n')
        .count();
    let lead = match (text.is_empty(), trailing) {
        (true, _) => "",
        (false, 0) => "\n\n",
        (false, 1) => "\n",
        (false, _) => "",
    };
    let markdown = format!("{lead}{paragraph}\n");
    let mut editor = Editor::new(text);
    let inserted = match editor.insert(text.len(), &markdown) {
        Ok(()) => editor
            .edits()
            .first()
            .map_or(markdown.clone(), |edit| edit.text.clone()),
        Err(_) => markdown,
    };
    (text.len(), inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open::open_folder;

    fn notebook(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("temp dir");
        for (name, bytes) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, bytes).expect("write");
        }
        dir
    }

    fn open(dir: &tempfile::TempDir) -> Session {
        let mut session = Session::new();
        let opened = open_folder(dir.path()).expect("scan");
        let id = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&id, dir.path());
        session.load_current_page();
        session
    }

    fn text(session: &Session) -> String {
        session
            .current_page_source()
            .map(|s| s.text.to_owned())
            .unwrap_or_default()
    }

    #[test]
    fn a_picture_is_copied_and_linked_in_one_undoable_step() {
        let dir = notebook(&[("Sec/A.md", b"# A\n\nbody\n"), ("src/My Photo.png", b"png")]);
        let mut session = open(&dir);
        let source = dir.path().join("src/My Photo.png");
        let done = session
            .insert_attachment(&source, AttachKind::Picture)
            .expect("insert");
        assert!(!done.linked);
        assert!(done.link.starts_with("attachments/") && done.link.ends_with("-My%20Photo.png"));
        let copied = dir.path().join("Sec").join(decode_link_path(&done.link));
        assert_eq!(std::fs::read(copied).expect("copy"), b"png");
        assert_eq!(
            text(&session),
            format!("# A\n\nbody\n\n![My Photo]({})\n", done.link)
        );
        assert_eq!(
            session.history().undo_label(Some(&page_id(&session))),
            Some("Insert picture")
        );
        // One step: undo restores the text exactly, redo brings the paragraph back.
        assert_eq!(session.undo().as_deref(), Some("Insert picture"));
        assert_eq!(text(&session), "# A\n\nbody\n");
        assert!(session.redo().is_some());
        assert!(text(&session).ends_with(&format!("![My Photo]({})\n", done.link)));
        // The page on disk is untouched until auto-save writes it.
        assert_eq!(
            std::fs::read_to_string(dir.path().join("Sec/A.md")).expect("read"),
            "# A\n\nbody\n"
        );
    }

    #[test]
    fn a_pasted_picture_is_copied_whatever_its_size() {
        let dir = notebook(&[("Sec/A.md", b"# A\n"), ("tmp/Pasted.png", b"png")]);
        let mut session = open(&dir);
        let source = dir.path().join("tmp/Pasted.png");
        // Over a 2-byte threshold, an inserted file is linked in place.
        let linked = session
            .insert_attachment_with(&source, AttachKind::Picture, 2)
            .expect("insert");
        assert!(linked.linked);
        // With no threshold (as edit.paste_picture runs), it is copied, so deleting the
        // temporary source afterwards leaves the picture in the notebook.
        let copied = session
            .insert_attachment_with(&source, AttachKind::Picture, u64::MAX)
            .expect("paste");
        assert!(!copied.linked);
        std::fs::remove_file(&source).expect("remove temp");
        let kept = dir.path().join("Sec").join(decode_link_path(&copied.link));
        assert_eq!(std::fs::read(kept).expect("copy"), b"png");
    }

    fn page_id(session: &Session) -> String {
        session.selection().page.clone().unwrap_or_default()
    }

    #[test]
    fn a_file_becomes_a_link_and_keeps_line_endings() {
        let dir = notebook(&[
            ("Sec/A.md", b"one\r\n\r\ntwo"),
            ("Q3 [draft].xlsx", b"cells"),
        ]);
        let mut session = open(&dir);
        let done = session
            .insert_attachment(&dir.path().join("Q3 [draft].xlsx"), AttachKind::File)
            .expect("insert");
        assert_eq!(done.name, "Q3 [draft].xlsx");
        let after = text(&session);
        assert!(
            after.starts_with("one\r\n\r\ntwo\r\n\r\n[Q3 (draft).xlsx](attachments/"),
            "{after:?}"
        );
        assert!(after.ends_with(")\r\n"), "{after:?}");
        assert_eq!(session.undo().as_deref(), Some("Insert file"));
        assert_eq!(text(&session), "one\r\n\r\ntwo");
    }

    #[test]
    fn an_empty_page_gets_just_the_paragraph() {
        let dir = notebook(&[("Sec/A.md", b""), ("pic.webp", b"w")]);
        let mut session = open(&dir);
        let done = session
            .insert_attachment(&dir.path().join("pic.webp"), AttachKind::Picture)
            .expect("insert");
        assert_eq!(text(&session), format!("![pic]({})\n", done.link));
    }

    #[test]
    fn refusals_change_nothing() {
        let dir = notebook(&[("Sec/A.md", b"x\n"), ("notes.txt", b"t")]);
        let mut session = open(&dir);
        // A picture must look like one.
        let err = session
            .insert_attachment(&dir.path().join("notes.txt"), AttachKind::Picture)
            .expect_err("not a picture");
        assert!(err.0.contains("PNG, JPEG, WebP, or SVG"), "{err}");
        // A missing file.
        let err = session
            .insert_attachment(&dir.path().join("gone.png"), AttachKind::Picture)
            .expect_err("missing");
        assert!(err.0.contains("could not be found"), "{err}");
        assert_eq!(text(&session), "x\n");
        assert!(!dir.path().join("Sec/attachments").exists());
        assert!(!session.can_undo());
        // The mock notebooks have no folder.
        let mut mock = Session::new();
        mock.set_notebooks(crate::mock::notebooks());
        let err = mock
            .insert_attachment(&dir.path().join("notes.txt"), AttachKind::File)
            .expect_err("no folder");
        assert!(err.0.contains("notebook folder"), "{err}");
    }

    #[test]
    fn a_read_only_page_is_refused() {
        let dir = notebook(&[("Sec/A.md", b"x\n"), ("n.txt", b"t")]);
        let mut session = open(&dir);
        let page = page_id(&session);
        session.page_notices.insert(
            page,
            crate::session::PageNotice {
                text: "cut short".to_owned(),
                read_only: true,
            },
        );
        let err = session
            .insert_attachment(&dir.path().join("n.txt"), AttachKind::File)
            .expect_err("read-only");
        assert!(err.0.contains("read-only"), "{err}");
        assert_eq!(text(&session), "x\n");
        assert!(!dir.path().join("Sec/attachments").exists());
    }

    #[test]
    fn links_resolve_under_the_page_folder_only() {
        let dir = notebook(&[("Sec/A.md", b"x\n")]);
        let session = open(&dir);
        let folder = dir.path().join("Sec");
        assert_eq!(
            session.attachment_path("attachments/a%20b.png"),
            Some(folder.join("attachments/a b.png"))
        );
        for bad in [
            "",
            "#top",
            "https://example.org/a.png",
            "mailto:me@example.org",
            "../Other/attachments/a.png",
            "attachments/../../x.png",
            r"\\server\share\a.png",
            "//server/share/a.png",
            "%5C%5Cserver%5Cshare%5Ca.png",
            r"\\?\C:\a.png",
            r"\\.\pipe\x",
            r"/\server\share\a.png",
        ] {
            assert_eq!(session.attachment_path(bad), None, "{bad}");
        }
        let abs = dir.path().join("big.bin");
        let link = notecraft_storage::attachments::encode_link_path(
            &abs.to_string_lossy().replace('\\', "/"),
        );
        assert_eq!(session.attachment_path(&link), Some(abs));
    }

    #[test]
    fn appended_paragraph_spacing() {
        for (before, expect) in [
            ("", "P\n"),
            ("a", "\n\nP\n"),
            ("a\n", "\nP\n"),
            ("a\n\n", "P\n"),
            ("a\n\n\n", "P\n"),
        ] {
            let (at, inserted) = append_paragraph(before, "P");
            assert_eq!(
                (at, inserted.as_str()),
                (before.len(), expect),
                "{before:?}"
            );
        }
    }
}
