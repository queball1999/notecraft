//! NoteCraft L2: notebook, section, page, canvas objects, links.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

pub mod container;
pub mod notebook;
pub mod page;
pub mod section;

pub use container::Container;
pub use notebook::Notebook;
pub use page::PageRef;
pub use section::{Section, SectionGroup, SectionNode};

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 2;
