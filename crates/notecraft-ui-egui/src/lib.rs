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
pub mod canvas;
pub mod color;
pub mod icons;
pub mod nav;
pub mod ribbon;
pub mod themes;
pub mod toast;

pub use app_shell::{AppShell, REGIONS};
pub use toast::Toasts;

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 6;
