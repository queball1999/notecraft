//! Microsoft 365 preset (docs/themes.md section 4). Fluent-style: soft surfaces, rounded
//! corners, light blur feel (simulated with layered translucent fills; egui has no backdrop
//! blur), accent on selection only.
//!
//! Original values picked by eye to suggest the era; never sampled from Microsoft software
//! (docs/themes.md section 1).

use super::tokens::{Palette, Shape};
use super::{ButtonStyle, ShadowStyle};

pub(super) const LIGHT: Palette = Palette {
    accent: 0x8250B8,
    accent_text: 0xFFFFFF,
    ribbon_bg: 0xF6F4F8,
    ribbon_tab_bg: 0xEFEBF3,
    ribbon_text: 0x242424,
    sidebar_bg: 0xF1EEF5,
    sidebar_text: 0x242424,
    sidebar_selected: 0xE3D8F0,
    canvas_bg: 0xFFFFFF,
    canvas_rule: 0xD5E2EF,
    text: 0x1C1C1C,
    border: 0xE2DDE8,
};

pub(super) const DARK: Palette = Palette {
    accent: 0xB48EDB,
    accent_text: 0x161616,
    ribbon_bg: 0x2A2830,
    ribbon_tab_bg: 0x24222A,
    ribbon_text: 0xEEEEEE,
    sidebar_bg: 0x211F26,
    sidebar_text: 0xE6E6E6,
    sidebar_selected: 0x3F3550,
    canvas_bg: 0x1A1A1C,
    canvas_rule: 0x2F3640,
    text: 0xEDEDED,
    border: 0x3B3842,
};

pub(super) const SHAPE: Shape = Shape {
    radius: 6,
    spacing: 6.0,
    shadow: ShadowStyle::Soft,
    button_style: ButtonStyle::Filled,
};
