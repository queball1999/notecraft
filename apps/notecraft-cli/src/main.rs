//! NoteCraft command line. Skeleton only: prints its name, version, and build info.
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

/// "notecraft-cli <version> (<sha>, <date>)", as printed by --version.
fn version_line() -> String {
    format!(
        "notecraft-cli {} ({}, {})",
        env!("CARGO_PKG_VERSION"),
        env!("NOTECRAFT_BUILD_SHA"),
        env!("NOTECRAFT_BUILD_DATE")
    )
}

fn main() {
    // A closed stdout is not an error worth crashing over.
    let _ = writeln!(std::io::stdout(), "{}", version_line());
}
