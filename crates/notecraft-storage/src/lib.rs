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
