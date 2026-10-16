//! NoteCraft L1: notebook folders, page files, index, auto-save, attachments.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 1;

#[cfg(not(target_arch = "wasm32"))]
pub mod index;
pub mod notebook_dir;
pub mod notebook_toml;
pub mod page_file;
pub mod paths;
pub mod section_toml;
pub mod structure;

pub use notebook_dir::{
    scan_notebook, scan_notebook_with, GroupEntry, NotebookTree, PageEntry, PageKind, ScanError,
    ScanLimits, ScanWarning, SectionEntry, TreeNode, WarningKind,
};
pub use notebook_toml::{NotebookConfig, ReadOutcome};
pub use page_file::{
    read_page_file, read_page_file_with, write_page_text, PageReadError, PageText, WriteOutcome,
    MAX_PAGE_BYTES,
};
pub use section_toml::{PageListing, SectionConfig, SectionKind};
