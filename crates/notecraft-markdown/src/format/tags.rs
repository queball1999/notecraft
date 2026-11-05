//! Note tags as edits (PR 2.5): applying a tag to the paragraphs the selection touches puts
//! `#slug ` at the start of the paragraph text, after any list marker, check box, heading
//! marker, or quote prefix and after the tags already there; applying it again takes it
//! away. Remove Tag takes every known tag (and a To Do check box) off the paragraphs.

use std::ops::Range;

use super::blocks::{finish, removal_start, targets};
use super::lines::Prefix;
use super::{fence_of, is_marker, Ctx, FormatEdit, Line, Rep, MAX_LINES};
use crate::markup;
use crate::tags::{scan_tags, InlineTag};

/// One paragraph the selection touches.
struct Para {
    first: Line,
    prefix: Prefix,
    tags: Vec<InlineTag>,
}

/// Whether `s` starts a new block, so a paragraph does not run into it.
fn breaks(s: &str) -> bool {
    let p = Prefix::parse(s);
    s.trim().is_empty()
        || is_marker(s)
        || p.list.is_some()
        || p.heading.is_some()
        || fence_of(s).is_some()
        || markup::parse_align_comment(s).is_some()
}

/// The paragraph that holds `line`: a heading and a list item are their own paragraph; plain
/// text runs between blank lines (and list items, headings, fences).
fn paragraph_at(text: &str, line: Line, floor: usize) -> Para {
    let prefix = Prefix::parse(line.str(text));
    let mut first = line;
    if prefix.list.is_none() && prefix.heading.is_none() {
        let mut steps = 0;
        while let Some(prev) = first.previous(text) {
            let s = prev.str(text);
            let p = Prefix::parse(s);
            steps += 1;
            if prev.start < floor
                || steps > MAX_LINES
                || s.trim().is_empty()
                || is_marker(s)
                || p.heading.is_some()
                || fence_of(s).is_some()
                || markup::parse_align_comment(s).is_some()
            {
                break;
            }
            first = prev;
            if p.list.is_some() {
                break;
            }
        }
    }
    let prefix = Prefix::parse(first.str(text));
    let mut end = first.end;
    if prefix.heading.is_none() {
        let mut cur = first;
        for _ in 0..MAX_LINES {
            if cur.next >= text.len() {
                break;
            }
            let next = Line::at(text, cur.next);
            if breaks(next.str(text)) {
                break;
            }
            end = next.end;
            cur = next;
        }
    }
    let tags = scan_tags(text.get(first.start..end).unwrap_or(""), first.start);
    Para {
        first,
        prefix,
        tags,
    }
}

/// The paragraphs the selection touches, each once.
fn paragraphs(ctx: &Ctx<'_>) -> Vec<Para> {
    let mut out: Vec<Para> = Vec::new();
    for (line, _) in targets(ctx) {
        let para = paragraph_at(ctx.text, line, ctx.floor);
        if !out.iter().any(|p| p.first == para.first) {
            out.push(para);
        }
    }
    out
}

/// The bytes to delete to take `tag` out: the tag and the space after it, or the space before
/// it when nothing follows.
fn tag_cut(text: &str, tag: &InlineTag) -> Range<usize> {
    let rest = text.get(tag.span.end..).unwrap_or("");
    if rest.starts_with(' ') {
        tag.span.start..tag.span.end + 1
    } else if tag.span.start > 0
        && text
            .get(..tag.span.start)
            .is_some_and(|before| before.ends_with(' '))
    {
        tag.span.start - 1..tag.span.end
    } else {
        tag.span.start..tag.span.end
    }
}

/// Reps that delete `cuts`, with overlapping and touching ranges merged.
fn delete_all(mut cuts: Vec<Range<usize>>) -> Vec<Rep> {
    cuts.sort_by_key(|c| c.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for cut in cuts {
        match merged.last_mut() {
            Some(last) if cut.start <= last.end => last.end = last.end.max(cut.end),
            _ => merged.push(cut),
        }
    }
    merged.into_iter().map(Rep::delete).collect()
}

/// Where a new tag goes in `para`: after the prefix and the tags that start the text.
fn insert_rep(text: &str, para: &Para, slug: &str) -> Rep {
    let content = para.first.start + para.prefix.content;
    let mut at = content;
    for tag in &para.tags {
        if tag.span.start == at {
            at = tag.span.end;
            if text.get(at..).is_some_and(|t| t.starts_with(' ')) {
                at += 1;
            }
        } else {
            break;
        }
    }
    let after_text = at >= para.first.end && at > content;
    let spaced_before = text.get(..at).is_some_and(|b| b.ends_with(' '));
    let ins = if after_text && !spaced_before {
        format!(" #{slug}")
    } else {
        format!("#{slug} ")
    };
    Rep::insert(at, ins)
}

/// Apply the tag `slug` to the paragraphs the selection touches, or take it off when they all
/// have it. (To Do is a check box: the caller uses [`super::toggle_todo`] for it.)
pub fn toggle_tag(ctx: &Ctx<'_>, slug: &str) -> Option<FormatEdit> {
    let slug = slug.trim().to_ascii_lowercase();
    let paras = paragraphs(ctx);
    if paras.is_empty() {
        return None;
    }
    let has = |p: &Para| p.tags.iter().any(|t| t.name == slug);
    let remove = paras.iter().all(has);
    let mut reps = Vec::new();
    let mut cuts = Vec::new();
    for para in &paras {
        if remove {
            cuts.extend(
                para.tags
                    .iter()
                    .filter(|t| t.name == slug)
                    .map(|t| tag_cut(ctx.text, t)),
            );
        } else if !has(para) {
            reps.push(insert_rep(ctx.text, para, &slug));
        }
    }
    reps.extend(delete_all(cuts));
    finish(ctx, reps)
}

/// Take every tag `known` accepts, and a To Do check box, off the paragraphs the selection
/// touches (Remove Tag).
pub fn remove_tags(ctx: &Ctx<'_>, known: &dyn Fn(&str) -> bool) -> Option<FormatEdit> {
    let mut cuts = Vec::new();
    for para in paragraphs(ctx) {
        cuts.extend(
            para.tags
                .iter()
                .filter(|t| known(&t.name))
                .map(|t| tag_cut(ctx.text, t)),
        );
        if let Some(list) = &para.prefix.list {
            if list.task.is_some() {
                let base = para.first.start;
                cuts.push(base + removal_start(&para.prefix)..base + list.whole().end);
            }
        }
    }
    finish(ctx, delete_all(cuts))
}
