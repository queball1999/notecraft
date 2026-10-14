//! User settings, stored in `config.toml` by craft-settings. Phase 0 starts with the window
//! geometry; theme and recent files join in later PRs. Every section uses `#[serde(default)]`,
//! so a missing or damaged key falls back to its default instead of failing the load.

use serde::{Deserialize, Serialize};

/// Smallest window, in points (build_plan/02 PR 0.1).
pub const MIN_SIZE: [f32; 2] = [800.0, 600.0];
/// Largest restored size or position, in points. Anything beyond is treated as damaged.
const MAX_COORD: f32 = 16_384.0;

#[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Prefs {
    pub window: WindowPrefs,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
