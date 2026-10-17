//! Inline marks: bold, italic, strikethrough, underline, subscript, superscript, highlight,
//! font color, size, and family, as delimiters around the selection or the word at the caret.

use super::lines::Prefix;
use super::scan::{scan, SpanInfo};
use super::{blocks, fences, in_fence, is_marker, touched_lines, Ctx, FormatEdit, Line, Rep, Work};
use crate::markup::{self, Mark};

/// What a mark command wants.
#[derive(Clone, Debug)]
pub(crate) enum Want {
    /// Add the mark, or remove it when all the text already has it.
    Toggle,
    /// Add the mark where it is missing; do nothing when all the text has it.
    Add,
    /// Replace any mark of this kind with this value; `None` removes it.
    Set(Option<String>),
}

/// The part of one line a command works on.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Seg {
    pub line: Line,
    /// Where the line's text starts (after list and heading markers).
    pub content: usize,
    /// The selected part, without surrounding spaces.
    pub a: usize,
    pub b: usize,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

/// The word around `at` on its line, within `content..end`.
fn word_around(text: &str, at: usize, content: usize, end: usize) -> (usize, usize) {
    let mut a = at;
    while a > content {
        match text.get(..a).and_then(|t| t.chars().next_back()) {
            Some(c) if is_word(c) => a -= c.len_utf8(),
            _ => break,
        }
    }
    let mut b = at;
    while b < end {
        match text.get(b..).and_then(|t| t.chars().next()) {
            Some(c) if is_word(c) => b += c.len_utf8(),
            _ => break,
        }
    }
    (a, b)
}

/// The pieces of lines the selection covers. With a caret, the word at the caret (none when
/// the caret is not on a word).
pub(crate) fn segments(ctx: &Ctx<'_>) -> Vec<Seg> {
    let text = ctx.text;
    let (lo, hi) = ctx.range();
    let ranges = fences(text, ctx.floor);
    let mut out = Vec::new();
    for line in touched_lines(text, lo, hi) {
        let s = line.str(text);
        if line.start < ctx.floor || is_marker(s) || in_fence(&ranges, line.start) {
            continue;
        }
        let content = line.start + Prefix::parse(s).content;
        let (mut a, mut b) = if lo == hi {
            word_around(text, lo.clamp(content, line.end), content, line.end)
        } else {
            (lo.max(content), hi.min(line.end))
        };
        // Spaces at the edges stay outside the marks.
        while a < b
            && text
                .get(a..)
                .and_then(|t| t.chars().next())
                .is_some_and(char::is_whitespace)
        {
            a += text
                .get(a..)
                .and_then(|t| t.chars().next())
                .map_or(1, char::len_utf8);
        }
        while b > a
            && text
                .get(..b)
                .and_then(|t| t.chars().next_back())
                .is_some_and(char::is_whitespace)
        {
            b -= text
                .get(..b)
                .and_then(|t| t.chars().next_back())
                .map_or(1, char::len_utf8);
        }
        if a < b {
            out.push(Seg {
                line,
                content,
                a,
                b,
            });
        }
    }
    out
}

/// Widen `a..b` over delimiters that touch its edges, so removing a mark from the text inside
/// `**one *two* three**` also takes the nested marks around it with it.
fn expand(spans: &[SpanInfo], mut a: usize, mut b: usize) -> (usize, usize) {
    for _ in 0..8 {
        let mut changed = false;
        for span in spans {
            for token in [&span.open, &span.close] {
                if token.end == a && token.start < a {
                    a = token.start;
                    changed = true;
                }
                if token.start == b && token.end > b {
                    b = token.end;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    (a, b)
}

fn skip_space_forward(text: &str, mut at: usize, limit: usize) -> usize {
    while at < limit {
        match text.get(at..).and_then(|t| t.chars().next()) {
            Some(c) if c.is_whitespace() => at += c.len_utf8(),
            _ => break,
        }
    }
    at
}

fn skip_space_back(text: &str, mut at: usize, limit: usize) -> usize {
    while at > limit {
        match text.get(..at).and_then(|t| t.chars().next_back()) {
            Some(c) if c.is_whitespace() => at -= c.len_utf8(),
            _ => break,
        }
    }
    at
}

/// Edits that take the spans picked by `pick` off the text `a..b`, splitting a span that
/// reaches beyond it. Spaces next to a cut stay outside the delimiters.
pub(crate) fn unmark_reps(
    text: &str,
    spans: &[SpanInfo],
    pick: impl Fn(&SpanInfo) -> bool,
    a: usize,
    b: usize,
) -> Vec<Rep> {
    let mut reps = Vec::new();
    for span in spans.iter().filter(|s| pick(s)) {
        let content = span.content();
        let x = a.max(content.start);
        let y = b.min(content.end);
        if x >= y {
            continue;
        }
        let open = text.get(span.open.clone()).unwrap_or("").to_owned();
        let close = text.get(span.close.clone()).unwrap_or("").to_owned();
        let left_whole = x == content.start;
        let right_whole = y == content.end;
        if left_whole {
            reps.push(Rep::delete(span.open.clone()));
        }
        if right_whole {
            reps.push(Rep::delete(span.close.clone()));
        }
        match (left_whole, right_whole) {
            (true, true) => {}
            (true, false) => reps.push(Rep::insert(skip_space_forward(text, y, content.end), open)),
            (false, true) => reps.push(Rep::insert_after(
                skip_space_back(text, x, content.start),
                close,
            )),
            (false, false) => {
                reps.push(Rep::insert_after(
                    skip_space_back(text, x, content.start),
                    close,
                ));
                reps.push(Rep::insert(skip_space_forward(text, y, content.end), open));
            }
        }
    }
    reps
}

/// Whether `a..b` lies inside a span of `mark` (with or without the delimiters selected).
fn covered(spans: &[SpanInfo], mark: Mark, a: usize, b: usize) -> bool {
    spans
        .iter()
        .any(|s| s.mark == mark && s.open.start <= a && b <= s.close.end)
}

/// The marks that `mark` replaces when it is applied.
fn conflicts(mark: Mark, other: Mark) -> bool {
    mark == other
        || matches!(
            (mark, other),
            (Mark::Subscript, Mark::Superscript) | (Mark::Superscript, Mark::Subscript)
        )
}

/// Toggle `mark` on the selection or the word at the caret. A caret that is not on a word
/// puts an empty pair of delimiters at the caret to type into (or takes an empty pair away).
pub fn toggle_mark(ctx: &Ctx<'_>, mark: Mark) -> Option<FormatEdit> {
    run(ctx, mark, &Want::Toggle)
}

/// Set a value mark (highlight, color, size, family) on the selection or the word at the
/// caret. `None` removes the mark.
pub fn set_mark(ctx: &Ctx<'_>, mark: Mark, value: Option<&str>) -> Option<FormatEdit> {
    let value = match value {
        Some(v) => Some(markup::normalize_value(mark, v)?),
        None => None,
    };
    run(ctx, mark, &Want::Set(value))
}

/// Add `mark` to every line of the selection that lacks it (used by styles that are marks).
pub(crate) fn add_mark(ctx: &Ctx<'_>, mark: Mark) -> Option<FormatEdit> {
    run(ctx, mark, &Want::Add)
}

fn run(ctx: &Ctx<'_>, mark: Mark, want: &Want) -> Option<FormatEdit> {
    let segs = segments(ctx);
    let (lo, hi) = ctx.range();
    if segs.is_empty() {
        return if lo == hi {
            typing_pair(ctx, mark, want)
        } else {
            None
        };
    }
    let all_covered = segs
        .iter()
        .all(|s| covered(&scan(ctx.text, s.content, s.line.end), mark, s.a, s.b));
    let remove_only = match want {
        Want::Toggle => all_covered,
        Want::Add => {
            if all_covered {
                return None;
            }
            false
        }
        Want::Set(value) => value.is_none(),
    };
    let value = match want {
        Want::Set(value) => value.clone(),
        _ => None,
    };
    let mut work = Work::new(ctx);
    for seg in segs.iter().rev() {
        let spans = scan(&work.text, seg.content, seg.line.end);
        let pick = |s: &SpanInfo| conflicts(mark, s.mark);
        if remove_only {
            let (a, b) = expand(&spans, seg.a, seg.b);
            let reps = unmark_reps(&work.text, &spans, pick, a, b);
            work.run(reps);
            continue;
        }
        let reps = unmark_reps(&work.text, &spans, pick, seg.a, seg.b);
        let a = Work::mapped(&reps, seg.a, true);
        let b = Work::mapped(&reps, seg.b, false);
        work.run(reps);
        let (open, close) = markup::delimiters(mark, value.as_deref());
        work.run(vec![Rep::insert(a, open), Rep::insert_after(b, close)]);
    }
    work.finish(ctx.text)
}

/// A caret off any word: type into a new pair of delimiters, or remove an empty pair.
fn typing_pair(ctx: &Ctx<'_>, mark: Mark, want: &Want) -> Option<FormatEdit> {
    let text = ctx.text;
    let (at, _) = ctx.range();
    let line = Line::at(text, at);
    let s = line.str(text);
    if line.start < ctx.floor || is_marker(s) || in_fence(&fences(text, ctx.floor), line.start) {
        return None;
    }
    let content = line.start + Prefix::parse(s).content;
    let spans = scan(text, content, line.end);
    // Inside an empty pair of this mark: take it away.
    let (plain_open, plain_close) = markup::delimiters(mark, None);
    let before = text.get(..at).unwrap_or("");
    let after = text.get(at..).unwrap_or("");
    if !plain_open.is_empty()
        && before.len() - line.start.min(before.len()) >= plain_open.len()
        && before.ends_with(&plain_open)
        && after.starts_with(&plain_close)
        && before.len() - plain_open.len() >= content
    {
        let mut work = Work::new(ctx);
        let start = at - plain_open.len();
        work.run(vec![Rep::delete(start..at + plain_close.len())]);
        return work.finish_at(text, start, start);
    }
    if let Some(span) = spans
        .iter()
        .find(|s| s.mark == mark && s.content().is_empty() && s.content().start == at)
    {
        let mut work = Work::new(ctx);
        work.run(vec![
            Rep::delete(span.open.clone()),
            Rep::delete(span.close.clone()),
        ]);
        let pos = span.open.start;
        return work.finish_at(text, pos, pos);
    }
    // Inside a span of this mark with no word to cut it from: leave it.
    if spans
        .iter()
        .any(|s| s.mark == mark && s.content().start <= at && at <= s.content().end)
    {
        return None;
    }
    let value = match want {
        Want::Set(None) => return None,
        Want::Set(value) => value.clone(),
        _ => None,
    };
    let (open, close) = markup::delimiters(mark, value.as_deref());
    let caret = at.max(content);
    let mut work = Work::new(ctx);
    let inside = caret + open.len();
    work.run(vec![Rep::insert(caret, format!("{open}{close}"))]);
    work.finish_at(text, inside, inside)
}

/// Clear All Formatting: take every mark off the selection (or the paragraph line at the
/// caret) and reset its heading, quote, and alignment.
pub fn clear_formatting(ctx: &Ctx<'_>) -> Option<FormatEdit> {
    let (lo, hi) = ctx.range();
    let mut work = Work::new(ctx);
    let collapsed = lo == hi;
    let sel_ctx = if collapsed {
        // Treat the whole line as selected.
        let line = Line::at(ctx.text, lo);
        let content = line.start + Prefix::parse(line.str(ctx.text)).content;
        Ctx {
            anchor: content.min(line.end),
            head: line.end,
            ..*ctx
        }
    } else {
        *ctx
    };
    for seg in segments(&sel_ctx).iter().rev() {
        let spans = scan(&work.text, seg.content, seg.line.end);
        let (a, b) = expand(&spans, seg.a, seg.b);
        let reps = unmark_reps(&work.text, &spans, |_| true, a, b);
        work.run(reps);
    }
    blocks::clear_block_style(&mut work, ctx);
    work.finish(ctx.text)
}
