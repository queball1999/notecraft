//! Session state for text editing in containers (PR 1.5): the caret and selection, container
//! geometry, which pages changed since they were read, and whether a page may be edited at all.

use std::ops::Range;

use notecraft_model::Container;

use super::Session;

/// The caret and selection in a page's markdown source, as byte offsets on character
/// boundaries. `head` is where the caret blinks; `anchor` is the other end of the selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextCursor {
    pub anchor: usize,
    pub head: usize,
}

impl TextCursor {
    pub const fn caret(at: usize) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }

    /// True when nothing is selected.
    pub const fn is_caret(&self) -> bool {
        self.anchor == self.head
    }

    /// The selected bytes (empty for a caret).
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}

impl Session {
    /// The caret of the page shown now, kept inside its text. `None` when the page is not being
    /// edited (no click in its text yet) or has no text.
    pub fn text_cursor(&self) -> Option<TextCursor> {
        let page = self.selection().page.as_deref()?;
        let (owner, cursor) = self.text_cursor.as_ref()?;
        if owner != page {
            return None;
        }
        let text = self.page_text(page)?;
        let snap = |at: usize| notecraft_canvas::text::snap(text, at);
        Some(TextCursor {
            anchor: snap(cursor.anchor),
            head: snap(cursor.head),
        })
    }

    /// Put the caret on `page`. Not a command: the commands call it after they validate.
    pub(crate) fn set_text_cursor(&mut self, page: &str, cursor: TextCursor) {
        self.text_cursor = Some((page.to_owned(), cursor));
        self.mark_changed();
    }

    /// Drop the caret (editing ends).
    pub fn clear_text_cursor(&mut self) {
        if self.text_cursor.take().is_some() {
            self.mark_changed();
        }
    }

    /// Whether page `id` is cut short or unreadable, so it must not be edited or saved over.
    pub fn page_is_read_only(&self, id: &str) -> bool {
        self.page_notices.get(id).is_some_and(|n| n.read_only)
    }

    /// Whether text in page `id` may be edited: it is not read-only, and a page that comes
    /// from a notebook folder has been read (a page that was not read, such as a `.notecraft`
    /// page before PR 1.9, has no text to edit and must not be written over).
    pub fn page_is_editable(&self, id: &str) -> bool {
        if self.page_is_read_only(id) {
            return false;
        }
        self.page_text(id).is_some() || !self.is_disk_page(id)
    }

    fn is_disk_page(&self, id: &str) -> bool {
        self.notebook_roots.keys().any(|notebook| {
            id.strip_prefix(notebook.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        })
    }

    /// Whether page `id` changed since its text was read or last saved.
    pub fn is_page_dirty(&self, id: &str) -> bool {
        self.dirty_pages.contains(id)
    }

    /// The pages with unsaved changes, in id order. PR 1.7 (auto-save) writes these.
    pub fn dirty_pages(&self) -> impl Iterator<Item = &str> {
        self.dirty_pages.iter().map(String::as_str)
    }

    /// Record that page `id` is saved. Not a command: auto-save calls it after a write.
    pub fn mark_page_saved(&mut self, id: &str) {
        self.dirty_pages.remove(id);
    }

    /// Replace the text of page `id` as an edit: like `set_page_source`, but the page counts
    /// as changed. Only undoable operations call this.
    pub(crate) fn replace_page_text(&mut self, id: &str, text: String) {
        self.put_page_text(id, text);
        self.dirty_pages.insert(id.to_owned());
    }

    /// The geometry of container `container` on `page`; a flow container at the default
    /// width when none was set.
    pub fn container(&self, page: &str, container: &str) -> Container {
        self.containers
            .get(page)
            .and_then(|set| set.get(container))
            .cloned()
            .unwrap_or_else(|| Container::flow(container))
    }

    /// The containers of `page` that have geometry of their own, in id order.
    pub fn page_containers(&self, page: &str) -> Vec<Container> {
        self.containers
            .get(page)
            .map(|set| set.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Set (`Some`) or clear (`None`, back to the flow layout) the geometry of a container.
    pub(crate) fn put_container(&mut self, page: &str, id: &str, container: Option<Container>) {
        match container {
            Some(container) => {
                self.containers
                    .entry(page.to_owned())
                    .or_default()
                    .insert(id.to_owned(), container);
            }
            None => {
                if let Some(set) = self.containers.get_mut(page) {
                    set.remove(id);
                    if set.is_empty() {
                        self.containers.remove(page);
                    }
                }
            }
        }
    }

    /// The geometry stored for a container, without the flow default.
    pub(crate) fn stored_container(&self, page: &str, id: &str) -> Option<Container> {
        self.containers.get(page)?.get(id).cloned()
    }

    /// Make page `id` read-only, as the notice for a page cut short would.
    #[cfg(test)]
    pub(crate) fn mark_read_only_for_test(&mut self, id: &str) {
        let notice = super::PageNotice {
            text: "read only".to_owned(),
            read_only: true,
        };
        self.page_notices.insert(id.to_owned(), notice);
    }
}
