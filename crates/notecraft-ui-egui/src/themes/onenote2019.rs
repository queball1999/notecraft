//! OneNote 2019 preset (docs/themes.md section 4). Slightly deeper purple, cleaner spacing,
//! 2 px corners.
//!
//! Original values picked by eye to suggest the era; never sampled from Microsoft software
//! (docs/themes.md section 1).

use super::tokens::{Palette, Shape};
use super::{ButtonStyle, ShadowStyle};

pub(super) const LIGHT: Palette = Palette {
    accent: 0x6E3196,
    accent_text: 0xFFFFFF,
    ribbon_bg: 0xF7F5F9,
    ribbon_tab_bg: 0x6E3196,
    ribbon_text: 0x262626,
    sidebar_bg: 0xF0EEF2,
    sidebar_text: 0x262626,
    sidebar_selected: 0xDDD0EA,
    canvas_bg: 0xFFFFFF,
    canvas_rule: 0xCADCEB,
    canvas_margin_rule: 0xD46262,
    text: 0x1E1E1E,
    border: 0xD6D1DC,
};

pub(super) const DARK: Palette = Palette {
    accent: 0x915FB9,
    accent_text: 0xFFFFFF,
    ribbon_bg: 0x29252E,
    ribbon_tab_bg: 0x372C42,
    ribbon_text: 0xECE8F0,
    sidebar_bg: 0x232026,
    sidebar_text: 0xE2DEE6,
    sidebar_selected: 0x463A54,
    canvas_bg: 0x1D1D1D,
    canvas_rule: 0x333E49,
    canvas_margin_rule: 0x984549,
    text: 0xECECEC,
    border: 0x3E3845,
};

pub(super) const SHAPE: Shape = Shape {
    radius: 2,
    spacing: 5.0,
    shadow: ShadowStyle::None,
    button_style: ButtonStyle::Flat,
};
