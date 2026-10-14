//! NoteCraft workspace tooling: `cargo xtask <command>` (or `cargo run -p xtask -- <command>`).
//!
//! Pure Rust (std plus serde_json). External tools (`cargo`, `git`) run through
//! `std::process::Command`. The command layout and the CI step summary follow the GridCraft
//! xtask (MIT OR Apache-2.0, see ATTRIBUTION.md).
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![forbid(unsafe_code)]

mod ci;
mod util;

use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  ci        every CI gate in order, stopping at the first failure:
            fmt --check, clippy -D warnings, test
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci::run_all(),
        Some("-h" | "--help" | "help") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
