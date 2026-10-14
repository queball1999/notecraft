//! NoteCraft desktop app. Skeleton only: prints its name and version.
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

fn main() {
    // A closed stdout is not an error worth crashing over.
    let _ = writeln!(std::io::stdout(), "NoteCraft {}", env!("CARGO_PKG_VERSION"));
}
