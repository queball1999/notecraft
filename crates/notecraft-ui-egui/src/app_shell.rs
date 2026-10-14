//! The window's top-level layout. PR 0.1 lays out empty, labeled regions where the ribbon,
//! navigation, canvas, and status bar land in the next Phase 0 PRs (build_plan/02). PR 0.2 adds
//! command dispatch and toasts; PR 0.3 installs the theme.

use notecraft_engine::cmd::{CommandError, Registry, Value};
use notecraft_engine::Session;

use crate::themes::{self, Mode, Preset, ThemeTokens};
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
    /// The preset and mode last installed into egui, so it is redone only on a change.
    installed: Option<(Preset, Mode)>,
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
            installed: None,
        }
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn toasts(&self) -> &Toasts {
        &self.toasts
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Set the theme without a command, for restoring saved settings at startup.
    pub fn set_theme(&mut self, preset: Preset, mode: Mode) {
        let view = self.session.view_mut();
        view.theme_preset = preset;
        view.theme_mode = mode;
    }

    /// The tokens of the theme egui is showing now.
    pub fn tokens(&self, ctx: &egui::Context) -> ThemeTokens {
        ThemeTokens::for_variant(
            self.session.view().theme_preset,
            ctx.theme() == egui::Theme::Dark,
        )
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
    pub fn logic(&mut self, ctx: &egui::Context) {
        self.session.view_mut().system_dark = ctx.system_theme() == Some(egui::Theme::Dark);
        let view = self.session.view();
        let wanted = (view.theme_preset, view.theme_mode);
        if self.installed != Some(wanted) {
            themes::install(ctx, wanted.0, wanted.1);
            self.installed = Some(wanted);
            ctx.request_repaint();
        }
    }

    /// Draw the window contents (eframe `ui`).
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let tokens = self.tokens(ui.ctx());
        let panel = egui::Frame::side_top_panel(ui.style());
        egui::Panel::top("ribbon")
            .exact_size(RIBBON_HEIGHT)
            .frame(panel.fill(tokens.ribbon_bg))
            .show(ui, |ui| placeholder(ui, "Ribbon"));
        egui::Panel::bottom("status_bar")
            .exact_size(STATUS_HEIGHT)
            .frame(panel.fill(tokens.ribbon_tab_bg))
            .show(ui, |ui| placeholder(ui, "Status bar"));
        egui::Panel::left("notebooks")
            .default_size(SIDE_WIDTH)
            .resizable(true)
            .frame(panel.fill(tokens.sidebar_bg))
            .show(ui, |ui| placeholder(ui, "Notebooks"));
        egui::Panel::right("pages")
            .default_size(SIDE_WIDTH)
            .resizable(true)
            .frame(panel.fill(tokens.sidebar_bg))
            .show(ui, |ui| placeholder(ui, "Pages"));
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).fill(tokens.canvas_bg))
            .show(ui, |ui| placeholder(ui, "Canvas"));
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

    const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

    /// A headless window running the shell the way eframe does: logic, then ui.
    fn harness(shell: AppShell, size: egui::Vec2) -> egui_kittest::Harness<'static, AppShell> {
        egui_kittest::Harness::builder()
            .with_size(size)
            .build_ui_state(
                |ui, shell: &mut AppShell| {
                    shell.logic(ui.ctx());
                    shell.ui(ui);
                },
                shell,
            )
    }

    #[test]
    fn renders_a_frame_with_every_region() {
        let mut harness = harness(AppShell::new(), WINDOW);

        harness.run();
        for region in REGIONS {
            let _ = harness.get_by_label(region);
        }
    }

    #[test]
    fn coming_soon_command_shows_a_toast_for_three_seconds() {
        let mut shell = AppShell::new();
        shell.dispatch(ids::EDIT_PASTE.as_str(), Value::Null);
        let mut harness = harness(shell, WINDOW);

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
    fn installs_the_theme_from_the_session() {
        let mut shell = AppShell::new();
        shell.set_theme(Preset::M365, Mode::Dark);
        let mut harness = harness(shell, WINDOW);
        harness.run();
        assert_eq!(harness.ctx.theme(), egui::Theme::Dark);
        let tokens = harness.state().tokens(&harness.ctx);
        assert!(tokens.dark);
        assert_eq!(
            harness.ctx.global_style().visuals.panel_fill,
            tokens.sidebar_bg
        );

        harness.state_mut().set_theme(Preset::M365, Mode::Light);
        harness.run();
        assert_eq!(harness.ctx.theme(), egui::Theme::Light);
    }

    #[test]
    fn system_mode_follows_the_os_live() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.input_mut().system_theme = Some(egui::Theme::Dark);
        harness.run();
        assert_eq!(harness.ctx.theme(), egui::Theme::Dark);
        assert!(harness.state().session().view().system_dark);
        assert!(harness.state().tokens(&harness.ctx).dark);

        // The OS switches to light while the app runs.
        harness.input_mut().system_theme = Some(egui::Theme::Light);
        harness.run();
        assert_eq!(harness.ctx.theme(), egui::Theme::Light);
        assert!(!harness.state().session().view().system_dark);
        assert!(!harness.state().tokens(&harness.ctx).dark);
    }

    #[test]
    fn survives_a_tiny_window() {
        let mut harness = harness(AppShell::new(), egui::vec2(1.0, 1.0));

        harness.run();
    }
}
