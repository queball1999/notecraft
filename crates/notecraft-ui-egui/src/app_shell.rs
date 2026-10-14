//! The window's top-level layout. PR 0.1 lays out empty, labeled regions where the ribbon,
//! navigation, canvas, and status bar land in the next Phase 0 PRs (build_plan/02). PR 0.2 adds
//! command dispatch and toasts.

use notecraft_engine::cmd::{CommandError, Registry, Value};
use notecraft_engine::Session;

use crate::toast::{self, Toasts};

/// Height of the ribbon region, in points.
pub const RIBBON_HEIGHT: f32 = 120.0;
/// Height of the status bar, in points.
pub const STATUS_HEIGHT: f32 = 24.0;
/// Default width of the notebook and page panes, in points.
pub const SIDE_WIDTH: f32 = 220.0;

/// The labels of the placeholder regions, in drawing order. UI tests look them up by name.
pub const REGIONS: [&str; 5] = ["Ribbon", "Status bar", "Notebooks", "Pages", "Canvas"];

/// Everything inside the main window.
#[derive(Debug)]
pub struct AppShell {
    registry: Registry,
    session: Session,
    toasts: Toasts,
}

impl Default for AppShell {
    fn default() -> Self {
        Self::new()
    }
}

impl AppShell {
    pub fn new() -> Self {
        Self {
            registry: Registry::builtin(),
            session: Session::new(),
            toasts: Toasts::new(),
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn toasts(&self) -> &Toasts {
        &self.toasts
    }

    /// Run a command by id, as a ribbon button or shortcut does. A command that does not
    /// run (Coming Soon, unknown, bad arguments, or caught panic) shows a toast instead.
    pub fn dispatch(&mut self, id: &str, args: Value) {
        if let Err(err) = self.registry.run(&mut self.session, id, args) {
            match err {
                CommandError::ComingSoon { .. } => tracing::debug!("{err}"),
                _ => tracing::warn!("{err}"),
            }
            self.toasts.push(toast::error_text(&self.registry, &err));
        }
    }

    /// Per-frame work that must run even when the window is hidden (eframe `logic`).
    pub fn logic(&mut self, _ctx: &egui::Context) {}

    /// Draw the window contents (eframe `ui`).
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("ribbon")
            .exact_size(RIBBON_HEIGHT)
            .show(ui, |ui| placeholder(ui, "Ribbon"));
        egui::Panel::bottom("status_bar")
            .exact_size(STATUS_HEIGHT)
            .show(ui, |ui| placeholder(ui, "Status bar"));
        egui::Panel::left("notebooks")
            .default_size(SIDE_WIDTH)
            .resizable(true)
            .show(ui, |ui| placeholder(ui, "Notebooks"));
        egui::Panel::right("pages")
            .default_size(SIDE_WIDTH)
            .resizable(true)
            .show(ui, |ui| placeholder(ui, "Pages"));
        egui::CentralPanel::default().show(ui, |ui| placeholder(ui, "Canvas"));
        self.toasts.show(ui.ctx());
    }
}

fn placeholder(ui: &mut egui::Ui, name: &str) {
    ui.centered_and_justified(|ui| {
        ui.weak(name);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use notecraft_engine::cmd::ids;

    #[test]
    fn renders_a_frame_with_every_region() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .build_ui_state(|ui, shell: &mut AppShell| shell.ui(ui), AppShell::new());
        harness.run();
        for region in REGIONS {
            let _ = harness.get_by_label(region);
        }
    }

    #[test]
    fn coming_soon_command_shows_a_toast_for_three_seconds() {
        let mut shell = AppShell::new();
        shell.dispatch(ids::EDIT_PASTE.as_str(), Value::Null);
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .build_ui_state(|ui, shell: &mut AppShell| shell.ui(ui), shell);
        harness.run();
        let _ = harness.get_by_label("Coming Soon: Paste");

        // egui's clock advances one predicted frame (1/60 s) per step without real input.
        harness.run_steps(200);
        assert!(harness.query_by_label("Coming Soon: Paste").is_none());
        assert!(harness.state().toasts().is_empty());
    }

    #[test]
    fn unknown_command_shows_a_toast() {
        let mut shell = AppShell::new();
        shell.dispatch("no.such_command", Value::Null);
        let texts: Vec<&str> = shell.toasts().texts().collect();
        assert_eq!(texts, ["Unknown command: no.such_command"]);
    }

    #[test]
    fn survives_a_tiny_window() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1.0, 1.0))
            .build_ui_state(|ui, shell: &mut AppShell| shell.ui(ui), AppShell::new());
        harness.run();
    }
}
