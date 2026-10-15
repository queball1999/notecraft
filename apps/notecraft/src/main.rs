//! NoteCraft desktop app: opens the main window on wgpu and hosts the egui front end.
//!
//! Usage: `notecraft [--version] [--help] [--control <port>] [<notebook folder or page file>]`
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]
// Release builds on Windows are GUI-subsystem binaries (no console window opens with the app;
// packaging/windows/package.ps1 checks this).
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod args;
mod control;
mod logging;
mod picker;
mod prefs;

use craft_settings::{Location, SaveStatus, SettingsStore};
use notecraft_ui_egui::font_list::FontList;
use notecraft_ui_egui::themes::OsFonts;
use notecraft_ui_egui::AppShell;
use prefs::Prefs;
use std::io::Write;
use std::time::Instant;

/// Reverse-DNS app ID: window class on Linux, matches the desktop entry and icon names.
const APP_ID: &str = "io.github.queball1999.notecraft";

/// "NoteCraft <version> (<sha>, <date>)", as printed by --version.
fn version_line() -> String {
    format!(
        "NoteCraft {} ({}, {})",
        env!("CARGO_PKG_VERSION"),
        env!("NOTECRAFT_BUILD_SHA"),
        env!("NOTECRAFT_BUILD_DATE")
    )
}

struct App {
    shell: AppShell,
    store: SettingsStore<Prefs>,
    /// The UI test control channel, when started with `--control`.
    control: Option<control::Control>,
    /// Last settings save error already logged, so a failing disk is logged once, not per frame.
    logged_save_error: Option<String>,
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // First, so a command from the control channel is in the session before the shell
        // installs the theme and the UI draws.
        if let Some(control) = &mut self.control {
            control.poll(ctx, &mut self.shell);
        }
        self.shell.logic(ctx);

        // Remember the theme; commands change it in the session.
        let view = *self.shell.session().view();
        self.store.update(|p| {
            p.theme.observe(view.theme_preset, view.theme_mode);
            p.ribbon.observe(view.ribbon_layout, view.ribbon_show);
        });
        let recents = self.shell.session().recents();
        self.store.update(|p| p.recent.observe(recents));
        let colors = self.shell.recent_colors();
        self.store.update(|p| p.colors.observe(colors));

        // Remember the window geometry; update() only marks the store dirty on a real change.
        let info = ctx.input(|i| i.viewport().clone());
        self.store.update(|p| p.window.observe(&info));
        let tick = self.store.tick(Instant::now());
        match &tick.status {
            SaveStatus::Failed(err) => {
                let msg = err.to_string();
                if self.logged_save_error.as_deref() != Some(msg.as_str()) {
                    tracing::warn!(error = %msg, "saving settings failed; retrying");
                    self.logged_save_error = Some(msg);
                }
            }
            _ => self.logged_save_error = None,
        }
        // eframe repaints only on input, so ask for one at the save deadline.
        if let Some(deadline) = tick.next_deadline {
            ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
        }

        // Last, so it sees any CancelClose the app sends (craft-quit docs).
        craft_quit::on_close_request(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.ui(ui);
        if let Some(control) = &mut self.control {
            control.end_frame(ui.ctx());
        }
    }

    fn on_exit(&mut self) {
        if let Some(control) = &self.control {
            control.shutdown();
        }
        if let Err(e) = self.store.flush() {
            tracing::warn!(error = %e, "saving settings on exit failed");
        }
        tracing::info!("NoteCraft exiting");
        // macOS: end the process before eframe tears the window down; AppKit's Touch Bar
        // observer otherwise throws during the next display cycle (craft-quit docs).
        #[cfg(target_os = "macos")]
        std::process::exit(0);
    }
}

fn icon() -> Option<egui::IconData> {
    #[cfg(target_os = "macos")]
    let bytes = include_bytes!("../../../assets/app-icon/notecraft-macos-512.png");
    #[cfg(not(target_os = "macos"))]
    let bytes = include_bytes!(
        "../../../assets/app-icon/hicolor/256x256/apps/io.github.queball1999.notecraft.png"
    );
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    })
}

fn open_settings() -> SettingsStore<Prefs> {
    // NOTECRAFT_NO_PREFS keeps settings in memory (agent and test runs); craft-settings also
    // honors NOTECRAFT_CONFIG_DIR.
    let location = if logging::no_prefs() {
        Location::Memory
    } else {
        Location::Auto
    };
    let (store, report) = SettingsStore::<Prefs>::open(location);
    if report.has_problems() {
        tracing::warn!(report = ?report, "settings loaded with problems; defaults used where needed");
    }
    store
}

fn main() -> eframe::Result<()> {
    let (open, control_port) = match args::parse(std::env::args_os().skip(1)) {
        Ok(args::Cli::Version) => {
            // A closed stdout is not an error worth crashing over.
            let _ = writeln!(std::io::stdout(), "{}", version_line());
            return Ok(());
        }
        Ok(args::Cli::Help) => {
            let _ = write!(std::io::stdout(), "{}", args::USAGE);
            return Ok(());
        }
        Ok(args::Cli::Launch { open, control }) => (open, control),
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "notecraft: {e}\n");
            let _ = write!(std::io::stderr(), "{}", args::USAGE);
            std::process::exit(2);
        }
    };
    let log_path = logging::init();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        build_sha = env!("NOTECRAFT_BUILD_SHA"),
        build_date = env!("NOTECRAFT_BUILD_DATE"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        "NoteCraft starting"
    );
    match &log_path {
        Some(p) => tracing::info!(log = %p.display(), "file logging on"),
        None => tracing::info!("file logging off (NOTECRAFT_NO_PREFS or no settings folder)"),
    }

    let store = open_settings();
    // The control token lives beside config.toml; with no settings folder (NOTECRAFT_NO_PREFS)
    // there is nowhere to put it, so refuse rather than listen without a way to log in.
    let control_dir = match (control_port, store.dir()) {
        (Some(port), Some(dir)) => Some((port, dir.to_path_buf())),
        (Some(_), None) => {
            let _ = writeln!(
                std::io::stderr(),
                "notecraft: --control needs a settings folder to hold control.token; \
                 unset NOTECRAFT_NO_PREFS or set NOTECRAFT_CONFIG_DIR"
            );
            std::process::exit(2);
        }
        (None, _) => None,
    };
    let window = store.get().window.clone();
    let theme = store.get().theme.clone();
    let ribbon = (store.get().ribbon.layout(), store.get().ribbon.show());
    let recents = store.get().recent.recents();
    let recent_colors = store.get().colors.recent.clone();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("NoteCraft")
        .with_app_id(APP_ID)
        .with_inner_size(window.size())
        .with_min_inner_size(prefs::MIN_SIZE)
        .with_maximized(window.maximized)
        .with_drag_and_drop(true);
    if let Some(pos) = window.position() {
        viewport = viewport.with_position(pos);
    }
    if let Some(i) = icon() {
        viewport = viewport.with_icon(i);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let mut pending = Some(store);
    eframe::run_native(
        "NoteCraft",
        options,
        Box::new(move |cc| {
            if let Some(rs) = &cc.wgpu_render_state {
                let info = rs.adapter.get_info();
                tracing::info!(backend = ?info.backend, adapter = %info.name, "graphics ready");
            }
            if let Err(e) = craft_quit::install_app_menu(&cc.egui_ctx, "NoteCraft") {
                tracing::warn!(error = %e, "macOS app menu not installed; Cmd+Q quits without asking");
            }
            let store = pending.take().ok_or("the app was already created")?;
            let ctx = cc.egui_ctx.clone();
            let font_list = FontList::scan_in_background(move || ctx.request_repaint());
            let mut shell = AppShell::empty()
                .with_fonts(OsFonts::load())
                .with_font_list(font_list)
                .with_picker(Box::new(picker::OsPicker));
            shell.set_theme(theme.preset(), theme.mode());
            shell.set_ribbon(ribbon.0, ribbon.1);
            shell.set_recents(recents);
            shell.set_recent_colors(recent_colors);
            // A path on the command line wins; if it cannot open (the shell toasts why), or
            // none was given, reopen what was open last. With nothing recent, the start page
            // shows.
            let opened = open.as_deref().is_some_and(|path| shell.open_path(path));
            if !opened {
                shell.reopen_last();
            }
            let control = match &control_dir {
                Some((port, dir)) => {
                    let control = control::Control::start(*port, dir, &cc.egui_ctx)?;
                    tracing::info!(addr = %control.addr(), "control channel listening");
                    Some(control)
                }
                None => None,
            };
            Ok(Box::new(App {
                shell,
                store,
                control,
                logged_save_error: None,
            }))
        }),
    )
}
