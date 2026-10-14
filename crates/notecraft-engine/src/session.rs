//! The editing session: the state commands read and change. Phase 0 holds the view state,
//! the open notebooks (mock data until Phase 1 reads them from disk), which notebook,
//! section, and page are shown, and each page's style; page content and undo history land
//! in Phase 1.

use std::collections::BTreeMap;

use notecraft_model::{Notebook, PageRef, Section};

use crate::view::{PageStyle, ViewState};

/// Which notebook, section, and page are shown, by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub notebook: Option<String>,
    pub section: Option<String>,
    pub page: Option<String>,
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
