//! Turn theme tokens into an egui [`Style`] and install it.

use craft_ui::theme::WidgetPalette;
use egui::{Color32, CornerRadius, Margin, Shadow, Stroke, Style, Theme, ThemePreference};

use super::{mix, ButtonStyle, Mode, Preset, ShadowStyle, ThemeTokens};

/// The egui style for one theme variant.
pub fn style(tokens: &ThemeTokens) -> Style {
    let mut style = Theme::from_dark_mode(tokens.dark).default_style();
    let radius = CornerRadius::same(tokens.radius);

    let visuals = &mut style.visuals;
    let at_rest = match tokens.button_style {
        ButtonStyle::Flat => tokens.ribbon_bg,
        ButtonStyle::Raised | ButtonStyle::Filled => mix(tokens.ribbon_bg, tokens.text, 0.05),
    };
    WidgetPalette {
        noninteractive: tokens.ribbon_bg,
        inactive: at_rest,
        hovered: tokens.hover,
        active: tokens.pressed,
        open: tokens.pressed,
    }
    .apply_to(&mut visuals.widgets, radius, Stroke::new(1.0, tokens.text));
    let border = Stroke::new(1.0, tokens.border);
    visuals.widgets.noninteractive.bg_stroke = border;
    visuals.widgets.inactive.bg_stroke = match tokens.button_style {
        ButtonStyle::Raised => border,
        ButtonStyle::Flat | ButtonStyle::Filled => Stroke::NONE,
    };
    if tokens.button_style == ButtonStyle::Flat {
        visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    }
    visuals.widgets.hovered.bg_stroke = border;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, tokens.accent);

    visuals.selection.bg_fill = mix(tokens.accent, tokens.canvas_bg, 0.55);
    visuals.selection.stroke = Stroke::new(1.0, tokens.accent);
    visuals.hyperlink_color = tokens.accent;
    visuals.override_text_color = None;

    visuals.panel_fill = tokens.sidebar_bg;
    visuals.window_fill = tokens.canvas_bg;
    visuals.window_stroke = border;
    visuals.window_corner_radius = radius;
    visuals.menu_corner_radius = radius;
    visuals.extreme_bg_color = tokens.canvas_bg;
    visuals.faint_bg_color = mix(tokens.canvas_bg, tokens.text, 0.03);
    let shadow = shadow(tokens.shadow, tokens.dark);
    visuals.window_shadow = shadow;
    visuals.popup_shadow = shadow;

    let unit = if tokens.spacing.is_nan() {
        0.0
    } else {
        tokens.spacing.clamp(0.0, 32.0)
    };
    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(unit * 1.5, unit);
    spacing.button_padding = egui::vec2(unit * 1.5, unit * 0.5);
    // At most 64, so the casts cannot overflow i8.
    spacing.window_margin = Margin::same((unit * 2.0) as i8);
    spacing.menu_margin = Margin::same(unit as i8);

    style
}

fn shadow(kind: ShadowStyle, dark: bool) -> Shadow {
    let alpha = |light: u8, dark_alpha: u8| {
        Color32::from_black_alpha(if dark { dark_alpha } else { light })
    };
    match kind {
        ShadowStyle::None => Shadow::NONE,
        ShadowStyle::Subtle => Shadow {
            offset: [0, 1],
            blur: 4,
            spread: 0,
            color: alpha(30, 60),
        },
        ShadowStyle::Soft => Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: alpha(40, 80),
        },
    }
}

/// Install `preset` for both egui themes and pick light, dark, or the OS setting. egui then
/// follows the OS on its own while `mode` is `System`.
pub fn install(ctx: &egui::Context, preset: Preset, mode: Mode) {
    for theme in [Theme::Light, Theme::Dark] {
        let tokens = ThemeTokens::for_variant(preset, theme == Theme::Dark);
        ctx.set_style_of(theme, style(&tokens));
    }
    ctx.set_theme(match mode {
        Mode::Light => ThemePreference::Light,
        Mode::Dark => ThemePreference::Dark,
        Mode::System => ThemePreference::System,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_reads_the_tokens() {
        for preset in Preset::ALL {
            for dark in [false, true] {
                let tokens = ThemeTokens::for_variant(preset, dark);
                let style = style(&tokens);
                let v = &style.visuals;
                assert_eq!(v.dark_mode, dark);
                assert_eq!(v.panel_fill, tokens.sidebar_bg);
                assert_eq!(v.window_fill, tokens.canvas_bg);
                assert_eq!(v.hyperlink_color, tokens.accent);
                assert_eq!(v.widgets.hovered.bg_fill, tokens.hover);
                assert_eq!(v.widgets.active.bg_fill, tokens.pressed);
                assert_eq!(v.widgets.inactive.fg_stroke.color, tokens.text);
                assert_eq!(v.window_corner_radius, CornerRadius::same(tokens.radius));
                assert_eq!(style.spacing.item_spacing.y, tokens.spacing);
            }
        }
    }

    #[test]
    fn hostile_spacing_is_clamped() {
        let mut tokens = ThemeTokens::for_variant(Preset::M365, false);
        tokens.spacing = f32::INFINITY;
        let big = style(&tokens);
        assert_eq!(big.spacing.window_margin, Margin::same(64));
        for bad in [-5.0, f32::NAN] {
            tokens.spacing = bad;
            assert_eq!(style(&tokens).spacing.window_margin, Margin::same(0));
        }
    }

    #[test]
    fn install_sets_both_themes_and_the_preference() {
        let ctx = egui::Context::default();
        install(&ctx, Preset::M365, Mode::Dark);
        assert_eq!(ctx.theme(), Theme::Dark);
        let dark = ThemeTokens::for_variant(Preset::M365, true);
        assert_eq!(
            ctx.style_of(Theme::Dark).visuals.panel_fill,
            dark.sidebar_bg
        );
        install(&ctx, Preset::M365, Mode::Light);
        assert_eq!(ctx.theme(), Theme::Light);
    }
}
