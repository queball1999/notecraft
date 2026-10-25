//! Page links: `[[Page]]`, `[[Page|shown text]]`, and `[[Section/Page]]` (docs/markdown.md
//! section 5.1).
//!
//! Two readers share one rule set. [`find_page_links`] scans page source text and returns each
//! link with its exact source span (for the link index and for rewriting links on rename).
//! [`split_runs`] turns the page links inside already-parsed inline runs into link runs, so the
//! canvas can draw them. Neither resolves a target; resolution is a later step.

use crate::parse::parse_blocks_plain;
use crate::{Block, BlockKind, Run, Span};

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
    /// The pipe that separates target and display (`\|` in a table cell), if there is one.
    pub sep: Option<Span>,
}

impl PageLink {
    /// The text a reader sees: the display text, or else the target.
    pub fn shown(&self) -> &str {
        self.display.as_deref().unwrap_or(&self.target)
    }
}

/// The parts of the text between `[[` and `]]`.
struct Inner {
    target: String,
    display: Option<String>,
    /// Byte range of the separator within the inner text.
    sep: Option<(usize, usize)>,
}

/// Split the text between `[[` and `]]` into a target and optional display text. In a table
/// cell the separator is `\|` (GFM requires the escape). Everywhere else it is a plain `|`,
/// and a `\|` is a literal pipe that stays in the text (docs/markdown.md section 5.1). `None`
/// when the text cannot be a link: empty target, a bracket or line break inside, or longer
/// than [`MAX_LINK_BYTES`].
fn split_inner(inner: &str, in_table: bool) -> Option<Inner> {
    if inner.len() > MAX_LINK_BYTES || inner.contains(['[', ']', '\n', '\r']) {
        return None;
    }
    let bytes = inner.as_bytes();
    let mut sep = None;
    for (at, &b) in bytes.iter().enumerate() {
        if b != b'|' {
            continue;
        }
        let escaped = at > 0 && bytes.get(at - 1) == Some(&b'\\');
        match (escaped, in_table) {
            (true, true) => sep = Some((at - 1, at + 1)),
            (false, _) => sep = Some((at, at + 1)),
            (true, false) => continue,
        }
        break;
    }
    let unescape = |text: &str| {
        let text = if in_table {
            text.to_owned()
        } else {
            text.replace("\\|", "|")
        };
        text.trim().to_owned()
    };
    let (head, tail) = match sep {
        Some((from, to)) => (inner.get(..from)?, inner.get(to..)),
        None => (inner, None),
    };
    let target = unescape(head);
    if target.is_empty() {
        return None;
    }
    let display = tail.map(unescape).filter(|d| !d.is_empty());
    Some(Inner {
        target,
        display,
        sep,
    })
}

/// Every page link in `source`, in order, with source spans. Links inside code spans, code
/// blocks, and raw HTML are skipped, as are links with an escaped opening bracket.
/// `source` is the page body (after the frontmatter); spans are offsets into it plus `base`.
pub fn find_page_links(source: &str, base: usize) -> Vec<PageLink> {
    let mut skip = Vec::new();
    let mut tables = Vec::new();
    skipped_blocks(&parse_blocks_plain(source, 0), &mut skip, &mut tables);
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
            Some(b'[') if bytes.get(i + 1) == Some(&b'[') => {
                let in_table = tables.iter().any(|t| t.start <= i && i < t.end);
                match link_at(source, i, in_table) {
                    Some((link, end)) => {
                        out.push(PageLink {
                            span: Span::new(base + i, base + end),
                            sep: link.sep.map(|s| Span::new(base + s.start, base + s.end)),
                            ..link
                        });
                        i = end;
                    }
                    None => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// The link that starts at `at` (which holds `[[`), and the offset just past it. Spans in the
/// result are offsets into `source`.
fn link_at(source: &str, at: usize, in_table: bool) -> Option<(PageLink, usize)> {
    let rest = source.get(at + 2..)?;
    // Look a little past the cap so an over-long link is rejected, not truncated.
    let mut window = rest.len().min(MAX_LINK_BYTES + 2);
    while !rest.is_char_boundary(window) {
        window -= 1;
    }
    let close = rest.get(..window)?.find("]]")?;
    let inner = split_inner(rest.get(..close)?, in_table)?;
    let end = at + 2 + close + 2;
    let link = PageLink {
        span: Span::new(at, end),
        target: inner.target,
        display: inner.display,
        sep: inner
            .sep
            .map(|(from, to)| Span::new(at + 2 + from, at + 2 + to)),
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

/// Spans of code blocks and raw HTML (`skip`) and of tables (`tables`) in `blocks`, found
/// anywhere in the tree.
fn skipped_blocks(blocks: &[Block], skip: &mut Vec<Span>, tables: &mut Vec<Span>) {
    for block in blocks {
        match &block.kind {
            BlockKind::Code { .. } | BlockKind::Html { .. } => skip.push(block.span),
            BlockKind::Table(_) => tables.push(block.span),
            BlockKind::Quote { blocks } => skipped_blocks(blocks, skip, tables),
            BlockKind::List { items, .. } => {
                for item in items {
                    skipped_blocks(&item.blocks, skip, tables);
                }
            }
            _ => {}
        }
    }
}

// Marking. The inline text of a page reaches the run splitter with its backslash escapes
// already removed, so `\[\[x\]\]` (plain text) and `[[x]]` (a link) would look the same
// there. To keep them apart, the source is copied before it is parsed and each real link's
// brackets and separator are swapped for characters that cannot occur in a page. Each
// stand-in has the same byte length as what it replaces, so every span stays valid.
const OPEN: char = '\u{90}';
const CLOSE: char = '\u{91}';
/// Stands for `\|` (two bytes).
const SEP_ESCAPED: char = '\u{92}';
/// Stands for `|` (one byte).
const SEP_PLAIN: char = '\u{1f}';
const STAND_INS: [char; 4] = [OPEN, CLOSE, SEP_ESCAPED, SEP_PLAIN];

/// A copy of `source` with its page links marked, or `None` when there is nothing to mark
/// (no links, the page is too big to parse, or it already holds a stand-in character).
pub(crate) fn mark(source: &str) -> Option<String> {
    if !source.contains("[[") || source.len() > crate::MAX_PARSE_BYTES || source.contains(STAND_INS)
    {
        return None;
    }
    let links = find_page_links(source, 0);
    if links.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for link in &links {
        out.push_str(source.get(at..link.span.start)?);
        out.push(OPEN);
        let inner_start = link.span.start + 2;
        let inner_end = link.span.end.checked_sub(2)?;
        match link.sep {
            Some(sep) => {
                out.push_str(source.get(inner_start..sep.start)?);
                out.push(if sep.len() == 1 {
                    SEP_PLAIN
                } else {
                    SEP_ESCAPED
                });
                out.push_str(source.get(sep.end..inner_end)?);
            }
            None => out.push_str(source.get(inner_start..inner_end)?),
        }
        out.push(CLOSE);
        at = link.span.end;
    }
    out.push_str(source.get(at..)?);
    Some(out)
}

/// Put the stand-in characters back as plain text (for code, images, URLs, and links that
/// did not survive inline parsing whole).
pub(crate) fn restore(text: &str) -> String {
    if !text.contains(STAND_INS) {
        return text.to_owned();
    }
    text.chars()
        .map(|c| match c {
            OPEN | CLOSE => "[[".to_owned(),
            SEP_ESCAPED | SEP_PLAIN => "|".to_owned(),
            other => other.to_string(),
        })
        .collect()
}

/// Replace the marked page links inside plain-text runs with link runs, and restore the
/// stand-ins everywhere else. A link inside code, an image, or an ordinary link is not a page
/// link (a link cannot hold another link).
pub fn split_runs(runs: Vec<Run>) -> Vec<Run> {
    let mut out = Vec::with_capacity(runs.len());
    for mut run in runs {
        if !run.text.contains(STAND_INS) {
            out.push(run);
        } else if run.code || run.image || run.link.is_some() {
            run.text = restore(&run.text);
            out.push(run);
        } else {
            split_run(&run, &mut out);
        }
    }
    out
}

fn split_run(run: &Run, out: &mut Vec<Run>) {
    let text = run.text.as_str();
    let plain = |text: &str| Run {
        text: restore(text),
        ..run.clone()
    };
    let mut plain_from = 0;
    let mut i = 0;
    while let Some(found) = text.get(i..).and_then(|t| t.find(OPEN)) {
        let at = i + found;
        let inner_from = at + OPEN.len_utf8();
        let close = text.get(inner_from..).and_then(|t| t.find(CLOSE));
        let inner = close.and_then(|c| text.get(inner_from..inner_from + c));
        let parsed = inner.and_then(|inner| {
            if inner.contains(OPEN) {
                return None;
            }
            let (head, tail) = match inner.find([SEP_PLAIN, SEP_ESCAPED]) {
                Some(s) => {
                    let width = inner.get(s..)?.chars().next()?.len_utf8();
                    (inner.get(..s)?, inner.get(s + width..))
                }
                None => (inner, None),
            };
            let head = head.trim();
            let tail = tail.map(str::trim);
            let shown = tail.filter(|t| !t.is_empty()).unwrap_or(head);
            (!head.is_empty()).then(|| (head.to_owned(), shown.to_owned()))
        });
        match (parsed, close) {
            (Some((target, shown)), Some(c)) => {
                if at > plain_from {
                    out.push(plain(text.get(plain_from..at).unwrap_or("")));
                }
                out.push(Run {
                    text: restore(&shown),
                    page_link: Some(restore(&target)),
                    ..run.clone()
                });
                let end = inner_from + c + CLOSE.len_utf8();
                plain_from = end;
                i = end;
            }
            _ => i = inner_from,
        }
    }
    if plain_from < text.len() {
        out.push(plain(text.get(plain_from..).unwrap_or("")));
    }
}
