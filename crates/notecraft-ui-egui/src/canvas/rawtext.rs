//! The part of a container shown as raw markdown while the caret or selection is in it
//! (PR 1.5). A [`Raw`] keeps the laid-out text with the byte range of the page it shows, and
//! maps between page byte offsets and egui's character cursors. egui counts characters, the
//! engine counts bytes, and an IME composition (the preedit) sits in the galley but not in the
//! page text, so every conversion goes through here.

use std::ops::Range;
use std::sync::Arc;

use egui::text::CCursor;
use egui::{Galley, Pos2, Rect};

/// One raw region on screen.
pub(super) struct Raw {
    /// Index of the container run it belongs to.
    pub run: usize,
    /// The page bytes it shows.
    pub span: Range<usize>,
    /// Top left corner in page units.
    pub pos: Pos2,
    /// Laid out at `zoom`, so its sizes are screen points.
    pub galley: Arc<Galley>,
    pub zoom: f32,
    /// The text of `span`.
    pub text: String,
    /// An IME composition shown in the galley: the character index of `text` where it sits,
    /// and how many characters it has.
    pub preedit: Option<(usize, usize)>,
}

/// The character index of byte `byte` of `text`.
fn chars_before(text: &str, byte: usize) -> usize {
    text.get(..byte)
        .map_or_else(|| text.chars().count(), |h| h.chars().count())
}

/// The byte offset of character `index` of `text` (the length when past the end).
fn byte_of(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(byte, _)| byte)
}

impl Raw {
    /// The character index in `text` for a page byte offset (clamped into the region).
    fn char_of_byte(&self, page_byte: usize) -> usize {
        let local = page_byte
            .saturating_sub(self.span.start)
            .min(self.text.len());
        chars_before(&self.text, local)
    }

    /// The page byte offset of character `index` of `text`.
    fn byte_of_char(&self, index: usize) -> usize {
        self.span.start + byte_of(&self.text, index)
    }

    /// The galley character index for character `c` of `text`: the composition sits before
    /// the characters at and after where it was typed.
    fn galley_index(&self, c: usize) -> usize {
        match self.preedit {
            Some((at, n)) if c >= at => c + n,
            _ => c,
        }
    }

    /// The character of `text` for a galley character index (inside the composition counts
    /// as where it was typed).
    fn text_index(&self, g: usize) -> usize {
        match self.preedit {
            Some((at, n)) if g > at + n => g - n,
            Some((at, n)) if g > at && g <= at + n => at,
            _ => g,
        }
    }

    fn cursor(&self, page_byte: usize) -> CCursor {
        CCursor::new(self.galley_index(self.char_of_byte(page_byte)))
    }

    /// The caret rectangle for a page byte offset, in page units.
    pub fn caret_rect(&self, page_byte: usize) -> Rect {
        let local = self.galley.pos_from_cursor(self.cursor(page_byte));
        let min = self.pos + local.min.to_vec2() / self.zoom;
        let max = self.pos + local.max.to_vec2() / self.zoom;
        Rect::from_min_max(min, max)
    }

    /// The page rectangle of the laid-out text, in page units.
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.pos, self.galley.size() / self.zoom)
    }

    /// The page byte offset nearest to a point in page units.
    pub fn offset_at(&self, page_point: Pos2) -> usize {
        let local = (page_point - self.pos) * self.zoom;
        let g = self.galley.cursor_from_pos(local).index.0;
        self.byte_of_char(self.text_index(g))
    }

    /// One visual row up or down from `page_byte`. `h_pos` is the screen x to keep (from a
    /// previous vertical move). `None` when the caret is already on the first (or last) row.
    pub fn vertical(
        &self,
        page_byte: usize,
        down: bool,
        h_pos: Option<f32>,
    ) -> Option<(usize, f32)> {
        let cursor = self.cursor(page_byte);
        let row = self.galley.layout_from_cursor(cursor).row;
        if (down && row + 1 >= self.galley.rows.len()) || (!down && row == 0) {
            return None;
        }
        let (next, h) = if down {
            self.galley.cursor_down_one_row(&cursor, h_pos)
        } else {
            self.galley.cursor_up_one_row(&cursor, h_pos)
        };
        let h = h.unwrap_or_default();
        Some((self.byte_of_char(self.text_index(next.index.0)), h))
    }

    /// The x (screen points) of the caret, for remembering across vertical moves.
    pub fn caret_x(&self, page_byte: usize) -> f32 {
        self.galley
            .pos_from_cursor(self.cursor(page_byte))
            .center()
            .x
    }

    /// The start or end of the visual row that holds `page_byte`.
    pub fn row_edge(&self, page_byte: usize, end: bool) -> usize {
        let cursor = self.cursor(page_byte);
        let to = if end {
            self.galley.cursor_end_of_row(&cursor)
        } else {
            self.galley.cursor_begin_of_row(&cursor)
        };
        self.byte_of_char(self.text_index(to.index.0))
    }

    /// Selection rectangles for the page bytes `from..to`, in page units.
    pub fn selection_rects(&self, from: usize, to: usize) -> Vec<Rect> {
        let (lo, hi) = (from.max(self.span.start), to.min(self.span.end));
        if lo >= hi {
            return Vec::new();
        }
        let a = self.galley_index(self.char_of_byte(lo));
        let b = self.galley_index(self.char_of_byte(hi));
        let min = self.galley.layout_from_cursor(CCursor::new(a));
        let max = self.galley.layout_from_cursor(CCursor::new(b));
        let mut rects = Vec::new();
        for row_index in min.row..=max.row {
            let Some(placed) = self.galley.rows.get(row_index) else {
                break;
            };
            let left = if row_index == min.row {
                placed.x_offset(min.column)
            } else {
                0.0
            };
            let right = if row_index == max.row {
                placed.x_offset(max.column)
            } else {
                // A little past the end of the text shows that the line break is selected.
                placed.size.x + 4.0 * self.zoom
            };
            let top = placed.pos.y;
            let rect = Rect::from_min_max(
                Pos2::new(placed.pos.x + left, top),
                Pos2::new(placed.pos.x + right.max(left), top + placed.size.y),
            );
            rects.push(Rect::from_min_max(
                self.pos + rect.min.to_vec2() / self.zoom,
                self.pos + rect.max.to_vec2() / self.zoom,
            ));
        }
        rects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_and_byte_offsets_convert_both_ways() {
        let text = "a\u{e9}\u{1F44D}b";
        assert_eq!(chars_before(text, 0), 0);
        assert_eq!(chars_before(text, 3), 2);
        assert_eq!(chars_before(text, text.len()), 4);
        assert_eq!(byte_of(text, 2), 3);
        assert_eq!(byte_of(text, 99), text.len());
        // Off a boundary falls back to counting all.
        assert_eq!(chars_before(text, 2), 4);
    }
}
