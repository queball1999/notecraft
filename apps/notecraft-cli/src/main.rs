//! NoteCraft command line. Before full parity it only reports its version (docs/cli.md).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

use std::io::Write;
use std::process::ExitCode;

const USAGE: &str =
    "usage: notecraft-cli --version\n\nOther commands are post-parity options (docs/cli.md).\n";

/// "notecraft-cli <version> (<sha>, <date>)", as printed by --version.
fn version_line() -> String {
    format!(
        "notecraft-cli {} ({}, {})",
        env!("CARGO_PKG_VERSION"),
        env!("NOTECRAFT_BUILD_SHA"),
        env!("NOTECRAFT_BUILD_DATE")
    )
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Write errors (a closed pipe) are not worth crashing over; exit codes still tell the story.
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            let _ = writeln!(std::io::stdout(), "{}", version_line());
            ExitCode::SUCCESS
        }
        Some("--help" | "-h") | None => {
            let _ = write!(std::io::stdout(), "{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            let _ = write!(std::io::stderr(), "unknown argument `{other}`\n\n{USAGE}");
            // Exit code 2: bad arguments (docs/cli.md).
            ExitCode::from(2)
        }
    }
}
