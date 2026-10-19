//! Block formatting: styles (headings, quote, code), lists, task boxes, indent, and alignment.
//! Each is a prefix on the lines the selection touches, or a comment line above a paragraph.

use super::lines::Prefix;
use super::{
    fence_of, fences, in_fence, inline, is_marker, touched_lines, Ctx, FormatEdit, Line, Rep, Work,
};
use crate::markup::{self, Mark};
use crate::Align;

/// A paragraph style from the Styles gallery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// `#` to `######`.
    Heading(u8),
    /// Page Title: a level 1 heading.
    Title,
    /// Citation: the paragraph in italics.
    Citation,
    /// A block quote.
    Quote,
    /// A fenced code block.
    Code,
    /// Plain paragraph text: no heading, quote, or code fence.
    Normal,
}

impl Style {
    /// The style a `style.apply` name stands for.
    pub fn from_name(name: &str) -> Option<Self> {
        if let Some(level) = name.strip_prefix("heading") {
            let level: u8 = level.parse().ok()?;
            return (1..=6).contains(&level).then_some(Self::Heading(level));
        }
        match name {
            "title" => Some(Self::Title),
            "citation" => Some(Self::Citation),
            "quote" => Some(Self::Quote),
            "code" => Some(Self::Code),
            "normal" => Some(Self::Normal),
            _ => None,
        }
    }
}

/// The lines a block command changes: the touched lines that are text. A caret on an empty line
/// still counts; a selection skips empty lines.
fn targets(ctx: &Ctx<'_>) -> Vec<(Line, Prefix)> {
    let text = ctx.text;
    let (lo, hi) = ctx.range();
    let ranges = fences(text, ctx.floor);
    touched_lines(text, lo, hi)
        .into_iter()
        .filter_map(|line| {
            let s = line.str(text);
            let skip = line.start < ctx.floor
                || is_marker(s)
                || in_fence(&ranges, line.start)
                || (lo != hi && s.trim().is_empty());
            (!skip).then(|| (line, Prefix::parse(s)))
        })
        .collect()
}

fn finish(ctx: &Ctx<'_>, reps: Vec<Rep>) -> Option<FormatEdit> {
    if reps.is_empty() {
        return None;
    }
    let mut work = Work::new(ctx);
    work.run(reps);
    work.finish(ctx.text)
}

/// Apply a paragraph style to the lines the selection touches.
pub fn apply_style(ctx: &Ctx<'_>, style: Style) -> Option<FormatEdit> {
    match style {
        Style::Heading(level) => heading(ctx, level.clamp(1, 6)),
        Style::Title => heading(ctx, 1),
        Style::Normal => normal(ctx),
        Style::Quote => {
            let reps = targets(ctx)
                .into_iter()
                .filter(|(_, p)| p.quote_depth == 0)
                .map(|(line, p)| Rep::insert(line.start + p.indent, "> "))
                .collect();
            finish(ctx, reps)
        }
        Style::Code => code_block(ctx),
        Style::Citation => {
            let lines = targets(ctx);
            let (first, last) = (lines.first()?, lines.last()?);
            let inner = Ctx {
                anchor: first.0.start + first.1.content,
                head: last.0.end,
                ..*ctx
            };
            inline::add_mark(&inner, Mark::Italic)
        }
    }
}

fn heading(ctx: &Ctx<'_>, level: u8) -> Option<FormatEdit> {
    let marker = format!("{} ", "#".repeat(usize::from(level)));
    let reps = targets(ctx)
        .into_iter()
        .map(|(line, p)| Rep::replace(line.start..line.start + p.content, marker.clone()))
        .collect();
    finish(ctx, reps)
}

/// Whole-line deletion (the line and its line ending).
fn delete_line(line: Line) -> Rep {
    Rep::delete(line.start..line.next)
}

fn normal(ctx: &Ctx<'_>) -> Option<FormatEdit> {
    let text = ctx.text;
    let (lo, hi) = ctx.range();
    let mut reps = normal_reps(&targets(ctx));
    // A fenced block the selection touches loses its fence lines.
    for range in fences(text, ctx.floor) {
        let (open, close) = (Line::at(text, range.start), Line::at(text, range.end));
        let touched = range.start <= hi && lo <= range.end;
        if !touched || open.start < ctx.floor {
            continue;
        }
        reps.push(delete_line(open));
        if close.start > open.start && fence_of(close.str(text)).is_some() {
            // A closing fence at the end of the text takes the line break before it.
            let start = if close.next == close.end {
                Line::at(text, close.start.saturating_sub(1)).end
            } else {
                close.start
            };
            reps.push(Rep::delete(start..close.next));
        }
    }
    finish(ctx, reps)
}

/// Edits that remove heading markers and quote prefixes from `lines`.
fn normal_reps(lines: &[(Line, Prefix)]) -> Vec<Rep> {
    let mut reps = Vec::new();
    for (line, p) in lines {
        if let Some((_, end)) = p.heading {
            reps.push(Rep::delete(line.start + p.quote_end..line.start + end));
        }
        if p.quote_depth > 0 {
            reps.push(Rep::delete(line.start + p.indent..line.start + p.quote_end));
        }
    }
    reps
}

fn code_block(ctx: &Ctx<'_>) -> Option<FormatEdit> {
    let text = ctx.text;
    let (lo, hi) = ctx.range();
    let ranges = fences(text, ctx.floor);
    let mut lines = touched_lines(text, lo, hi);
    // Markers at the ends of the selection stay outside the fence.
    while lines.first().is_some_and(|l| is_marker(l.str(text))) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| is_marker(l.str(text))) {
        lines.pop();
    }
    let (first, last) = (*lines.first()?, *lines.last()?);
    if first.start < ctx.floor
        || lines
            .iter()
            .any(|l| in_fence(&ranges, l.start) || is_marker(l.str(text)))
    {
        return None;
    }
    let fence = format!("```{}", ctx.eol);
    let mut reps = vec![Rep::insert(first.start, fence.clone())];
    if last.next > last.end {
        reps.push(Rep::insert_after(last.next, fence));
    } else {
        reps.push(Rep::insert_after(last.end, format!("{}```", ctx.eol)));
    }
    finish(ctx, reps)
}

/// Where a line's list marker, quote, and indent start for removal: after the quote markers,
/// or at the start of the line without any.
fn removal_start(p: &Prefix) -> usize {
    if p.quote_depth > 0 {
        p.quote_end
    } else {
        0
    }
}

/// Turn the selected lines into a bulleted list, or back to plain lines when they all are.
pub fn toggle_bullets(ctx: &Ctx<'_>) -> Option<FormatEdit> {
    list_marker(
        ctx,
        |_| "- ".to_owned(),
        |p| {
            p.list
                .as_ref()
                .is_some_and(|l| l.number.is_none() && l.task.is_none())
        },
    )
}

/// Turn the selected lines into a numbered list, or back to plain lines when they all are.
pub fn toggle_numbering(ctx: &Ctx<'_>) -> Option<FormatEdit> {
    let text = ctx.text;
    let (lo, _) = ctx.range();
    // A list that already runs above the selection goes on from its last number.
    let first = Line::at(text, lo);
    let mut next = 1;
    if let Some(prev) = first.previous(text) {
        let p = Prefix::parse(prev.str(text));
        let first_prefix = Prefix::parse(first.str(text));
        if let Some(n) = p.list.as_ref().and_then(|l| l.number) {
            if p.indent == first_prefix.indent && prev.start >= ctx.floor {
                next = n.saturating_add(1);
            }
        }
    }
    let mut counter = next;
    list_marker(
        ctx,
        move |_| {
            let marker = format!("{counter}. ");
            counter = counter.saturating_add(1);
            marker
        },
        |p| {
            p.list
                .as_ref()
                .is_some_and(|l| l.number.is_some() && l.task.is_none())
        },
    )
}

fn list_marker(
    ctx: &Ctx<'_>,
    mut marker: impl FnMut(&Prefix) -> String,
    is_this_list: impl Fn(&Prefix) -> bool,
) -> Option<FormatEdit> {
    let lines = targets(ctx);
    if lines.is_empty() {
        return None;
    }
    let remove = lines.iter().all(|(_, p)| is_this_list(p));
    let mut reps = Vec::new();
    for (line, p) in &lines {
        let base = line.start;
        if remove {
            if let Some(list) = &p.list {
                reps.push(Rep::delete(
                    base + removal_start(p)..base + list.whole().end,
                ));
            }
            continue;
        }
        let new = marker(p);
        if let Some(list) = &p.list {
            reps.push(Rep::replace(
                base + list.whole().start..base + list.whole().end,
                new,
            ));
        } else if let Some((_, end)) = p.heading {
            reps.push(Rep::replace(base + p.quote_end..base + end, new));
        } else {
            reps.push(Rep::insert(base + p.quote_end, new));
        }
    }
    finish(ctx, reps)
}

/// Add a To Do check box to the selected lines (making them list items), or take it away when
/// they all have one.
pub fn toggle_todo(ctx: &Ctx<'_>) -> Option<FormatEdit> {
    let lines = targets(ctx);
    if lines.is_empty() {
        return None;
    }
    let remove = lines
        .iter()
        .all(|(_, p)| p.list.as_ref().is_some_and(|l| l.task.is_some()));
    let mut reps = Vec::new();
    for (line, p) in &lines {
        let base = line.start;
        match (&p.list, remove) {
            (Some(list), true) => {
                reps.push(Rep::delete(
                    base + removal_start(p)..base + list.whole().end,
                ));
            }
            (Some(list), false) if list.task.is_none() => {
                reps.push(Rep::insert(base + list.marker.end, "[ ] "));
            }
            (Some(_), false) => {}
            (None, _) => match p.heading {
                Some((_, end)) => reps.push(Rep::replace(base + p.quote_end..base + end, "- [ ] ")),
                None => reps.push(Rep::insert(base + p.quote_end, "- [ ] ")),
            },
        }
    }
    finish(ctx, reps)
}

/// Move the selected list items one level in (or out), and nest (or unnest) quoted lines.
/// Plain paragraphs have no indent in markdown and are left alone.
pub fn indent(ctx: &Ctx<'_>, outdent: bool) -> Option<FormatEdit> {
    let text = ctx.text;
    let mut reps = Vec::new();
    for (line, p) in targets(ctx) {
        let base = line.start;
        if let Some(list) = &p.list {
            let at = base + removal_start(&p);
            let step = list.marker.len().max(2);
            if !outdent {
                reps.push(Rep::insert(at, " ".repeat(step)));
                continue;
            }
            let rest = text.get(at..line.end).unwrap_or("");
            let cut = if rest.starts_with('\t') {
                1
            } else {
                rest.bytes().take_while(|&b| b == b' ').count().min(step)
            };
            if cut > 0 {
                reps.push(Rep::delete(at..at + cut));
            }
        } else if p.quote_depth > 0 {
            let at = base + p.indent;
            if !outdent {
                reps.push(Rep::insert(at, "> "));
            } else {
                let rest = text.get(at..line.end).unwrap_or("");
                let cut = if rest.starts_with("> ") { 2 } else { 1 };
                reps.push(Rep::delete(at..at + cut));
            }
        }
    }
    finish(ctx, reps)
}

/// The first line of the paragraph that holds `line`: a heading and a list item are their own
/// paragraph; plain text runs back to a blank line, heading, or alignment comment.
pub(crate) fn paragraph_start(text: &str, line: Line, floor: usize) -> Line {
    let mut first = line;
    if Prefix::parse(line.str(text)).heading.is_some() {
        return first;
    }
    while let Some(prev) = first.previous(text) {
        let s = prev.str(text);
        let p = Prefix::parse(s);
        if prev.start < floor
            || s.trim().is_empty()
            || is_marker(s)
            || markup::parse_align_comment(s).is_some()
            || p.heading.is_some()
        {
            break;
        }
        first = prev;
    }
    first
}

/// The alignment comment line directly above `first`, if there is one.
pub(crate) fn align_marker(text: &str, first: Line, floor: usize) -> Option<(Line, Align)> {
    let prev = first.previous(text)?;
    if prev.start < floor {
        return None;
    }
    let align = markup::parse_align_comment(prev.str(text))?;
    Some((prev, align))
}

/// Align the paragraphs the selection touches. Left removes the alignment comment.
pub fn set_align(ctx: &Ctx<'_>, align: Align) -> Option<FormatEdit> {
    let text = ctx.text;
    let mut firsts: Vec<Line> = Vec::new();
    for (line, _) in targets(ctx) {
        let first = paragraph_start(text, line, ctx.floor);
        let p = Prefix::parse(first.str(text));
        let plain = p.list.is_none() && p.quote_depth == 0;
        if plain && first.start >= ctx.floor && !firsts.contains(&first) {
            firsts.push(first);
        }
    }
    let comment = markup::align_comment(align);
    let mut reps = Vec::new();
    for first in firsts {
        let current = align_marker(text, first, ctx.floor);
        match (&comment, current) {
            (Some(c), Some((marker, _))) => {
                reps.push(Rep::replace(marker.start..marker.end, c.clone()))
            }
            (Some(c), None) => reps.push(Rep::insert(first.start, format!("{c}{}", ctx.eol))),
            (None, Some((marker, _))) => reps.push(delete_line(marker)),
            (None, None) => {}
        }
    }
    finish(ctx, reps)
}

/// The part of Clear All Formatting that is not inline: heading markers, quote prefixes, and
/// alignment comments on the lines the working selection touches.
pub(crate) fn clear_block_style(work: &mut Work, ctx: &Ctx<'_>) {
    let reps = {
        let inner = Ctx {
            text: &work.text,
            anchor: work.lo,
            head: work.hi,
            ..*ctx
        };
        let lines = targets(&inner);
        let mut reps = normal_reps(&lines);
        let mut seen: Vec<usize> = Vec::new();
        for (line, _) in &lines {
            let first = paragraph_start(inner.text, *line, inner.floor);
            if seen.contains(&first.start) {
                continue;
            }
            seen.push(first.start);
            if let Some((marker, _)) = align_marker(inner.text, first, inner.floor) {
                reps.push(delete_line(marker));
            }
        }
        reps
    };
    work.run(reps);
}
