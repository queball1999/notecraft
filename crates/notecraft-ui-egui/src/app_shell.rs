//! The window's top-level layout. PR 0.1 lays out empty, labeled regions where the ribbon,
//! navigation, canvas, and status bar land in the next Phase 0 PRs (build_plan/02). PR 0.2 adds
//! command dispatch and toasts; PR 0.3 installs the theme; PR 0.5 adds the ribbon; PR 0.6 adds
//! the notebook pane, section tabs, and page list on mock notebooks; PR 0.7 adds the canvas.

use notecraft_engine::cmd::{ids, CommandError, CommandId, Map, Registry, Value};
use notecraft_engine::Session;

use craft_ui::buttons::IconButton;

use crate::canvas::{self, CanvasUi};
use crate::icons::{self, Icon, IconColors};
use crate::nav::{self, NavUi};
use crate::picker::{FilePicker, NoPicker};
use crate::ribbon::{self, Clicked, Ribbon};
use crate::shortcuts::Shortcuts;
use crate::themes::{self, Mode, OsFonts, Preset, ThemeTokens};
use crate::toast::{self, Toasts};
use crate::{start, status};

/// Height of the ribbon region (tab strip and body), in points.
pub const RIBBON_HEIGHT: f32 = ribbon::HEIGHT;
/// Height of the status bar, in points.
pub const STATUS_HEIGHT: f32 = 24.0;
/// Default width of the notebook and page panes, in points.
pub const SIDE_WIDTH: f32 = 220.0;

/// Labels of the main regions with mock notebooks open, in drawing order. UI tests look them
/// up by name.
pub const REGIONS: [&str; 4] = ["Zoom 100%", "Notebooks", "Add Page", "Canvas"];

/// Give the panel resize handles a role and a name. egui leaves them unlabeled, and they are
/// stops in the keyboard Tab order, so a screen reader would otherwise announce nothing.
fn name_splitters(ctx: &egui::Context) {
    for (panel, name) in [
        ("notebooks", "Notebook pane width"),
        ("pages", "Page list width"),
    ] {
        let id = egui::Id::new(panel).with("__resize");
        ctx.accesskit_node_builder(id, |node| {
            node.set_role(egui::accesskit::Role::Splitter);
            node.set_label(name);
        });
    }
}

/// File commands that open something, and close the File backstage when they do.
const FILE_OPENS: [CommandId; 3] = [
    ids::FILE_NEW_NOTEBOOK,
    ids::FILE_OPEN_NOTEBOOK,
    ids::FILE_OPEN,
];

/// Everything inside the main window.
#[derive(Debug)]
pub struct AppShell {
    registry: Registry,
    session: Session,
    toasts: Toasts,
    ribbon: Ribbon,
    nav: NavUi,
    canvas: CanvasUi,
    /// Asked for a path when a file command arrives without one.
    picker: Box<dyn FilePicker>,
    shortcuts: Shortcuts,
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
    /// A shell with the mock notebooks open, as after opening a notebook.
    pub fn new() -> Self {
        let mut session = Session::new();
        session.set_notebooks(notecraft_engine::mock::notebooks());
        Self::with_session(session)
    }

    /// A shell with nothing open: it shows the start page.
    pub fn empty() -> Self {
        Self::with_session(Session::new())
    }

    fn with_session(session: Session) -> Self {
        Self {
            registry: Registry::builtin(),
            session,
            toasts: Toasts::new(),
            ribbon: Ribbon::new(),
            nav: NavUi::default(),
            canvas: CanvasUi::default(),
            picker: Box::new(NoPicker),
            shortcuts: Shortcuts::new(),
            installed: None,
            fonts: OsFonts::default(),
            installed_font: None,
        }
    }

    /// Ask `picker` for paths (the desktop app passes the OS dialogs).
    pub fn with_picker(mut self, picker: Box<dyn FilePicker>) -> Self {
        self.picker = picker;
        self
    }

    /// Restore the recent list without a command, from saved settings.
    pub fn set_recents(&mut self, recents: notecraft_engine::recent::Recents) {
        self.session.set_recents(recents);
    }

    /// Reopen the newest recent notebook or page, as OneNote reopens what was open at the
    /// last exit. Returns whether something opened.
    pub fn reopen_last(&mut self) -> bool {
        let Some(item) = self.session.recents().items().first() else {
            return false;
        };
        let open = start::reopen(item);
        self.dispatch(open.command.as_str(), open.args)
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

    pub fn canvas(&self) -> &CanvasUi {
        &self.canvas
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
    /// run (Coming Soon, unknown, bad arguments, or caught panic) shows a toast instead. A
    /// file command without a path asks the picker first; canceling runs nothing. Returns
    /// whether the command ran.
    pub fn dispatch(&mut self, id: &str, args: Value) -> bool {
        let Some(args) = self.fill_path(id, args) else {
            return false;
        };
        match self.registry.run(&mut self.session, id, args) {
            Ok(_) => {
                // Opening something from the File backstage returns to the notes.
                if FILE_OPENS.iter().any(|open| open.as_str() == id) {
                    self.ribbon.close_backstage();
                }
                true
            }
            Err(err) => {
                match err {
                    CommandError::ComingSoon { .. } => tracing::debug!("{err}"),
                    _ => tracing::warn!("{err}"),
                }
                self.toasts.push(toast::error_text(&self.registry, &err));
                false
            }
        }
    }

    /// `args` with a `path` from the picker if `id` is a file command sent without one.
    /// `None` when the user canceled the picker.
    fn fill_path(&mut self, id: &str, args: Value) -> Option<Value> {
        let mut map = match args {
            Value::Null => Map::new(),
            Value::Object(map) => map,
            other => return Some(other),
        };
        if map.contains_key("path") {
            return Some(Value::Object(map));
        }
        let path = if id == ids::FILE_NEW_NOTEBOOK.as_str() {
            self.picker
                .pick_folder("Choose a Folder for the New Notebook")
        } else if id == ids::FILE_OPEN_NOTEBOOK.as_str() {
            self.picker.pick_folder("Open Notebook")
        } else if id == ids::FILE_OPEN.as_str() {
            self.picker.pick_file("Open File")
        } else {
            return Some(Value::Object(map));
        };
        map.insert("path".to_owned(), Value::from(path?));
        Some(Value::Object(map))
    }

    /// Per-frame work that must run even when the window is hidden (eframe `logic`).
    pub fn logic(&mut self, ctx: &egui::Context) {
        // Ctrl+=, Ctrl+-, and Ctrl+0 belong to OneNote's shortcuts, not to egui's UI scaling.
        if ctx.options(|options| options.zoom_with_keyboard) {
            ctx.options_mut(|options| options.zoom_with_keyboard = false);
        }
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
        let pressed = self.shortcuts.pressed(ui.ctx());
        self.dispatch_all(pressed);
        let look = ribbon::Look {
            tokens: &tokens,
            registry: &self.registry,
        };
        if self.ribbon.backstage_open() {
            let clicked = self.ribbon.show_backstage(ui, look, self.session.recents());
            self.dispatch_all(clicked);
            self.toasts.show(ui.ctx());
            return;
        }
        if self.session.view().full_page && !self.session.notebooks().is_empty() {
            self.full_page_ui(ui, &tokens);
            return;
        }

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

        let session = &self.session;
        egui::Panel::bottom("status_bar")
            .exact_size(STATUS_HEIGHT)
            .frame(panel.fill(tokens.ribbon_bg))
            .show(ui, |ui| status::show(ui, session, &tokens, &mut clicked));
        if session.notebooks().is_empty() {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(tokens.canvas_bg))
                .show(ui, |ui| {
                    start::show(ui, session.recents(), &tokens, &mut clicked);
                });
            self.dispatch_all(clicked);
            self.toasts.show(ui.ctx());
            return;
        }
        let nav_ui = &mut self.nav;
        let canvas_ui = &mut self.canvas;
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
        let canvas_width = egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(tokens.canvas_bg))
            .show(ui, |ui| {
                let full = ui.max_rect();
                let strip = egui::Rect::from_min_size(
                    full.min,
                    egui::vec2(full.width(), nav::SECTION_STRIP_HEIGHT),
                );
                nav::section_tabs(ui, strip, session, &tokens, &mut clicked);
                let page = egui::Rect::from_min_max(egui::pos2(full.min.x, strip.max.y), full.max);
                canvas::show(ui, page, session, canvas_ui, &tokens, &mut clicked)
            })
            .inner;
        name_splitters(ui.ctx());
        // Like the OS dark mode, the canvas width is reported, not commanded.
        self.session.view_mut().canvas_width = canvas_width.max(0.0) as u32;
        self.dispatch_all(clicked);
        self.toasts.show(ui.ctx());
    }

    /// Full Page View: only the page, with a button at the top right (and Esc or F11) to go
    /// back to the normal view.
    fn full_page_ui(&mut self, ui: &mut egui::Ui, tokens: &ThemeTokens) {
        let mut clicked = Vec::new();
        let leave =
            ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let session = &self.session;
        let canvas_ui = &mut self.canvas;
        let canvas_width = egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(tokens.canvas_bg))
            .show(ui, |ui| {
                let page = ui.max_rect();
                let width = canvas::show(ui, page, session, canvas_ui, tokens, &mut clicked);
                let size = egui::vec2(32.0, 32.0);
                // Left of the vertical scroll bar.
                let at = egui::pos2(page.max.x - size.x - 20.0, page.min.y + 8.0);
                let rect = egui::Rect::from_min_size(at, size);
                let style = ribbon::button_style(tokens);
                let exit = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        IconButton::new("Normal View", size, &style).show(ui, |ui, rect, color| {
                            let colors = IconColors {
                                fg: color,
                                accent: tokens.accent,
                            };
                            icons::paint(ui.painter(), rect.shrink(6.0), Icon::NormalView, colors);
                        })
                    })
                    .inner;
                if exit
                    .on_hover_text("Exit Full Page View (F11 or Esc)")
                    .clicked()
                    || leave
                {
                    clicked.push(Clicked {
                        command: ids::VIEW_NORMAL,
                        args: Value::Null,
                    });
                }
                width
            })
            .inner;
        self.session.view_mut().canvas_width = canvas_width.max(0.0) as u32;
        self.dispatch_all(clicked);
        self.toasts.show(ui.ctx());
    }

    fn dispatch_all(&mut self, clicked: Vec<Clicked>) {
        for Clicked { command, args } in clicked {
            self.dispatch(command.as_str(), args);
        }
    }
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
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F1);
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

        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F1);
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
        harness.get_by_label("#DCEBF7").click();
        harness.run();
        let style = harness.state().session().page_style();
        assert_eq!(style.color, Some([0xDC, 0xEB, 0xF7, 0xFF]));
        harness.get_by_label("Page Color").click();
        harness.run();
        harness.get_by_label("No Color").click();
        harness.run();
        assert_eq!(harness.state().session().page_style().color, None);
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn every_page_setup_and_zoom_menu_item_runs() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("View").click();
        harness.run();
        let menus = [
            ("Rule Lines", crate::ribbon::spec::RULE_LINES),
            ("Paper Size", crate::ribbon::spec::PAPER_SIZES),
            ("Zoom", crate::ribbon::spec::ZOOM_LEVELS),
        ];
        for (button, items) in menus {
            for item in items {
                harness.get_by_label(button).click();
                harness.run();
                // The 100% item shares its label with the 100% button; the menu is drawn last.
                let found = harness.get_all_by_label(item.label).last();
                found.unwrap().click();
                harness.run();
                assert!(toasts(&harness).is_empty(), "{button}: {}", item.label);
            }
        }
        let view = harness.state().session();
        assert_eq!(view.view().zoom_percent, 200);
        assert_eq!(view.page_style().paper_size.id(), "a5");
        assert_eq!(view.page_style().rule_lines.id(), "grid_xlarge");
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

    /// A point well inside the canvas, and the page point under it.
    fn canvas_point(harness: &egui_kittest::Harness<'_, AppShell>) -> egui::Pos2 {
        harness.get_by_label("Canvas").rect().min + egui::vec2(300.0, 200.0)
    }

    fn origin(harness: &egui_kittest::Harness<'_, AppShell>) -> egui::Vec2 {
        let origin = harness.state().canvas().viewport().origin();
        egui::vec2(origin.x, origin.y)
    }

    #[test]
    fn zoom_buttons_zoom_the_canvas() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("View").click();
        harness.run();
        harness.get_by_label("Zoom In").click();
        harness.run();
        assert_eq!(harness.state().session().view().zoom_percent, 125);
        assert_eq!(harness.state().canvas().viewport().percent(), 125);
        harness.get_by_label("Zoom Out").click();
        harness.get_by_label("Zoom Out").click();
        harness.run();
        assert_eq!(harness.state().canvas().viewport().percent(), 75);
        harness.get_by_label("Page Width").click();
        harness.run();
        let width = harness.state().session().view().canvas_width;
        let expected = notecraft_canvas::viewport::fit_width_percent(816.0, width as f32);
        assert!(width > 300, "the canvas reports its width: {width}");
        assert_eq!(harness.state().canvas().viewport().percent(), expected);
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn ctrl_wheel_and_pinch_zoom_about_the_pointer() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let pointer = canvas_point(&harness);
        harness.hover_at(pointer);
        harness.run();
        let before = harness.state().canvas().viewport();
        let rect_min = harness.get_by_label("Canvas").rect().min;
        let anchor = pointer - rect_min;
        let page = before.screen_to_page(notecraft_core::Vec2::new(anchor.x, anchor.y));

        harness.event(egui::Event::Zoom(2.0));
        harness.run();
        harness.run();
        assert_eq!(harness.state().session().view().zoom_percent, 200);
        let after = harness.state().canvas().viewport();
        assert_eq!(after.percent(), 200);
        let moved = after.screen_to_page(notecraft_core::Vec2::new(anchor.x, anchor.y));
        assert!(
            (moved.x - page.x).abs() < 1.0 && (moved.y - page.y).abs() < 1.0,
            "{page:?} drifted to {moved:?}"
        );

        // Ctrl+wheel down zooms out.
        harness.event_modifiers(
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, -3.0),
                modifiers: egui::Modifiers::CTRL,
                phase: egui::TouchPhase::Move,
            },
            egui::Modifiers::CTRL,
        );
        harness.run();
        assert!(harness.state().session().view().zoom_percent < 200);
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn middle_drag_and_space_drag_pan_the_canvas() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let start = canvas_point(&harness);
        assert_eq!(origin(&harness), egui::Vec2::ZERO);
        let press = |pos, button, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let end = start - egui::vec2(120.0, 80.0);
        harness.event(press(start, egui::PointerButton::Middle, true));
        harness.run();
        for step in 1..=4 {
            harness.hover_at(start - egui::vec2(30.0, 20.0) * step as f32);
            harness.run();
        }
        harness.event(press(end, egui::PointerButton::Middle, false));
        harness.run();
        let panned = origin(&harness);
        assert!(
            (panned - egui::vec2(120.0, 80.0)).length() < 2.0,
            "{panned:?}"
        );

        // A primary drag pans only while Space is held.
        harness.drag_at(end);
        harness.hover_at(end - egui::vec2(50.0, 0.0));
        harness.run();
        harness.drop_at(end - egui::vec2(50.0, 0.0));
        harness.run();
        assert_eq!(origin(&harness), panned);
        harness.key_down(egui::Key::Space);
        harness.drag_at(end);
        harness.run();
        harness.hover_at(end - egui::vec2(25.0, 0.0));
        harness.run();
        harness.hover_at(end - egui::vec2(50.0, 0.0));
        harness.run();
        harness.drop_at(end - egui::vec2(50.0, 0.0));
        harness.key_up(egui::Key::Space);
        harness.run();
        let spaced = origin(&harness);
        assert!((spaced.x - panned.x - 50.0).abs() < 2.0, "{spaced:?}");
    }

    #[test]
    fn wheel_scrolls_the_canvas() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.hover_at(canvas_point(&harness));
        harness.run();
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -200.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        harness.run_steps(30);
        assert!(origin(&harness).y > 50.0, "{:?}", origin(&harness));
        assert_eq!(harness.state().session().view().zoom_percent, 100);
    }

    /// A picker that answers with fixed paths, or cancels when they are `None`.
    #[derive(Debug, Default)]
    struct FakePicker {
        folder: Option<String>,
        file: Option<String>,
    }

    impl FilePicker for FakePicker {
        fn pick_folder(&mut self, _title: &str) -> Option<String> {
            self.folder.clone()
        }

        fn pick_file(&mut self, _title: &str) -> Option<String> {
            self.file.clone()
        }
    }

    fn picking(folder: Option<&str>, file: Option<&str>) -> AppShell {
        AppShell::empty().with_picker(Box::new(FakePicker {
            folder: folder.map(str::to_owned),
            file: file.map(str::to_owned),
        }))
    }

    fn recent_paths(shell: &AppShell) -> Vec<String> {
        let items = shell.session().recents().items();
        items.iter().map(|item| item.path.clone()).collect()
    }

    #[test]
    fn start_page_shows_while_nothing_is_open() {
        let mut harness = harness(AppShell::empty(), WINDOW);
        harness.run();
        for label in ["New Notebook", "Open Notebook", "Open File", "Recent"] {
            let _ = harness.get_by_label(label);
        }
        assert!(harness.query_by_label("Canvas").is_none());
        assert!(harness.query_by_label("Notebooks").is_none());
        let _ = harness.get_by_label("Notebooks and pages you open show up here.");
    }

    #[test]
    fn open_notebook_asks_the_picker_and_opens() {
        let mut harness = harness(picking(Some("C:\\Notes\\Work"), None), WINDOW);
        harness.run();
        harness.get_by_label("Open Notebook").click();
        harness.run();
        assert_eq!(harness.state().session().notebooks().len(), 2);
        assert_eq!(recent_paths(harness.state()), ["C:\\Notes\\Work"]);
        let _ = harness.get_by_label("Canvas");
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn canceling_the_picker_does_nothing() {
        let mut harness = harness(AppShell::empty(), WINDOW);
        harness.run();
        for label in ["New Notebook", "Open Notebook", "Open File"] {
            harness.get_by_label(label).click();
            harness.run();
        }
        assert!(harness.state().session().notebooks().is_empty());
        assert!(recent_paths(harness.state()).is_empty());
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn a_wrong_file_type_shows_a_toast() {
        let mut harness = harness(picking(None, Some("/notes/todo.txt")), WINDOW);
        harness.run();
        harness.get_by_label("Open File").click();
        harness.run();
        assert!(harness.state().session().notebooks().is_empty());
        assert_eq!(toasts(&harness).len(), 1);
    }

    #[test]
    fn recent_rows_reopen_without_the_picker() {
        let mut shell = AppShell::empty();
        let recents = notecraft_engine::recent::Recents::from_items([
            (
                notecraft_engine::recent::RecentKind::Page,
                "/n/Plans.md".to_owned(),
            ),
            (
                notecraft_engine::recent::RecentKind::Notebook,
                "/n/Work".to_owned(),
            ),
        ]);
        shell.set_recents(recents);
        let mut harness = harness(shell, WINDOW);
        harness.run();
        harness.get_by_label("Work").click();
        harness.run();
        assert_eq!(harness.state().session().notebooks().len(), 2);
        assert_eq!(recent_paths(harness.state()), ["/n/Work", "/n/Plans.md"]);
    }

    #[test]
    fn backstage_open_page_lists_recents_and_opens_files() {
        let mut shell = AppShell::new().with_picker(Box::new(FakePicker {
            folder: None,
            file: Some("/n/Ideas.notecraft".to_owned()),
        }));
        shell.set_recents(notecraft_engine::recent::Recents::from_items([(
            notecraft_engine::recent::RecentKind::Notebook,
            "/n/Work".to_owned(),
        )]));
        let mut harness = harness(shell, WINDOW);
        harness.run();
        harness.get_by_label("File").click();
        harness.run();
        harness.get_by_label("Open").click();
        harness.run();
        assert!(toasts(&harness).is_empty(), "choosing Open runs nothing");
        let _ = harness.get_by_label("Work");
        harness.get_by_label("Open File").click();
        harness.run();
        assert!(
            !harness.state().ribbon().backstage_open(),
            "opening returns to the notes"
        );
        assert_eq!(
            recent_paths(harness.state()),
            ["/n/Ideas.notecraft", "/n/Work"]
        );

        harness.get_by_label("File").click();
        harness.run();
        harness.get_by_label("Open").click();
        harness.run();
        harness.get_by_label("Clear Recent").click();
        harness.run();
        assert!(recent_paths(harness.state()).is_empty());
        let _ = harness.get_by_label("Notebooks and pages you open show up here.");
    }

    #[test]
    fn backstage_new_page_asks_for_a_folder() {
        let shell = AppShell::new().with_picker(Box::new(FakePicker {
            folder: Some("/n/Fresh".to_owned()),
            file: None,
        }));
        let mut harness = harness(shell, WINDOW);
        harness.run();
        harness.get_by_label("File").click();
        harness.run();
        harness.get_by_label("New").click();
        harness.run();
        assert!(
            recent_paths(harness.state()).is_empty(),
            "choosing New runs nothing"
        );
        harness.get_by_label("Choose a Folder").click();
        harness.run();
        assert_eq!(recent_paths(harness.state()), ["/n/Fresh"]);
        assert!(!harness.state().ribbon().backstage_open());
    }

    #[test]
    fn reopen_last_opens_the_newest_entry_without_reordering() {
        let mut shell = AppShell::empty();
        assert!(!shell.reopen_last(), "nothing to reopen");
        shell.set_recents(notecraft_engine::recent::Recents::from_items([
            (
                notecraft_engine::recent::RecentKind::Page,
                "/n/Plans.md".to_owned(),
            ),
            (
                notecraft_engine::recent::RecentKind::Notebook,
                "/n/Work".to_owned(),
            ),
        ]));
        assert!(shell.reopen_last());
        assert_eq!(shell.session().notebooks().len(), 2);
        assert_eq!(recent_paths(&shell), ["/n/Plans.md", "/n/Work"]);
    }

    #[test]
    fn shortcuts_run_their_commands() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let command = egui::Modifiers::COMMAND;
        harness.key_press_modifiers(command, egui::Key::N);
        harness.run();
        harness.key_press_modifiers(command, egui::Key::Num3);
        harness.run();
        assert_eq!(
            toasts(&harness),
            ["Coming Soon: Add Page", "Coming Soon: Tags"]
        );

        // Ctrl+Alt+= zooms in; the more specific chord wins over Ctrl+=.
        harness.key_press_modifiers(command | egui::Modifiers::ALT, egui::Key::Equals);
        harness.run();
        assert_eq!(harness.state().session().view().zoom_percent, 125);
        assert!(!toasts(&harness).iter().any(|t| t.contains("Zoom")));
    }

    #[test]
    fn shortcuts_wait_while_a_text_field_has_focus() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Draw").click();
        harness.run();
        harness.get_by_label("Color").click();
        harness.run();
        harness.get_by_label("More Colors").click();
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .focus();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::N);
        harness.run();
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn full_page_view_shows_only_the_page() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let shows_chrome = |harness: &egui_kittest::Harness<'_, AppShell>| {
            harness.query_by_label("Home").is_some()
                && harness.query_by_label("Notebooks").is_some()
                && harness.query_all_by_label("Quick Notes").next().is_some()
        };
        assert!(shows_chrome(&harness));

        // F11 in, F11 out.
        harness.key_press(egui::Key::F11);
        harness.run();
        assert!(harness.state().session().view().full_page);
        assert!(harness.query_by_label("Home").is_none());
        assert!(harness.query_by_label("Notebooks").is_none());
        assert!(harness.query_all_by_label("Quick Notes").next().is_none());
        let _ = harness.get_by_label("Canvas");
        harness.key_press(egui::Key::F11);
        harness.run();
        assert!(shows_chrome(&harness));

        // The View tab button in, Esc out.
        harness.get_by_label("View").click();
        harness.run();
        harness.get_by_label("Full Page View").click();
        harness.run();
        assert!(harness.state().session().view().full_page);
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(!harness.state().session().view().full_page);

        // The corner button out.
        harness.key_press(egui::Key::F11);
        harness.run();
        harness.get_by_label("Normal View").click();
        harness.run();
        assert!(shows_chrome(&harness));
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn status_bar_shows_the_page_and_zoom_menu() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let position = status::page_position(harness.state().session()).unwrap();
        let _ = harness.get_by_label(&position);
        harness.get_by_label("Grocery list").click();
        harness.run();
        let _ = harness.get_by_label("Page 2 of 5");

        harness.get_by_label("Zoom 100%").click();
        harness.run();
        harness.get_by_label("Zoom In").click();
        harness.run();
        assert_eq!(harness.state().session().view().zoom_percent, 125);
        harness.get_by_label("Zoom 125%").click();
        harness.run();
        harness.get_by_label("200%").click();
        harness.run();
        let _ = harness.get_by_label("Zoom 200%");
        assert_eq!(harness.state().canvas().viewport().percent(), 200);
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn status_bar_is_empty_on_the_start_page() {
        let mut harness = harness(AppShell::empty(), WINDOW);
        harness.run();
        assert!(harness.query_by_label("Zoom 100%").is_none());
    }

    /// Roles a user can act on; each must have a name for screen readers.
    const ACTIONABLE: [egui::accesskit::Role; 14] = {
        use egui::accesskit::Role;
        [
            Role::Button,
            Role::Tab,
            Role::CheckBox,
            Role::RadioButton,
            Role::MenuItem,
            Role::ComboBox,
            Role::TextInput,
            Role::Slider,
            Role::SpinButton,
            Role::Link,
            Role::TreeItem,
            Role::ListBoxOption,
            Role::Splitter,
            Role::Unknown,
        ]
    };

    /// Every actionable node without a label or value, as "role at rect".
    fn unnamed(harness: &egui_kittest::Harness<'_, AppShell>) -> Vec<String> {
        use egui_kittest::kittest::{By, NodeT};
        harness
            .query_all(By::new().predicate(|node| ACTIONABLE.contains(&node.role())))
            .filter_map(|node| {
                let ak = node.accesskit_node();
                let named = ak.label().is_some_and(|l| !l.trim().is_empty())
                    || ak.value().is_some_and(|v| !v.trim().is_empty());
                (!named).then(|| format!("{:?} at {:?}", ak.role(), node.rect()))
            })
            .collect()
    }

    #[test]
    fn every_control_has_a_screen_reader_name() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        for tab in ["Home", "Insert", "Draw", "History", "Review", "View"] {
            harness.get_by_label(tab).click();
            harness.run();
            assert_eq!(unnamed(&harness), Vec::<String>::new(), "{tab} tab");
        }
        harness.get_by_label("File").click();
        harness.run();
        for entry in ["Info", "New", "Open"] {
            // The entry comes first; the open page repeats its name as a heading.
            harness.get_all_by_label(entry).next().unwrap().click();
            harness.run();
            assert_eq!(unnamed(&harness), Vec::<String>::new(), "backstage {entry}");
        }
        harness.key_press(egui::Key::Escape);
        harness.key_press(egui::Key::F11);
        harness.run();
        assert_eq!(unnamed(&harness), Vec::<String>::new(), "full page view");

        let mut start = harness_for(AppShell::empty());
        start.run();
        assert_eq!(unnamed(&start), Vec::<String>::new(), "start page");
    }

    fn harness_for(shell: AppShell) -> egui_kittest::Harness<'static, AppShell> {
        harness(shell, WINDOW)
    }

    #[test]
    fn survives_a_tiny_window() {
        let mut harness = harness(AppShell::new(), egui::vec2(1.0, 1.0));

        harness.run();
    }
}
