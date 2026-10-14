//! Process and workspace helpers shared by every xtask command.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Workspace root (the parent of the xtask crate).
pub fn root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

/// A `cargo` command (the same cargo that runs xtask) rooted at the workspace.
pub fn cargo() -> Command {
    let mut c = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    c.current_dir(root());
    c
}

/// Run a command to completion, echoing it first. Fails on spawn errors and non-zero exits.
pub fn run(mut cmd: Command, what: &str) -> Result<(), String> {
    eprintln!("$ {what}");
    let status = cmd
        .status()
        .map_err(|e| format!("{what}: failed to start: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what}: exited with {status}"))
    }
}
