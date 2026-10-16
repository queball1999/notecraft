//! A whole page: frontmatter plus body blocks.

use crate::frontmatter::{self, Frontmatter};
use crate::parse::parse_blocks;
use crate::Block;

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
