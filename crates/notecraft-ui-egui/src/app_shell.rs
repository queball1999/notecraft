//! The window's top-level layout. PR 0.1 lays out empty, labeled regions where the ribbon,
//! navigation, canvas, and status bar land in the next Phase 0 PRs (build_plan/02).

/// Height of the ribbon region, in points.
pub const RIBBON_HEIGHT: f32 = 120.0;
/// Height of the status bar, in points.
pub const STATUS_HEIGHT: f32 = 24.0;
/// Default width of the notebook and page panes, in points.
pub const SIDE_WIDTH: f32 = 220.0;

/// The labels of the placeholder regions, in drawing order. UI tests look them up by name.
pub const REGIONS: [&str; 5] = ["Ribbon", "Status bar", "Notebooks", "Pages", "Canvas"];

/// Everything inside the main window.
#[derive(Debug, Default)]
pub struct AppShell {}

impl AppShell {
    pub fn new() -> Self {
        Self::default()
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
    fn survives_a_tiny_window() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(1.0, 1.0))
            .build_ui_state(|ui, shell: &mut AppShell| shell.ui(ui), AppShell::new());
        harness.run();
    }
}
