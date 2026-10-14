//! Theme tokens (docs/themes.md section 3). Widgets read tokens only; no widget hard-codes a
//! color.

use egui::Color32;

use super::{onenote2016, onenote2019, onenote2021, Mode, Preset};

/// How strongly floating surfaces (menus, popups) cast a shadow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowStyle {
    None,
    Subtle,
    Soft,
}

/// How buttons look at rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    /// No fill until hovered.
    Flat,
    /// A light fill and a border.
    Raised,
    /// A light fill, no border.
    Filled,
}

/// Every value a theme variant (one preset in one mode) defines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeTokens {
    pub dark: bool,
    /// Ribbon background (2016, 2019), selected tab underline, focus rings.
    pub accent: Color32,
    /// Text on `accent`.
    pub accent_text: Color32,
    pub ribbon_bg: Color32,
    /// The tab strip above the ribbon body.
    pub ribbon_tab_bg: Color32,
    pub ribbon_text: Color32,
    pub sidebar_bg: Color32,
    pub sidebar_text: Color32,
    pub sidebar_selected: Color32,
    /// Page background when the page has no color.
    pub canvas_bg: Color32,
    /// Rule lines and grid.
    pub canvas_rule: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    /// Separators and control borders.
    pub border: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    /// The "Coming Soon" badge.
    pub badge_bg: Color32,
    pub badge_text: Color32,
    /// Corner radius in points.
    pub radius: u8,
    /// Base spacing unit in points.
    pub spacing: f32,
    pub shadow: ShadowStyle,
    pub button_style: ButtonStyle,
}

/// `#RRGGBB` as a color.
pub const fn hex(rgb: u32) -> Color32 {
    let [_, r, g, b] = rgb.to_be_bytes();
    Color32::from_rgb(r, g, b)
}

/// Mix `a` toward `b`: `t` = 0 gives `a`, 1 gives `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let channel = |x: u8, y: u8| {
        let v = f32::from(x) + (f32::from(y) - f32::from(x)) * t;
        // In 0..=255 by construction; `as` saturates anyway.
        v.round() as u8
    };
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

/// The colors a preset picks; the remaining tokens are derived from them.
#[derive(Clone, Copy, Debug)]
pub(super) struct Palette {
    pub accent: u32,
    pub accent_text: u32,
    pub ribbon_bg: u32,
    pub ribbon_tab_bg: u32,
    pub ribbon_text: u32,
    pub sidebar_bg: u32,
    pub sidebar_text: u32,
    pub sidebar_selected: u32,
    pub canvas_bg: u32,
    pub canvas_rule: u32,
    pub text: u32,
    pub border: u32,
}

/// Shape tokens shared by a preset's light and dark variants.
#[derive(Clone, Copy, Debug)]
pub(super) struct Shape {
    pub radius: u8,
    pub spacing: f32,
    pub shadow: ShadowStyle,
    pub button_style: ButtonStyle,
}

impl ThemeTokens {
    /// The tokens for `preset` in light (`dark` false) or dark mode.
    ///
    /// Presets without their own module yet (m365) use the Microsoft 365 values.
    pub fn for_variant(preset: Preset, dark: bool) -> Self {
        let (light_palette, dark_palette, shape) = match preset {
            Preset::OneNote2016 => (onenote2016::LIGHT, onenote2016::DARK, onenote2016::SHAPE),
            Preset::OneNote2019 => (onenote2019::LIGHT, onenote2019::DARK, onenote2019::SHAPE),
            Preset::OneNote2021 => (onenote2021::LIGHT, onenote2021::DARK, onenote2021::SHAPE),
            Preset::M365 => (BASE_PALETTE.0, BASE_PALETTE.1, BASE_SHAPE),
        };
        let palette = if dark { dark_palette } else { light_palette };
        Self::derive(palette, shape, dark)
    }

    /// The tokens for `preset` in `mode`, resolving `System` with `system_dark`.
    pub fn for_mode(preset: Preset, mode: Mode, system_dark: bool) -> Self {
        let dark = match mode {
            Mode::Light => false,
            Mode::Dark => true,
            Mode::System => system_dark,
        };
        Self::for_variant(preset, dark)
    }

    pub(super) fn derive(p: Palette, shape: Shape, dark: bool) -> Self {
        let text = hex(p.text);
        let canvas_bg = hex(p.canvas_bg);
        let ribbon_bg = hex(p.ribbon_bg);
        let accent = hex(p.accent);
        Self {
            dark,
            accent,
            accent_text: hex(p.accent_text),
            ribbon_bg,
            ribbon_tab_bg: hex(p.ribbon_tab_bg),
            ribbon_text: hex(p.ribbon_text),
            sidebar_bg: hex(p.sidebar_bg),
            sidebar_text: hex(p.sidebar_text),
            sidebar_selected: hex(p.sidebar_selected),
            canvas_bg,
            canvas_rule: hex(p.canvas_rule),
            text,
            text_muted: mix(text, canvas_bg, 0.35),
            border: hex(p.border),
            hover: mix(ribbon_bg, text, 0.08),
            pressed: mix(ribbon_bg, text, 0.16),
            badge_bg: accent,
            badge_text: hex(p.accent_text),
            radius: shape.radius,
            spacing: shape.spacing,
            shadow: shape.shadow,
            button_style: shape.button_style,
        }
    }
}

/// Microsoft 365 preset, light and dark (docs/themes.md section 4).
const BASE_PALETTE: (Palette, Palette) = (
    Palette {
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
    },
    Palette {
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
    },
);

const BASE_SHAPE: Shape = Shape {
    radius: 6,
    spacing: 6.0,
    shadow: ShadowStyle::Soft,
    button_style: ButtonStyle::Filled,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_mix() {
        assert_eq!(hex(0x8250B8), Color32::from_rgb(0x82, 0x50, 0xB8));
        let black = Color32::BLACK;
        let white = Color32::WHITE;
        assert_eq!(mix(black, white, 0.0), black);
        assert_eq!(mix(black, white, 1.0), white);
        assert_eq!(mix(black, white, 0.5), Color32::from_gray(128));
        assert_eq!(mix(black, white, f32::NAN), black);
        assert_eq!(mix(black, white, 7.0), white);
    }

    #[test]
    fn every_variant_has_tokens() {
        for preset in Preset::ALL {
            let light = ThemeTokens::for_variant(preset, false);
            let dark = ThemeTokens::for_variant(preset, true);
            assert!(!light.dark && dark.dark);
            assert_ne!(light.canvas_bg, dark.canvas_bg);
            assert_eq!(ThemeTokens::for_mode(preset, Mode::System, true), dark);
            assert_eq!(ThemeTokens::for_mode(preset, Mode::Light, true), light);
        }
    }
}
