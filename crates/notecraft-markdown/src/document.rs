//! A whole page: frontmatter plus body blocks.

use crate::frontmatter::{self, Frontmatter};
use crate::parse::parse_blocks;
use crate::{Block, BlockKind, Run};

/// Most bytes of text `Document::plain_text` returns.
const MAX_PLAIN_TEXT: usize = 16 * 1024;

/// A parsed page. Every span in it is an offset into the page source text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Document {
    pub frontmatter: Option<Frontmatter>,
    /// Where the body (the markdown after the frontmatter) starts.
    pub body_start: usize,
    pub blocks: Vec<Block>,
}

/// Parse a page: cut off the frontmatter, then parse the rest into blocks. Never fails; bad
/// or hostile input gives a best-effort result.
pub fn parse(source: &str) -> Document {
    let frontmatter = frontmatter::parse(source);
    let body_start = frontmatter.as_ref().map_or(0, |f| f.span.end);
    let body = source.get(body_start..).unwrap_or("");
    Document {
        frontmatter,
        body_start,
        blocks: parse_blocks(body, body_start),
    }
}

impl Document {
    /// The page text without markdown syntax, one block per line, capped at 16 KiB. For screen
    /// readers and tests.
    pub fn plain_text(&self) -> String {
        self.plain_text_up_to(MAX_PLAIN_TEXT)
    }

    /// Like [`Self::plain_text`], but capped at `max` bytes (for the command line, which
    /// prints a whole page).
    pub fn plain_text_up_to(&self, max: usize) -> String {
        let mut out = String::new();
        append_blocks(&self.blocks, &mut out, max);
        if out.len() > max {
            let mut cut = max;
            while !out.is_char_boundary(cut) {
                cut -= 1;
            }
            out.truncate(cut);
        }
        out
    }
}

fn append_runs(runs: &[Run], out: &mut String) {
    for run in runs {
        out.push_str(&run.text);
    }
}

fn append_blocks(blocks: &[Block], out: &mut String, max: usize) {
    for block in blocks {
        if out.len() > max {
            return;
        }
        let before = out.len();
        match &block.kind {
            BlockKind::Heading { runs, .. } | BlockKind::Paragraph { runs } => {
                append_runs(runs, out);
            }
            BlockKind::List { items, .. } => {
                for item in items {
                    append_blocks(&item.blocks, out, max);
                }
            }
            BlockKind::Table(table) => {
                for row in std::iter::once(&table.header).chain(&table.rows) {
                    for cell in row {
                        append_runs(&cell.runs, out);
                        out.push(' ');
                    }
                    out.push('\n');
                }
            }
            BlockKind::Code { literal, .. } | BlockKind::Html { literal } => out.push_str(literal),
            BlockKind::Quote { blocks } => append_blocks(blocks, out, max),
            BlockKind::Rule => {}
        }
        if out.len() > before && !out.ends_with('\n') {
            out.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_drops_markdown_syntax() {
        let doc =
            parse("---\ntitle: T\n---\n# Head\n\nSome **bold** [link](http://x.y).\n\n- a\n- b\n");
        assert_eq!(doc.plain_text(), "Head\nSome bold link.\na\nb\n");
    }

    #[test]
    fn plain_text_up_to_takes_a_larger_cap() {
        let doc = parse(&"word ".repeat(10_000));
        assert!(doc.plain_text_up_to(1 << 20).len() > MAX_PLAIN_TEXT);
        assert!(doc.plain_text_up_to(10).len() <= 10);
    }

    #[test]
    fn plain_text_is_capped() {
        let doc = parse(&"word ".repeat(10_000));
        assert!(doc.plain_text().len() <= MAX_PLAIN_TEXT);
    }
}
