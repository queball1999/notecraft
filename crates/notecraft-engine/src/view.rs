//! View state that commands change and the front end draws: the theme preset and color mode
//! (docs/themes.md section 5), whether the ribbon is collapsed, the canvas zoom, and how a
//! page looks (rule lines, page color, paper size). The colors themselves live in the front
//! end.

use notecraft_canvas::{PaperSize, RuleLines};

/// One of the four theme presets. Ids are the `theme.preset` setting values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemePreset {
    OneNote2016,
    OneNote2019,
    OneNote2021,
    #[default]
    M365,
}

impl ThemePreset {
    pub const ALL: [Self; 4] = [
        Self::OneNote2016,
        Self::OneNote2019,
        Self::OneNote2021,
        Self::M365,
    ];

    /// The setting and command argument value, for example `onenote2016`.
    pub fn id(self) -> &'static str {
        match self {
            Self::OneNote2016 => "onenote2016",
            Self::OneNote2019 => "onenote2019",
            Self::OneNote2021 => "onenote2021",
            Self::M365 => "m365",
        }
    }

    /// The name shown in the UI.
    pub fn label(self) -> &'static str {
        match self {
            Self::OneNote2016 => "OneNote 2016",
            Self::OneNote2019 => "OneNote 2019",
            Self::OneNote2021 => "OneNote 2021",
            Self::M365 => "Microsoft 365",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|preset| preset.id() == id)
    }
}

/// Light, dark, or whatever the OS uses. Ids are the `theme.mode` setting values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];

    /// The setting and command argument value: `light`, `dark`, or `system`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.id() == id)
    }
}

/// How a page looks behind its content. Phase 0 keeps one per page in the session; Phase 1
/// stores it in the page file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageStyle {
    pub rule_lines: RuleLines,
    /// Background color as RGBA, or `None` for the theme's canvas color.
    pub color: Option<[u8; 4]>,
    pub paper_size: PaperSize,
}

/// What the view looks like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewState {
    pub theme_preset: ThemePreset,
    pub theme_mode: ThemeMode,
    /// Whether the OS is in dark mode, as last reported by the front end. Only used to
    /// resolve [`ThemeMode::System`].
    pub system_dark: bool,
    /// The ribbon shows only its tab names (`view.ribbon.toggle`, Ctrl+F1).
    pub ribbon_collapsed: bool,
    /// Canvas zoom in percent, 10 to 400 (`view.zoom` and friends).
    pub zoom_percent: u16,
    /// Width of the canvas in screen points, as last reported by the front end. Only used
    /// by `view.zoom_page_width`; zero until the canvas is first drawn.
    pub canvas_width: u32,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            theme_preset: ThemePreset::default(),
            theme_mode: ThemeMode::default(),
            system_dark: false,
            ribbon_collapsed: false,
            zoom_percent: 100,
            canvas_width: 0,
        }
    }
}

impl ViewState {
    /// Whether the view is dark right now, resolving `System` through the OS setting.
    pub fn is_dark(&self) -> bool {
        match self.theme_mode {
            ThemeMode::Light => false,
            ThemeMode::Dark => true,
            ThemeMode::System => self.system_dark,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for preset in ThemePreset::ALL {
            assert_eq!(ThemePreset::from_id(preset.id()), Some(preset));
        }
        for mode in ThemeMode::ALL {
            assert_eq!(ThemeMode::from_id(mode.id()), Some(mode));
        }
        assert_eq!(ThemePreset::from_id("M365"), None);
        assert_eq!(ThemeMode::from_id(""), None);
    }

    #[test]
    fn defaults_match_the_settings_table() {
        let view = ViewState::default();
        assert_eq!(view.theme_preset.id(), "m365");
        assert_eq!(view.theme_mode.id(), "system");
        assert_eq!(view.zoom_percent, 100);
        assert_eq!(PageStyle::default().rule_lines, RuleLines::None);
    }

    #[test]
    fn system_mode_follows_the_os() {
        let mut view = ViewState::default();
        assert!(!view.is_dark());
        view.system_dark = true;
        assert!(view.is_dark());
        view.theme_mode = ThemeMode::Light;
        assert!(!view.is_dark());
    }
}
