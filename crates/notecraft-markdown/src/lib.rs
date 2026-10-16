//! NoteCraft L3: markdown parse and write, page links, note tags, equations.
//!
//! Reading (PR 1.3): [`parse_blocks`] turns page text into [`Block`]s that keep their exact
//! source byte spans.
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
mod document;
pub mod frontmatter;
#[cfg(test)]
mod frontmatter_tests;
mod parse;
#[cfg(test)]
mod parse_tests;

pub use block::{Align, Block, BlockKind, Cell, ListItem, ListKind, Run, Span, Table};
pub use document::{parse, Document};
pub use frontmatter::{parse as parse_frontmatter, Entry, Frontmatter, Value};
pub use parse::{parse_blocks, MAX_DEPTH, MAX_PARSE_BYTES};

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 3;
