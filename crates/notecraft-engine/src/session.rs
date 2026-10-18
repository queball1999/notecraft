//! The editing session: the state commands read and change. Phase 0 holds the view state,
//! the open notebooks (read from disk, or the mock notebooks until one is opened), which notebook,
//! section, and page are shown, each page's style and strokes, and the undo history (`undo`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use notecraft_model::{Notebook, PageRef, Section};
use notecraft_storage::{read_page_file, PageReadError, PageText};

mod nav;
mod save;
mod saving;
pub use save::{ConvertOutcome, CONVERT_WARNING};
pub use saving::{ReadPurpose, ReadRequest, RecoveryItem, SaveOutcome, SaveState};

mod attach;
mod objects;

pub use attach::{AttachKind, Attached, InlineEdit, InsertError};

use crate::ink::InkState;

use crate::mock;
use crate::recent::Recents;
use crate::undo::History;
use crate::view::{PageStyle, ViewState};

mod structure;
mod text_state;

pub use structure::EditError;
pub use text_state::TextCursor;

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

impl<'a> PageSource<'a> {
    /// The text to lay out: the page without its `notecraft-canvas` block, which holds
    /// positions and ink and is never shown as text.
    pub fn visible_text(&self) -> &'a str {
        match notecraft_storage::find_canvas_block(self.text) {
            Some(block) => self.text.get(..block.block.start).unwrap_or(self.text),
            None => self.text,
        }
    }
}

/// Why the page shown now cannot be edited or did not read cleanly. It is shown on the canvas
/// as a short plain-text line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageNotice {
    pub text: String,
    /// The page must not be edited or saved over (it was cut short, or it did not read).
    pub read_only: bool,
}

fn read_error_notice(text: &str) -> PageNotice {
    PageNotice {
        text: text.to_owned(),
        read_only: true,
    }
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
    /// The rule style new pages start with (Always Create Pages with Rule Lines); `None` is
    /// the plain page.
    default_rule_lines: notecraft_canvas::RuleLines,
    /// Markdown source text by page id, with a counter that goes up on every change. Pages
    /// missing here have no text. Phase 1 fills this from the folder reader (PR 1.1).
    page_sources: BTreeMap<String, (u64, String)>,
    /// Notices from reading page files, by page id. A page with a notice has been tried.
    page_notices: BTreeMap<String, PageNotice>,
    /// The folder of each notebook read from disk, by notebook id.
    notebook_roots: BTreeMap<String, PathBuf>,
    recents: Recents,
    /// Notebooks changed on disk by a structure step that must be read again (see `structure`).
    dirty: std::collections::BTreeSet<String>,
    /// Frontmatter titles from the notebook index, by page id (see `nav`).
    frontmatter_titles: BTreeMap<String, String>,
    /// Per notebook, goes up when the tree is read again; guards background index results.
    tree_generations: BTreeMap<String, u64>,
    ink: InkState,
    /// The caret and selection of text editing, with the page they belong to.
    text_cursor: Option<(String, TextCursor)>,
    /// Container geometry by page id, then container id. A container missing here is in the
    /// flow layout at the default width. PR 1.9 stores it in the page's canvas block.
    containers: BTreeMap<String, BTreeMap<String, notecraft_model::Container>>,
    /// Pictures and file attachments with a place of their own, by page id (see `objects`).
    objects: BTreeMap<String, Vec<notecraft_model::PlacedObject>>,
    pub(crate) history: History,
    /// Saving, recovery, and outside-edit state (`saving.rs`).
    saving: saving::SavingState,
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

    pub(crate) fn notebooks_mut(&mut self) -> &mut [Notebook] {
        &mut self.notebooks
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
        self.history.clear();
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
        // Opened again: unsaved pages go to disk first, because their text is dropped below.
        let prefix = format!("{notebook_id}/");
        let unsaved: Vec<String> = self
            .page_sources
            .keys()
            .filter(|id| id.starts_with(&prefix) && self.is_page_dirty(id))
            .cloned()
            .collect();
        for id in unsaved {
            self.save_page(&id);
        }
        self.notebook_roots
            .insert(notebook_id.to_owned(), folder.to_path_buf());
        self.bump_tree_generation(notebook_id);
        // Pages of a notebook that is opened again are read again.
        self.page_sources.retain(|id, _| !id.starts_with(&prefix));
        self.page_notices.retain(|id, _| !id.starts_with(&prefix));
        self.saving.forget_notebook(&prefix);
    }

    /// Read the page shown now from disk if it comes from a notebook folder and has not been
    /// read yet, and hand its text to [`Self::set_page_source`]. Only `.md` pages are read. A
    /// page cut at the size cap is read-only; one that cannot be read gets a short error
    /// notice. Never fails and never panics. Returns whether a read was tried.
    ///
    /// With background reads on ([`Self::set_background_reads`]) the read is queued as a
    /// [`ReadRequest`] for the IO worker instead, and the answer comes back through
    /// [`Self::finish_read`].
    pub fn load_current_page(&mut self) -> bool {
        let Some(page_id) = self.selection.page.clone() else {
            return false;
        };
        if self.page_sources.contains_key(&page_id)
            || self.page_notices.contains_key(&page_id)
            || self.saving.loading.contains(&page_id)
        {
            return false;
        }
        match self.resolve_page_file(&page_id) {
            saving::Resolved::NotAFile => false,
            saving::Resolved::Invalid => {
                let text = "This page could not be opened: its path is not valid.";
                self.page_notices.insert(page_id, read_error_notice(text));
                true
            }
            saving::Resolved::File(path) => {
                if self.saving.background {
                    self.queue_read(page_id, path, ReadPurpose::Load);
                } else {
                    let result = read_page_file(&path);
                    self.apply_load_result(&page_id, result);
                }
                true
            }
        }
    }

    /// Store the outcome of reading the page file for `page_id` the first time.
    pub(crate) fn apply_load_result(
        &mut self,
        page_id: &str,
        result: Result<PageText, PageReadError>,
    ) {
        match result {
            Ok(page) => {
                if let Some(text) = page.warning() {
                    let read_only = page.truncated;
                    self.page_notices
                        .insert(page_id.to_owned(), PageNotice { text, read_only });
                }
                self.saving
                    .disk_hash
                    .insert(page_id.to_owned(), saving::hash_text(&page.text));
                // Text restored from the recovery journal while the read was out wins.
                if !self.page_sources.contains_key(page_id) {
                    self.load_canvas(page_id, &page.text, page.truncated);
                    self.set_page_source(page_id, page.text);
                }
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
                self.page_notices
                    .insert(page_id.to_owned(), read_error_notice(text));
            }
        }
        self.saving.loading.remove(page_id);
        self.mark_changed();
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
        self.put_page_text(id, text.into());
    }

    /// Store `text` as page `id`'s source and bump its revision.
    fn put_page_text(&mut self, id: &str, text: String) {
        let next = self.page_sources.get(id).map_or(1, |(rev, _)| rev + 1);
        self.page_sources.insert(id.to_owned(), (next, text));
    }

    /// The source text of page `id`, if it has any.
    pub fn page_text(&self, id: &str) -> Option<&str> {
        self.page_sources.get(id).map(|(_, text)| text.as_str())
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

    /// Replace the Draw tab's pens and highlighters without a command, for restoring saved
    /// settings.
    pub fn set_pen_tools(&mut self, tools: Vec<crate::ink::PenTool>) {
        self.ink.replace_tools(tools);
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

    /// The rule style new pages start with; `RuleLines::None` when Always Create Pages with
    /// Rule Lines is off.
    pub fn default_rule_lines(&self) -> notecraft_canvas::RuleLines {
        self.default_rule_lines
    }

    /// Set the rule style new pages start with, without a command, for restoring saved
    /// settings.
    pub fn set_default_rule_lines(&mut self, style: notecraft_canvas::RuleLines) {
        self.default_rule_lines = style;
    }

    /// The style of page `id` (the default style for a page that never had one set).
    pub(crate) fn style_of(&self, id: &str) -> PageStyle {
        self.page_styles.get(id).copied().unwrap_or_default()
    }

    /// Set the style of page `id`.
    pub(crate) fn set_style_of(&mut self, id: &str, style: PageStyle) {
        let id = id.to_owned();
        if style == PageStyle::default() {
            self.page_styles.remove(&id);
        } else {
            self.page_styles.insert(id, style);
        }
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
    fn mock_pages_are_not_read_and_notecraft_pages_are() {
        let mut session = Session::new();
        session.set_notebooks(crate::mock::notebooks());
        assert!(!session.load_current_page());
        let dir = notebook(&[("Sec/Sketch.notecraft", b"PK")]);
        let mut session = open(&dir);
        assert!(session.load_current_page());
        assert_eq!(session.current_page_source().expect("text").text, "PK");
        assert_eq!(session.current_page_notice(), None);
        assert!(session.current_ink().is_none());
    }
}

#[cfg(test)]
mod saving_tests;
