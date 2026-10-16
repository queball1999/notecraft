//! NoteCraft L3: markdown parse and write, page links, note tags, equations.
//!
//! Reading (PR 1.3): [`parse_blocks`] turns page text into [`Block`]s that keep their exact
//! source byte spans.
//!
//! Writing (PR 1.4): an [`Editor`] applies edits as span replacements on the original text, so
//! untouched bytes survive exactly.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

mod block;
mod canonical;
#[cfg(test)]
mod canonical_tests;
mod document;
mod edit;
#[cfg(test)]
mod edit_tests;
pub mod frontmatter;
#[cfg(test)]
mod frontmatter_tests;
mod parse;
#[cfg(test)]
mod parse_tests;
mod style;

pub use block::{Align, Block, BlockKind, Cell, ListItem, ListKind, Run, Span, Table};
pub use canonical::{block_to_markdown, blocks_to_markdown, page_to_markdown, runs_to_markdown};
pub use document::{parse, Document};
pub use edit::{apply_edits, Edit, EditError, Editor};
pub use frontmatter::{parse as parse_frontmatter, Entry, Frontmatter, Value};
pub use parse::{parse_blocks, MAX_DEPTH, MAX_PARSE_BYTES};
pub use style::{LineEnding, Style};

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 3;
