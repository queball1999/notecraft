//! Text editing logic that does not depend on any UI: caret movement, word and line ranges,
//! and the splices Backspace, Delete, Enter, and Tab make (PR 1.5).
//!
//! All offsets are byte offsets into the page's markdown source, always on character
//! boundaries. Functions never panic: an offset past the end or inside a character is
//! snapped to the nearest boundary first. A [`TextEdit`] is one splice (remove some bytes at
//! `at`, put `insert` there) plus where the selection ends up; the engine applies it through
//! `Session::edit_page_text`, so every edit is one undo step.

use std::ops::Range;

/// One splice and the selection after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    pub at: usize,
    pub remove: usize,
    pub insert: String,
    /// Selection after the edit (equal when it is only a caret).
    pub anchor: usize,
    pub head: usize,
}

impl TextEdit {
    fn caret(at: usize, remove: usize, insert: String) -> Self {
        let caret = at + insert.len();
        Self {
            at,
            remove,
            insert,
            anchor: caret,
            head: caret,
        }
    }
}

/// Where the caret goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    /// Start of the text.
    Start,
    /// End of the text.
    End,
}

impl Motion {
    /// The stable name used as the `motion` argument of `text.move`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::WordLeft => "word_left",
            Self::WordRight => "word_right",
            Self::LineStart => "line_start",
            Self::LineEnd => "line_end",
            Self::Start => "start",
            Self::End => "end",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::Left,
            Self::Right,
            Self::WordLeft,
            Self::WordRight,
            Self::LineStart,
            Self::LineEnd,
            Self::Start,
            Self::End,
        ]
        .into_iter()
        .find(|m| m.name() == name)
    }
}

/// `at` moved down to a character boundary within `text`.
pub fn snap(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// Marks that attach to the character before them (combining marks, joiners, variation
/// selectors, skin tone modifiers).
fn is_extend(c: char) -> bool {
    matches!(c,
        '\u{0300}'..='\u{036F}'
        | '\u{1AB0}'..='\u{1AFF}'
        | '\u{1DC0}'..='\u{1DFF}'
        | '\u{20D0}'..='\u{20FF}'
        | '\u{200D}'
        | '\u{FE00}'..='\u{FE0F}'
        | '\u{FE20}'..='\u{FE2F}'
        | '\u{1F3FB}'..='\u{1F3FF}'
        | '\u{E0100}'..='\u{E01EF}')
}

/// The boundary of the character (with its marks, or a CRLF pair) before `at`.
pub fn prev_boundary(text: &str, at: usize) -> usize {
    let at = snap(text, at);
    let head = text.get(..at).unwrap_or("");
    let mut chars = head.char_indices().rev();
    let Some((mut start, mut c)) = chars.next() else {
        return 0;
    };
    if c == '\n' && head.get(..start).is_some_and(|h| h.ends_with('\r')) {
        return start - 1;
    }
    while is_extend(c) || c == '\u{200D}' {
        match chars.next() {
            Some((s, prev)) => {
                start = s;
                c = prev;
            }
            None => return 0,
        }
    }
    start
}

/// The boundary of the character (with its marks, or a CRLF pair) after `at`.
pub fn next_boundary(text: &str, at: usize) -> usize {
    let at = snap(text, at);
    let tail = text.get(at..).unwrap_or("");
    let mut chars = tail.char_indices();
    let Some((_, first)) = chars.next() else {
        return text.len();
    };
    let mut end = at + first.len_utf8();
    if first == '\r' && tail.get(1..).is_some_and(|t| t.starts_with('\n')) {
        return end + 1;
    }
    let mut joined = first == '\u{200D}';
    for (i, c) in chars {
        if is_extend(c) || joined {
            joined = c == '\u{200D}';
            end = at + i + c.len_utf8();
        } else {
            break;
        }
    }
    end
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Word,
    Other,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Other
    }
}

/// Start of the previous word: skips spaces to the left, then a run of one class.
fn word_left(text: &str, at: usize) -> usize {
    let mut at = snap(text, at);
    let step_back = |at: usize| -> Option<(usize, char)> {
        let head = text.get(..at)?;
        head.char_indices().next_back()
    };
    while let Some((i, c)) = step_back(at) {
        if class(c) != Class::Space || c == '\n' {
            break;
        }
        at = i;
    }
    // A line break counts as one word step on its own.
    if let Some((i, '\n')) = step_back(at) {
        return prev_boundary(text, i + 1);
    }
    let Some((_, last)) = step_back(at) else {
        return at;
    };
    let kind = class(last);
    while let Some((i, c)) = step_back(at) {
        if class(c) != kind {
            break;
        }
        at = i;
    }
    at
}

/// Start of the next word: skips the rest of the current run, then the spaces after it.
fn word_right(text: &str, at: usize) -> usize {
    let mut at = snap(text, at);
    let peek = |at: usize| text.get(at..).and_then(|t| t.chars().next());
    if peek(at) == Some('\r') || peek(at) == Some('\n') {
        return next_boundary(text, at);
    }
    if let Some(first) = peek(at) {
        let kind = class(first);
        if kind != Class::Space {
            while let Some(c) = peek(at) {
                if class(c) != kind {
                    break;
                }
                at += c.len_utf8();
            }
        }
    }
    while let Some(c) = peek(at) {
        if class(c) != Class::Space || c == '\n' || c == '\r' {
            break;
        }
        at += c.len_utf8();
    }
    at
}

/// Start of the line (after the previous `\n`) that holds `at`.
pub fn line_start(text: &str, at: usize) -> usize {
    let at = snap(text, at);
    text.get(..at)
        .and_then(|head| head.rfind('\n'))
        .map_or(0, |i| i + 1)
}

/// End of the line that holds `at`, before its `\n` (and before the `\r` of a CRLF).
pub fn line_end(text: &str, at: usize) -> usize {
    let at = snap(text, at);
    let end = text
        .get(at..)
        .and_then(|tail| tail.find('\n'))
        .map_or(text.len(), |i| at + i);
    if end > at && text.get(..end).is_some_and(|h| h.ends_with('\r')) {
        end - 1
    } else {
        end
    }
}

/// The offset after moving the caret from `from` by `motion`.
pub fn move_caret(text: &str, from: usize, motion: Motion) -> usize {
    let from = snap(text, from);
    match motion {
        Motion::Left => prev_boundary(text, from),
        Motion::Right => next_boundary(text, from),
        Motion::WordLeft => word_left(text, from),
        Motion::WordRight => word_right(text, from),
        Motion::LineStart => line_start(text, from),
        Motion::LineEnd => line_end(text, from),
        Motion::Start => 0,
        Motion::End => text.len(),
    }
}

/// The word (or run of spaces, or run of punctuation) around `at`, for a double click.
pub fn word_range(text: &str, at: usize) -> Range<usize> {
    let at = snap(text, at);
    let after = text.get(at..).and_then(|t| t.chars().next());
    let before = text.get(..at).and_then(|h| h.chars().next_back());
    let Some(kind) = after
        .filter(|c| class(*c) != Class::Space)
        .or(before)
        .or(after)
        .map(class)
    else {
        return at..at;
    };
    let mut start = at;
    while let Some(c) = text.get(..start).and_then(|h| h.chars().next_back()) {
        if class(c) != kind || c == '\n' {
            break;
        }
        start -= c.len_utf8();
    }
    let mut end = at;
    while let Some(c) = text.get(end..).and_then(|t| t.chars().next()) {
        if class(c) != kind || c == '\n' {
            break;
        }
        end += c.len_utf8();
    }
    start..end
}

/// The line around `at`, without its line break, for a triple click.
pub fn line_range(text: &str, at: usize) -> Range<usize> {
    line_start(text, at)..line_end(text, at)
}

/// The smallest range that holds the lines from `lo` to `hi` and every block (given as byte
/// ranges, sorted by start) it touches, kept inside `bounds`. This is the part of the page shown
/// as raw markdown while the caret or selection is in it.
pub fn active_region(
    text: &str,
    blocks: &[Range<usize>],
    lo: usize,
    hi: usize,
    bounds: Range<usize>,
) -> Range<usize> {
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let mut start = line_start(text, lo).max(bounds.start);
    let mut end = line_end(text, hi).min(bounds.end).max(start);
    // Growing the region can make it touch more blocks; blocks are few per pass.
    for _ in 0..blocks.len().min(64).saturating_add(1) {
        let mut grown = false;
        for block in blocks {
            let touches = block.start <= end && block.end >= start && block.start < block.end;
            // A block that merely ends where an empty region starts does not count.
            let touches = touches && !(start == end && (block.end == start || block.start == end));
            if touches && (block.start < start || block.end > end) {
                start = start.min(block.start).max(bounds.start);
                end = end.max(block.end).min(bounds.end);
                grown = true;
            }
        }
        if !grown {
            break;
        }
    }
    start..end
}

/// The edit that replaces the selection `anchor..head` (or the empty selection at the caret)
/// with `inserted`.
pub fn insert(anchor: usize, head: usize, inserted: &str) -> Option<TextEdit> {
    let (lo, hi) = (anchor.min(head), anchor.max(head));
    if lo == hi && inserted.is_empty() {
        return None;
    }
    Some(TextEdit::caret(lo, hi - lo, inserted.to_owned()))
}

/// Backspace: delete the selection, or the character (or word) before the caret.
pub fn backspace(text: &str, anchor: usize, head: usize, word: bool) -> Option<TextEdit> {
    let (anchor, head) = (snap(text, anchor), snap(text, head));
    if anchor != head {
        return insert(anchor, head, "");
    }
    let to = if word {
        word_left(text, head)
    } else {
        prev_boundary(text, head)
    };
    (to < head).then(|| TextEdit::caret(to, head - to, String::new()))
}

/// Delete: delete the selection, or the character (or word) after the caret.
pub fn delete_forward(text: &str, anchor: usize, head: usize, word: bool) -> Option<TextEdit> {
    let (anchor, head) = (snap(text, anchor), snap(text, head));
    if anchor != head {
        return insert(anchor, head, "");
    }
    let to = if word {
        word_right(text, head)
    } else {
        next_boundary(text, head)
    };
    (to > head).then(|| TextEdit::caret(head, to - head, String::new()))
}

/// A list or quote prefix at the start of a line.
struct Prefix {
    /// Bytes of the prefix, indent included.
    len: usize,
    /// The prefix to start the next line with.
    next: String,
    /// Width to indent or outdent by.
    step: usize,
}

fn list_prefix(line: &str) -> Option<Prefix> {
    let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let indent = line.get(..indent_len)?;
    let rest = line.get(indent_len..)?;
    let mut quote = String::new();
    let mut body = rest;
    // Block quote markers: `>` or `> `, possibly nested.
    while let Some(after) = body.strip_prefix('>') {
        quote.push('>');
        body = match after.strip_prefix(' ') {
            Some(b) => {
                quote.push(' ');
                b
            }
            None => after,
        };
    }
    let (marker, after_marker, step) = if let Some(b) = ["- ", "* ", "+ "]
        .iter()
        .find_map(|m| body.strip_prefix(m).map(|b| (m, b)))
    {
        ((*b.0).to_owned(), b.1, 2)
    } else {
        let digits = body.chars().take_while(char::is_ascii_digit).count();
        let sep = body.get(digits..)?.chars().next();
        let ok = (1..=9).contains(&digits)
            && matches!(sep, Some('.' | ')'))
            && body.get(digits + 1..).is_some_and(|t| t.starts_with(' '));
        if !ok {
            if quote.is_empty() {
                return None;
            }
            let len = indent_len + quote.len();
            return Some(Prefix {
                len,
                next: format!("{indent}{quote}"),
                step: 2,
            });
        }
        let number: u64 = body.get(..digits)?.parse().unwrap_or(0);
        let sep = sep.unwrap_or('.');
        (
            format!("{}{sep} ", number.saturating_add(1)),
            body.get(digits + 2..)?,
            3,
        )
    };
    let (task, after_task) = ["[ ] ", "[x] ", "[X] "]
        .iter()
        .find_map(|t| after_marker.strip_prefix(t).map(|rest| (*t, rest)))
        .map_or(("", after_marker), |(t, rest)| (t, rest));
    let next_task = if task.is_empty() { "" } else { "[ ] " };
    let len = line.len() - after_task.len();
    Some(Prefix {
        len,
        next: format!("{indent}{quote}{marker}{next_task}"),
        step,
    })
}

/// Enter. A plain Enter starts a new paragraph (a blank line between, as OneNote's separate
/// paragraphs read in markdown); Shift+Enter (`soft`) is a line break inside the paragraph. In
/// a list the next item starts with the same kind of marker, and Enter on an empty item ends
/// the list. Inside a code block (`in_code`) Enter is a line break.
pub fn enter(
    text: &str,
    anchor: usize,
    head: usize,
    eol: &str,
    soft: bool,
    in_code: bool,
) -> TextEdit {
    let (anchor, head) = (snap(text, anchor), snap(text, head));
    let (lo, hi) = (anchor.min(head), anchor.max(head));
    if lo != hi || soft || in_code {
        return TextEdit::caret(lo, hi - lo, eol.to_owned());
    }
    let (ls, le) = (line_start(text, head), line_end(text, head));
    let line = text.get(ls..le).unwrap_or("");
    if let Some(prefix) = list_prefix(line) {
        if head >= ls + prefix.len {
            let rest = text.get(ls + prefix.len..le).unwrap_or("");
            if rest.trim().is_empty() {
                // An empty item ends the list.
                return TextEdit::caret(ls, le - ls, String::new());
            }
            return TextEdit::caret(head, 0, format!("{eol}{}", prefix.next));
        }
    }
    let on_blank_line = line.trim().is_empty();
    if on_blank_line {
        TextEdit::caret(head, 0, eol.to_owned())
    } else {
        TextEdit::caret(head, 0, format!("{eol}{eol}"))
    }
}

/// Tab and Shift+Tab. In a list, the lines touched by the selection move one level in or out.
/// Elsewhere Tab inserts a tab character inside a line; Shift+Tab and a Tab at the start of a
/// line do nothing (so a line is never turned into an indented code block by accident).
pub fn tab(text: &str, anchor: usize, head: usize, shift: bool) -> Option<TextEdit> {
    let (anchor, head) = (snap(text, anchor), snap(text, head));
    let (lo, hi) = (anchor.min(head), anchor.max(head));
    let first = line_start(text, lo);
    let last_end = line_end(text, hi);
    let block = text.get(first..last_end)?;
    let in_list = block
        .split('\n')
        .next()
        .is_some_and(|l| list_prefix(l.trim_end_matches('\r')).is_some());
    if !in_list {
        if shift || lo == line_start(text, lo) {
            return None;
        }
        return Some(TextEdit::caret(lo, hi - lo, "\t".to_owned()));
    }
    let mut out = String::with_capacity(block.len() + 8);
    // Offsets of the old text mapped to the new text.
    let mut map: Vec<(usize, isize)> = Vec::new();
    let mut offset = first;
    for (n, raw) in block.split('\n').enumerate() {
        if n > 0 {
            out.push('\n');
            offset += 1;
        }
        let line = raw.trim_end_matches('\r');
        let step = list_prefix(line).map_or(2, |p| p.step);
        let delta: isize = if shift {
            let spaces = line.len() - line.trim_start_matches(' ').len();
            let cut = if line.starts_with('\t') {
                1
            } else {
                spaces.min(step)
            };
            out.push_str(raw.get(cut..).unwrap_or(raw));
            -(cut as isize)
        } else {
            out.push_str(&" ".repeat(step));
            out.push_str(raw);
            step as isize
        };
        map.push((offset, delta));
        offset += raw.len();
    }
    let shifted = |pos: usize| -> usize {
        let delta: isize = map
            .iter()
            .filter(|(line_start, _)| *line_start <= pos)
            .next_back()
            .map_or(0, |(_, d)| *d);
        let moved = pos as isize + delta;
        moved.max(first as isize) as usize
    };
    (out != block).then(|| TextEdit {
        at: first,
        remove: block.len(),
        insert: out,
        anchor: shifted(anchor),
        head: shifted(head),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str, edit: &TextEdit) -> String {
        format!(
            "{}{}{}",
            text.get(..edit.at).unwrap_or(""),
            edit.insert,
            text.get(edit.at + edit.remove..).unwrap_or("")
        )
    }

    #[test]
    fn character_moves_respect_clusters_and_crlf() {
        let t = "a\u{301}b\r\nc\u{1F44D}\u{1F3FD}d";
        assert_eq!(next_boundary(t, 0), 3);
        assert_eq!(prev_boundary(t, 3), 0);
        let after_b = t.find('b').expect("b") + 1;
        assert_eq!(next_boundary(t, after_b), after_b + 2);
        assert_eq!(prev_boundary(t, after_b + 2), after_b);
        let thumb = t.find('\u{1F44D}').expect("thumb");
        assert_eq!(next_boundary(t, thumb), thumb + 8);
        assert_eq!(prev_boundary(t, thumb + 8), thumb);
        // Inside a character, or past the end.
        assert_eq!(snap("\u{e9}", 1), 0);
        assert_eq!(next_boundary("ab", 99), 2);
        assert_eq!(prev_boundary("", 5), 0);
    }

    #[test]
    fn word_moves_land_at_word_starts() {
        let t = "hello  world, foo";
        assert_eq!(move_caret(t, 0, Motion::WordRight), 7);
        assert_eq!(move_caret(t, 7, Motion::WordRight), 12);
        assert_eq!(move_caret(t, 12, Motion::WordRight), 14);
        assert_eq!(move_caret(t, 17, Motion::WordLeft), 14);
        assert_eq!(move_caret(t, 14, Motion::WordLeft), 12);
        assert_eq!(move_caret(t, 12, Motion::WordLeft), 7);
        assert_eq!(move_caret(t, 7, Motion::WordLeft), 0);
        assert_eq!(move_caret(t, 0, Motion::WordLeft), 0);
        assert_eq!(move_caret(t, 17, Motion::WordRight), 17);
    }

    #[test]
    fn word_moves_stop_at_line_breaks() {
        let t = "one\ntwo";
        assert_eq!(move_caret(t, 0, Motion::WordRight), 3);
        assert_eq!(move_caret(t, 3, Motion::WordRight), 4);
        assert_eq!(move_caret(t, 7, Motion::WordLeft), 4);
        assert_eq!(move_caret(t, 4, Motion::WordLeft), 3);
    }

    #[test]
    fn line_moves_use_logical_lines() {
        let t = "ab\r\ncd\nef";
        assert_eq!(move_caret(t, 1, Motion::LineStart), 0);
        assert_eq!(move_caret(t, 1, Motion::LineEnd), 2);
        assert_eq!(move_caret(t, 5, Motion::LineStart), 4);
        assert_eq!(move_caret(t, 5, Motion::LineEnd), 6);
        assert_eq!(move_caret(t, 5, Motion::End), t.len());
        assert_eq!(move_caret(t, 5, Motion::Start), 0);
        assert_eq!(line_range(t, 8), 7..9);
    }

    #[test]
    fn double_click_selects_a_word_or_the_run_it_is_in() {
        let t = "hello  world!";
        assert_eq!(word_range(t, 2), 0..5);
        assert_eq!(word_range(t, 5), 0..5);
        assert_eq!(word_range(t, 6), 5..7);
        assert_eq!(word_range(t, 10), 7..12);
        assert_eq!(word_range(t, 12), 12..13);
        assert_eq!(word_range("", 0), 0..0);
    }

    #[test]
    fn region_grows_to_whole_blocks() {
        let t = "Hello\nWorld\n\nNext\n";
        let blocks = [0..11, 13..17];
        // A caret in the first block shows the whole block raw, and not the next one.
        assert_eq!(active_region(t, &blocks, 8, 8, 0..t.len()), 0..11);
        assert_eq!(active_region(t, &blocks, 14, 14, 0..t.len()), 13..17);
        // On the blank line between them: just that line.
        assert_eq!(active_region(t, &blocks, 12, 12, 0..t.len()), 12..12);
        // A caret right after a new line below a block does not take the block in.
        let t2 = "Hello\n";
        assert_eq!(active_region(t2, &[0..5], 6, 6, 0..6), 6..6);
        // A selection across blocks covers both.
        assert_eq!(active_region(t, &blocks, 2, 14, 0..t.len()), 0..17);
        // Bounds hold.
        assert_eq!(active_region(t, &blocks, 2, 2, 3..8), 3..8);
    }

    #[test]
    fn backspace_and_delete_remove_selection_or_one_unit() {
        let t = "hello world";
        let e = backspace(t, 5, 5, false).expect("edit");
        assert_eq!(apply(t, &e), "hell world");
        assert_eq!((e.anchor, e.head), (4, 4));
        let e = backspace(t, 11, 11, true).expect("edit");
        assert_eq!(apply(t, &e), "hello ");
        let e = backspace(t, 2, 8, false).expect("edit");
        assert_eq!(apply(t, &e), "herld");
        assert_eq!(e.head, 2);
        assert_eq!(backspace(t, 0, 0, false), None);
        let e = delete_forward(t, 0, 0, true).expect("edit");
        assert_eq!(apply(t, &e), "world");
        assert_eq!(delete_forward(t, 11, 11, false), None);
        // CRLF goes as one unit.
        let c = "a\r\nb";
        let e = backspace(c, 3, 3, false).expect("edit");
        assert_eq!(apply(c, &e), "ab");
        let e = delete_forward(c, 1, 1, false).expect("edit");
        assert_eq!(apply(c, &e), "ab");
    }

    #[test]
    fn typing_replaces_the_selection() {
        let e = insert(2, 5, "XY").expect("edit");
        assert_eq!(apply("abcdefg", &e), "abXYfg");
        assert_eq!(e.head, 4);
        assert_eq!(insert(1, 1, ""), None);
        let e = insert(5, 2, "").expect("edit");
        assert_eq!((e.at, e.remove), (2, 3));
    }

    #[test]
    fn enter_makes_paragraphs_and_continues_lists() {
        let t = "Hello";
        let e = enter(t, 5, 5, "\n", false, false);
        assert_eq!(apply(t, &e), "Hello\n\n");
        assert_eq!(e.head, 7);
        // On an empty line a second Enter adds just one line break.
        let t = "Hello\n\n";
        let e = enter(t, 7, 7, "\n", false, false);
        assert_eq!(apply(t, &e), "Hello\n\n\n");
        // Soft break and code blocks.
        assert_eq!(apply("ab", &enter("ab", 1, 1, "\r\n", true, false)), "a\r\nb");
        assert_eq!(apply("ab", &enter("ab", 1, 1, "\n", false, true)), "a\nb");
        // Lists keep going.
        let t = "- one";
        assert_eq!(apply(t, &enter(t, 5, 5, "\n", false, false)), "- one\n- ");
        let t = "2. two";
        assert_eq!(apply(t, &enter(t, 6, 6, "\n", false, false)), "2. two\n3. ");
        let t = "- [x] done";
        assert_eq!(
            apply(t, &enter(t, 10, 10, "\n", false, false)),
            "- [x] done\n- [ ] "
        );
        let t = "  * nested";
        assert_eq!(
            apply(t, &enter(t, 10, 10, "\n", false, false)),
            "  * nested\n  * "
        );
        let t = "> quote";
        assert_eq!(apply(t, &enter(t, 7, 7, "\n", false, false)), "> quote\n> ");
        // An empty item ends the list.
        let t = "- one\n- ";
        let e = enter(t, 8, 8, "\n", false, false);
        assert_eq!(apply(t, &e), "- one\n");
        assert_eq!(e.head, 6);
        // A selection is replaced by the line break.
        assert_eq!(apply("abcd", &enter("abcd", 1, 3, "\n", false, false)), "a\nd");
    }

    #[test]
    fn tab_moves_list_items_in_and_out() {
        let t = "- a\n- b";
        let e = tab(t, 6, 6, false).expect("edit");
        assert_eq!(apply(t, &e), "- a\n  - b");
        assert_eq!(e.head, 8);
        let t = "- a\n  - b";
        let e = tab(t, 8, 8, true).expect("edit");
        assert_eq!(apply(t, &e), "- a\n- b");
        assert_eq!(e.head, 6);
        // Several lines at once.
        let t = "- a\n- b\n- c";
        let e = tab(t, 1, 10, false).expect("edit");
        assert_eq!(apply(t, &e), "  - a\n  - b\n  - c");
        assert_eq!((e.anchor, e.head), (3, 12));
        // Nothing to outdent.
        assert_eq!(tab("- a", 1, 1, true), None);
        // Numbered lists indent by three.
        let t = "1. a";
        assert_eq!(apply(t, &tab(t, 4, 4, false).expect("edit")), "   1. a");
        // Outside lists: a tab inside a line only.
        assert_eq!(apply("ab", &tab("ab", 1, 1, false).expect("edit")), "a\tb");
        assert_eq!(tab("ab", 0, 0, false), None);
        assert_eq!(tab("ab", 1, 1, true), None);
    }

    #[test]
    fn nothing_panics_on_odd_offsets() {
        let t = "\u{e9}\u{1F44D}x\r\n- y";
        for from in 0..t.len() + 3 {
            for m in [
                Motion::Left,
                Motion::Right,
                Motion::WordLeft,
                Motion::WordRight,
                Motion::LineStart,
                Motion::LineEnd,
            ] {
                let to = move_caret(t, from, m);
                assert!(t.is_char_boundary(to));
            }
            let _ = word_range(t, from);
            let _ = backspace(t, from, from + 1, true);
            let _ = delete_forward(t, from, from, true);
            let _ = enter(t, from, from + 2, "\n", false, false);
            let _ = tab(t, from, from + 3, true);
            let _ = tab(t, from, from, false);
            let _ = active_region(t, &[0..4, 6..9], from, from + 2, 0..t.len());
        }
    }

    #[test]
    fn motion_names_round_trip() {
        for name in [
            "left",
            "right",
            "word_left",
            "word_right",
            "line_start",
            "line_end",
            "start",
            "end",
        ] {
            assert_eq!(Motion::from_name(name).map(Motion::name), Some(name));
        }
        assert_eq!(Motion::from_name("up"), None);
    }
}
