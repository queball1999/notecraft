//! `cargo xtask wasm`: the L0 to L5 crates build for `wasm32-unknown-unknown`.
//!
//! Keeps the engine portable for the Phase 4 browser build. Crates that gain native-only
//! dependencies must gate them with `cfg(not(target_arch = "wasm32"))`.

use crate::layers::{self, Layer};
use crate::util::{cargo, run};
use std::path::Path;

/// Highest layer that must build for wasm32.
const MAX_LAYER: u8 = 5;

pub fn run_check(root: &Path) -> Result<(), String> {
    let crates = layers::from_metadata(&layers::metadata()?, root)?;
    let set: Vec<String> = crates
        .into_iter()
        .filter(|c| matches!(c.layer, Ok(Layer::L(l)) if l <= MAX_LAYER))
        .map(|c| c.name)
        .collect();
    if set.is_empty() {
        return Err("no L0 to L5 crates found".into());
    }
    let mut c = cargo();
    c.args(["check", "--target", "wasm32-unknown-unknown"]);
    for p in &set {
        c.args(["-p", p]);
    }
    let result = run(c, "cargo check --target wasm32-unknown-unknown (L0 to L5)");
    match &result {
        Ok(()) => println!(
            "wasm: {} crates build for wasm32 ({})",
            set.len(),
            set.join(", ")
        ),
        Err(_) => {
            println!("wasm: failed (missing target? rustup target add wasm32-unknown-unknown)")
        }
    }
    result
}
