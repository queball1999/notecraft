//! NoteCraft L4: ink engine, shapes, layout, hit testing, zoom and pan.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

pub mod paper;
pub mod viewport;

pub use paper::{PaperSize, RuleKind, RuleLines};
pub use viewport::Viewport;

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 4;
