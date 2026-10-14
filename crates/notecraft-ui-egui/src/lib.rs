//! NoteCraft L6: the OneNote-style egui front end.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

mod app_shell;

pub use app_shell::{AppShell, REGIONS};

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 6;
