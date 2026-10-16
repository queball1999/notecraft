//! Text formatting as edits on the markdown source (PR 1.6).
//!
//! Every function takes the page text and the selection (byte offsets) and returns one
//! [`FormatEdit`]: a single splice, trimmed to the bytes that really change, plus where the
//! selection ends up. Nothing here re-serializes a paragraph: a command inserts, moves, or
//! removes the delimiters it needs and leaves every other byte alone, so saving rewrites only
//! what the user touched.
//!
//! Inline marks (bold, italic, strikethrough, underline, subscript, superscript, highlight,
//! font color, font size, font family) are delimiters around the selection or, with a caret,
//! around the word at the caret. Block formatting (headings, quote, code, lists, task boxes,
//! indent, alignment) is a prefix on each line the selection touches. The vocabulary is in
//! [`crate::markup`]; the format is in docs/markdown.md section 3.2.

mod blocks;
mod inline;
mod lines;
mod scan;
mod state;
#[cfg(test)]
mod tests;

use std::ops::Range;

pub use blocks::{
    apply_style, indent, set_align, toggle_bullets, toggle_numbering, toggle_todo, Style,
};
pub use inline::{clear_formatting, set_mark, toggle_mark};
pub use scan::{scan, SpanInfo};
pub use state::{state_at, FormatState, ListState};

/// One splice of the page text and the selection after it. The fields match the editor's own
/// text edit type, so the engine can pass them on as they are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatEdit {
    pub at: usize,
    pub remove: usize,
    pub insert: String,
    pub anchor: usize,
    pub head: usize,
}

/// Most lines one command changes.
pub const MAX_LINES: usize = 20_000;

/// What a formatting command works on: the page text, the selection, and the limits.
#[derive(Clone, Copy, Debug)]
pub struct Ctx<'a> {
    pub text: &'a str,
    /// Offsets before this are never changed (the frontmatter, and the start of the container
    /// that holds the selection, where code fences are counted from).
    pub floor: usize,
    pub anchor: usize,
    pub head: usize,
    /// The line ending to write.
    pub eol: &'a str,
}

impl Ctx<'_> {
    /// The selection as `(low, high)`, snapped to character boundaries inside the text.
    pub fn range(&self) -> (usize, usize) {
        let a = snap(self.text, self.anchor);
        let h = snap(self.text, self.head);
        (a.min(h), a.max(h))
    }
}

/// Move `at` back to a character boundary and clamp it to the text.
pub(crate) fn snap(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// One replacement in a working text: remove `del` bytes at `at`, put `ins` there.
#[derive(Clone, Debug)]
pub(crate) struct Rep {
    pub at: usize,
    pub del: usize,
    pub ins: String,
    /// An insertion that ends the text before it (a closing delimiter): a selection that starts
    /// exactly there stays before it.
    pub after: bool,
}

impl Rep {
    pub fn insert(at: usize, ins: impl Into<String>) -> Self {
        Self {
            at,
            del: 0,
            ins: ins.into(),
            after: false,
        }
    }

    /// An insertion after the text it follows, such as a closing delimiter.
    pub fn insert_after(at: usize, ins: impl Into<String>) -> Self {
        Self {
            after: true,
            ..Self::insert(at, ins)
        }
    }

    pub fn delete(range: Range<usize>) -> Self {
        Self {
            at: range.start,
            del: range.end.saturating_sub(range.start),
            ins: String::new(),
            after: false,
        }
    }

    pub fn replace(range: Range<usize>, ins: impl Into<String>) -> Self {
        Self {
            at: range.start,
            del: range.end.saturating_sub(range.start),
            ins: ins.into(),
            after: false,
        }
    }
}

fn sorted(mut reps: Vec<Rep>) -> Vec<Rep> {
    // Insertions go before a deletion that starts at the same place.
    reps.sort_by_key(|r| (r.at, r.del > 0));
    reps
}

fn apply_reps(text: &str, reps: &[Rep]) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut cursor = 0;
    for rep in reps {
        if rep.at < cursor || rep.at > text.len() {
            continue;
        }
        out.push_str(text.get(cursor..rep.at).unwrap_or(""));
        out.push_str(&rep.ins);
        cursor = (rep.at + rep.del).min(text.len());
    }
    out.push_str(text.get(cursor..).unwrap_or(""));
    out
}

/// Where offset `p` of the old text is in the new one. `start` picks the side of an insertion
/// exactly at `p`: the start of a selection moves past it, the end stays before it.
fn map_pos(reps: &[Rep], p: usize, start: bool) -> usize {
    let mut shift: isize = 0;
    for rep in reps {
        let grow = rep.ins.len() as isize - rep.del as isize;
        if rep.del == 0 {
            if rep.at < p || (rep.at == p && start && !rep.after) {
                shift += grow;
            }
        } else if rep.at + rep.del <= p {
            shift += grow;
        } else if rep.at < p {
            return usize::try_from(rep.at as isize + shift + rep.ins.len() as isize).unwrap_or(0);
        }
    }
    usize::try_from(p as isize + shift).unwrap_or(0)
}

/// A text being edited by a command, with the selection followed through every change.
pub(crate) struct Work {
    pub text: String,
    pub lo: usize,
    pub hi: usize,
    /// True when the selection ran from the head back to the anchor.
    backwards: bool,
}

impl Work {
    pub fn new(ctx: &Ctx<'_>) -> Self {
        let (lo, hi) = ctx.range();
        Self {
            text: ctx.text.to_owned(),
            lo,
            hi,
            backwards: snap(ctx.text, ctx.head) < snap(ctx.text, ctx.anchor),
        }
    }

    /// Apply `reps` (offsets in the current text) and follow the selection.
    pub fn run(&mut self, reps: Vec<Rep>) {
        let reps = sorted(reps);
        self.text = apply_reps(&self.text, &reps);
        self.lo = map_pos(&reps, self.lo, true);
        self.hi = map_pos(&reps, self.hi, false).max(self.lo);
    }

    /// Where an old offset is after `reps`, without applying them.
    pub fn mapped(reps: &[Rep], p: usize, start: bool) -> usize {
        map_pos(&sorted(reps.to_vec()), p, start)
    }

    /// The edit from `original` to the working text, or `None` when nothing changed.
    pub fn finish(self, original: &str) -> Option<FormatEdit> {
        let (lo, hi) = (self.lo, self.hi);
        self.finish_at(original, lo, hi)
    }

    /// Like [`Work::finish`], with the selection placed at `lo` and `hi`.
    pub fn finish_at(self, original: &str, lo: usize, hi: usize) -> Option<FormatEdit> {
        if self.text == original {
            return None;
        }
        let new = self.text.as_str();
        let max_common = original.len().min(new.len());
        let mut pre = original
            .bytes()
            .zip(new.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        while pre > 0 && !(original.is_char_boundary(pre) && new.is_char_boundary(pre)) {
            pre -= 1;
        }
        let mut suf = original
            .bytes()
            .rev()
            .zip(new.bytes().rev())
            .take_while(|(a, b)| a == b)
            .count()
            .min(max_common - pre);
        while suf > 0
            && !(original.is_char_boundary(original.len() - suf)
                && new.is_char_boundary(new.len() - suf))
        {
            suf -= 1;
        }
        let insert = new.get(pre..new.len() - suf)?.to_owned();
        let (anchor, head) = if self.backwards { (hi, lo) } else { (lo, hi) };
        Some(FormatEdit {
            at: pre,
            remove: original.len() - pre - suf,
            insert,
            anchor,
            head,
        })
    }
}

/// A line of the page: where it starts, where its text ends (before the line ending), and
/// where the next line starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    pub start: usize,
    pub end: usize,
    pub next: usize,
}

impl Line {
    /// The line that holds offset `at`.
    pub fn at(text: &str, at: usize) -> Self {
        let at = snap(text, at);
        let start = text
            .get(..at)
            .and_then(|head| head.rfind('\n'))
            .map_or(0, |i| i + 1);
        let tail = text.get(start..).unwrap_or("");
        let (mut end, next) = match tail.find('\n') {
            Some(i) => (start + i, start + i + 1),
            None => (text.len(), text.len()),
        };
        if end > start && text.as_bytes().get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        Self { start, end, next }
    }

    pub fn str<'a>(&self, text: &'a str) -> &'a str {
        text.get(self.start..self.end).unwrap_or("")
    }

    /// The line before this one, if there is one.
    pub fn previous(&self, text: &str) -> Option<Self> {
        (self.start > 0).then(|| Self::at(text, self.start - 1))
    }
}

/// The lines the selection touches: from the line of `lo` to the line of `hi`, except that a
/// selection that ends at the very start of a line does not take that line. Capped at
/// [`MAX_LINES`].
pub(crate) fn touched_lines(text: &str, lo: usize, hi: usize) -> Vec<Line> {
    let mut out = Vec::new();
    let mut line = Line::at(text, lo);
    loop {
        out.push(line);
        if line.next >= text.len() || line.next > hi || out.len() >= MAX_LINES {
            break;
        }
        // A selection that ends exactly at the start of the next line leaves it out.
        if line.next == hi && hi > lo {
            break;
        }
        line = Line::at(text, line.next);
    }
    out
}

/// Whether `line` is a container marker or other comment NoteCraft keeps out of commands.
pub(crate) fn is_marker(line: &str) -> bool {
    line.trim_start().starts_with("<!-- notecraft:")
}

/// The fence character of a code fence line, if it is one.
fn fence_of(line: &str) -> Option<(char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = line.get(indent..)?;
    let c = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let n = rest.chars().take_while(|&x| x == c).count();
    (n >= 3).then_some((c, n))
}

/// Where each fenced code block lies, as ranges from the start of its opening fence line to
/// the end of its closing fence line (or the end of the text), counted from `from`.
pub(crate) fn fences(text: &str, from: usize) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut open: Option<(char, usize, usize)> = None;
    let mut line = Line::at(text, snap(text, from));
    let mut count = 0usize;
    loop {
        let s = line.str(text);
        if let Some((c, n)) = fence_of(s) {
            match open {
                None => open = Some((c, n, line.start)),
                Some((oc, on, start)) if oc == c && n >= on && s.trim().chars().all(|x| x == c) => {
                    out.push(start..line.end);
                    open = None;
                }
                Some(_) => {}
            }
        }
        count += 1;
        if line.next >= text.len() || count >= MAX_LINES * 5 {
            break;
        }
        line = Line::at(text, line.next);
    }
    if let Some((_, _, start)) = open {
        out.push(start..text.len());
    }
    out
}

/// True when `at` is inside any of the fenced `ranges`.
pub(crate) fn in_fence(ranges: &[Range<usize>], at: usize) -> bool {
    ranges.iter().any(|r| r.start <= at && at <= r.end)
}
