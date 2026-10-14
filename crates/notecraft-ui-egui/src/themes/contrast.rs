//! WCAG contrast checks (docs/themes.md section 6). Every theme variant must reach WCAG AA
//! (4.5:1) for the text and background pairs listed in [`required_pairs`].

use egui::Color32;

use super::ThemeTokens;

/// WCAG AA minimum contrast for normal-size text.
pub const AA_NORMAL_TEXT: f64 = 4.5;

/// WCAG relative luminance of an sRGB color, 0 (black) to 1 (white). Alpha is ignored.
pub fn relative_luminance(color: Color32) -> f64 {
    let linear = |channel: u8| {
        let c = f64::from(channel) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
}

/// WCAG contrast ratio between two colors, 1 to 21. Order does not matter.
pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// One text color on one background, named for error messages.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pair {
    pub name: &'static str,
    pub text: Color32,
    pub background: Color32,
}

impl Pair {
    pub fn ratio(&self) -> f64 {
        contrast_ratio(self.text, self.background)
    }
}

/// The pairs docs/themes.md section 6 requires to pass AA.
pub fn required_pairs(tokens: &ThemeTokens) -> [Pair; 5] {
    let pair = |name, text, background| Pair {
        name,
        text,
        background,
    };
    [
        pair("text on canvas_bg", tokens.text, tokens.canvas_bg),
        pair(
            "ribbon_text on ribbon_bg",
            tokens.ribbon_text,
            tokens.ribbon_bg,
        ),
        pair("accent_text on accent", tokens.accent_text, tokens.accent),
        pair(
            "sidebar_text on sidebar_bg",
            tokens.sidebar_text,
            tokens.sidebar_bg,
        ),
        pair(
            "sidebar_text on sidebar_selected",
            tokens.sidebar_text,
            tokens.sidebar_selected,
        ),
    ]
}

/// The required pairs below AA, with their ratios. Empty means the variant passes.
pub fn failures(tokens: &ThemeTokens) -> Vec<(Pair, f64)> {
    required_pairs(tokens)
        .into_iter()
        .map(|pair| (pair, pair.ratio()))
        .filter(|(_, ratio)| *ratio < AA_NORMAL_TEXT)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::themes::Preset;

    #[test]
    fn known_ratios() {
        let ratio = contrast_ratio(Color32::BLACK, Color32::WHITE);
        assert!((ratio - 21.0).abs() < 1e-9);
        assert!((contrast_ratio(Color32::RED, Color32::RED) - 1.0).abs() < 1e-9);
        // #767676 on white is the classic smallest AA gray, about 4.54:1.
        let gray = contrast_ratio(Color32::from_gray(0x76), Color32::WHITE);
        assert!((4.5..4.6).contains(&gray), "{gray}");
        assert_eq!(
            contrast_ratio(Color32::WHITE, Color32::BLUE),
            contrast_ratio(Color32::BLUE, Color32::WHITE)
        );
    }

    #[test]
    fn failing_pairs_are_reported() {
        let mut tokens = ThemeTokens::for_variant(Preset::M365, false);
        tokens.text = tokens.canvas_bg;
        let failed = failures(&tokens);
        assert_eq!(failed.len(), 1);
        assert_eq!(
            failed.first().map(|(pair, _)| pair.name),
            Some("text on canvas_bg")
        );
    }

    #[test]
    fn every_variant_passes_aa() {
        for preset in Preset::ALL {
            for dark in [false, true] {
                let failed = failures(&ThemeTokens::for_variant(preset, dark));
                assert!(
                    failed.is_empty(),
                    "{} {}: {failed:?}",
                    preset.label(),
                    if dark { "dark" } else { "light" }
                );
            }
        }
    }
}
