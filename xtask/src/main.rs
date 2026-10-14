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

mod assets;
mod ci;
mod icons;
mod layers;
mod parity;
mod text;
mod util;
mod version;
mod wasm;

use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  ci        every CI gate in order, stopping at the first failure:
            fmt --check, clippy -D warnings, test, text, assets, layers,
            wasm, parity --check
  text      plain ASCII text outside testdata/ and locales/ (AGENTS.md section 2)
  assets    every file under assets/ and testdata/ has an ATTRIBUTION.md row
  layers    crates depend only on lower layers; no UI crates below L6
  wasm      L0 to L5 crates build for wasm32-unknown-unknown
  parity [--check]
            regenerate the parity checklist summary (--check: fail if stale) and
            check build plan hour totals against ROADMAP.md
  version [set X.Y.Z[-pre]]
            print or set the workspace version (also refreshes Cargo.lock)
  ico <out.ico> <in.png>...
  icns <out.icns> <in.png>...
            pack rendered PNGs into app icons (see packaging/icons.sh)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("ci") => ci::run_all(),
        Some("text") => text::run(&util::root()),
        Some("assets") => assets::run(&util::root()),
        Some("layers") => layers::run(&util::root()),
        Some("wasm") => wasm::run_check(&util::root()),
        Some("parity") => {
            let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
            parity::run(&util::root(), &rest)
        }
        Some(cmd @ ("version" | "ico" | "icns")) => {
            let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
            match cmd {
                "version" => version::run(&util::root(), &rest),
                kind => icons::run(kind, &rest),
            }
        }
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
