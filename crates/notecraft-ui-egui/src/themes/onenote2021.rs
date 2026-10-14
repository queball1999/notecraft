//! OneNote 2021 preset (docs/themes.md section 4). Light ribbon with a purple accent only
//! on the selected tab, muted colors, minimal shadows.
//!
//! Original values picked by eye to suggest the era; never sampled from Microsoft software
//! (docs/themes.md section 1).

use super::tokens::{Palette, Shape};
use super::{ButtonStyle, ShadowStyle};

pub(super) const LIGHT: Palette = Palette {
    accent: 0x7A45A8,
    accent_text: 0xFFFFFF,
    ribbon_bg: 0xFAFAFA,
    ribbon_tab_bg: 0xFAFAFA,
    ribbon_text: 0x2B2B2B,
    sidebar_bg: 0xF3F2F4,
    sidebar_text: 0x2B2B2B,
    sidebar_selected: 0xE6DDF0,
    canvas_bg: 0xFFFFFF,
    canvas_rule: 0xD3E1EE,
    text: 0x202020,
    border: 0xE0DEE3,
};

pub(super) const DARK: Palette = Palette {
    accent: 0xA77FD0,
    accent_text: 0x1B1B1B,
    ribbon_bg: 0x262626,
    ribbon_tab_bg: 0x262626,
    ribbon_text: 0xE8E8E8,
    sidebar_bg: 0x202020,
    sidebar_text: 0xE0E0E0,
    sidebar_selected: 0x3D3447,
    canvas_bg: 0x1B1B1B,
    canvas_rule: 0x30383F,
    text: 0xEAEAEA,
    border: 0x3A3A3A,
};

pub(super) const SHAPE: Shape = Shape {
    radius: 3,
    spacing: 6.0,
    shadow: ShadowStyle::Subtle,
    button_style: ButtonStyle::Flat,
};
