//! NoteCraft desktop app. Phase -1 skeleton: `--version`, logging, and startup facts.
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

use std::io::Write;

/// "NoteCraft <version> (<sha>, <date>)", as printed by --version.
fn version_line() -> String {
    format!(
        "NoteCraft {} ({}, {})",
        env!("CARGO_PKG_VERSION"),
        env!("NOTECRAFT_BUILD_SHA"),
        env!("NOTECRAFT_BUILD_DATE")
    )
}

fn main() {
    if std::env::args()
        .skip(1)
        .any(|a| a == "--version" || a == "-V")
    {
        // A closed stdout is not an error worth crashing over.
        let _ = writeln!(std::io::stdout(), "{}", version_line());
        return;
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
    // Phase 0 (build_plan/02, PR 0.1) opens the window here and logs the GPU backend.
    tracing::info!("no window yet: the UI shell lands in Phase 0");
}
