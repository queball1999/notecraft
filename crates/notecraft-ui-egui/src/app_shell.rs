//! The window's top-level layout. PR 0.1 lays out empty, labeled regions where the ribbon,
//! navigation, canvas, and status bar land in the next Phase 0 PRs (build_plan/02). PR 0.2 adds
//! command dispatch and toasts; PR 0.3 installs the theme; PR 0.5 adds the ribbon; PR 0.6 adds
//! the notebook pane, section tabs, and page list on mock notebooks; PR 0.7 adds the canvas.

use notecraft_engine::cmd::{ids, CommandError, CommandId, CommandOutput, Map, Registry, Value};
use notecraft_engine::view::{RibbonLayout, RibbonShow};
use notecraft_engine::Session;

use craft_ui::buttons::IconButton;

use crate::about::AppInfo;
use crate::canvas::{self, CanvasUi};
use crate::font_list::FontList;
use crate::icons::{self, Icon, IconColors};
use crate::nav::{self, NavUi};
use crate::picker::{FilePicker, NoPicker};
use crate::ribbon::{self, Clicked, Ribbon};
use crate::shortcuts::Shortcuts;
use crate::themes::{self, Mode, OsFonts, Preset, ThemeTokens};
use crate::toast::{self, Toasts};
use crate::{bars, start, status};
/// Height of the status bar, in points.
pub const STATUS_HEIGHT: f32 = 24.0;
/// Default width of the notebook and page panes, in points.
pub const SIDE_WIDTH: f32 = 220.0;

/// Labels of the main regions with mock notebooks open, in drawing order. UI tests look them
/// up by name.
pub const REGIONS: [&str; 4] = ["Zoom 100%", "Notebooks", "Add Page", "Canvas"];

/// The panel resize handles: their egui ids and accessible names.
const SPLITTERS: [(&str, &str); 2] = [
    ("notebooks", "Notebook pane width"),
    ("pages", "Page list width"),
];

/// Give the panel resize handles a role and a name, and a focus ring. egui leaves them
/// unlabeled and draws nothing when they have keyboard focus, yet they are stops in the Tab
/// order, so a screen reader would announce nothing and a keyboard user would lose the focus.
fn finish_splitters(ui: &egui::Ui, tokens: &ThemeTokens) {
    let ctx = ui.ctx();
    for (panel, name) in SPLITTERS {
        let id = egui::Id::new(panel).with("__resize");
        ctx.accesskit_node_builder(id, |node| {
            node.set_role(egui::accesskit::Role::Splitter);
            node.set_label(name);
        });
        if ctx.memory(|memory| memory.has_focus(id)) {
            if let Some(handle) = ctx.read_response(id) {
                let stroke = egui::Stroke::new(1.5, tokens.accent);
                ui.painter()
                    .rect_stroke(handle.rect, 0.0, stroke, egui::StrokeKind::Inside);
            }
        }
    }
}

/// File commands that open something, and close the File backstage when they do.
const FILE_OPENS: [CommandId; 3] = [
    ids::FILE_NEW_NOTEBOOK,
    ids::FILE_OPEN_NOTEBOOK,
    ids::FILE_OPEN,
];

/// Most files taken from one drop.
const MAX_DROPPED_FILES: usize = 50;
/// How far the insert point moves down and right after each insert or dropped file, in page
/// units, so the objects do not cover each other.
const INSERT_STEP: f32 = 24.0;

/// Whether a file name has a picture extension the canvas draws.
fn is_picture_name(name: &str) -> bool {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    matches!(
        ext.as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "svg")
    )
}

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
    /// The installed font families, filled in by a background scan.
    font_list: FontList,
    /// The families once the scan is done; the font box offers a short list until then.
    families: Vec<String>,
    /// Recent Colors from settings, put into the color menus on the first frame.
    restore_colors: Option<Vec<String>>,
    /// The Recent Colors as of the last frame, for saving.
    recent_colors: Vec<String>,
    /// Build and system details for the About page, from the app.
    about: AppInfo,
    /// The What's New dialog.
    whats_new: crate::whats_new::WhatsNew,
    /// Text for the clipboard, put there on the next frame.
    pending_copy: Option<String>,
    /// The temporary file holding a clipboard picture while Paste as Picture runs.
    paste_temp: Option<String>,
    /// A text paste (`Event::Paste`) came in since the last Ctrl+V key release, so that Ctrl+V
    /// was a text paste.
    paste_seen: bool,
    /// Ids of the commands that ran through [`AppShell::dispatch`], for tests.
    #[cfg(test)]
    ran: Vec<String>,
    /// Auto-save, the recovery journal, and the folder watcher (the desktop app turns them on).
    io: Option<notecraft_engine::io::SessionIo>,
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
        session.set_page_source(
            notecraft_engine::mock::WELCOME_ID,
            notecraft_engine::mock::WELCOME_SOURCE,
        );
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
            paste_temp: None,
            paste_seen: false,
            shortcuts: Shortcuts::new(),
            installed: None,
            fonts: OsFonts::default(),
            installed_font: None,
            font_list: FontList::none(),
            families: Vec::new(),
            restore_colors: None,
            recent_colors: Vec::new(),
            about: AppInfo::default(),
            whats_new: crate::whats_new::WhatsNew::new(),
            pending_copy: None,
            #[cfg(test)]
            ran: Vec::new(),
            io: None,
        }
    }

    /// Turn on auto-save, crash recovery, and outside edit watching. Without this (tests)
    /// pages are read inline and never written.
    pub fn with_io(mut self, io: notecraft_engine::io::SessionIo) -> Self {
        self.io = Some(io);
        self
    }

    /// Save every unsaved page now and journal what could not be saved. Call on quit.
    pub fn flush_unsaved(&mut self) {
        if let Some(io) = &mut self.io {
            io.flush(&mut self.session);
        }
    }

    /// Show `info` on the About page (the app passes its build details).
    pub fn with_about(mut self, info: AppInfo) -> Self {
        self.about = info;
        self
    }

    /// Open the What's New dialog now (the app calls this at the first launch of a version).
    /// The same dialog opens from `help.whats_new`.
    pub fn open_whats_new(&mut self) {
        self.whats_new.open();
    }

    /// Whether the What's New dialog is open.
    pub fn whats_new_open(&self) -> bool {
        self.whats_new.is_open()
    }

    /// The text `file.copy_about_info` copies, for tests.
    pub fn pending_copy(&self) -> Option<&str> {
        self.pending_copy.as_deref()
    }

    /// Restore the Recent Colors of the color menus, from saved settings.
    pub fn set_recent_colors(&mut self, colors: Vec<String>) {
        self.recent_colors.clone_from(&colors);
        self.restore_colors = Some(colors);
    }

    /// The Recent Colors of the color menus, newest first.
    /// Replace the Draw tab's pens and highlighters (restoring saved settings).
    pub fn set_pen_tools(&mut self, tools: Vec<notecraft_engine::PenTool>) {
        self.session.set_pen_tools(tools);
    }

    /// Set the rule style new pages start with, by its `view.rule_lines` id (restoring saved
    /// settings). An unknown id means none.
    pub fn set_default_rule_lines(&mut self, id: &str) {
        let style = notecraft_canvas::RuleLines::from_id(id).unwrap_or_default();
        self.session.set_default_rule_lines(style);
    }

    /// The rule style new pages start with, for saving in the settings (`none` when Always
    /// Create Pages with Rule Lines is off).
    pub fn default_rule_lines(&self) -> &'static str {
        self.session.default_rule_lines().id()
    }

    /// The Draw tab's pens and highlighters, for saving in the settings.
    pub fn pen_tools(&self) -> &[notecraft_engine::PenTool] {
        self.session.ink().tools()
    }

    pub fn recent_colors(&self) -> &[String] {
        &self.recent_colors
    }

    /// List the installed fonts in the font box once `list` is filled.
    pub fn with_font_list(mut self, list: FontList) -> Self {
        self.font_list = list;
        self
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

    /// Open the notebook folder or page file named on the command line, as OneNote does for
    /// a file it is asked to open. A relative path resolves against the current directory.
    /// A folder runs `file.open_notebook`, anything else `file.open`. A path that is missing,
    /// not valid UTF-8, or refused by the command shows an error toast and returns false, so
    /// the caller can fall back to [`Self::reopen_last`].
    pub fn open_path(&mut self, path: &std::path::Path) -> bool {
        let full = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let Some(text) = full.to_str() else {
            tracing::warn!(path = %full.display(), "launch path is not valid UTF-8");
            self.toasts.push(format!(
                "Cannot open {}: the path is not valid text.",
                full.display()
            ));
            return false;
        };
        let id = if full.is_dir() {
            ids::FILE_OPEN_NOTEBOOK
        } else if full.exists() {
            ids::FILE_OPEN
        } else {
            tracing::warn!(path = %full.display(), "launch path does not exist");
            self.toasts.push(format!(
                "Cannot open {}: it does not exist.",
                full.display()
            ));
            return false;
        };
        let mut args = Map::new();
        args.insert("path".to_owned(), Value::String(text.to_owned()));
        self.dispatch(id.as_str(), Value::Object(args))
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

    #[cfg(test)]
    pub(crate) fn screen_point_of(&self, page: notecraft_core::Vec2) -> Option<egui::Pos2> {
        self.canvas.screen_point_of(page)
    }

    #[cfg(test)]
    pub(crate) fn picture_counts(&self) -> (usize, usize) {
        self.canvas.picture_counts()
    }

    /// Show a notice line on page `id`; a read-only notice also stops text editing there.
    pub fn set_page_notice(&mut self, id: &str, text: &str, read_only: bool) {
        self.session.set_page_notice(id, text, read_only);
    }

    /// Give page `id` its markdown source text (the whole file). The canvas shows it in flow
    /// layout when that page is open. The folder reader (PR 1.1) calls this when it loads a page.
    pub fn set_page_source(&mut self, id: &str, text: impl Into<String>) {
        self.session.set_page_source(id, text);
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

    /// Set the ribbon's layout and show mode without a command, for restoring saved settings.
    pub fn set_ribbon(&mut self, layout: RibbonLayout, show: RibbonShow) {
        let view = self.session.view_mut();
        view.ribbon_layout = layout;
        view.ribbon_show = show;
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
        match self.run_command(id, args) {
            Some(Ok(_)) => {
                #[cfg(test)]
                self.ran.push(id.to_owned());
                true
            }
            None => false,
            Some(Err(err)) => {
                match err {
                    CommandError::ComingSoon { .. } => tracing::debug!("{err}"),
                    _ => tracing::warn!("{err}"),
                }
                self.toasts.push(toast::error_text(&self.registry, &err));
                false
            }
        }
    }

    /// The one path every command takes, whether from the ribbon, a shortcut, or the control
    /// channel: pick a path if a file command has none, run it through the registry, and
    /// close the File backstage after an open. `None` when the user canceled the picker.
    /// Unlike [`AppShell::dispatch`] it does not toast, so the caller reports the error.
    pub fn run_command(
        &mut self,
        id: &str,
        args: Value,
    ) -> Option<Result<CommandOutput, CommandError>> {
        let args = self.fill_path(id, args)?;
        let result = self.registry.run(&mut self.session, id, args);
        if let Some(temp) = self.paste_temp.take() {
            // The clipboard picture was copied into the notebook; the temporary file goes.
            let _ = std::fs::remove_file(temp);
        }
        if id == ids::ATTACHMENT_OPEN.as_str() {
            if let Ok(output) = &result {
                let path = output.value.get("path").and_then(Value::as_str);
                if !path.is_some_and(|p| self.picker.open_in_default_app(p)) {
                    self.toasts.push("The file could not be opened.");
                }
            }
        }
        // Opening something from the File backstage returns to the notes.
        if let Ok(output) = &result {
            if FILE_OPENS.iter().any(|open| open.as_str() == id) {
                self.ribbon.close_backstage();
                self.toast_open_warnings(output);
            } else if id == ids::FILE_ABOUT.as_str() {
                self.ribbon.open_backstage_on(ids::FILE_ABOUT);
            } else if id == ids::HELP_WHATS_NEW.as_str() {
                self.whats_new.open();
            } else if id == ids::FILE_COPY_ABOUT_INFO.as_str() {
                self.pending_copy = Some(self.about.plain_text());
                self.toasts.push("Copied the version and build details.");
            }
        }
        Some(result)
    }

    /// Ctrl+V with a picture (and no text) on the clipboard inserts the picture. The window
    /// system layer turns Ctrl+V into a text paste event only when the clipboard holds text and
    /// swallows the key press otherwise, but it still reports the key release. So a release of
    /// V with the command key down, with no text paste since the last one, means the clipboard
    /// had no text: look for a picture and insert it (`edit.paste_picture`). Nothing is said
    /// when there is none, because Ctrl+V on an empty clipboard is not an error.
    fn paste_clipboard_picture(&mut self, ctx: &egui::Context) {
        let events = ctx.input(|input| input.events.clone());
        let mut wanted = false;
        for event in &events {
            match event {
                egui::Event::Paste(_) => self.paste_seen = true,
                egui::Event::Key {
                    key: egui::Key::V,
                    pressed: false,
                    modifiers,
                    ..
                } if modifiers.command => {
                    wanted |= !self.paste_seen;
                    self.paste_seen = false;
                }
                _ => {}
            }
        }
        if !wanted {
            return;
        }
        if let Some(path) = self.picker.clipboard_picture() {
            self.paste_temp = Some(path.clone());
            let mut args = Map::new();
            args.insert("path".to_owned(), Value::from(path));
            self.dispatch(ids::EDIT_PASTE_PICTURE.as_str(), Value::Object(args));
        }
    }

    /// Tell the user when opening a notebook folder skipped something (a folder nested too
    /// deep, a settings file that could not be read). The notebook is open either way.
    fn toast_open_warnings(&mut self, output: &CommandOutput) {
        let Some(warnings) = output.value.get("warnings").and_then(Value::as_array) else {
            return;
        };
        let first = warnings.first().and_then(Value::as_str).unwrap_or_default();
        let more = warnings.len().saturating_sub(1);
        let text = if more == 0 {
            format!("Opened with a warning: {first}")
        } else {
            format!("Opened with {} warnings. First: {first}", warnings.len())
        };
        self.toasts.push(text);
    }

    /// A picture or file inserted without a point goes where the user last clicked on empty
    /// canvas (docs/markdown.md section 3.4). With the caret in a note, or on a page not
    /// clicked yet, it flows with the text instead.
    fn add_insert_point(&mut self, id: &str, args: &mut Map<String, Value>) {
        let places = [
            ids::INSERT_PICTURE,
            ids::INSERT_FILE_ATTACHMENT,
            ids::EDIT_PASTE_PICTURE,
        ];
        if !places.iter().any(|p| p.as_str() == id) || args.contains_key("x") {
            return;
        }
        // With the caret in a note the object goes inline at the caret, as OneNote does.
        if self.session.text_cursor().is_some() {
            return;
        }
        let Some(page) = self.session.selection().page.clone() else {
            return;
        };
        let Some(at) = self.canvas.insert_point(&page) else {
            return;
        };
        args.insert("x".to_owned(), Value::from(f64::from(at.x)));
        args.insert("y".to_owned(), Value::from(f64::from(at.y)));
        self.canvas.step_insert_point(INSERT_STEP);
    }

    /// `args` with a `path` from the picker if `id` is a file command sent without one.
    /// `None` when the user canceled the picker.
    fn fill_path(&mut self, id: &str, args: Value) -> Option<Value> {
        let mut map = match args {
            Value::Null => Map::new(),
            Value::Object(map) => map,
            other => return Some(other),
        };
        self.add_insert_point(id, &mut map);
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
        } else if id == ids::INSERT_PICTURE.as_str() {
            self.picker.pick_picture("Insert Picture")
        } else if id == ids::INSERT_FILE_ATTACHMENT.as_str() {
            self.picker.pick_attachment("Attach File")
        } else if id == ids::EDIT_PASTE_PICTURE.as_str() {
            let path = self.picker.clipboard_picture();
            if path.is_none() {
                self.toasts.push("There is no picture on the clipboard.");
            }
            self.paste_temp = path.clone();
            path
        } else {
            return Some(Value::Object(map));
        };
        map.insert("path".to_owned(), Value::from(path?));
        Some(Value::Object(map))
    }

    /// Per-frame work that must run even when the window is hidden (eframe `logic`).
    pub fn logic(&mut self, ctx: &egui::Context) {
        if let Some(io) = &mut self.io {
            let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
            if ctx.input(|i| i.viewport().close_requested()) {
                io.flush(&mut self.session);
            }
            let now = std::time::Instant::now();
            if let Some(wait) = io.pump(&mut self.session, now, focused) {
                ctx.request_repaint_after(wait);
            }
        }
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

    /// Files dropped on the window become pictures (PNG, JPEG, WebP) or file attachments, one
    /// command each, so each drop is its own undo step. Only a page that is shown takes them.
    fn insert_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input_mut(|input| std::mem::take(&mut input.raw.dropped_files));
        if dropped.is_empty() || self.session.notebooks().is_empty() || self.ribbon.backstage_open()
        {
            return;
        }
        // A file dropped on the page lands where it was dropped, each further file a step lower
        // and to the right.
        let drop_at = ctx
            .input(|input| input.pointer.hover_pos().or(input.pointer.latest_pos()))
            .and_then(|pos| self.canvas.page_point_at(pos));
        for (index, file) in dropped.into_iter().take(MAX_DROPPED_FILES).enumerate() {
            let path = file.path().to_path_buf();
            let text = path.to_string_lossy().into_owned();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let id = if is_picture_name(&name) {
                ids::INSERT_PICTURE
            } else {
                ids::INSERT_FILE_ATTACHMENT
            };
            let mut args = Map::new();
            args.insert("path".to_owned(), Value::from(text));
            if let Some(at) = drop_at {
                let step = INSERT_STEP * index as f32;
                args.insert("x".to_owned(), Value::from(f64::from(at.x + step)));
                args.insert("y".to_owned(), Value::from(f64::from(at.y + step)));
            }
            self.dispatch(id.as_str(), Value::Object(args));
        }
    }

    /// Put text a command asked to copy on the clipboard.
    fn flush_copy(&mut self, ctx: &egui::Context) {
        if let Some(text) = self.pending_copy.take() {
            ctx.copy_text(text);
        }
    }

    /// Draw the window contents (eframe `ui`).
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let tokens = self.tokens(ui.ctx());
        // First, so Esc closes the dialog before any shortcut sees it.
        self.whats_new.show(ui.ctx(), &tokens, &self.about.version);
        let pressed = self.shortcuts.pressed(ui.ctx());
        self.dispatch_all(pressed);
        self.insert_dropped_files(ui.ctx());
        self.paste_clipboard_picture(ui.ctx());
        self.flush_copy(ui.ctx());
        self.font_list.load_previews(ui.ctx());
        if let Some(colors) = self.restore_colors.take() {
            ribbon::set_recent_colors(ui.ctx(), &colors);
        }
        self.recent_colors = ribbon::recent_colors(ui.ctx());
        if self.families.is_empty() {
            if let Some(families) = self.font_list.families() {
                self.families = families;
            }
        }
        let tool_views: Vec<ribbon::ToolView> = self
            .session
            .ink()
            .tools()
            .iter()
            .map(|tool| ribbon::ToolView {
                highlighter: tool.kind == notecraft_engine::ToolKind::Highlighter,
                color: tool.style.color.0,
                width_pt: tool.style.width_pt,
                texture: tool.style.texture,
            })
            .collect();
        let look = ribbon::Look {
            tokens: &tokens,
            registry: &self.registry,
            fonts: &self.families,
            rule_lines: self.session.page_style().rule_lines.id(),
            always_rule_lines: self.session.default_rule_lines()
                != notecraft_canvas::RuleLines::None,
            pen: active_pen(self.session.ink()),
            tools: &tool_views,
            selected_tool: self.session.ink().selected_tool(),
            can_undo: self.session.can_undo(),
            can_redo: self.session.can_redo(),
            can_convert: self.session.can_convert_to_markdown(),
        };
        if self.ribbon.backstage_open() {
            let clicked = self
                .ribbon
                .show_backstage(ui, look, self.session.recents(), &self.about);
            self.dispatch_all(clicked);
            self.flush_copy(ui.ctx());
            self.toasts.show(ui.ctx());
            return;
        }
        if self.session.view().full_page && !self.session.notebooks().is_empty() {
            self.full_page_ui(ui, &tokens);
            return;
        }

        let (layout, show) = {
            let view = self.session.view();
            (view.ribbon_layout, view.ribbon_show)
        };
        let panel = egui::Frame::side_top_panel(ui.style());
        let height = ribbon::height(layout, show);
        let mut clicked = if height > 0.0 {
            egui::Panel::top("ribbon")
                .exact_size(height)
                .frame(egui::Frame::NONE.fill(tokens.ribbon_bg))
                .show(ui, |ui| self.ribbon.show(ui, look, layout, show))
                .inner
        } else {
            // Full-screen mode: the ribbon takes no room and shows over the page on demand.
            self.ribbon.show(ui, look, layout, show)
        };

        let session = &self.session;
        // The status bar's space is taken now, so the panes and the page fit around it, but
        // its contents are drawn last so it comes last in the keyboard Tab order.
        let status_rect = egui::Panel::bottom("status_bar")
            .exact_size(STATUS_HEIGHT)
            .frame(panel.fill(tokens.ribbon_bg))
            .show(ui, |ui| ui.max_rect())
            .inner;
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
            .show(ui, |ui| {
                nav::page_list(ui, session, nav_ui, &tokens, &mut clicked)
            });
        let canvas_width = egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(tokens.canvas_bg))
            .show(ui, |ui| {
                let full = ui.max_rect();
                let strip = egui::Rect::from_min_size(
                    full.min,
                    egui::vec2(full.width(), nav::SECTION_STRIP_HEIGHT),
                );
                nav::section_tabs(ui, strip, session, nav_ui, &tokens, &mut clicked);
                let bar = egui::Rect::from_min_size(
                    egui::pos2(full.min.x, strip.max.y),
                    egui::vec2(full.width(), bars::height(session)),
                );
                bars::show(ui, bar, session, &tokens, &mut clicked);
                let page = egui::Rect::from_min_max(egui::pos2(full.min.x, bar.max.y), full.max);
                canvas::show(ui, page, session, canvas_ui, &tokens, &mut clicked)
            })
            .inner;
        ui.scope_builder(egui::UiBuilder::new().max_rect(status_rect), |ui| {
            status::show(ui, session, &tokens, &mut clicked);
        });
        finish_splitters(ui, &tokens);
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
                if crate::a11y::plain_button(exit)
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

/// The tool to mark in the Draw tab menus: the pen, highlighter, or eraser while it is the
/// tool in use, and nothing while typing.
fn active_pen(ink: &notecraft_engine::InkState) -> Option<ribbon::ActivePen> {
    use notecraft_engine::DrawTool;
    let tool = match ink.tool() {
        DrawTool::Type => return None,
        DrawTool::Pen => ribbon::ActiveTool::Pen,
        DrawTool::Highlighter => ribbon::ActiveTool::Highlighter,
        DrawTool::Eraser => ribbon::ActiveTool::Eraser(ink.eraser().id(), ink.eraser_size().id()),
    };
    let style = ink.brush();
    let [r, g, b, _] = style.color.0;
    Some(ribbon::ActivePen {
        tool,
        color: u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b),
        width_pt: style.width_pt,
        texture: style.texture.id(),
    })
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
        assert!(texts.is_empty(), "Bold runs and shows nothing: {texts:?}");
        assert_eq!(last_ran(&harness).as_deref(), Some("format.bold"));
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
            for tab in [
                "Home", "Insert", "Draw", "History", "Review", "View", "Help",
            ] {
                harness.get_by_label(tab).click();
                harness.run();
                assert_eq!(harness.state().ribbon().selected(), tab);
            }
        }
    }

    fn ribbon_show(harness: &egui_kittest::Harness<'_, AppShell>) -> RibbonShow {
        harness.state().session().view().ribbon_show
    }

    /// Open Ribbon Display Options and pick `choice`.
    fn display_option(harness: &mut egui_kittest::Harness<'_, AppShell>, choice: &str) {
        harness.get_by_label("Ribbon Display Options").click();
        harness.run();
        harness.get_by_label(choice).click();
        harness.run();
    }

    #[test]
    fn ribbon_collapses_with_ctrl_f1_and_pins_open_again() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F1);
        harness.run();
        assert_eq!(ribbon_show(&harness), RibbonShow::TabsOnly);
        assert!(harness.query_by_label("Paste").is_none());

        // With tabs only, clicking a tab shows its groups until a button is used.
        harness.get_by_label("Home").click();
        harness.run();
        assert!(harness.state().ribbon().peeking());
        harness.get_by_label("Bold").click();
        harness.run();
        assert!(!harness.state().ribbon().peeking());
        assert_eq!(last_ran(&harness).as_deref(), Some("format.bold"));

        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F1);
        harness.run();
        assert_eq!(ribbon_show(&harness), RibbonShow::Always);

        // The display options menu collapses it; the pin on a peek brings it back.
        display_option(&mut harness, "Show tabs only");
        assert_eq!(ribbon_show(&harness), RibbonShow::TabsOnly);
        harness.get_by_label("Home").click();
        harness.run();
        harness.get_by_label("Pin the Ribbon").click();
        harness.run();
        assert_eq!(ribbon_show(&harness), RibbonShow::Always);
        assert!(harness.query_by_label("Paste").is_some());
    }

    #[test]
    fn the_canvas_reports_its_visible_part_to_screen_readers() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let canvas = harness.get_by_label("Canvas").rect();
        assert!(
            canvas.max.x <= WINDOW.x && canvas.max.y <= WINDOW.y,
            "{canvas:?}"
        );
        // So a click at its center lands on the page and does not scroll it.
        let before = harness.state().canvas().viewport().origin();
        harness.get_by_label("Canvas").click();
        harness.run();
        assert_eq!(harness.state().canvas().viewport().origin(), before);
    }

    /// What a screen reader gets for the canvas: the page text, once a page has markdown.
    fn canvas_text(harness: &egui_kittest::Harness<'_, AppShell>) -> Option<String> {
        use egui_kittest::kittest::NodeT;
        harness.get_by_label("Canvas").accesskit_node().value()
    }

    #[test]
    fn a_page_with_markdown_shows_its_text_on_the_canvas() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let text = canvas_text(&harness).expect("the welcome page has text");
        assert!(text.contains("Welcome to NoteCraft"), "{text}");
        assert!(text.contains("plain markdown"), "{text}");
        assert!(!text.contains("**"), "{text}");

        // Another page has no text yet; replacing the source of the shown page redraws it.
        harness.state_mut().set_page_source(
            notecraft_engine::mock::WELCOME_ID,
            "---\ntitle: Welcome\n---\nOnly this line.\n",
        );
        harness.run();
        assert_eq!(canvas_text(&harness).as_deref(), Some("Only this line.\n"));
    }

    #[test]
    fn a_page_without_text_has_no_canvas_text() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Grocery list").click();
        harness.run();
        assert_eq!(canvas_text(&harness), None);
    }

    fn type_into(harness: &mut egui_kittest::Harness<'_, AppShell>, label: &str, text: &str) {
        harness.get_by_label(label).focus();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.event(egui::Event::Text(text.to_owned()));
        harness.run();
    }

    fn last_ran(harness: &egui_kittest::Harness<'_, AppShell>) -> Option<String> {
        harness.state().ran.last().cloned()
    }

    fn last_toast(harness: &egui_kittest::Harness<'_, AppShell>) -> Option<String> {
        harness.state().toasts().texts().last().map(str::to_owned)
    }

    #[test]
    fn font_box_shows_the_font_and_filters_installed_families_as_you_type() {
        use egui_kittest::kittest::NodeT;
        let families = ["Arial", "Arial Black", "Calibri", "Consolas", "Segoe UI"]
            .map(str::to_owned)
            .to_vec();
        let shell = AppShell::new().with_font_list(FontList::from_families(families));
        let mut harness = harness(shell, WINDOW);
        harness.run();
        assert_eq!(
            harness
                .get_by_label("Font")
                .accesskit_node()
                .value()
                .as_deref(),
            Some("Calibri")
        );
        type_into(&mut harness, "Font", "black");
        assert!(harness.query_by_label("Arial Black").is_some());
        assert!(harness.query_by_label("Consolas").is_none(), "filtered");
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(last_ran(&harness).as_deref(), Some("format.font_family"));
        assert!(
            harness.query_by_label("Arial Black").is_none(),
            "list closed"
        );

        // Esc puts the current font back.
        type_into(&mut harness, "Font", "Cons");
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert_eq!(
            harness
                .get_by_label("Font")
                .accesskit_node()
                .value()
                .as_deref(),
            Some("Calibri")
        );
    }

    #[test]
    fn box_text_is_centered_in_the_box() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        for label in ["Font", "Font Size"] {
            let text = harness.get_by_label(label).rect();
            let arrow = harness.get_by_label(&format!("{label} list")).rect();
            assert!(
                (text.center().y - arrow.center().y).abs() <= 0.5,
                "{label}: text {text:?} box {arrow:?}"
            );
        }
    }

    #[test]
    fn size_box_takes_typed_sizes_and_ignores_text() {
        use egui_kittest::kittest::NodeT;
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        assert_eq!(
            harness
                .get_by_label("Font Size")
                .accesskit_node()
                .value()
                .as_deref(),
            Some("11")
        );
        type_into(&mut harness, "Font Size", "abc");
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(last_toast(&harness), None, "not a size");
        type_into(&mut harness, "Font Size", "13.5");
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(last_ran(&harness).as_deref(), Some("format.font_size"));
    }

    #[test]
    fn box_arrows_list_every_entry_and_a_pick_runs_the_command() {
        let families = ["Arial", "Calibri", "Consolas", "Segoe UI"]
            .map(str::to_owned)
            .to_vec();
        let shell = AppShell::new().with_font_list(FontList::from_families(families));
        let mut harness = harness(shell, WINDOW);
        harness.run();
        harness.get_by_label("Font list").click();
        harness.run();
        assert!(harness.query_by_label("Segoe UI").is_some());
        harness.get_by_label("Consolas").click();
        harness.run();
        assert_eq!(last_ran(&harness).as_deref(), Some("format.font_family"));
        assert!(harness.query_by_label("Segoe UI").is_none(), "list closed");

        let mut harness = harness_for(AppShell::new());
        harness.run();
        harness.get_by_label("Font Size list").click();
        harness.run();
        harness.get_by_label("24").click();
        harness.run();
        assert_eq!(last_ran(&harness).as_deref(), Some("format.font_size"));
    }

    #[test]
    fn display_options_check_the_current_layout_and_mode() {
        use egui_kittest::kittest::NodeT;
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Ribbon Display Options").click();
        harness.run();
        let checked = |label: &str| {
            harness
                .get_by_label(label)
                .accesskit_node()
                .data()
                .toggled()
                .map(|t| t == egui::accesskit::Toggled::True)
        };
        assert_eq!(checked("Classic Ribbon"), Some(true));
        assert_eq!(checked("Simplified Ribbon"), Some(false));
        assert_eq!(checked("Always show Ribbon"), Some(true));
        assert_eq!(checked("Show tabs only"), Some(false));
        assert_eq!(checked("Full-screen mode"), Some(false));
    }

    #[test]
    fn simplified_ribbon_is_one_row_that_runs_the_same_commands() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let classic = harness.get_by_label("Paste").rect();
        display_option(&mut harness, "Simplified Ribbon");
        assert_eq!(
            harness.state().session().view().ribbon_layout,
            RibbonLayout::Simplified
        );
        let paste = harness.get_by_label("Paste").rect();
        let bold = harness.get_by_label("Bold").rect();
        assert!(paste.height() < classic.height(), "icon only, not large");
        assert!((paste.center().y - bold.center().y).abs() < 1.0, "one row");
        assert!(
            harness.get_by_label("Canvas").rect().top()
                < classic.bottom() + ribbon::TAB_STRIP_HEIGHT,
            "the page moves up"
        );
        harness.get_by_label("Bold").click();
        harness.run();
        assert_eq!(last_ran(&harness).as_deref(), Some("format.bold"));
        display_option(&mut harness, "Classic Ribbon");
        assert_eq!(
            harness.state().session().view().ribbon_layout,
            RibbonLayout::Classic
        );
    }

    #[test]
    fn simplified_ribbon_puts_what_does_not_fit_in_more_options() {
        let mut shell = AppShell::new();
        shell.set_ribbon(RibbonLayout::Simplified, RibbonShow::Always);
        let mut harness = harness(shell, egui::vec2(800.0, 600.0));
        harness.run();
        assert!(harness.query_by_label("Meeting Details").is_none());
        harness.get_by_label("More Options").click();
        harness.run();
        harness.get_by_label("Meeting Details").click();
        harness.run();
        assert!(harness
            .state()
            .toasts()
            .texts()
            .any(|t| t == "Coming Soon: Meeting Details"));
    }

    fn last_toast_text(harness: &egui_kittest::Harness<'_, AppShell>) -> Option<String> {
        harness.state().toasts().texts().last().map(str::to_owned)
    }

    #[test]
    fn gallery_entries_run_their_commands() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Heading 2").click();
        harness.run();
        assert_eq!(last_ran(&harness).as_deref(), Some("style.apply"));
        harness.get_by_label("Important").click();
        harness.run();
        assert_eq!(
            last_toast_text(&harness).as_deref(),
            Some("Coming Soon: Tags")
        );
        // The separate large buttons stay beside the Tags gallery.
        let _ = harness.get_by_label("To Do Tag");
        let _ = harness.get_by_label("Find Tags");
    }

    #[test]
    fn gallery_arrows_scroll_two_entries_at_a_time() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        assert!(harness.query_by_label("Heading 3").is_none());
        harness.get_by_label("Styles down").click();
        harness.run();
        assert!(harness.query_by_label("Heading 1").is_none());
        let _ = harness.get_by_label("Heading 3");
        let _ = harness.get_by_label("Heading 4");
        harness.get_by_label("Styles up").click();
        harness.run();
        let _ = harness.get_by_label("Heading 1");
        assert!(harness.query_by_label("Heading 3").is_none());

        assert!(harness.query_by_label("Question").is_none());
        harness.get_by_label("Tags down").click();
        harness.run();
        let _ = harness.get_by_label("Question");
        let _ = harness.get_by_label("Remember for Later");
        assert!(harness.query_by_label("Important").is_none());
        harness.get_by_label("Tags down").click();
        harness.get_by_label("Tags down").click();
        harness.run();
        harness.run();
        // Customize Tags and Remove Tag are in the More list only.
        assert!(harness.query_by_label("Customize Tags").is_none());
        assert!(toasts(&harness).is_empty(), "scrolling runs nothing");
    }

    #[test]
    fn gallery_more_arrow_opens_the_full_list() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        assert!(harness.query_by_label("Page Title").is_none());
        harness.get_by_label("More Styles").click();
        harness.run();
        let _ = harness.get_by_label("Heading 6");
        harness.get_by_label("Page Title").click();
        harness.run();
        assert_eq!(last_ran(&harness).as_deref(), Some("style.apply"));
        assert!(harness.query_by_label("Heading 6").is_none(), "menu closes");

        harness.get_by_label("More Tags").click();
        harness.run();
        let _ = harness.get_by_label("Customize Tags");
        harness.get_by_label("Idea").click();
        harness.run();
        assert_eq!(
            last_toast_text(&harness).as_deref(),
            Some("Coming Soon: Tags")
        );
    }

    #[test]
    fn galleries_are_one_dropdown_each_in_the_simplified_ribbon() {
        let mut shell = AppShell::new();
        shell.set_ribbon(RibbonLayout::Simplified, RibbonShow::Always);
        let mut harness = harness(shell, WINDOW);
        harness.run();
        assert!(harness.query_by_label("Styles up").is_none());
        harness.get_by_label("Styles").click();
        harness.run();
        harness.get_by_label("Quote").click();
        harness.run();
        assert_eq!(last_ran(&harness).as_deref(), Some("style.apply"));
        harness.get_by_label("Tags").click();
        harness.run();
        harness.get_by_label("Idea").click();
        harness.run();
        assert_eq!(
            last_toast_text(&harness).as_deref(),
            Some("Coming Soon: Tags")
        );
    }

    #[test]
    fn galleries_are_in_the_more_options_menu_when_the_window_is_narrow() {
        let mut shell = AppShell::new();
        shell.set_ribbon(RibbonLayout::Simplified, RibbonShow::Always);
        let mut harness = harness(shell, egui::vec2(700.0, 1400.0));
        harness.run();
        assert!(harness.query_by_label("Tags").is_none());
        harness.get_by_label("More Options").click();
        harness.run();
        // A button with a menu is a submenu here; egui adds an arrow to its name.
        harness.get_by_label("Tags \u{23F5}").click();
        harness.run();
        harness.get_by_label("Remember for Later").click();
        harness.run();
        assert_eq!(
            last_toast_text(&harness).as_deref(),
            Some("Coming Soon: Tags")
        );
    }

    #[test]
    fn help_tab_has_its_buttons() {
        use egui::accesskit::Role;
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Help").click();
        harness.run();
        assert_eq!(harness.state().ribbon().selected(), "Help");
        let _ = harness.get_by_role_and_label(Role::Button, "What's New");
        for label in ["Help", "Support", "Feedback", "Show Training"] {
            harness.get_by_role_and_label(Role::Button, label).click();
            harness.run();
            assert_eq!(
                last_toast_text(&harness),
                Some(format!("Coming Soon: {label}"))
            );
        }
    }

    #[test]
    fn full_screen_mode_hides_the_ribbon_until_the_pointer_reaches_the_top() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        display_option(&mut harness, "Full-screen mode");
        assert_eq!(ribbon_show(&harness), RibbonShow::FullScreen);
        harness.event(egui::Event::PointerMoved(egui::pos2(400.0, 400.0)));
        harness.run();
        assert!(harness.query_by_label("Home").is_none());
        assert!(harness.query_by_label("Paste").is_none());

        harness.event(egui::Event::PointerMoved(egui::pos2(400.0, 1.0)));
        harness.run();
        assert!(harness.state().ribbon().revealed());
        assert!(harness.query_by_label("Paste").is_some());

        // Leaving it hides it again; Ctrl+F1 brings the whole ribbon back.
        harness.event(egui::Event::PointerMoved(egui::pos2(400.0, 500.0)));
        harness.run();
        assert!(!harness.state().ribbon().revealed());
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F1);
        harness.run();
        assert_eq!(ribbon_show(&harness), RibbonShow::Always);
        assert!(harness.query_by_label("Paste").is_some());
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

        // N opens Insert; then the code for Screen Clipping presses it.
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
            .find(|(button, _)| button.label == "Screen Clipping")
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
            .any(|t| t == "Coming Soon: Screen Clipping"));

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
    fn editing_a_sample_notebook_says_to_open_a_folder_first() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Add Page").click();
        harness.run();
        let texts: Vec<&str> = harness.state().toasts().texts().collect();
        assert_eq!(texts.len(), 1);
        assert!(texts[0].contains("sample notebook"), "{texts:?}");
    }

    /// A shell with a real notebook folder open: sections Work (Alpha, Beta) and Home (Gamma).
    fn disk_shell() -> (tempfile::TempDir, egui_kittest::Harness<'static, AppShell>) {
        let dir = tempfile::TempDir::new().unwrap();
        for (rel, text) in [
            ("Work/Alpha.md", "a"),
            ("Work/Beta.md", "b"),
            ("Home/Gamma.md", "g"),
        ] {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let mut shell = AppShell::empty();
        assert!(shell.open_path(dir.path()));
        let mut harness = harness(shell, WINDOW);
        harness.run();
        (dir, harness)
    }

    fn page_titles(shell: &AppShell) -> Vec<String> {
        shell
            .session()
            .current_section()
            .map(|s| s.pages.iter().map(|p| p.title.clone()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn add_page_and_new_section_edit_the_folder() {
        let (dir, mut harness) = disk_shell();
        harness.get_by_label("Add Page").click();
        harness.run();
        assert!(dir.path().join("Home/Untitled Page.md").is_file());
        assert_eq!(page_titles(harness.state()), ["Gamma", "Untitled Page"]);
        harness.get_by_label("New Section").click();
        harness.run();
        assert!(dir.path().join("New Section/.section.toml").is_file());
        assert_eq!(
            harness.state().session().current_section().unwrap().name,
            "New Section"
        );
        assert!(toasts(&harness).is_empty());
        // Undo takes the new section away again.
        harness.state_mut().dispatch("edit.undo", Value::Null);
        harness.run();
        assert!(!dir.path().join("New Section").exists());
    }

    #[test]
    fn a_page_is_renamed_in_place_from_its_context_menu() {
        let (dir, mut harness) = disk_shell();
        harness.get_by_label("Gamma").click_secondary();
        harness.run();
        harness.get_by_label("Rename").click();
        harness.run();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.event(egui::Event::Text("Delta".to_owned()));
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        harness.run();
        assert!(dir.path().join("Home/Delta.md").is_file());
        assert!(!dir.path().join("Home/Gamma.md").exists());
        assert_eq!(page_titles(harness.state()), ["Delta"]);
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn escape_cancels_a_rename() {
        let (dir, mut harness) = disk_shell();
        harness.get_by_label("Gamma").click_secondary();
        harness.run();
        harness.get_by_label("Rename").click();
        harness.run();
        harness.run();
        harness.event(egui::Event::Text("Nope".to_owned()));
        harness.key_press(egui::Key::Escape);
        harness.run();
        harness.run();
        assert!(dir.path().join("Home/Gamma.md").is_file());
        assert_eq!(page_titles(harness.state()), ["Gamma"]);
    }

    #[test]
    fn deleting_a_page_from_the_context_menu_can_be_undone() {
        let (dir, mut harness) = disk_shell();
        harness.get_by_label("Gamma").click_secondary();
        harness.run();
        harness.get_by_label("Delete Page").click();
        harness.run();
        assert!(!dir.path().join("Home/Gamma.md").exists());
        assert!(dir.path().join(".notecraft/trash").is_dir());
        assert!(page_titles(harness.state()).is_empty());
        harness.state_mut().dispatch("edit.undo", Value::Null);
        harness.run();
        assert!(dir.path().join("Home/Gamma.md").is_file());
        assert_eq!(page_titles(harness.state()), ["Gamma"]);
    }

    #[test]
    fn page_menu_moves_and_indents_pages() {
        let (dir, mut harness) = disk_shell();
        let work = harness.state().session().notebooks()[0].all_sections()[1]
            .id
            .clone();
        harness
            .state_mut()
            .dispatch("nav.section.select", serde_json::json!({ "id": work }));
        harness.run();
        assert_eq!(page_titles(harness.state()), ["Alpha", "Beta"]);
        harness.get_by_label("Alpha").click_secondary();
        harness.run();
        harness.get_by_label("Move Down").click();
        harness.run();
        assert_eq!(page_titles(harness.state()), ["Beta", "Alpha"]);
        harness.get_by_label("Alpha").click_secondary();
        harness.run();
        harness.get_by_label("Make Subpage").click();
        harness.run();
        let levels: Vec<u8> = harness
            .state()
            .session()
            .current_section()
            .unwrap()
            .pages
            .iter()
            .map(|p| p.level)
            .collect();
        assert_eq!(levels, [0, 1]);
        assert!(dir.path().join("Work/.section.toml").is_file());
    }

    #[test]
    fn section_context_menu_colors_and_deletes() {
        let (dir, mut harness) = disk_shell();
        harness
            .get_all_by_label("Home")
            .nth(1)
            .unwrap()
            .click_secondary();
        harness.run();
        harness.get_by_label_contains("Section Color").click();
        harness.run();
        harness.get_by_label("Blue").click();
        harness.run();
        let toml = std::fs::read_to_string(dir.path().join("Home/.section.toml")).unwrap();
        assert!(toml.contains("#9FD3E8"), "{toml}");
        harness
            .get_all_by_label("Home")
            .nth(1)
            .unwrap()
            .click_secondary();
        harness.run();
        harness.get_by_label("Delete Section").click();
        harness.run();
        assert!(!dir.path().join("Home").exists());
        assert!(toasts(&harness).is_empty());
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
        harness.get_by_label("View").click();
        harness.run();

        harness.get_by_label("Page Color").click();
        harness.run();
        harness.get_by_label("#DCEBF7").click();
        harness.run();
        // view.page_color is Ready: it sets the page color, with no toast.
        assert!(toasts(&harness).is_empty());
        assert_eq!(
            harness.state().session().page_style().color,
            Some([0xDC, 0xEB, 0xF7, 0xFF])
        );
        assert!(harness.query_by_label("#DCEBF7").is_none(), "picker closes");

        // The pick joins Recent Colors.
        harness.get_by_label("Page Color").click();
        harness.run();
        let _ = harness.get_by_label("Recent Colors");
        assert!(
            harness.get_all_by_label("#DCEBF7").count() >= 2,
            "grid and recent"
        );

        // More Colors opens the Colors dialog; OK runs the command with its color.
        harness.get_by_label("More Colors...").click();
        harness.run();
        assert!(
            harness.query_by_label("More Colors...").is_none(),
            "menu closes"
        );
        let _ = harness.get_by_label("Standard");
        harness.get_by_label("OK").click();
        harness.run();
        assert!(harness.query_by_label("OK").is_none(), "dialog closes");
        assert_eq!(
            harness.state().recent_colors().first().map(String::as_str),
            Some("#DCEBF7"),
            "the dialog starts from the last color"
        );
    }

    #[test]
    fn font_color_lays_out_theme_standard_and_applies_its_last_color() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("Font Color").click();
        harness.run();
        // The main part applies the default color; the menu opens from the arrow.
        assert!(toasts(&harness).is_empty());
        assert_eq!(last_ran(&harness).as_deref(), Some("format.font_color"));
        assert_eq!(harness.state().recent_colors(), ["#E53935"]);
        let arrow = harness.get_by_label("Font Color").rect().right_center() - egui::vec2(3.0, 0.0);
        harness.event(egui::Event::PointerMoved(arrow));
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: arrow,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            });
        }
        harness.run();
        for caption in [
            "Automatic",
            "Theme Colors",
            "Standard Colors",
            "Recent Colors",
        ] {
            let _ = harness.get_by_label(caption);
        }
        harness.get_by_label("#1E88E5").click();
        harness.run();
        assert_eq!(
            harness.state().recent_colors(),
            ["#1E88E5", "#E53935"],
            "newest first"
        );
    }

    #[test]
    fn highlight_offers_fifteen_colors_with_no_color_last() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let arrow = harness
            .get_by_label("Text Highlight Color")
            .rect()
            .right_center()
            - egui::vec2(3.0, 0.0);
        harness.event(egui::Event::PointerMoved(arrow));
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos: arrow,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            });
        }
        harness.run();
        let last_swatch = harness.get_by_label("#616161").rect();
        let no_color = harness.get_by_label("No Color").rect();
        assert!(no_color.top() > last_swatch.bottom(), "No Color comes last");
        assert!(
            harness.query_by_label("Theme Colors").is_none(),
            "one plain grid"
        );
    }

    #[test]
    fn colors_dialog_picker_square_and_sliders_change_the_color() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("View").click();
        harness.run();
        harness.get_by_label("Page Color").click();
        harness.run();
        harness.get_by_label("More Colors...").click();
        harness.run();
        harness.get_by_label("Custom").click();
        harness.run();
        let _ = harness.get_by_label("New #DCEBF7");
        harness.get_by_label("Saturation and brightness").click();
        harness.run();
        assert!(
            harness.query_by_label("New #DCEBF7").is_none(),
            "the square sets it"
        );
        let _ = harness.get_by_label("Hue");
        let _ = harness.get_by_label("Transparency");
        // One set of R, G, B, A fields: egui's own picker header is not drawn.
        let numbers = harness
            .query_all(egui_kittest::kittest::By::new().role(egui::accesskit::Role::SpinButton))
            .count();
        assert_eq!(numbers, 4, "R, G, B, A once");
    }

    #[test]
    fn recent_colors_restore_from_settings_and_drop_junk() {
        let mut shell = AppShell::new();
        shell.set_recent_colors(vec![
            "#123456".to_owned(),
            "not a color".to_owned(),
            "#123456".to_owned(),
            "#ABCDEF80".to_owned(),
        ]);
        let mut harness = harness(shell, WINDOW);
        harness.run();
        harness.run();
        assert_eq!(harness.state().recent_colors(), ["#123456", "#ABCDEF80"]);
    }

    #[test]
    fn custom_color_takes_rgba_text_and_number_fields() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("View").click();
        harness.run();
        harness.get_by_label("Page Color").click();
        harness.run();
        harness.get_by_label("More Colors...").click();
        harness.run();
        harness.get_by_label("Custom").click();
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
        let _ = harness.get_by_label("New #FF000080");

        // Text that is not a color is flagged and Apply is disabled until it is fixed.
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("not a color");
        harness.run();
        let _ = harness.get_by_label("Not a color: use #RRGGBB, #RRGGBBAA, rgb(), or rgba()");
        harness.get_by_label("OK").click();
        harness.run();
        assert!(
            harness.query_by_label("Custom").is_some(),
            "OK does nothing while the text is not a color"
        );
        harness.get_by_label("Cancel").click();
        harness.run();
        assert!(harness.query_by_label("Custom").is_none(), "Cancel closes");
        assert!(toasts(&harness).is_empty(), "and runs nothing");
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
        assert_eq!(view.page_style().rule_lines.id(), "dotted");
    }

    /// Open the View tab's Rule Lines menu.
    fn open_rule_lines(harness: &mut egui_kittest::Harness<'static, AppShell>) {
        harness.run();
        harness.get_by_label("View").click();
        harness.run();
        harness.get_by_label("Rule Lines").click();
        harness.run();
    }

    #[test]
    fn rule_lines_menu_shows_a_named_card_per_item_under_group_headings() {
        use egui::accesskit::Role;
        let mut harness = harness(AppShell::new(), WINDOW);
        open_rule_lines(&mut harness);
        let _ = harness.get_by_label("Ruled Lines");
        let _ = harness.get_by_label("Grid Lines");
        for item in crate::ribbon::spec::RULE_LINES {
            // A card is a radio button named like the text item it replaces.
            let _ = harness.get_by_role_and_label(Role::RadioButton, item.label);
        }
        assert_eq!(
            harness
                .query_all(egui_kittest::kittest::By::new().role(Role::RadioButton))
                .count(),
            crate::ribbon::spec::RULE_LINES.len()
        );
    }

    #[test]
    fn clicking_a_rule_lines_card_sets_the_page_style_and_closes_the_menu() {
        use egui::accesskit::Role;
        let mut harness = harness(AppShell::new(), WINDOW);
        open_rule_lines(&mut harness);
        harness
            .get_by_role_and_label(Role::RadioButton, "Large Grid")
            .click();
        harness.run();
        assert_eq!(
            harness.state().session().page_style().rule_lines.id(),
            "grid_large"
        );
        assert!(toasts(&harness).is_empty());
        assert!(harness
            .query_by_role_and_label(Role::RadioButton, "Large Grid")
            .is_none());
        // Again, for a ruled style.
        harness.get_by_label("Rule Lines").click();
        harness.run();
        harness
            .get_by_role_and_label(Role::RadioButton, "College Ruled")
            .click();
        harness.run();
        assert_eq!(
            harness.state().session().page_style().rule_lines.id(),
            "college"
        );
    }

    #[test]
    fn rule_lines_cards_mark_the_current_style_and_press_from_the_keyboard() {
        use egui::accesskit::{Role, Toggled};
        use egui_kittest::kittest::NodeT;
        let mut harness = harness(AppShell::new(), WINDOW);
        open_rule_lines(&mut harness);
        let toggled = |harness: &egui_kittest::Harness<'_, AppShell>, label: &str| {
            harness
                .get_by_role_and_label(Role::RadioButton, label)
                .accesskit_node()
                .toggled()
        };
        assert_eq!(toggled(&harness, "None"), Some(Toggled::True));
        assert_eq!(toggled(&harness, "Dotted"), Some(Toggled::False));
        harness
            .get_by_role_and_label(Role::RadioButton, "Dotted")
            .focus();
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        assert_eq!(
            harness.state().session().page_style().rule_lines.id(),
            "dotted"
        );
        harness.get_by_label("Rule Lines").click();
        harness.run();
        assert_eq!(toggled(&harness, "Dotted"), Some(Toggled::True));
        assert_eq!(toggled(&harness, "None"), Some(Toggled::False));
    }

    #[test]
    fn table_grid_opens() {
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
        shell.dispatch(ids::MEETING_DETAILS.as_str(), Value::Null);
        let mut harness = harness(shell, WINDOW);

        harness.run();
        let _ = harness.get_by_label("Coming Soon: Meeting Details");

        // egui's clock advances one predicted frame (1/60 s) per step without real input.
        harness.run_steps(200);
        assert!(harness
            .query_by_label("Coming Soon: Meeting Details")
            .is_none());
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

    fn sample_notebook_path() -> String {
        let path: std::path::PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "testdata",
            "notebooks",
            "basic",
        ]
        .iter()
        .collect();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn opening_a_real_folder_shows_its_real_sections_and_pages() {
        let path = sample_notebook_path();
        let mut harness = harness(picking(Some(&path), None), WINDOW);
        harness.run();
        harness.get_by_label("Open Notebook").click();
        harness.run();
        assert_eq!(harness.state().session().notebooks().len(), 1);
        assert_eq!(recent_paths(harness.state()), [path.as_str()]);
        assert!(toasts(&harness).is_empty());
        // The notebook pane lists the notebook, its group, and every section.
        for label in [
            "Basic Sample",
            "Projects",
            "Journal",
            "Beta",
            "Alpha",
            "Reference",
        ] {
            assert!(
                harness.query_all_by_label(label).next().is_some(),
                "{label}"
            );
        }
        // The first section's pages show in their saved order, subpages included.
        for label in [
            "Welcome",
            "Daily log",
            "Monday",
            "Morning run",
            "Tuesday",
            "Sketch ideas",
        ] {
            let _ = harness.get_by_label(label);
        }
        assert!(harness.query_by_label("Grocery list").is_none());

        let reference = harness.get_all_by_label("Reference").next().unwrap();
        reference.click();
        harness.run();
        for label in ["Archive", "Reading list", "Recipes"] {
            let _ = harness.get_by_label(label);
        }
        assert!(harness.query_by_label("Welcome").is_none());
        harness.get_by_label("Recipes").click();
        harness.run();
        assert_eq!(
            harness.state().session().selection().page.as_deref(),
            Some("nb_basic_sample/Reference/Recipes.md")
        );
    }

    #[test]
    fn collapsing_a_parent_hides_its_subpages_in_the_page_list() {
        // A temporary notebook: collapsing writes the section file.
        let dir = tempfile::TempDir::new().unwrap();
        let section = dir.path().join("Journal");
        std::fs::create_dir_all(&section).unwrap();
        for name in ["Welcome", "Daily log", "Monday", "Morning run", "Tuesday"] {
            std::fs::write(section.join(format!("{name}.md")), "text").unwrap();
        }
        std::fs::write(
            section.join(".section.toml"),
            "kind = \"section\"
pages = [\"Welcome.md\", \"Daily log.md\", { file = \"Monday.md\", level = 1 }, { file = \"Morning run.md\", level = 2 }, { file = \"Tuesday.md\", level = 1 }]
",
        )
        .unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let mut harness = harness(picking(Some(&path), None), WINDOW);
        harness.run();
        harness.get_by_label("Open Notebook").click();
        harness.run();
        let notebook = harness.state().session().notebooks()[0].id.clone();
        let parent = format!("{notebook}/Journal/Daily log.md");
        let subpages = ["Monday", "Morning run", "Tuesday"];
        for label in subpages {
            let _ = harness.get_by_label(label);
        }
        let run = |h: &mut egui_kittest::Harness<'_, AppShell>, id: &str| {
            let args = serde_json::json!({ "id": &parent });
            let done = h.state_mut().run_command(id, args);
            assert!(matches!(done, Some(Ok(_))), "{id}");
            h.run();
        };
        run(&mut harness, "page.collapse_subpages");
        for label in subpages {
            assert!(harness.query_by_label(label).is_none(), "{label}");
        }
        let _ = harness.get_by_label("Daily log");
        let _ = harness.get_by_label("Welcome");
        run(&mut harness, "page.expand_subpages");
        for label in subpages {
            let _ = harness.get_by_label(label);
        }
    }

    #[test]
    fn opened_pages_show_their_text_from_disk() {
        let dir = tempfile::TempDir::new().unwrap();
        let section = dir.path().join("Notes");
        std::fs::create_dir_all(&section).unwrap();
        std::fs::write(section.join("First.md"), "# First\n\nHello from disk.\n").unwrap();
        std::fs::write(section.join("Second.md"), "Second page text.\n").unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let mut harness = harness(picking(Some(&path), None), WINDOW);
        harness.run();
        harness.get_by_label("Open Notebook").click();
        harness.run();
        let text = canvas_text(&harness).expect("the first page has text");
        assert!(text.contains("Hello from disk."), "{text}");
        // Picking another page reads that file.
        harness.get_by_label("Second").click();
        harness.run();
        let text = canvas_text(&harness).expect("the second page has text");
        assert!(text.contains("Second page text."), "{text}");
        assert!(!text.contains("Hello from disk."), "{text}");
        // Going back keeps the page that was read.
        harness.get_by_label("First").click();
        harness.run();
        let text = canvas_text(&harness).expect("text");
        assert!(text.contains("Hello from disk."), "{text}");
    }

    #[test]
    fn an_unreadable_page_shows_a_short_error_on_the_canvas() {
        let dir = tempfile::TempDir::new().unwrap();
        let section = dir.path().join("Notes");
        std::fs::create_dir_all(&section).unwrap();
        std::fs::write(section.join("Here.md"), "Here.\n").unwrap();
        std::fs::write(section.join("Vanishing.md"), "Soon gone.\n").unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let mut harness = harness(picking(Some(&path), None), WINDOW);
        harness.run();
        harness.get_by_label("Open Notebook").click();
        harness.run();
        std::fs::remove_file(section.join("Vanishing.md")).unwrap();
        harness.get_by_label("Vanishing").click();
        harness.run();
        let text = canvas_text(&harness).expect("the error is on the canvas");
        assert!(text.contains("could not be opened"), "{text}");
        assert!(harness.state().session().current_page_notice().is_some());
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn an_empty_folder_opens_as_an_empty_notebook() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let mut harness = harness(picking(Some(&path), None), WINDOW);
        harness.run();
        harness.get_by_label("Open Notebook").click();
        harness.run();
        harness.run();
        assert_eq!(harness.state().session().notebooks().len(), 1);
        assert!(harness.state().session().current_page().is_none());
        let _ = harness.get_by_label("Canvas");
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn a_folder_with_a_bad_settings_file_opens_with_a_toast() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("S")).unwrap();
        std::fs::write(dir.path().join("S").join("Page.md"), "x").unwrap();
        std::fs::write(dir.path().join("S").join(".section.toml"), "= broken").unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let mut harness = harness(picking(Some(&path), None), WINDOW);
        harness.run();
        harness.get_by_label("Open Notebook").click();
        harness.run();
        assert_eq!(harness.state().session().notebooks().len(), 1);
        let _ = harness.get_by_label("Page");
        let shown = toasts(&harness);
        assert_eq!(shown.len(), 1);
        assert!(shown[0].starts_with("Opened with a warning"), "{shown:?}");
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
    fn open_path_opens_a_folder_or_a_page_file() {
        let dir = std::env::temp_dir().join(format!("notecraft-open-path-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&dir).is_ok());
        let page = dir.join("Plans.md");
        assert!(std::fs::write(&page, "# Plans\n").is_ok());

        let mut shell = AppShell::empty();
        assert!(shell.open_path(&dir));
        // The folder is a real (here empty) notebook, so it replaces nothing and shows no sample.
        assert_eq!(shell.session().notebooks().len(), 1);
        let first = shell.session().recents().items().first().map(|i| i.kind);
        assert_eq!(first, Some(notecraft_engine::recent::RecentKind::Notebook));

        assert!(shell.open_path(&page));
        let first = shell.session().recents().items().first().map(|i| i.kind);
        assert_eq!(first, Some(notecraft_engine::recent::RecentKind::Page));
        assert!(shell.toasts().is_empty());

        // Missing and wrong-type paths toast and leave the recent list alone.
        assert!(!shell.open_path(&dir.join("nope.md")));
        std::fs::write(dir.join("todo.txt"), "x").ok();
        assert!(!shell.open_path(&dir.join("todo.txt")));
        assert_eq!(recent_paths(&shell).len(), 2);
        std::fs::remove_dir_all(&dir).ok();
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
        // Ctrl+N runs page.new, which a sample notebook refuses with a reason.
        let texts = toasts(&harness);
        assert_eq!(texts.len(), 2, "{texts:?}");
        assert!(texts[0].starts_with("Add Page: "), "{texts:?}");
        assert_eq!(texts[1], "Coming Soon: Tags");

        // Ctrl+Alt+= zooms in; the more specific chord wins over Ctrl+=.
        harness.key_press_modifiers(command | egui::Modifiers::ALT, egui::Key::Equals);
        harness.run();
        assert_eq!(harness.state().session().view().zoom_percent, 125);
        assert!(!toasts(&harness).iter().any(|t| t.contains("Zoom")));
    }

    fn strokes_on_page(harness: &egui_kittest::Harness<'_, AppShell>) -> usize {
        harness
            .state()
            .session()
            .current_ink()
            .map_or(0, |layer| layer.len())
    }

    fn add_stroke(harness: &mut egui_kittest::Harness<'_, AppShell>) {
        let args = serde_json::json!({ "points": [[10, 10], [40, 40]] });
        assert!(harness.state_mut().dispatch("ink.add_stroke", args));
        harness.run();
    }

    fn button_disabled(harness: &egui_kittest::Harness<'_, AppShell>, label: &str) -> bool {
        use egui_kittest::kittest::NodeT;
        harness
            .get_by_label(label)
            .accesskit_node()
            .data()
            .is_disabled()
    }

    #[test]
    fn undo_and_redo_buttons_follow_the_history() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        // Nothing to undo or redo yet: both buttons are disabled.
        assert!(button_disabled(&harness, "Undo"));
        assert!(button_disabled(&harness, "Redo"));

        add_stroke(&mut harness);
        assert_eq!(strokes_on_page(&harness), 1);
        assert!(!button_disabled(&harness, "Undo"));
        assert!(button_disabled(&harness, "Redo"));

        harness.get_by_label("Undo").click();
        harness.run();
        assert_eq!(strokes_on_page(&harness), 0);
        assert!(button_disabled(&harness, "Undo"));
        assert!(!button_disabled(&harness, "Redo"));

        harness.get_by_label("Redo").click();
        harness.run();
        assert_eq!(strokes_on_page(&harness), 1);
        assert!(!button_disabled(&harness, "Undo"));
        assert!(button_disabled(&harness, "Redo"));
    }

    #[test]
    fn undo_and_redo_shortcuts_run_the_commands() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let command = egui::Modifiers::COMMAND;
        // With nothing to undo the shortcut does nothing and says nothing.
        harness.key_press_modifiers(command, egui::Key::Z);
        harness.run();
        assert!(toasts(&harness).is_empty());

        add_stroke(&mut harness);
        add_stroke(&mut harness);
        harness.key_press_modifiers(command, egui::Key::Z);
        harness.run();
        assert_eq!(strokes_on_page(&harness), 1);
        harness.key_press_modifiers(command, egui::Key::Y);
        harness.run();
        assert_eq!(strokes_on_page(&harness), 2);
        harness.key_press_modifiers(command, egui::Key::Z);
        harness.key_press_modifiers(command, egui::Key::Z);
        harness.run();
        assert_eq!(strokes_on_page(&harness), 0);
        // Ctrl+Shift+Z redoes too, and wins over Ctrl+Z.
        harness.key_press_modifiers(command | egui::Modifiers::SHIFT, egui::Key::Z);
        harness.run();
        assert_eq!(strokes_on_page(&harness), 1);
        assert!(toasts(&harness).is_empty());
    }

    #[test]
    fn shortcuts_wait_while_a_text_field_has_focus() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        harness.get_by_label("View").click();
        harness.run();
        harness.get_by_label("Page Color").click();
        harness.run();
        harness.get_by_label("More Colors...").click();
        harness.run();
        harness.get_by_label("Custom").click();
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
    const ACTIONABLE: [egui::accesskit::Role; 16] = {
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
            Role::ListItem,
            Role::ListBoxOption,
            Role::Splitter,
            Role::ScrollBar,
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
        for tab in [
            "Home", "Insert", "Draw", "History", "Review", "View", "Help",
        ] {
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

    #[test]
    fn canvas_scroll_bars_have_names() {
        use egui::accesskit::Role;
        use egui_kittest::kittest::NodeT;
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        for name in ["Horizontal", "Vertical"] {
            assert_eq!(
                harness.get_by_label(name).accesskit_node().role(),
                Role::ScrollBar
            );
        }
    }

    #[test]
    fn buttons_are_push_buttons_and_rows_report_selection() {
        use egui::accesskit::Role;
        use egui_kittest::kittest::{By, NodeT};
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        // A toggled state makes Narrator say "toggle button, off" and drops UIA Invoke.
        let toggles: Vec<String> = harness
            .query_all(By::new().role(Role::Button))
            .filter(|node| node.accesskit_node().data().toggled().is_some())
            .map(|node| node.accesskit_node().label().unwrap_or_default())
            .collect();
        assert_eq!(toggles, Vec::<String>::new());
        // Rows are list items with a selection state (UIA SelectionItem); Add Page is a
        // command, so it stays a push button.
        let row = |label: &str| {
            let node = harness.get_by_label(label);
            let ak = node.accesskit_node();
            (ak.role(), ak.data().is_selected())
        };
        assert_eq!(row("Welcome"), (Role::ListItem, Some(true)));
        assert_eq!(row("Grocery list"), (Role::ListItem, Some(false)));
        assert_eq!(row("My Notebook"), (Role::ListItem, Some(false)));
        assert_eq!(row("Add Page"), (Role::Button, None));
    }

    /// Every control a screen reader presses (button, tab, menu item, row) that does not
    /// offer the Click action, as "role label". Without it Narrator cannot activate the
    /// control: UI Automation gets no Invoke, Toggle, Selection, or Expand pattern to use.
    fn unpressable(harness: &egui_kittest::Harness<'_, AppShell>) -> Vec<String> {
        use egui::accesskit::{Action, Role};
        use egui_kittest::kittest::{By, NodeT};
        const PRESSABLE: [Role; 9] = [
            Role::Button,
            Role::Tab,
            Role::CheckBox,
            Role::RadioButton,
            Role::MenuItem,
            Role::Link,
            Role::TreeItem,
            Role::ListItem,
            Role::ListBoxOption,
        ];
        harness
            .query_all(By::new().predicate(|node| PRESSABLE.contains(&node.role())))
            .filter_map(|node| {
                let ak = node.accesskit_node();
                let data = ak.data();
                (!data.supports_action(Action::Click) && !data.is_disabled())
                    .then(|| format!("{:?} {}", ak.role(), ak.label().unwrap_or_default()))
            })
            .collect()
    }

    #[test]
    fn every_control_can_be_pressed_by_a_screen_reader() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        for tab in [
            "Home", "Insert", "Draw", "History", "Review", "View", "Help",
        ] {
            harness.get_by_label(tab).click();
            harness.run();
            assert_eq!(unpressable(&harness), Vec::<String>::new(), "{tab} tab");
        }
        harness.get_by_label("File").click();
        harness.run();
        assert_eq!(unpressable(&harness), Vec::<String>::new(), "backstage");

        let mut start = harness_for(AppShell::empty());
        start.run();
        assert_eq!(unpressable(&start), Vec::<String>::new(), "start page");
    }

    fn harness_for(shell: AppShell) -> egui_kittest::Harness<'static, AppShell> {
        harness(shell, WINDOW)
    }

    /// The label of the focused node, after each of `presses` Tab presses.
    fn tab_through(
        harness: &mut egui_kittest::Harness<'_, AppShell>,
        presses: usize,
    ) -> Vec<String> {
        use egui_kittest::kittest::{By, NodeT};
        (0..presses)
            .map(|_| {
                harness.key_press(egui::Key::Tab);
                harness.run();
                harness
                    .query_all(By::new().predicate(|node| node.is_focused()))
                    .next()
                    .map(|node| node.accesskit_node().label().unwrap_or_default())
                    .unwrap_or_default()
            })
            .collect()
    }

    #[test]
    fn tab_moves_through_ribbon_navigation_page_and_status_bar() {
        let mut harness = harness(AppShell::new(), WINDOW);
        harness.run();
        let order = tab_through(&mut harness, 70);
        let at = |label: &str| {
            order
                .iter()
                .position(|l| l == label)
                .unwrap_or_else(|| panic!("{label} never got focus: {order:?}"))
        };
        let stops = [
            at("File"),
            at("Paste"),
            at("Ribbon Display Options"),
            at("My Notebook"),
            at("Notebook pane width"),
            at("Add Page"),
            at("Welcome"),
            at("Page list width"),
            at("New Section"),
            at("Canvas"),
            at("Zoom 100%"),
        ];
        assert!(stops.windows(2).all(|w| w[0] < w[1]), "{order:?}");
        assert!(
            order.iter().all(|l| !l.is_empty()),
            "an unnamed stop: {order:?}"
        );
        assert!(toasts(&harness).is_empty(), "Tab runs nothing");
    }

    #[test]
    fn survives_a_tiny_window() {
        let mut harness = harness(AppShell::new(), egui::vec2(1.0, 1.0));

        harness.run();
    }

    /// A notebook with one page, opened in a harness; returns the folder to edit files in.
    fn open_one_page(text: &str) -> (tempfile::TempDir, egui_kittest::Harness<'static, AppShell>) {
        let dir = tempfile::TempDir::new().unwrap();
        let section = dir.path().join("Notes");
        std::fs::create_dir_all(&section).unwrap();
        std::fs::write(section.join("Only.md"), text).unwrap();
        let path = dir.path().to_string_lossy().into_owned();
        let mut harness = harness(picking(Some(&path), None), WINDOW);
        harness.run();
        harness.get_by_label("Open Notebook").click();
        harness.run();
        (dir, harness)
    }

    #[test]
    fn the_conflict_bar_offers_both_versions_and_runs_commands() {
        let (dir, mut harness) = open_one_page("one\n");
        let page = harness.state().session().selection().page.clone().unwrap();
        harness
            .state_mut()
            .session
            .edit_page_text(&page, 0, 0, "mine ", false);
        std::fs::write(dir.path().join("Notes/Only.md"), "theirs\n").unwrap();
        harness
            .state_mut()
            .session
            .note_outside_change(&dir.path().join("Notes/Only.md"));
        harness.run();
        let _ = harness.get_by_label_contains("changed outside NoteCraft");
        let _ = harness.get_by_label("Unsaved changes");
        harness.get_by_label("Use outside version").click();
        harness.run();
        assert_eq!(harness.state().session().page_text(&page), Some("theirs\n"));
        assert!(harness
            .query_by_label_contains("changed outside NoteCraft")
            .is_none());
        let _ = harness.get_by_label("Saved");
    }

    #[test]
    fn the_recovery_bar_restores_the_unsaved_text() {
        let (dir, mut harness) = open_one_page("on disk\n");
        let path = dir.path().join("Notes/Only.md");
        let page = harness.state().session().selection().page.clone().unwrap();
        let entry = notecraft_engine::RecoveryEntry {
            path,
            text: "unsaved\n".to_owned(),
        };
        harness.state_mut().session.offer_recovery(vec![entry]);
        harness.run();
        let _ = harness.get_by_label_contains("unsaved changes to 1 page");
        harness.get_by_label("Restore").click();
        harness.run();
        assert_eq!(
            harness.state().session().page_text(&page),
            Some("unsaved\n")
        );
        assert!(harness.state().session().is_page_dirty(&page));
        assert!(harness.query_by_label("Restore").is_none());
    }
}
