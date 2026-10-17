#![no_main]

#[path = "../checks/storage.rs"]
mod checks;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    checks::check_canvas(data);
});
