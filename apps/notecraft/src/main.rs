//! NoteCraft desktop app. Skeleton only: prints its name, version, and build info.
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
    // A closed stdout is not an error worth crashing over.
    let _ = writeln!(std::io::stdout(), "{}", version_line());
}
