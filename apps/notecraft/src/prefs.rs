//! User settings, stored in `config.toml` by craft-settings: window geometry, the theme
//! (docs/themes.md section 5), the ribbon state, and the recent list. Every section uses
//! `#[serde(default)]`, so a missing or damaged key falls back to its default instead of
//! failing the load.

use notecraft_engine::recent::{RecentKind, Recents};
use notecraft_engine::view::{RibbonLayout, RibbonShow};
use notecraft_engine::{default_pen_tools, PenTool, MAX_PEN_TOOLS};
use notecraft_ui_egui::themes::{Mode, Preset};
use serde::{Deserialize, Serialize};

/// Smallest window, in points (build_plan/02 PR 0.1).
pub const MIN_SIZE: [f32; 2] = [800.0, 600.0];
/// Largest restored size or position, in points. Anything beyond is treated as damaged.
const MAX_COORD: f32 = 16_384.0;

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Prefs {
    pub window: WindowPrefs,
    pub theme: ThemePrefs,
    pub ribbon: RibbonPrefs,
    pub recent: RecentPrefs,
    pub colors: ColorPrefs,
    pub draw: DrawPrefs,
    pub whats_new: WhatsNewPrefs,
}

/// Which version's What's New dialog the user has already been shown. Stored as text; a value
/// of the wrong type is read as "never shown" instead of failing the file.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct WhatsNewPrefs {
    #[serde(deserialize_with = "lenient_string")]
    pub seen_version: String,
}

fn lenient_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Text(String),
        Other(serde::de::IgnoredAny),
    }
    Ok(match Any::deserialize(deserializer)? {
        Any::Text(text) => text,
        Any::Other(_) => String::new(),
    })
}

impl WhatsNewPrefs {
    /// Decide whether What's New opens at this launch, and if so record that `version` was
    /// shown (so a crash right after does not show it again). `allowed` is false for runs
    /// that must never show it: the control channel, and runs with no settings folder
    /// (agent and test runs) where nothing could be remembered. Not allowed leaves the
    /// record alone.
    pub fn take_launch_slot(&mut self, version: &str, allowed: bool) -> bool {
        if !allowed
            || !notecraft_engine::whats_new::should_show_at_launch(&self.seen_version, version)
        {
            return false;
        }
        version.trim().clone_into(&mut self.seen_version);
        true
    }
}

/// The color menus' Recent Colors, newest first, as `#RRGGBB` or `#RRGGBBAA`. The front end
/// drops anything that is not a color when it restores them.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct ColorPrefs {
    pub recent: Vec<String>,
}

impl ColorPrefs {
    /// Record the current list; leaves it alone when nothing changed.
    pub fn observe(&mut self, recent: &[String]) {
        if self.recent != recent {
            self.recent = recent.to_vec();
        }
    }
}

/// The Draw tab's pens and highlighters. `tools` is `None` until the user changes the list,
/// which keeps the built-in set (and any later change to it) for people who never touched it.
/// Kinds and colors are strings, so an entry from a newer version that does not parse is
/// dropped instead of failing the file.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct DrawPrefs {
    pub tools: Option<Vec<ToolPref>>,
    /// The rule style new pages start with (Always Create Pages with Rule Lines): a
    /// `view.rule_lines` id, or `none`. An id this version does not know is read as `none`.
    pub new_page_rules: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct ToolPref {
    /// `pen` or `highlighter`.
    pub kind: String,
    /// `#RRGGBB` or `#RRGGBBAA`.
    pub color: String,
    /// Width in points.
    pub width: f32,
}

impl Default for ToolPref {
    fn default() -> Self {
        Self {
            kind: "pen".to_owned(),
            color: "#000000".to_owned(),
            width: 1.0,
        }
    }
}

impl DrawPrefs {
    /// The saved rule style for new pages; `none` when never set.
    pub fn default_rules(&self) -> &str {
        if self.new_page_rules.is_empty() {
            "none"
        } else {
            &self.new_page_rules
        }
    }

    /// Record the session's rule style for new pages; leaves it alone when nothing changed.
    pub fn observe_rules(&mut self, id: &str) {
        if self.default_rules() != id {
            self.new_page_rules = id.to_owned();
        }
    }

    /// The saved tools, with bad entries dropped and the count capped; the built-in set when
    /// the list was never changed.
    pub fn tools(&self) -> Vec<PenTool> {
        match &self.tools {
            None => default_pen_tools(),
            Some(saved) => saved
                .iter()
                .filter_map(|t| PenTool::from_saved(&t.kind, &t.color, t.width))
                .take(MAX_PEN_TOOLS)
                .collect(),
        }
    }

    /// Record the session's list; leaves it alone when nothing changed. A list equal to the
    /// built-in set is stored as `None`.
    pub fn observe(&mut self, tools: &[PenTool]) {
        if self.tools().as_slice() == tools {
            return;
        }
        self.tools = if tools == default_pen_tools().as_slice() {
            None
        } else {
            Some(
                tools
                    .iter()
                    .map(|tool| {
                        let (kind, color, width) = tool.to_saved();
                        ToolPref {
                            kind: kind.to_owned(),
                            color,
                            width,
                        }
                    })
                    .collect(),
            )
        };
    }
}

/// The recent notebooks and pages, newest first. Kinds are strings so an unknown kind (from
/// a newer version) drops that entry instead of failing the file; the engine checks the
/// paths again on restore.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct RecentPrefs {
    pub items: Vec<RecentEntry>,
}

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct RecentEntry {
    pub kind: String,
    pub path: String,
}

impl RecentPrefs {
    /// The saved list, with bad entries dropped.
    pub fn recents(&self) -> Recents {
        Recents::from_items(self.items.iter().filter_map(|entry| {
            RecentKind::from_id(&entry.kind).map(|kind| (kind, entry.path.clone()))
        }))
    }

    /// Record the session's list; leaves the entries alone when nothing changed.
    pub fn observe(&mut self, recents: &Recents) {
        let items: Vec<RecentEntry> = recents
            .items()
            .iter()
            .map(|item| RecentEntry {
                kind: item.kind.id().to_owned(),
                path: item.path.clone(),
            })
            .collect();
        if self.items != items {
            self.items = items;
        }
    }
}

/// The ribbon's layout and show mode (Ribbon Display Options), kept across runs as in
/// OneNote. Strings, so an unknown value falls back to the default. `collapsed` is the older
/// setting: it is read when `show` is missing and kept in step so an older build still opens
/// the ribbon the way it was left.
#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct RibbonPrefs {
    pub collapsed: bool,
    pub layout: String,
    pub show: String,
}

impl RibbonPrefs {
    pub fn layout(&self) -> RibbonLayout {
        RibbonLayout::from_id(&self.layout).unwrap_or_default()
    }

    pub fn show(&self) -> RibbonShow {
        match RibbonShow::from_id(&self.show) {
            Some(show) => show,
            None if self.collapsed => RibbonShow::TabsOnly,
            None => RibbonShow::default(),
        }
    }

    /// Record the current layout and show mode; leaves the strings alone when nothing
    /// changed.
    pub fn observe(&mut self, layout: RibbonLayout, show: RibbonShow) {
        if self.layout != layout.id() {
            layout.id().clone_into(&mut self.layout);
        }
        if self.show != show.id() {
            show.id().clone_into(&mut self.show);
        }
        let collapsed = show == RibbonShow::TabsOnly;
        if self.collapsed != collapsed {
            self.collapsed = collapsed;
        }
    }
}

impl craft_settings::Settings for Prefs {
    const APP_ID: &'static str = "notecraft";
    const SCHEMA_VERSION: u32 = 1;
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct WindowPrefs {
    pub width: f32,
    pub height: f32,
    /// Outer position of the window; None lets the OS place it.
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub maximized: bool,
}

impl Default for WindowPrefs {
    fn default() -> Self {
        Self {
            width: 1280.0,
            height: 800.0,
            x: None,
            y: None,
            maximized: false,
        }
    }
}

impl WindowPrefs {
    /// The size to open with: the saved size, clamped to sane bounds.
    pub fn size(&self) -> [f32; 2] {
        let clamp = |v: f32, min: f32| {
            if v.is_finite() {
                v.clamp(min, MAX_COORD)
            } else {
                min
            }
        };
        [
            clamp(self.width, MIN_SIZE[0]),
            clamp(self.height, MIN_SIZE[1]),
        ]
    }

    /// The position to open at, if one was saved and looks plausible.
    pub fn position(&self) -> Option<[f32; 2]> {
        let ok = |v: f32| v.is_finite() && v.abs() <= MAX_COORD;
        match (self.x, self.y) {
            (Some(x), Some(y)) if ok(x) && ok(y) => Some([x, y]),
            _ => None,
        }
    }

    /// Record what the OS reports about the window. Minimized frames change nothing, and a
    /// maximized window keeps its last normal size so un-maximizing restores it.
    pub fn observe(&mut self, info: &egui::ViewportInfo) {
        if info.minimized == Some(true) {
            return;
        }
        if info.maximized == Some(true) {
            self.maximized = true;
            return;
        }
        self.maximized = false;
        if let Some(inner) = info.inner_rect {
            self.width = inner.width().round();
            self.height = inner.height().round();
        }
        if let Some(outer) = info.outer_rect {
            self.x = Some(outer.min.x.round());
            self.y = Some(outer.min.y.round());
        }
    }
}

/// `theme.preset` and `theme.mode`. Kept as strings so an unknown value (a newer version's
/// preset, a typo) falls back to the default instead of failing the whole file.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct ThemePrefs {
    pub preset: String,
    pub mode: String,
}

impl Default for ThemePrefs {
    fn default() -> Self {
        Self {
            preset: Preset::default().id().to_owned(),
            mode: Mode::default().id().to_owned(),
        }
    }
}

impl ThemePrefs {
    pub fn preset(&self) -> Preset {
        Preset::from_id(&self.preset).unwrap_or_default()
    }

    pub fn mode(&self) -> Mode {
        Mode::from_id(&self.mode).unwrap_or_default()
    }

    /// Record the current theme; leaves the strings alone when nothing changed.
    pub fn observe(&mut self, preset: Preset, mode: Mode) {
        if self.preset != preset.id() {
            preset.id().clone_into(&mut self.preset);
        }
        if self.mode != mode.id() {
            mode.id().clone_into(&mut self.mode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whats_new_opens_on_the_first_launch_and_records_the_version() {
        let mut prefs = Prefs::default();
        assert!(prefs.whats_new.take_launch_slot("0.0.2", true));
        assert_eq!(prefs.whats_new.seen_version, "0.0.2");
        // It survives the settings file, so the next launch of the same version is quiet.
        let text = toml::to_string(&prefs).unwrap();
        let mut back: Prefs = toml::from_str(&text).unwrap();
        assert!(!back.whats_new.take_launch_slot("0.0.2", true));
        // A newer version shows it once more; a downgrade does too.
        assert!(back.whats_new.take_launch_slot("0.0.3", true));
        assert!(!back.whats_new.take_launch_slot("0.0.3", true));
        assert!(back.whats_new.take_launch_slot("0.0.2", true));
        assert_eq!(back.whats_new.seen_version, "0.0.2");
    }

    #[test]
    fn whats_new_survives_a_damaged_stored_value() {
        for text in [
            "[whats_new]\nseen_version = 7\n",
            "[whats_new]\nseen_version = [1, 2]\n",
            "[whats_new]\nseen_version = \"\"\n",
            "[whats_new]\nseen_version = \"not a version at all\"\n",
            "[whats_new]\n",
        ] {
            let mut prefs: Prefs = toml::from_str(text).unwrap();
            assert!(prefs.whats_new.take_launch_slot("0.0.2", true), "{text}");
            assert_eq!(prefs.whats_new.seen_version, "0.0.2");
        }
        // Other settings in the same file are not lost to the damaged key.
        let prefs: Prefs =
            toml::from_str("[whats_new]\nseen_version = 7\n[theme]\nmode = \"dark\"\n").unwrap();
        assert_eq!(prefs.theme.mode, "dark");
    }

    #[test]
    fn whats_new_never_opens_when_not_allowed() {
        let mut prefs = Prefs::default();
        assert!(!prefs.whats_new.take_launch_slot("0.0.2", false));
        assert_eq!(prefs.whats_new.seen_version, "");
    }

    #[test]
    fn the_pen_list_is_saved_only_when_changed_and_restores_bounded() {
        let mut prefs = DrawPrefs::default();
        assert_eq!(prefs.tools(), default_pen_tools());
        prefs.observe(&default_pen_tools());
        assert_eq!(prefs.tools, None);
        let mut edited = default_pen_tools();
        edited.pop();
        edited.push(PenTool::from_saved("pen", "#E36C09", 3.5).unwrap());
        prefs.observe(&edited);
        assert!(prefs.tools.is_some());
        assert_eq!(prefs.tools(), edited);
        // It survives the settings file.
        let text = toml::to_string(&Prefs {
            draw: prefs.clone(),
            ..Prefs::default()
        })
        .unwrap();
        let back: Prefs = toml::from_str(&text).unwrap();
        assert_eq!(back.draw.tools(), edited);
        // Putting the built-in set back stores nothing.
        prefs.observe(&default_pen_tools());
        assert_eq!(prefs.tools, None);
        // An emptied list is kept as empty, not replaced by the defaults.
        prefs.observe(&[]);
        assert_eq!(prefs.tools(), Vec::new());
    }

    #[test]
    fn damaged_pen_entries_are_dropped_or_put_in_range() {
        let text = r##"
            [draw]
            tools = [
                { kind = "pen", color = "#112233", width = 9999.0 },
                { kind = "chalk", color = "#112233", width = 1.0 },
                { kind = "pen", color = "red", width = 1.0 },
                { kind = "highlighter", color = "#FFF17680", width = -4.0 },
            ]
        "##;
        let prefs: Prefs = toml::from_str(text).unwrap();
        let tools = prefs.draw.tools();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].style.width_pt, 72.0);
        assert_eq!(tools[1].style.width_pt, 0.25);
        let many = format!(
            "[draw]
tools = [{}]",
            vec![r##"{ kind = "pen", color = "#000000", width = 1.0 }"##; 100].join(",")
        );
        let prefs: Prefs = toml::from_str(&many).unwrap();
        assert_eq!(prefs.draw.tools().len(), MAX_PEN_TOOLS);
    }

    #[test]
    fn damaged_sizes_fall_back_to_bounds() {
        let w = WindowPrefs {
            width: f32::NAN,
            height: 10.0,
            x: Some(f32::INFINITY),
            y: Some(5.0),
            maximized: false,
        };
        assert_eq!(w.size(), MIN_SIZE);
        assert_eq!(w.position(), None);
        let huge = WindowPrefs {
            width: 1e9,
            ..WindowPrefs::default()
        };
        assert_eq!(huge.size(), [MAX_COORD, 800.0]);
    }

    #[test]
    fn observes_normal_maximized_and_minimized_windows() {
        let mut w = WindowPrefs::default();
        let normal = egui::ViewportInfo {
            inner_rect: Some(egui::Rect::from_min_size(
                egui::pos2(10.0, 40.0),
                egui::vec2(1000.4, 700.6),
            )),
            outer_rect: Some(egui::Rect::from_min_size(
                egui::pos2(10.0, 10.0),
                egui::vec2(1000.0, 730.0),
            )),
            ..Default::default()
        };
        w.observe(&normal);
        assert_eq!(
            (w.width, w.height, w.x, w.y),
            (1000.0, 701.0, Some(10.0), Some(10.0))
        );

        let maximized = egui::ViewportInfo {
            maximized: Some(true),
            inner_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(2560.0, 1400.0),
            )),
            ..Default::default()
        };
        w.observe(&maximized);
        assert!(w.maximized);
        assert_eq!(w.width, 1000.0, "keeps the normal size while maximized");

        let minimized = egui::ViewportInfo {
            minimized: Some(true),
            ..Default::default()
        };
        w.observe(&minimized);
        assert!(w.maximized, "minimizing changes nothing");
    }

    #[test]
    fn missing_keys_use_defaults() {
        let p: Prefs = serde_json::from_str(r#"{"window":{"width":900.0}}"#).unwrap();
        assert_eq!(p.window.width, 900.0);
        assert_eq!(p.window.height, 800.0);
        let empty: Prefs = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, Prefs::default());
    }

    #[test]
    fn theme_defaults_and_bad_values() {
        let t = ThemePrefs::default();
        assert_eq!((t.preset.as_str(), t.mode.as_str()), ("m365", "system"));
        let p: Prefs =
            serde_json::from_str(r#"{"theme":{"preset":"neon","mode":"sepia"}}"#).unwrap();
        assert_eq!(p.theme.preset(), Preset::M365);
        assert_eq!(p.theme.mode(), Mode::System);
        let odd = ThemePrefs {
            preset: "onenote2019".into(),
            mode: "sepia".into(),
        };
        assert_eq!(
            (odd.preset(), odd.mode()),
            (Preset::OneNote2019, Mode::System)
        );
    }

    #[test]
    fn recents_round_trip_and_drop_bad_entries() {
        let p: Prefs = serde_json::from_str(
            r#"{"recent":{"items":[
                {"kind":"notebook","path":"/n/Work"},
                {"kind":"folder","path":"/n/Old"},
                {"kind":"page","path":""},
                {"kind":"page","path":"/n/a.md"},
                {"path":"/n/no-kind"}
            ]}}"#,
        )
        .unwrap();
        let recents = p.recent.recents();
        let paths: Vec<&str> = recents.items().iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, ["/n/Work", "/n/a.md"]);

        let mut saved = RecentPrefs::default();
        saved.observe(&recents);
        assert_eq!(saved.recents(), recents);
        assert_eq!(
            saved.items.first().map(|e| e.kind.as_str()),
            Some("notebook")
        );
    }

    #[test]
    fn ribbon_reads_old_and_bad_values_and_round_trips() {
        let old: Prefs = serde_json::from_str(r#"{"ribbon":{"collapsed":true}}"#).unwrap();
        assert_eq!(old.ribbon.show(), RibbonShow::TabsOnly);
        assert_eq!(old.ribbon.layout(), RibbonLayout::Classic);
        let bad: Prefs =
            serde_json::from_str(r#"{"ribbon":{"layout":"compact","show":"hidden"}}"#).unwrap();
        assert_eq!(
            (bad.ribbon.layout(), bad.ribbon.show()),
            (RibbonLayout::Classic, RibbonShow::Always)
        );
        let mut r = RibbonPrefs::default();
        r.observe(RibbonLayout::Simplified, RibbonShow::FullScreen);
        assert_eq!(
            (r.layout(), r.show(), r.collapsed),
            (RibbonLayout::Simplified, RibbonShow::FullScreen, false)
        );
        r.observe(RibbonLayout::Simplified, RibbonShow::TabsOnly);
        assert!(r.collapsed, "an older build opens it collapsed");
    }

    #[test]
    fn recent_colors_round_trip() {
        let p: Prefs = serde_json::from_str(r##"{"colors":{"recent":["#112233"]}}"##).unwrap();
        assert_eq!(p.colors.recent, ["#112233"]);
        let mut c = ColorPrefs::default();
        c.observe(&["#AABBCC".to_owned()]);
        assert_eq!(c.recent, ["#AABBCC"]);
        let empty: Prefs = serde_json::from_str("{}").unwrap();
        assert!(empty.colors.recent.is_empty());
    }

    #[test]
    fn theme_round_trips() {
        let mut t = ThemePrefs::default();
        t.observe(Preset::OneNote2016, Mode::Dark);
        assert_eq!((t.preset(), t.mode()), (Preset::OneNote2016, Mode::Dark));
        assert_eq!(t.preset, "onenote2016");
        assert_eq!(t.mode, "dark");
    }
}
