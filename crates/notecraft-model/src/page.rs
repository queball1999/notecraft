//! A reference to a page in a section's page list.

/// Deepest subpage level OneNote allows: a page, a subpage, and a sub-subpage.
pub const MAX_PAGE_LEVEL: u8 = 2;

/// One entry in a section's page list. The page content is loaded separately (Phase 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRef {
    /// Stable within its notebook.
    pub id: String,
    pub title: String,
    /// 0 for a page, 1 for a subpage, 2 for a sub-subpage. Larger values are clamped.
    pub level: u8,
    /// True when the page's subpages are folded away in the page list.
    pub collapsed: bool,
}

impl PageRef {
    pub fn new(id: impl Into<String>, title: impl Into<String>, level: u8) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            level: level.min(MAX_PAGE_LEVEL),
            collapsed: false,
        }
    }

    /// This page with its subpages folded away (or shown).
    pub fn with_collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    /// The title to show; an untitled page shows "Untitled Page", as in OneNote.
    pub fn display_title(&self) -> &str {
        if self.title.trim().is_empty() {
            "Untitled Page"
        } else {
            &self.title
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_clamped_and_untitled_pages_have_a_name() {
        let page = PageRef::new("p1", "  ", 9);
        assert_eq!(page.level, MAX_PAGE_LEVEL);
        assert_eq!(page.display_title(), "Untitled Page");
        assert_eq!(PageRef::new("p2", "Notes", 0).display_title(), "Notes");
    }
}
