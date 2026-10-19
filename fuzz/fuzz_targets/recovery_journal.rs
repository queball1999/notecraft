#![no_main]

#[path = "../checks/storage.rs"]
mod checks;

use std::sync::OnceLock;

/// One scratch folder per fuzzing process.
fn scratch() -> &'static std::path::Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| tempfile::TempDir::new().expect("scratch folder"))
        .path()
}

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    checks::check_journal(scratch(), data);
});
