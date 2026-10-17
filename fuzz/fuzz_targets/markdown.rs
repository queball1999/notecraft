#![no_main]

#[path = "../checks/markdown.rs"]
mod checks;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    checks::check_markdown(data);
});
