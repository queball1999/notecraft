//! Container runs for text editing (docs/markdown.md section 3.3). This is a view over the one
//! container model in [`crate::container`]: markers, fences, limits, and id repair all come
//! from there. A run adds what the editor needs on top: the content span with the marker line
//! split off, the blocks inside it, and the id the next save will give a bad marker.

use std::ops::Range;

use crate::container::{fix_container_ids, marker_line, read_containers};
use crate::{Block, Document, LineEnding, Span};

/// One container's run of the page text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerRun {
    /// The marker's id, or the fresh one the next save gives it when the marker's id is
    /// invalid, `c0`, or a duplicate (`id_repaired`).
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

/// The end of the line that contains `at` (just after its `\n`, or the end of the text).
fn line_end(source: &str, at: usize) -> usize {
    let tail = source.get(at..).unwrap_or("");
    tail.find('\n').map_or(source.len(), |i| at + i + 1)
}

/// Cut the body of `doc` into container runs, in page order. The first run is the implicit
/// container, kept only when it has content or the page has no markers at all. Never fails.
pub fn split_containers(source: &str, doc: &Document) -> Vec<ContainerRun> {
    let containers = read_containers(source, source.len());
    // The ids a save would write, from the same repair that save uses.
    let fixes = fix_container_ids(source, source.len());
    let mut runs = Vec::with_capacity(containers.len());
    for c in &containers {
        let (marker, content_start) = match c.marker {
            Some(m) => {
                let end = line_end(source, m.start).min(c.span.end);
                (Some(Span::new(m.start, end)), end)
            }
            None => (None, c.span.start),
        };
        let content = Span::new(content_start, c.span.end.max(content_start));
        let fix = c
            .id_span
            .and_then(|span| fixes.iter().find(|e| e.span == span));
        let (id, id_repaired) = match fix {
            Some(e) => (e.text.clone(), true),
            None => (c.id.clone(), false),
        };
        let first = doc
            .blocks
            .iter()
            .position(|b| b.span.start >= content.start)
            .unwrap_or(doc.blocks.len());
        let last = doc
            .blocks
            .iter()
            .position(|b| b.span.start >= content.end)
            .unwrap_or(doc.blocks.len());
        runs.push(ContainerRun {
            id,
            id_repaired,
            marker,
            content,
            blocks: first..last.max(first),
        });
    }
    runs
}

/// The container id to give a new container on a page with these runs.
pub fn next_container_id(runs: &[ContainerRun]) -> String {
    let highest = runs
        .iter()
        .filter_map(|r| r.id.strip_prefix('c')?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    format!("c{}", highest.saturating_add(1))
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
    out.push_str(&marker_line(id));
    out.push_str(eol.as_str());
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
        let text = "<!-- notecraft:container id=c1 -->\na\n\n<!-- notecraft:container id=c1 -->\nb\n\n<!-- notecraft:container id=we!rd -->\nc\n\n<!-- notecraft:container id=c0 -->\nd\n";
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
        assert_eq!(next_container_id(&r), "c8");
    }

    #[test]
    fn hostile_markers_do_not_panic() {
        let text = "<!-- notecraft:container id=\u{e9}\u{e9} -->".repeat(50);
        let _ = runs(&text);
        let _ = runs("<!--");
        let _ = runs("<!-- notecraft:container id= -->\n");
    }
}
