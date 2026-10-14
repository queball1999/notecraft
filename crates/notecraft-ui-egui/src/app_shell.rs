//! The window's top-level layout. PR 0.1 lays out empty, labeled regions where the ribbon,
//! navigation, canvas, and status bar land in the next Phase 0 PRs (build_plan/02). PR 0.2 adds
//! command dispatch and toasts; PR 0.3 installs the theme; PR 0.5 adds the ribbon; PR 0.6 adds
//! the notebook pane, section tabs, and page list on mock notebooks.

use notecraft_engine::cmd::{ids, CommandError, Registry, Value};
use notecraft_engine::Session;

use crate::nav::{self, NavUi};
use crate::ribbon::{self, Clicked, Ribbon};
use crate::themes::{self, Mode, OsFonts, Preset, ThemeTokens};
use crate::toast::{self, Toasts};

/// Height of the ribbon region (tab strip and body), in points.
pub const RIBBON_HEIGHT: f32 = ribbon::HEIGHT;
/// Height of the status bar, in points.
pub const STATUS_HEIGHT: f32 = 24.0;
/// Default width of the notebook and page panes, in points.
pub const SIDE_WIDTH: f32 = 220.0;

/// The labels of the placeholder regions, in drawing order. UI tests look them up by name.
pub const REGIONS: [&str; 4] = ["Status bar", "Notebooks", "Add Page", "Canvas"];

/// Everything inside the main window.
#[derive(Debug)]
pub struct AppShell {
    registry: Registry,
    session: Session,
    toasts: Toasts,
    ribbon: Ribbon,
    nav: NavUi,
    /// The preset and mode last installed into egui, so it is redone only on a change.
    installed: Option<(Preset, Mode)>,
    /// The OS UI fonts, if the app loaded them (tests run with egui's built-in font).
    fonts: OsFonts,
    /// The font choice last installed (see [`OsFonts::choice`]).
    installed_font: Option<Option<bool>>,
}

impl Default for AppShell {
    fn default() -> Self {
        Self::new()
    }
}

impl AppShell {
    pub fn new() -> Self {
        let mut session = Session::new();
        session.set_notebooks(nav::mock::notebooks());
        Self {
            registry: Registry::builtin(),
            session,
            toasts: Toasts::new(),
            ribbon: Ribbon::new(),
            nav: NavUi::default(),
            installed: None,
            fonts: OsFonts::default(),
            installed_font: None,
        }
    }

    /// Use the OS UI font (docs/themes.md section 2).
    pub fn with_fonts(mut self, fonts: OsFonts) -> Self {
        self.fonts = fonts;
        self
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

    pub fn ribbon(&self) -> &Ribbon {
        &self.ribbon
    }

    pub fn ribbon_mut(&mut self) -> &mut Ribbon {
        &mut self.ribbon
    }

    /// Set the theme without a command, for restoring saved settings at startup.
    pub fn set_theme(&mut self, preset: Preset, mode: Mode) {
        let view = self.session.view_mut();
        view.theme_preset = preset;
        view.theme_mode = mode;
    }

    /// Collapse or expand the ribbon without a command, for restoring saved settings.
    pub fn set_ribbon_collapsed(&mut self, collapsed: bool) {
        self.session.view_mut().ribbon_collapsed = collapsed;
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
        // Only the Microsoft 365 preset can pick a different font, so fonts are rebuilt
        // rarely; with no OS font egui keeps its built-in one.
        let font = self.fonts.choice(wanted.0);
        if self.installed_font != Some(font) {
            if font.is_some() || self.installed_font.is_some() {
                ctx.set_fonts(self.fonts.definitions(wanted.0));
            }
            self.installed_font = Some(font);
        }
    }

    /// Draw the window contents (eframe `ui`).
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let tokens = self.tokens(ui.ctx());
        let look = ribbon::Look {
            tokens: &tokens,
            registry: &self.registry,
        };
        if self.ribbon.backstage_open() {
            let clicked = self.ribbon.show_backstage(ui, look);
            self.dispatch_all(clicked);
            self.toasts.show(ui.ctx());
            return;
        }
        // Ctrl+F1 collapses or expands the ribbon, as in OneNote. The full shortcut map
        // arrives in PR 0.9.
        let toggle_ribbon =
            ui.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::F1));
        let collapsed = self.session.view().ribbon_collapsed;
        let panel = egui::Frame::side_top_panel(ui.style());
        let height = if collapsed {
            ribbon::TAB_STRIP_HEIGHT
        } else {
            RIBBON_HEIGHT
        };
        let mut clicked = egui::Panel::top("ribbon")
            .exact_size(height)
            .frame(egui::Frame::NONE.fill(tokens.ribbon_bg))
            .show(ui, |ui| self.ribbon.show(ui, look, collapsed))
            .inner;
        if toggle_ribbon {
            clicked.push(Clicked {
                command: ids::VIEW_RIBBON_TOGGLE,
                args: Value::Null,
            });
        }
        egui::Panel::bottom("status_bar")
            .exact_size(STATUS_HEIGHT)
            .frame(panel.fill(tokens.ribbon_bg))
            .show(ui, |ui| placeholder(ui, "Status bar"));
        let session = &self.session;
        let nav_ui = &mut self.nav;
        egui::Panel::left("notebooks")
            .default_size(SIDE_WIDTH)
            .resizable(true)
            .frame(panel.fill(tokens.sidebar_bg))
            .show(ui, |ui| {
                nav::notebook_pane(ui, session, nav_ui, &tokens, &mut clicked)
            });
        // The page list sits left or right of the page depending on the preset.
        let pages = if nav::pages_on_left(session.view().theme_preset) {
            egui::Panel::left("pages")
        } else {
            egui::Panel::right("pages")
        };
        pages
            .default_size(SIDE_WIDTH)
            .resizable(true)
            .frame(panel.fill(tokens.sidebar_bg))
            .show(ui, |ui| nav::page_list(ui, session, &tokens, &mut clicked));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(tokens.canvas_bg))
            .show(ui, |ui| {
                let full = ui.max_rect();
                let strip = egui::Rect::from_min_size(
                    full.min,
                    egui::vec2(full.width(), nav::SECTION_STRIP_HEIGHT),
                );
                nav::section_tabs(ui, strip, session, &tokens, &mut clicked);
                let page = egui::Rect::from_min_max(egui::pos2(full.min.x, strip.max.y), full.max);
                ui.scope_builder(egui::UiBuilder::new().max_rect(page.shrink(24.0)), |ui| {
                    if let Some(page) = session.current_page() {
                        ui.label(
                            egui::RichText::new(page.display_title())
                                .size(22.0)
                                .color(tokens.text),
                        );
                    }
                    placeholder(ui, "Canvas");
                });
            });
        self.dispatch_all(clicked);
        self.toasts.show(ui.ctx());
    }

    fn dispatch_all(&mut self, clicked: Vec<Clicked>) {
        for Clicked { command, args } in clicked {
            self.dispatch(command.as_str(), args);
        }
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
    fn ribbon_buttons_dispatch_their_commands() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        assert_eq!(harness.state().ribbon().selected(), "Home");
        harness.get_by_label("Bold").click();
        harness.run();
        let texts: Vec<String> = harness
            .state()
            .toasts()
            .texts()
            .map(str::to_owned)
            .collect();
        assert_eq!(texts, ["Coming Soon: Bold"]);
    }

    #[test]
    fn view_tab_switches_the_theme_with_no_toast() {
        let mut shell = AppShell::new();
        shell.set_theme(Preset::M365, Mode::Light);
        let mut harness = harness(shell, WINDOW);
        harness.run();
        harness.get_by_label("View").click();
        harness.run();
        assert_eq!(harness.state().ribbon().selected(), "View");
        harness.get_by_label("Dark Mode").click();
        harness.run();
        assert_eq!(harness.ctx.theme(), egui::Theme::Dark);
        assert_eq!(harness.state().session().view().theme_mode, Mode::Dark);
        assert!(harness.state().toasts().is_empty());
    }

    #[test]
    fn every_tab_draws_in_every_preset() {
        for preset in Preset::ALL {
            let mut shell = AppShell::new();
            shell.set_theme(preset, Mode::Dark);
            let mut harness = harness(shell, WINDOW);
            harness.run();
            for tab in ["Home", "Insert", "Draw", "History", "Review", "View"] {
                harness.get_by_label(tab).click();
                harness.run();
                assert_eq!(harness.state().ribbon().selected(), tab);
            }
        }
    }

    #[test]
    fn ribbon_collapses_with_ctrl_f1_and_the_chevron() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::F1);
        harness.run();
        assert!(harness.state().session().view().ribbon_collapsed);
        assert!(harness.query_by_label("Paste").is_none());

        // While collapsed, clicking a tab shows its groups until a button is used.
        harness.get_by_label("Home").click();
        harness.run();
        assert!(harness.state().ribbon().peeking());
        harness.get_by_label("Bold").click();
        harness.run();
        assert!(!harness.state().ribbon().peeking());
        assert!(harness
            .state()
            .toasts()
            .texts()
            .any(|t| t == "Coming Soon: Bold"));

        harness.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::F1);
        harness.run();
        assert!(!harness.state().session().view().ribbon_collapsed);
        harness.get_by_label("Collapse the Ribbon").click();
        harness.run();
        assert!(harness.state().session().view().ribbon_collapsed);
    }

    #[test]
    fn key_tips_reach_tabs_and_buttons() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        // Tap Alt on its own.
        harness.event(egui::Event::ModifiersChanged(egui::Modifiers::ALT));
        harness.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        harness.run();
        assert!(harness.state().ribbon().key_tips_showing());

        // N opens Insert; then the code for Pictures presses it.
        harness.key_press(egui::Key::N);
        harness.run();
        assert_eq!(harness.state().ribbon().selected(), "Insert");
        let insert = crate::ribbon::spec::TABS
            .iter()
            .position(|tab| tab.name == "Insert")
            .unwrap();
        let tips = crate::ribbon::keytips::tips_for_tab(insert);
        let code = tips
            .iter()
            .find(|(button, _)| button.label == "Pictures")
            .map(|(_, code)| code.clone())
            .unwrap();
        for c in code.chars() {
            harness.key_press(egui::Key::from_name(&c.to_string()).unwrap());
        }
        harness.run();
        assert!(!harness.state().ribbon().key_tips_showing());
        assert!(harness
            .state()
            .toasts()
            .texts()
            .any(|t| t == "Coming Soon: Pictures"));

        // F10, then F, opens the backstage; Esc closes it.
        harness.key_press(egui::Key::F10);
        harness.key_press(egui::Key::F);
        harness.run();
        assert!(harness.state().ribbon().backstage_open());
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(!harness.state().ribbon().backstage_open());
    }

    fn selected_ids(shell: &AppShell) -> (String, String, String) {
        let s = shell.session().selection();
        let id = |v: &Option<String>| v.clone().unwrap_or_default();
        (id(&s.notebook), id(&s.section), id(&s.page))
    }

    #[test]
    fn navigation_selects_notebooks_sections_and_pages() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let ids = selected_ids(harness.state());
        assert_eq!(
            ids,
            ("nb-personal".into(), "s-quick".into(), "p-welcome".into())
        );

        harness.get_by_label("Work Notebook").click();
        harness.run();
        let ids = selected_ids(harness.state());
        assert_eq!(
            ids,
            ("nb-work".into(), "s-meetings".into(), "p-sync".into())
        );

        harness.get_by_label("Action items").click();
        harness.run();
        assert_eq!(selected_ids(harness.state()).2, "p-actions");

        // Research shows as a section tab and in the notebook pane; either selects it.
        harness.get_all_by_label("Research").next().unwrap().click();
        harness.run();
        let ids = selected_ids(harness.state());
        assert_eq!(
            ids,
            ("nb-work".into(), "s-research".into(), "p-reading".into())
        );
        let _ = harness.get_by_label("Interview notes");
        assert!(harness.state().toasts().is_empty());
    }

    #[test]
    fn add_page_and_new_section_are_coming_soon() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Add Page").click();
        harness.run();
        harness.get_by_label("New Section").click();
        harness.run();
        let texts: Vec<&str> = harness.state().toasts().texts().collect();
        assert_eq!(texts, ["Coming Soon: Add Page", "Coming Soon: New Section"]);
    }

    #[test]
    fn section_groups_collapse_in_the_notebook_pane() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        // Old Projects is a section tab and, inside the expanded Archive group, a pane row.
        assert_eq!(harness.get_all_by_label("Old Projects").count(), 2);
        harness.get_by_label("Archive").click();
        harness.run();
        assert_eq!(harness.get_all_by_label("Old Projects").count(), 1);
    }

    #[test]
    fn section_tabs_overflow_into_a_menu() {
        let mut harness = harness(AppShell::new(), egui::vec2(640.0, 600.0));
        harness.run();
        let _ = harness.get_by_label("More Sections");
    }

    #[test]
    fn page_list_side_follows_the_preset() {
        assert!(!nav::pages_on_left(Preset::OneNote2016));
        assert!(!nav::pages_on_left(Preset::OneNote2019));
        assert!(nav::pages_on_left(Preset::OneNote2021));
        assert!(nav::pages_on_left(Preset::M365));
        for preset in Preset::ALL {
            let mut shell = AppShell::new();
            shell.set_theme(preset, Mode::Light);
            let mut harness = harness(shell, WINDOW);
            harness.run();
            let _ = harness.get_by_label("Grocery list");
        }
    }

    fn toasts(harness: &egui_kittest::Harness<'_, AppShell>) -> Vec<String> {
        harness
            .state()
            .toasts()
            .texts()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn color_picker_swatches_and_custom_color_run_the_command() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Draw").click();
        harness.run();

        harness.get_by_label("Color").click();
        harness.run();
        harness.get_by_label("#C62828").click();
        harness.run();
        assert_eq!(toasts(&harness), ["Coming Soon: Color"]);
        assert!(harness.query_by_label("#C62828").is_none(), "picker closes");

        // More Colors opens a full picker; Apply runs the command with the custom color.
        harness.get_by_label("Color").click();
        harness.run();
        harness.get_by_label("More Colors").click();
        harness.run();
        harness.get_by_label("Apply").click();
        harness.run();
        assert!(
            harness.query_by_label("More Colors").is_none(),
            "picker closes"
        );
    }

    #[test]
    fn custom_color_takes_rgba_text_and_number_fields() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Draw").click();
        harness.run();
        harness.get_by_label("Color").click();
        harness.run();
        harness.get_by_label("More Colors").click();
        harness.run();

        // Replace the field's text with an rgba() color: the editor shows it as hex.
        let field = harness.get_by_role(egui::accesskit::Role::TextInput);
        field.focus();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("rgba(255, 0, 0, 0.5)");
        harness.run();
        let _ = harness.get_by_label("#FF000080");

        // Text that is not a color is flagged and Apply is disabled until it is fixed.
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("not a color");
        harness.run();
        let _ = harness.get_by_label("Not a color: use #RRGGBB, #RRGGBBAA, rgb(), or rgba()");
        harness.get_by_label("Apply").click();
        harness.run();
        assert!(
            harness.query_by_label("More Colors").is_some(),
            "Apply does nothing while the text is not a color"
        );
    }

    #[test]
    fn page_color_offers_no_color_and_a_grid() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("View").click();
        harness.run();
        harness.get_by_label("Page Color").click();
        harness.run();
        let _ = harness.get_by_label("#DCEBF7");
        harness.get_by_label("No Color").click();
        harness.run();
        assert_eq!(toasts(&harness), ["Coming Soon: Page Color"]);
    }

    #[test]
    fn table_grid_and_width_previews_open() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Insert").click();
        harness.run();
        harness.get_by_label("Table").click();
        harness.run();
        let _ = harness.get_by_label("Table size");
        harness.get_by_label("Insert Table").click();
        harness.run();
        assert_eq!(toasts(&harness), ["Coming Soon: Table"]);

        harness.get_by_label("Draw").click();
        harness.run();
        harness.get_by_label("Thickness").click();
        harness.run();
        harness.get_by_label("2 pt").click();
        harness.run();
        assert!(toasts(&harness)
            .iter()
            .any(|t| t == "Coming Soon: Thickness"));
    }

    #[test]
    fn file_opens_the_backstage_and_back_closes_it() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("File").click();
        harness.run();
        assert!(harness.state().ribbon().backstage_open());
        harness.get_by_label("Export").click();
        harness.run();
        assert!(harness
            .state()
            .toasts()
            .texts()
            .any(|t| t == "Coming Soon: Export"));
        harness.get_by_label("Back").click();
        harness.run();
        assert!(!harness.state().ribbon().backstage_open());
        let _ = harness.get_by_label("Canvas");
    }

    #[test]
    fn every_preset_draws_the_ribbon() {
        for preset in Preset::ALL {
            for mode in [Mode::Light, Mode::Dark] {
                let mut shell = AppShell::new();
                shell.set_theme(preset, mode);
                let mut harness = harness(shell, WINDOW);
                harness.run();
                let _ = harness.get_by_label("Paste");
                let _ = harness.get_by_label("Meeting Details");
            }
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
    fn theme_commands_switch_the_theme_live() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.input_mut().system_theme = Some(egui::Theme::Light);
        harness.run();
        assert_eq!(harness.ctx.theme(), egui::Theme::Light);

        let dark = serde_json::json!({ "mode": "dark" });
        harness
            .state_mut()
            .dispatch(ids::VIEW_MODE_SET.as_str(), dark);
        harness.run();
        assert_eq!(harness.ctx.theme(), egui::Theme::Dark);

        harness
            .state_mut()
            .dispatch(ids::VIEW_MODE_TOGGLE.as_str(), Value::Null);
        let preset = serde_json::json!({ "preset": "onenote2016" });
        harness
            .state_mut()
            .dispatch(ids::VIEW_THEME_SET.as_str(), preset);
        harness.run();
        assert_eq!(harness.ctx.theme(), egui::Theme::Light);
        let view = harness.state().session().view();
        assert_eq!(view.theme_preset, Preset::OneNote2016);
        assert!(
            harness.state().toasts().is_empty(),
            "ready commands show no toast"
        );
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
