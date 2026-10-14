//! NoteCraft desktop app: opens the main window on wgpu and hosts the egui front end.
//!
//! Usage: `notecraft [--version]`
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

mod logging;
mod prefs;

use craft_settings::{Location, SaveStatus, SettingsStore};
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
    /// Last settings save error already logged, so a failing disk is logged once, not per frame.
    logged_save_error: Option<String>,
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.shell.logic(ctx);

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
    }

    fn on_exit(&mut self) {
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
    if std::env::args()
        .skip(1)
        .any(|a| a == "--version" || a == "-V")
    {
        // A closed stdout is not an error worth crashing over.
        let _ = writeln!(std::io::stdout(), "{}", version_line());
        return Ok(());
    }
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
    let window = store.get().window.clone();
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
            Ok(Box::new(App {
                shell: AppShell::new(),
                store,
                logged_save_error: None,
            }))
        }),
    )
}
