//! Themes: four presets, each in light and dark (docs/themes.md). Themes change colors,
//! spacing, corner radius, and control style, never layout or behavior.
//!
//! The preset and mode are view state in the engine session, so the theme commands
//! (`view.theme.set`, `view.mode.toggle`, `view.mode.set`) can change them; this module turns
//! them into colors.

mod apply;
pub mod contrast;
mod fonts;
mod m365;
mod onenote2016;
mod onenote2019;
mod onenote2021;
mod tokens;

pub use apply::{install, style};
pub use fonts::{is_usable, OsFonts};
pub use notecraft_engine::view::{ThemeMode as Mode, ThemePreset as Preset};
pub use tokens::{hex, mix, ButtonStyle, ShadowStyle, ThemeTokens};

#[cfg(test)]
mod tests {
    use super::*;

    const THEMES_DOC: &str = include_str!("../../../../docs/themes.md");

    /// The `| token | light | dark |` rows of one preset's table in docs/themes.md section 4.
    fn doc_rows(preset: Preset) -> Vec<(String, String, String)> {
        let heading = format!("### {}\n", preset.label());
        let start = THEMES_DOC.find(&heading).unwrap() + heading.len();
        let section = THEMES_DOC[start..].split("\n#").next().unwrap();
        section
            .lines()
            .filter_map(|line| {
                let cells: Vec<&str> = line
                    .trim()
                    .trim_matches('|')
                    .split('|')
                    .map(|c| c.trim().trim_matches('`'))
                    .collect();
                match cells.as_slice() {
                    [token, light, dark] if !token.is_empty() && *token != "Token" => {
                        Some((token.to_string(), light.to_string(), dark.to_string()))
                    }
                    _ => None,
                }
            })
            .filter(|(_, light, _)| !light.starts_with('-'))
            .collect()
    }

    fn token_text(tokens: &ThemeTokens, name: &str) -> String {
        let color = |c: egui::Color32| format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
        match name {
            "accent" => color(tokens.accent),
            "accent_text" => color(tokens.accent_text),
            "ribbon_bg" => color(tokens.ribbon_bg),
            "ribbon_tab_bg" => color(tokens.ribbon_tab_bg),
            "ribbon_text" => color(tokens.ribbon_text),
            "sidebar_bg" => color(tokens.sidebar_bg),
            "sidebar_text" => color(tokens.sidebar_text),
            "sidebar_selected" => color(tokens.sidebar_selected),
            "canvas_bg" => color(tokens.canvas_bg),
            "canvas_rule" => color(tokens.canvas_rule),
            "canvas_margin_rule" => color(tokens.canvas_margin_rule),
            "text" => color(tokens.text),
            "border" => color(tokens.border),
            "radius" => tokens.radius.to_string(),
            "spacing" => tokens.spacing.to_string(),
            "shadow" => format!("{:?}", tokens.shadow).to_lowercase(),
            "button_style" => format!("{:?}", tokens.button_style).to_lowercase(),
            other => panic!("docs/themes.md has an unknown token {other}"),
        }
    }

    #[test]
    fn palettes_match_the_themes_doc() {
        for preset in Preset::ALL {
            let rows = doc_rows(preset);
            assert_eq!(rows.len(), 17, "{} table rows", preset.label());
            let light = ThemeTokens::for_variant(preset, false);
            let dark = ThemeTokens::for_variant(preset, true);
            for (token, light_doc, dark_doc) in rows {
                let label = preset.label();
                assert_eq!(
                    token_text(&light, &token),
                    light_doc,
                    "{label} light {token}"
                );
                assert_eq!(token_text(&dark, &token), dark_doc, "{label} dark {token}");
            }
        }
    }

    #[test]
    fn presets_are_distinct() {
        for dark in [false, true] {
            let variants: Vec<ThemeTokens> = Preset::ALL
                .iter()
                .map(|preset| ThemeTokens::for_variant(*preset, dark))
                .collect();
            for (i, a) in variants.iter().enumerate() {
                for b in variants.iter().skip(i + 1) {
                    assert_ne!(a, b);
                }
            }
        }
    }
}
