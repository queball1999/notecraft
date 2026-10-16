//! Note container markers in a `.notecraft` page (docs/markdown.md section 3.3).
//!
//! A container is a run of markdown that starts with an HTML comment line:
//! `<!-- notecraft:container id=c1 -->`. Text before the first marker belongs to the implicit
//! container `c0`. Markers inside fenced code do not count. A marker whose id is not a valid
//! id, is `c0`, or repeats an earlier id gets a fresh id the next time the page is saved
//! ([`fix_container_ids`]), as a single span replacement of just the id.
//!
//! Reading never fails and is bounded: at most [`MAX_CONTAINERS`] markers are read.

use crate::block::Span;
use crate::edit::Edit;
use crate::frontmatter;

/// Id of the implicit container that holds text before the first marker.
pub const IMPLICIT_ID: &str = "c0";
/// Most containers read from one page; markers past this are left as plain comments.
pub const MAX_CONTAINERS: usize = 10_000;
/// Longest container id.
pub const MAX_ID_LEN: usize = 64;

/// One note container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Container {
    /// The id as written in the marker, or `c0` for the implicit container.
    pub id: String,
    /// The marker line (without its line ending); `None` for the implicit container.
    pub marker: Option<Span>,
    /// Where the id sits inside the marker (without quotes), for rewriting it.
    pub id_span: Option<Span>,
    /// The marker and everything up to the next marker (or the limit).
    pub span: Span,
}

impl Container {
    /// The markdown of the container without its marker line.
    pub fn content<'a>(&self, text: &'a str) -> &'a str {
        let start = self.marker.map_or(self.span.start, |m| m.end);
        let rest = text.get(start..self.span.end).unwrap_or("");
        rest.strip_prefix("\r\n")
            .or_else(|| rest.strip_prefix('\n'))
            .unwrap_or(rest)
    }
}

/// True if `id` can be used in a marker: 1 to 64 of `A-Z a-z 0-9 _ -`.
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// The marker line for `id` (no line ending).
pub fn marker_line(id: &str) -> String {
    format!("<!-- notecraft:container id={id} -->")
}

/// The fence a line opens or closes: its character and length, if the line is one.
fn fence_of(line: &str) -> Option<(u8, usize)> {
    let indent = line.bytes().take_while(|b| *b == b' ').count();
    if indent > 3 {
        return None;
    }
    let rest = line.get(indent..)?;
    let ch = *rest.as_bytes().first()?;
    if ch != b'`' && ch != b'~' {
        return None;
    }
    let run = rest.bytes().take_while(|b| *b == ch).count();
    if run < 3 {
        return None;
    }
    // A backtick fence's info string may not hold a backtick.
    if ch == b'`' && rest.get(run..)?.contains('`') {
        return None;
    }
    Some((ch, run))
}

/// True if `line` closes a fence opened with `open`.
fn closes(open: (u8, usize), line: &str) -> bool {
    let Some((ch, len)) = fence_of(line) else {
        return false;
    };
    ch == open.0 && len >= open.1 && line.trim().bytes().all(|b| b == ch)
}

/// Parse a marker line. Returns the id's byte range inside `line`, without quotes.
fn parse_marker(line: &str) -> Option<(usize, usize)> {
    let indent = line.bytes().take_while(|b| *b == b' ').count();
    if indent > 3 {
        return None;
    }
    let inner = line
        .trim_end()
        .get(indent..)?
        .strip_prefix("<!--")?
        .strip_suffix("-->")?;
    let body = inner.trim();
    let after = body.strip_prefix("notecraft:container")?;
    if !after.is_empty() && !after.starts_with(char::is_whitespace) {
        return None;
    }
    let token = after
        .split_whitespace()
        .find(|word| word.starts_with("id="))?;
    let value = token.strip_prefix("id=")?;
    let value = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(value);
    // `value` is a subslice of `line`; its offset is the distance between the two pointers.
    let start = (value.as_ptr() as usize).checked_sub(line.as_ptr() as usize)?;
    let end = start.checked_add(value.len())?;
    line.get(start..end)?;
    Some((start, end))
}

/// Read the containers of `text` before byte `limit` (pass the start of the canvas block, or
/// the text length). The first container is the implicit `c0` when there is text before the
/// first marker, or no marker at all.
pub fn read_containers(text: &str, limit: usize) -> Vec<Container> {
    let limit = limit.min(text.len());
    let body_start = frontmatter::parse(text)
        .map_or(0, |f| f.span.end)
        .min(limit);
    let mut markers: Vec<(Span, Span, String)> = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    let mut offset = 0usize;
    for raw in text.split_inclusive('\n') {
        let start = offset;
        offset += raw.len();
        if start >= limit {
            break;
        }
        if start < body_start {
            continue;
        }
        let line = raw.trim_end_matches(['\n', '\r']);
        match fence {
            Some(open) => {
                if closes(open, line) {
                    fence = None;
                }
            }
            None => {
                if let Some(open) = fence_of(line) {
                    fence = Some(open);
                } else if markers.len() < MAX_CONTAINERS {
                    if let Some((a, b)) = parse_marker(line) {
                        let id = line.get(a..b).unwrap_or("").to_owned();
                        let marker = Span::new(start, start + line.trim_end().len());
                        markers.push((marker, Span::new(start + a, start + b), id));
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    let first = markers.first().map_or(limit, |m| m.0.start);
    let head = text.get(body_start..first).unwrap_or("");
    if markers.is_empty() || !head.trim().is_empty() {
        out.push(Container {
            id: IMPLICIT_ID.to_owned(),
            marker: None,
            id_span: None,
            span: Span::new(body_start, first),
        });
    }
    for (i, (marker, id_span, id)) in markers.iter().enumerate() {
        let end = markers.get(i + 1).map_or(limit, |m| m.0.start);
        out.push(Container {
            id: id.clone(),
            marker: Some(*marker),
            id_span: Some(*id_span),
            span: Span::new(marker.start, end),
        });
    }
    out
}

/// A container id not used in `containers`, such as `c3`.
pub fn fresh_container_id(containers: &[Container]) -> String {
    let highest = containers
        .iter()
        .filter_map(|c| c.id.strip_prefix('c')?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    format!("c{}", highest.saturating_add(1))
}

/// Edits that give every marker with an invalid, reserved (`c0`), or repeated id a fresh id.
/// Only the id bytes change. Apply with [`crate::apply_edits`].
pub fn fix_container_ids(text: &str, limit: usize) -> Vec<Edit> {
    let containers = read_containers(text, limit);
    let mut used: Vec<String> = containers.iter().map(|c| c.id.clone()).collect();
    let mut next = fresh_container_id(&containers)
        .strip_prefix('c')
        .and_then(|n| n.parse::<u64>().ok())
        .unwrap_or(1);
    let mut seen: Vec<&str> = Vec::new();
    let mut edits = Vec::new();
    for c in &containers {
        let Some(id_span) = c.id_span else {
            continue;
        };
        let bad = !is_valid_id(&c.id) || c.id == IMPLICIT_ID || seen.contains(&c.id.as_str());
        if bad {
            let fresh = loop {
                let candidate = format!("c{next}");
                next = next.saturating_add(1);
                if !used.contains(&candidate) {
                    break candidate;
                }
            };
            used.push(fresh.clone());
            edits.push(Edit::new(id_span, fresh));
        } else {
            seen.push(c.id.as_str());
        }
    }
    edits
}

#[cfg(test)]
#[path = "container_tests.rs"]
mod tests;
