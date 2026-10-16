//! Note container runs (docs/markdown.md section 3.3): a page body is cut into runs by marker
//! comments of the form `<!-- notecraft:container id=c1 -->`. Text before the first marker is
//! the implicit container `c0`.
//!
//! This is the read side that text editing (PR 1.5) needs. PR 1.9 adds the canvas block and
//! the save-time id repair on top of it.

use std::collections::BTreeSet;
use std::ops::Range;

use crate::{Block, BlockKind, Document, LineEnding, Span};

/// Id of the implicit container that holds the text before the first marker.
pub const IMPLICIT_ID: &str = "c0";
/// Most container runs read from one page; later markers are treated as ordinary comments.
pub const MAX_CONTAINERS: usize = 4096;

/// One container's run of the page text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerRun {
    /// The marker's id, or a fresh one when the marker's id was missing, invalid, or a
    /// duplicate (`id_repaired`).
    pub id: String,
    /// True when `id` is not what the marker says; the file is fixed on the next save.
    pub id_repaired: bool,
    /// The marker line including its line ending; `None` for the implicit container.
    pub marker: Option<Span>,
    /// The text after the marker line, up to the next marker line or the end of the page.
    pub content: Span,
    /// Which of `Document::blocks` lie inside `content`.
    pub blocks: Range<usize>,
}

impl ContainerRun {
    /// True when the run has no text beyond whitespace.
    pub fn is_blank(&self, source: &str) -> bool {
        self.content.slice(source).trim().is_empty()
    }
}

/// The id a marker block names, if `text` is a container marker comment.
fn marker_id(text: &str) -> Option<Option<&str>> {
    let inner = text
        .trim()
        .strip_prefix("<!--")?
        .strip_suffix("-->")?
        .trim();
    let rest = inner.strip_prefix("notecraft:container")?;
    // A marker with no usable id is still a marker.
    let id = rest
        .split_whitespace()
        .find_map(|part| part.strip_prefix("id="));
    Some(id)
}

/// The end of the line that contains `at` (just after its `\n`, or the end of the text).
fn line_end(source: &str, at: usize) -> usize {
    let tail = source.get(at..).unwrap_or("");
    tail.find('\n').map_or(source.len(), |i| at + i + 1)
}

fn is_valid_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The smallest `cN` (N at least 1) not in `used`.
fn fresh_id(used: &BTreeSet<String>) -> String {
    (1_u32..)
        .map(|n| format!("c{n}"))
        .find(|id| !used.contains(id))
        .unwrap_or_else(|| "c-new".to_owned())
}

/// Cut the body of `doc` into container runs, in page order. The first run is the implicit
/// container, kept only when it has content or the page has no markers at all. Never fails.
pub fn split_containers(source: &str, doc: &Document) -> Vec<ContainerRun> {
    // Markers: (block index, id as written).
    let mut markers: Vec<(usize, Option<String>)> = Vec::new();
    for (index, block) in doc.blocks.iter().enumerate() {
        if markers.len() >= MAX_CONTAINERS {
            break;
        }
        if !matches!(block.kind, BlockKind::Html { .. }) {
            continue;
        }
        if let Some(id) = marker_id(block.source(source)) {
            markers.push((index, id.map(str::to_owned)));
        }
    }

    // Ids that stay as written: valid, first use, not the implicit id.
    let mut used: BTreeSet<String> = BTreeSet::new();
    used.insert(IMPLICIT_ID.to_owned());
    let mut resolved: Vec<(String, bool)> = Vec::new();
    for (_, id) in &markers {
        match id {
            Some(id) if is_valid_id(id) && !used.contains(id) => {
                used.insert(id.clone());
                resolved.push((id.clone(), false));
            }
            _ => resolved.push((String::new(), true)),
        }
    }
    for slot in &mut resolved {
        if slot.1 {
            let id = fresh_id(&used);
            used.insert(id.clone());
            slot.0 = id;
        }
    }

    let body_end = source.len();
    let first_marker_start = markers
        .first()
        .and_then(|(i, _)| doc.blocks.get(*i))
        .map_or(body_end, |b| line_start(source, b.span.start));
    let mut runs = Vec::new();

    let first_end = markers.first().map_or(doc.blocks.len(), |(i, _)| *i);
    let implicit = ContainerRun {
        id: IMPLICIT_ID.to_owned(),
        id_repaired: false,
        marker: None,
        content: Span::new(doc.body_start.min(first_marker_start), first_marker_start),
        blocks: 0..first_end,
    };
    if markers.is_empty() || !implicit.is_blank(source) {
        runs.push(implicit);
    }

    for (n, (index, _)) in markers.iter().enumerate() {
        let Some(block) = doc.blocks.get(*index) else {
            continue;
        };
        let start = line_start(source, block.span.start);
        let marker_end = line_end(source, block.span.end.saturating_sub(1).max(start));
        let next = markers.get(n + 1);
        let (end, block_end) = match next.and_then(|(i, _)| doc.blocks.get(*i).map(|b| (*i, b))) {
            Some((i, b)) => (line_start(source, b.span.start), i),
            None => (body_end, doc.blocks.len()),
        };
        let (id, id_repaired) = resolved.get(n).cloned().unwrap_or_default();
        runs.push(ContainerRun {
            id,
            id_repaired,
            marker: Some(Span::new(start, marker_end.min(end))),
            content: Span::new(marker_end.min(end), end),
            blocks: index + 1..block_end,
        });
    }
    runs
}

/// The start of the line that contains `at`.
fn line_start(source: &str, at: usize) -> usize {
    let head = source.get(..at.min(source.len())).unwrap_or("");
    head.rfind('\n').map_or(0, |i| i + 1)
}

/// The container id to give a new container on a page with these runs.
pub fn next_container_id(runs: &[ContainerRun]) -> String {
    let used: BTreeSet<String> = runs.iter().map(|r| r.id.clone()).collect();
    fresh_id(&used)
}

/// The marker line for container `id`, ending in `eol`.
pub fn marker_line(id: &str, eol: LineEnding) -> String {
    format!("<!-- notecraft:container id={id} -->{}", eol.as_str())
}

/// Text to append to the page `source` to start the new container `id`: a blank line when the
/// page has text, then the marker line. The text goes at `source.len()`, and the caret for the
/// new container belongs at the end of the page afterwards.
pub fn new_container_text(source: &str, id: &str) -> String {
    let eol = LineEnding::detect(source);
    let mut out = String::new();
    if !source.is_empty() {
        if !source.ends_with('\n') {
            out.push_str(eol.as_str());
        }
        out.push_str(eol.as_str());
    }
    out.push_str(&marker_line(id, eol));
    out
}

/// The blocks of `doc` that belong to `run`.
pub fn run_blocks<'a>(doc: &'a Document, run: &ContainerRun) -> &'a [Block] {
    doc.blocks.get(run.blocks.clone()).unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn runs(text: &str) -> Vec<ContainerRun> {
        split_containers(text, &parse(text))
    }

    #[test]
    fn a_page_without_markers_is_one_implicit_container() {
        let text = "# Title\n\nBody\n";
        let r = runs(text);
        assert_eq!(r.len(), 1);
        let run = r.first().expect("run");
        assert_eq!(run.id, "c0");
        assert_eq!(run.content, Span::new(0, text.len()));
        assert_eq!(run.blocks, 0..2);
        assert!(runs("").first().is_some_and(|r| r.id == "c0"));
    }

    #[test]
    fn markers_cut_the_page_into_runs() {
        let text = "intro\n\n<!-- notecraft:container id=c1 -->\n## Agenda\n\n- a\n\n<!-- notecraft:container id=c2 -->\nLast\n";
        let r = runs(text);
        let ids: Vec<&str> = r.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["c0", "c1", "c2"]);
        let c1 = r.get(1).expect("c1");
        assert!(c1.content.slice(text).starts_with("## Agenda"));
        assert!(c1.marker.expect("marker").slice(text).ends_with("-->\n"));
        assert_eq!(
            c1.content.end,
            r.get(2).expect("c2").marker.expect("m").start
        );
        assert_eq!(r.get(2).expect("c2").content.slice(text), "Last\n");
        // Runs tile the body: every byte is in exactly one marker or content span.
        let mut covered = 0;
        for run in &r {
            covered += run.content.len() + run.marker.map_or(0, |m| m.len());
        }
        assert_eq!(covered, text.len());
    }

    #[test]
    fn an_empty_implicit_run_is_dropped_when_markers_exist() {
        let r = runs("<!-- notecraft:container id=c1 -->\nHi\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r.first().expect("run").id, "c1");
    }

    #[test]
    fn bad_and_duplicate_ids_get_fresh_ones() {
        let text = "<!-- notecraft:container id=c1 -->\na\n\n<!-- notecraft:container id=c1 -->\nb\n\n<!-- notecraft:container -->\nc\n\n<!-- notecraft:container id=c0 -->\nd\n";
        let r = runs(text);
        let ids: Vec<(&str, bool)> = r.iter().map(|r| (r.id.as_str(), r.id_repaired)).collect();
        assert_eq!(
            ids,
            [("c1", false), ("c2", true), ("c3", true), ("c4", true)]
        );
    }

    #[test]
    fn frontmatter_is_not_part_of_the_first_container() {
        let text = "---\ntitle: T\n---\nHello\n";
        let r = runs(text);
        assert_eq!(r.first().expect("run").content.slice(text), "Hello\n");
    }

    #[test]
    fn crlf_markers_are_read() {
        let text = "a\r\n\r\n<!-- notecraft:container id=c1 -->\r\nb\r\n";
        let r = runs(text);
        assert_eq!(r.get(1).expect("c1").content.slice(text), "b\r\n");
    }

    #[test]
    fn other_comments_are_ordinary_html() {
        assert_eq!(runs("<!-- note -->\ntext\n").len(), 1);
    }

    #[test]
    fn new_container_text_follows_the_page_style() {
        assert_eq!(
            new_container_text("", "c1"),
            "<!-- notecraft:container id=c1 -->\n"
        );
        assert_eq!(
            new_container_text("a\n", "c1"),
            "\n<!-- notecraft:container id=c1 -->\n"
        );
        assert_eq!(
            new_container_text("a", "c2"),
            "\n\n<!-- notecraft:container id=c2 -->\n"
        );
        assert_eq!(
            new_container_text("a\r\n", "c1"),
            "\r\n<!-- notecraft:container id=c1 -->\r\n"
        );
        // The result reads back as a container.
        let text = format!("a\n{}", new_container_text("a\n", "c7"));
        let r = runs(&text);
        assert_eq!(r.last().expect("run").id, "c7");
        assert_eq!(next_container_id(&r), "c1");
    }

    #[test]
    fn hostile_markers_do_not_panic() {
        let text = "<!-- notecraft:container id=\u{e9}\u{e9} -->".repeat(50);
        let _ = runs(&text);
        let _ = runs("<!--");
        let _ = runs("<!-- notecraft:container id= -->\n");
    }
}
