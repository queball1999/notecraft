//! NoteCraft L0: geometry, colors, units, ink stroke types, input normalization.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

pub mod geometry;

pub use geometry::Vec2;

/// Layer number, read by `cargo xtask layers`.
pub const LAYER: u8 = 0;
