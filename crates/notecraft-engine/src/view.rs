//! View state that commands change and the front end draws: the theme preset and color mode
//! (docs/themes.md section 5), the ribbon's layout and how much of it shows, the canvas zoom, and how a
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

/// The ribbon's layout (Ribbon Display Options, Ribbon Layout). Ids are the
/// `ribbon.layout` setting values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RibbonLayout {
    /// Groups of large and small buttons under the tab strip.
    #[default]
    Classic,
    /// One row of icon buttons; what does not fit goes in a More Options menu.
    Simplified,
}

impl RibbonLayout {
    pub const ALL: [Self; 2] = [Self::Classic, Self::Simplified];

    /// The setting and command argument value: `classic` or `simplified`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Simplified => "simplified",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layout| layout.id() == id)
    }
}

/// How much of the ribbon shows (Ribbon Display Options, Show Ribbon). Ids are the
/// `ribbon.show` setting values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RibbonShow {
    /// Tabs and commands.
    #[default]
    Always,
    /// Tabs only; clicking a tab shows its commands over the page until one is used.
    TabsOnly,
    /// No tabs or commands until the pointer reaches the top of the window.
    FullScreen,
}

impl RibbonShow {
    pub const ALL: [Self; 3] = [Self::Always, Self::TabsOnly, Self::FullScreen];

    /// The setting and command argument value: `always`, `tabs`, or `full_screen`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::TabsOnly => "tabs",
            Self::FullScreen => "full_screen",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|show| show.id() == id)
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
    /// Classic or simplified ribbon (`view.ribbon.layout`).
    pub ribbon_layout: RibbonLayout,
    /// How much of the ribbon shows (`view.ribbon.show`; Ctrl+F1 switches between
    /// always and tabs only).
    pub ribbon_show: RibbonShow,
    /// Full Page View (`view.full_page`, F11): only the page shows.
    pub full_page: bool,
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
            ribbon_layout: RibbonLayout::default(),
            ribbon_show: RibbonShow::default(),
            full_page: false,
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
        for layout in RibbonLayout::ALL {
            assert_eq!(RibbonLayout::from_id(layout.id()), Some(layout));
        }
        for show in RibbonShow::ALL {
            assert_eq!(RibbonShow::from_id(show.id()), Some(show));
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
