//! OneNote 2016 preset (docs/themes.md section 4). Flat purple ribbon with white text,
//! light gray navigation, square corners, no shadows.
//!
//! Original values picked by eye to suggest the era; never sampled from Microsoft software
//! (docs/themes.md section 1).

use super::tokens::{Palette, Shape};
use super::{ButtonStyle, ShadowStyle};

pub(super) const LIGHT: Palette = Palette {
    accent: 0x7B3FA6,
    accent_text: 0xFFFFFF,
    ribbon_bg: 0xF4F1F7,
    ribbon_tab_bg: 0x7B3FA6,
    ribbon_text: 0x2A2A2A,
    sidebar_bg: 0xECEAEE,
    sidebar_text: 0x2A2A2A,
    sidebar_selected: 0xD9CCE6,
    canvas_bg: 0xFFFFFF,
    canvas_rule: 0xC9DDF0,
    canvas_margin_rule: 0xE8A0A0,
    text: 0x1F1F1F,
    border: 0xD2CDD8,
};

pub(super) const DARK: Palette = Palette {
    accent: 0x915EB8,
    accent_text: 0xFFFFFF,
    ribbon_bg: 0x2B2730,
    ribbon_tab_bg: 0x3A2F45,
    ribbon_text: 0xEDEAF0,
    sidebar_bg: 0x252228,
    sidebar_text: 0xE4E1E8,
    sidebar_selected: 0x4A3D57,
    canvas_bg: 0x1E1E1E,
    canvas_rule: 0x34404C,
    canvas_margin_rule: 0x7A3B3B,
    text: 0xEDEDED,
    border: 0x403A47,
};

pub(super) const SHAPE: Shape = Shape {
    radius: 0,
    spacing: 4.0,
    shadow: ShadowStyle::None,
    button_style: ButtonStyle::Flat,
};
