//! Page links: `[[Page]]`, `[[Page|shown text]]`, and `[[Section/Page]]` (docs/markdown.md
//! section 5.1).
//!
//! Two readers share one rule set. [`find_page_links`] scans page source text and returns each
//! link with its exact source span (for the link index and for rewriting links on rename).
//! [`split_runs`] turns the page links inside already-parsed inline runs into link runs, so the
//! canvas can draw them. Neither resolves a target; resolution is a later step.

use crate::{parse_blocks, Block, BlockKind, Run, Span};

/// Longest text between `[[` and `]]`, in bytes. A longer candidate is plain text.
pub const MAX_LINK_BYTES: usize = 1024;

/// One page link found in source text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageLink {
    /// The whole `[[...]]`, brackets included.
    pub span: Span,
    /// The page the link points to: a name or a `Section/Page` path, trimmed.
    pub target: String,
    /// The shown text after the pipe, if the link has one.
    pub display: Option<String>,
}

impl PageLink {
    /// The text a reader sees: the display text, or else the target.
    pub fn shown(&self) -> &str {
        self.display.as_deref().unwrap_or(&self.target)
    }
}

/// Split the text between `[[` and `]]` into a target and optional display text. The first
/// `|` (or the table form `\|`) separates them. `None` when the text cannot be a link: empty
/// target, a bracket or line break inside, or longer than [`MAX_LINK_BYTES`].
pub fn parse_inner(inner: &str) -> Option<(String, Option<String>)> {
    if inner.len() > MAX_LINK_BYTES || inner.contains(['[', ']', '\n', '\r']) {
        return None;
    }
    let (target, display) = match inner.find('|') {
        Some(at) => {
            let head = inner.get(..at)?;
            let head = head.strip_suffix('\\').unwrap_or(head);
            (head, inner.get(at + 1..))
        }
        None => (inner, None),
    };
    let target = target.trim();
    if target.is_empty() {
        return None;
    }
    let display = display.map(str::trim).filter(|d| !d.is_empty());
    Some((target.to_owned(), display.map(str::to_owned)))
}

/// Every page link in `source`, in order, with source spans. Links inside code spans, code
/// blocks, and raw HTML are skipped, as are links with an escaped opening bracket.
/// `source` is the page body (after the frontmatter); spans are offsets into it plus `base`.
pub fn find_page_links(source: &str, base: usize) -> Vec<PageLink> {
    let mut skip = Vec::new();
    skipped_blocks(&parse_blocks(source, 0), &mut skip);
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if let Some(end) = skip
            .iter()
            .find(|s| s.start <= i && i < s.end)
            .map(|s| s.end)
        {
            i = end.max(i + 1);
            continue;
        }
        match bytes.get(i) {
            Some(b'\\') => i += 2,
            Some(b'`') => i = skip_code_span(bytes, i),
            Some(b'[') if bytes.get(i + 1) == Some(&b'[') => match link_at(source, i) {
                Some((link, end)) => {
                    out.push(PageLink {
                        span: Span::new(base + i, base + end),
                        ..link
                    });
                    i = end;
                }
                None => i += 1,
            },
            _ => i += 1,
        }
    }
    out
}

/// The link that starts at `at` (which holds `[[`), and the offset just past it.
fn link_at(source: &str, at: usize) -> Option<(PageLink, usize)> {
    let rest = source.get(at + 2..)?;
    // Look a little past the cap so an over-long link is rejected, not truncated.
    let mut window = rest.len().min(MAX_LINK_BYTES + 2);
    while !rest.is_char_boundary(window) {
        window -= 1;
    }
    let close = rest.get(..window)?.find("]]")?;
    let (target, display) = parse_inner(rest.get(..close)?)?;
    let end = at + 2 + close + 2;
    let link = PageLink {
        span: Span::new(at, end),
        target,
        display,
    };
    Some((link, end))
}

/// The offset just past the code span that opens at `at` (a backtick run), or past the run
/// when it never closes.
fn skip_code_span(bytes: &[u8], at: usize) -> usize {
    let ticks = bytes
        .get(at..)
        .map_or(0, |b| b.iter().take_while(|&&c| c == b'`').count());
    let mut i = at + ticks;
    while i < bytes.len() {
        if bytes.get(i) == Some(&b'`') {
            let run = bytes
                .get(i..)
                .map_or(1, |b| b.iter().take_while(|&&c| c == b'`').count());
            if run == ticks {
                return i + run;
            }
            i += run;
        } else {
            i += 1;
        }
    }
    at + ticks
}

/// Spans of code blocks and raw HTML in `blocks`, found anywhere in the tree.
fn skipped_blocks(blocks: &[Block], out: &mut Vec<Span>) {
    for block in blocks {
        match &block.kind {
            BlockKind::Code { .. } | BlockKind::Html { .. } => out.push(block.span),
            BlockKind::Quote { blocks } => skipped_blocks(blocks, out),
            BlockKind::List { items, .. } => {
                for item in items {
                    skipped_blocks(&item.blocks, out);
                }
            }
            _ => {}
        }
    }
}

/// Replace the page links inside plain-text runs with link runs. Runs that are code, images,
/// or ordinary links are left alone (a link cannot hold another link).
pub fn split_runs(runs: Vec<Run>) -> Vec<Run> {
    let mut out = Vec::with_capacity(runs.len());
    for run in runs {
        if run.code || run.image || run.link.is_some() || !run.text.contains("[[") {
            out.push(run);
            continue;
        }
        split_run(&run, &mut out);
    }
    out
}

fn split_run(run: &Run, out: &mut Vec<Run>) {
    let text = run.text.as_str();
    let piece = |text: &str, page_link: Option<String>| Run {
        text: text.to_owned(),
        page_link,
        ..run.clone()
    };
    let mut plain_from = 0;
    let mut i = 0;
    while let Some(found) = text.get(i..).and_then(|t| t.find("[[")) {
        let at = i + found;
        match link_at(text, at) {
            Some((link, end)) => {
                if at > plain_from {
                    out.push(piece(text.get(plain_from..at).unwrap_or(""), None));
                }
                out.push(piece(link.shown(), Some(link.target.clone())));
                plain_from = end;
                i = end;
            }
            None => i = at + 1,
        }
    }
    if plain_from < text.len() {
        out.push(piece(text.get(plain_from..).unwrap_or(""), None));
    }
}
