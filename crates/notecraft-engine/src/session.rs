//! The editing session: the state commands read and change. Phase 0 holds the view state,
//! the open notebooks (read from disk, or the mock notebooks until one is opened), which notebook,
//! section, and page are shown, each page's style and strokes, and the undo history (`undo`).

use std::collections::BTreeMap;

use notecraft_model::{Notebook, PageRef, Section};

use crate::ink::InkState;
use crate::mock;
use crate::recent::Recents;
use crate::undo::History;
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
    recents: Recents,
    ink: InkState,
    pub(crate) history: History,
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
