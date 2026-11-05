//! Find on page, session side (TX-11): the find bar's state and the match list of the page
//! shown. The commands in `cmd/find.rs` call these; they are not undoable and never change
//! the page.

use crate::find::{self, FindState, Match};

use super::Session;

impl Session {
    /// The find bar's state.
    pub fn find(&self) -> &FindState {
        &self.find
    }

    /// The matches of the query on the page shown, in page order. Empty with no query, no
    /// page, or no text.
    pub fn find_matches(&self) -> Vec<Match> {
        if self.find.query.is_empty() {
            return Vec::new();
        }
        let Some(source) = self.current_page_source() else {
            return Vec::new();
        };
        let text = source.visible_text();
        let doc = notecraft_markdown::parse(text);
        let runs = notecraft_markdown::split_containers(text, &doc);
        find::find_in_units(&find::page_units(&doc, &runs), &self.find.query)
    }

    /// Show the find bar and ask it to take focus. The caret leaves the text, so the page
    /// shows as it will be searched.
    pub(crate) fn open_find(&mut self) {
        self.find.open = true;
        self.find.focus = self.find.focus.wrapping_add(1);
        self.clear_text_cursor();
        self.mark_changed();
    }

    /// Hide the find bar. The query is kept for the next time it opens.
    pub(crate) fn close_find(&mut self) {
        if self.find.open {
            self.find.open = false;
            self.mark_changed();
        }
    }

    /// Set the query (cut to the allowed length) and move to its first match.
    pub(crate) fn set_find_query(&mut self, query: &str) {
        self.find.query = find::clean_query(query);
        self.find.current = 0;
        self.find.scroll = self.find.scroll.wrapping_add(1);
        self.mark_changed();
    }

    /// Move to the next (or previous) match, wrapping around. Returns the match count.
    pub(crate) fn step_find(&mut self, forward: bool) -> usize {
        let count = self.find_matches().len();
        if let Some(now) = self.find.current_of(count) {
            self.find.current = if forward {
                (now + 1) % count
            } else {
                (now + count - 1) % count
            };
            self.find.scroll = self.find.scroll.wrapping_add(1);
            self.mark_changed();
        }
        count
    }
}
