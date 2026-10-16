//! The editing session: the state commands read and change. Phase 0 holds the view state,
//! the open notebooks (mock data until Phase 1 reads them from disk), which notebook,
//! section, and page are shown, and each page's style; page content and undo history land
//! in Phase 1.

use std::collections::BTreeMap;

use notecraft_model::{Notebook, PageRef, Section};

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

/// Everything a command can see and change. Commands run on the thread that owns it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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
