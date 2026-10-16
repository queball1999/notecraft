//! The editing session: the state commands read and change. Phase 0 holds the view state,
//! the open notebooks (read from disk, or the mock notebooks until one is opened), which notebook,
//! section, and page are shown, and each page's style; page content and undo history land
//! in Phase 1.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use notecraft_model::{Notebook, PageRef, Section};
use notecraft_storage::{read_page_file, PageReadError};

use crate::ink::InkState;
use crate::mock;
use crate::recent::Recents;
use crate::view::{PageStyle, ViewState};

/// Which notebook, section, and page are shown, by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub notebook: Option<String>,
    pub section: Option<String>,
    pub page: Option<String>,
}

/// The markdown text of the page shown now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageSource<'a> {
    pub page_id: &'a str,
    /// Changes whenever `text` does.
    pub revision: u64,
    pub text: &'a str,
}

/// Why the page shown now cannot be edited or did not read cleanly. It is shown on the canvas
/// as a short plain-text line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageNotice {
    pub text: String,
    /// The page must not be edited or saved over (it was cut short, or it did not read).
    pub read_only: bool,
}

/// Everything a command can see and change. Commands run on the thread that owns it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    revision: u64,
    view: ViewState,
    notebooks: Vec<Notebook>,
    selection: Selection,
    /// Page styles by page id; pages missing here use the default style.
    page_styles: BTreeMap<String, PageStyle>,
    /// Markdown source text by page id, with a counter that goes up on every change. Pages
    /// missing here have no text. Phase 1 fills this from the folder reader (PR 1.1).
    page_sources: BTreeMap<String, (u64, String)>,
    /// Notices from reading page files, by page id. A page with a notice has been tried.
    page_notices: BTreeMap<String, PageNotice>,
    /// The folder of each notebook read from disk, by notebook id.
    notebook_roots: BTreeMap<String, PathBuf>,
    recents: Recents,
    ink: InkState,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Goes up by one every time a command changes the session.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Record that a command changed the session.
    pub fn mark_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn view(&self) -> &ViewState {
        &self.view
    }

    /// For commands, and for the front end to restore settings and report the OS dark mode.
    pub fn view_mut(&mut self) -> &mut ViewState {
        &mut self.view
    }

    /// The open notebooks, in notebook list order.
    pub fn notebooks(&self) -> &[Notebook] {
        &self.notebooks
    }

    /// Replace the open notebooks and show the first page of the first section of the first
    /// notebook.
    pub fn set_notebooks(&mut self, notebooks: Vec<Notebook>) {
        let notebook = notebooks.first();
        let section = notebook.and_then(Notebook::first_section);
        self.selection = Selection {
            notebook: notebook.map(|nb| nb.id.clone()),
            section: section.map(|s| s.id.clone()),
            page: section.and_then(|s| s.pages.first()).map(|p| p.id.clone()),
        };
        self.notebooks = notebooks;
    }

    /// Open `notebook` read from disk and show it. The mock notebooks are dropped. A notebook
    /// with the same id that is already open is replaced in place, keeping the selection if
    /// the page it names still exists; otherwise the first page of the first section is shown.
    pub fn open_notebook(&mut self, notebook: Notebook) {
        self.notebooks.retain(|nb| !mock::is_mock_id(&nb.id));
        let id = notebook.id.clone();
        let existing = self.notebooks.iter_mut().find(|nb| nb.id == id);
        if let Some(slot) = existing {
            *slot = notebook;
        } else {
            self.notebooks.push(notebook);
        }
        let keeps_selection = self.selection.notebook.as_deref() == Some(id.as_str())
            && self.current_page().is_some();
        if keeps_selection {
            return;
        }
        let section = self
            .notebooks
            .iter()
            .find(|nb| nb.id == id)
            .and_then(Notebook::first_section);
        self.selection = Selection {
            notebook: Some(id),
            section: section.map(|s| s.id.clone()),
            page: section.and_then(|s| s.pages.first()).map(|p| p.id.clone()),
        };
    }

    /// Remember the folder `notebook_id` was read from, so its pages can be read from disk.
    pub fn set_notebook_root(&mut self, notebook_id: &str, folder: &Path) {
        self.notebook_roots
            .insert(notebook_id.to_owned(), folder.to_path_buf());
        // Pages of a notebook that is opened again are read again.
        let prefix = format!("{notebook_id}/");
        self.page_sources.retain(|id, _| !id.starts_with(&prefix));
        self.page_notices.retain(|id, _| !id.starts_with(&prefix));
    }

    /// Read the page shown now from disk if it comes from a notebook folder and has not been
    /// read yet, and hand its text to [`Self::set_page_source`]. Only `.md` pages are read. A
    /// page cut at the size cap is read-only; one that cannot be read gets a short error
    /// notice. Never fails and never panics. Returns whether a read was tried.
    pub fn load_current_page(&mut self) -> bool {
        let Some(page_id) = self.selection.page.clone() else {
            return false;
        };
        if self.page_sources.contains_key(&page_id) || self.page_notices.contains_key(&page_id) {
            return false;
        }
        let Some(notebook_id) = self.selection.notebook.as_deref() else {
            return false;
        };
        let Some(root) = self.notebook_roots.get(notebook_id) else {
            return false;
        };
        let Some(rel) = page_id
            .strip_prefix(notebook_id)
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            return false;
        };
        let rel = Path::new(rel);
        let is_markdown = rel
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
        if !is_markdown {
            return false;
        }
        let notice = |text: &str| PageNotice {
            text: text.to_owned(),
            read_only: true,
        };
        if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
            let text = "This page could not be opened: its path is not valid.";
            self.page_notices.insert(page_id, notice(text));
            return true;
        }
        match read_page_file(&root.join(rel)) {
            Ok(page) => {
                if let Some(text) = page.warning() {
                    let read_only = page.truncated;
                    self.page_notices
                        .insert(page_id.clone(), PageNotice { text, read_only });
                }
                self.set_page_source(&page_id, page.text);
            }
            Err(err) => {
                tracing::warn!(page = %page_id, "cannot read page: {err}");
                let text = match err {
                    PageReadError::NotFound => {
                        "This page could not be opened: the file is missing."
                    }
                    PageReadError::NotAFile => "This page could not be opened: it is not a file.",
                    PageReadError::Io(_) => {
                        "This page could not be opened: the file cannot be read."
                    }
                };
                self.page_notices.insert(page_id, notice(text));
            }
        }
        true
    }

    /// The notice for the page shown now: a read-only or error line, if it has one.
    pub fn current_page_notice(&self) -> Option<&PageNotice> {
        self.page_notices.get(self.selection.page.as_deref()?)
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub(crate) fn selection_mut(&mut self) -> &mut Selection {
        &mut self.selection
    }

    pub fn current_notebook(&self) -> Option<&Notebook> {
        let id = self.selection.notebook.as_deref()?;
        self.notebooks.iter().find(|nb| nb.id == id)
    }

    pub fn current_section(&self) -> Option<&Section> {
        let id = self.selection.section.as_deref()?;
        self.current_notebook()?.section(id)
    }

    pub fn current_page(&self) -> Option<&PageRef> {
        let id = self.selection.page.as_deref()?;
        self.current_section()?.page(id)
    }

    /// The style of the page shown now (the default style when no page is shown).
    pub fn page_style(&self) -> PageStyle {
        self.selection
            .page
            .as_deref()
            .and_then(|id| self.page_styles.get(id))
            .copied()
            .unwrap_or_default()
    }

    /// Set the markdown source text of page `id` (the whole file, frontmatter included).
    /// This is the seam the folder reader plugs into; it is not a command.
    pub fn set_page_source(&mut self, id: &str, text: impl Into<String>) {
        let next = self.page_sources.get(id).map_or(1, |(rev, _)| rev + 1);
        self.page_sources.insert(id.to_owned(), (next, text.into()));
    }

    /// The source text of the page shown now, with a number that changes whenever the text
    /// does (for caching what is derived from it).
    pub fn current_page_source(&self) -> Option<PageSource<'_>> {
        let id = self.selection.page.as_deref()?;
        let (rev, text) = self.page_sources.get(id)?;
        Some(PageSource {
            page_id: id,
            revision: *rev,
            text,
        })
    }

    /// The active draw tool, the pen, and every page's strokes.
    pub fn ink(&self) -> &InkState {
        &self.ink
    }

    pub(crate) fn ink_mut(&mut self) -> &mut InkState {
        &mut self.ink
    }

    /// The strokes of the page shown now, if it has any.
    pub fn current_ink(&self) -> Option<&notecraft_canvas::ink::InkLayer> {
        self.ink.layer(self.selection.page.as_deref()?)
    }

    /// Recently opened notebooks and pages, newest first.
    pub fn recents(&self) -> &Recents {
        &self.recents
    }

    /// Replace the recent list without a command, for restoring saved settings.
    pub fn set_recents(&mut self, recents: Recents) {
        self.recents = recents;
    }

    pub(crate) fn recents_mut(&mut self) -> &mut Recents {
        &mut self.recents
    }

    /// Set the style of the page shown now. Returns false, changing nothing, when no page
    /// is shown.
    pub(crate) fn set_page_style(&mut self, style: PageStyle) -> bool {
        let Some(id) = self.selection.page.clone() else {
            return false;
        };
        if style == PageStyle::default() {
            self.page_styles.remove(&id);
        } else {
            self.page_styles.insert(id, style);
        }
        true
    }
}

#[cfg(test)]
mod page_source_tests {
    use super::*;
    use crate::mock;

    #[test]
    fn page_source_follows_the_shown_page_and_counts_changes() {
        let mut session = Session::new();
        session.set_notebooks(mock::notebooks());
        assert_eq!(session.current_page_source(), None);
        session.set_page_source(mock::WELCOME_ID, "one");
        let first = session.current_page_source().expect("welcome has text");
        assert_eq!((first.page_id, first.text), (mock::WELCOME_ID, "one"));
        let first = first.revision;
        session.set_page_source(mock::WELCOME_ID, "two");
        let second = session.current_page_source().expect("welcome has text");
        assert_eq!(second.text, "two");
        assert!(second.revision > first);
        // A page without text shows none.
        session.selection_mut().page = Some("p-grocery".to_owned());
        assert_eq!(session.current_page_source(), None);
    }
}

#[cfg(test)]
mod page_load_tests {
    use super::*;
    use crate::open::open_folder;

    fn open(dir: &tempfile::TempDir) -> Session {
        let mut session = Session::new();
        let opened = open_folder(dir.path()).expect("scan");
        let id = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&id, dir.path());
        session
    }

    fn notebook(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("temp dir");
        for (name, bytes) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, bytes).expect("write");
        }
        dir
    }

    #[test]
    fn the_shown_page_is_read_once_from_disk() {
        let dir = notebook(&[("Sec/A.md", b"# Hello\n")]);
        let mut session = open(&dir);
        assert!(session.load_current_page());
        let source = session.current_page_source().expect("text");
        assert_eq!(source.text, "# Hello\n");
        let rev = source.revision;
        assert_eq!(session.current_page_notice(), None);
        // Already read: nothing happens, even if the file changes.
        std::fs::write(dir.path().join("Sec/A.md"), "changed").expect("write");
        assert!(!session.load_current_page());
        assert_eq!(session.current_page_source().expect("text").revision, rev);
    }

    #[test]
    fn a_missing_page_file_gets_an_error_notice() {
        let dir = notebook(&[("Sec/A.md", b"x")]);
        let mut session = open(&dir);
        std::fs::remove_file(dir.path().join("Sec/A.md")).expect("remove");
        assert!(session.load_current_page());
        assert_eq!(session.current_page_source(), None);
        let notice = session.current_page_notice().expect("notice");
        assert!(
            notice.read_only && notice.text.contains("missing"),
            "{notice:?}"
        );
        assert!(!session.load_current_page());
    }

    #[test]
    fn bad_bytes_show_text_with_a_warning_and_stay_editable() {
        let dir = notebook(&[("Sec/A.md", b"ok \xff")]);
        let mut session = open(&dir);
        session.load_current_page();
        assert_eq!(
            session.current_page_source().expect("text").text,
            "ok \u{fffd}"
        );
        let notice = session.current_page_notice().expect("notice");
        assert!(!notice.read_only);
    }

    #[test]
    fn mock_pages_and_other_file_types_are_not_read() {
        let mut session = Session::new();
        session.set_notebooks(crate::mock::notebooks());
        assert!(!session.load_current_page());
        let dir = notebook(&[("Sec/Sketch.notecraft", b"PK")]);
        let mut session = open(&dir);
        assert!(!session.load_current_page());
        assert_eq!(session.current_page_source(), None);
        assert_eq!(session.current_page_notice(), None);
    }
}
