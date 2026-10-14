//! Themes: four presets, each in light and dark (docs/themes.md). Themes change colors,
//! spacing, corner radius, and control style, never layout or behavior.
//!
//! The preset and mode are view state in the engine session, so the theme commands
//! (`view.theme.set`, `view.mode.toggle`, `view.mode.set`) can change them; this module turns
//! them into colors.

mod apply;
pub mod contrast;
mod onenote2016;
mod onenote2019;
mod tokens;

pub use apply::{install, style};
pub use notecraft_engine::view::{ThemeMode as Mode, ThemePreset as Preset};
pub use tokens::{hex, mix, ButtonStyle, ShadowStyle, ThemeTokens};
