//! Runs the built `notecraft-cli` binary on copies of the sample notebook (PR 1.16). The
//! testdata folder itself is never written to.
// Test code: clippy only exempts #[test] functions, not their helpers.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_notecraft-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn cli_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_notecraft-cli"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout(output)).unwrap()
}

/// A temp copy of the `basic` sample notebook.
fn basic_copy() -> tempfile::TempDir {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    let source: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "testdata",
        "notebooks",
        "basic",
    ]
    .iter()
    .collect();
    let dir = tempfile::tempdir().unwrap();
    copy(&source, dir.path());
    dir
}

fn path_arg(dir: &tempfile::TempDir) -> String {
    dir.path().to_str().unwrap().to_owned()
}

#[test]
fn info_counts_the_notebook_as_text_and_json() {
    let nb = basic_copy();
    let path = path_arg(&nb);
    let text = cli(&["info", "--notebook", &path]);
    assert!(text.status.success());
    let text = stdout(&text);
    assert!(text.contains("Notebook: Basic Sample"), "{text}");
    assert!(text.contains("Format version: 1"), "{text}");
    assert!(text.contains("Sections: 4"), "{text}");
    assert!(text.contains("Section groups: 1"), "{text}");
    assert!(text.contains("Pages: 12"), "{text}");

    // Global options work before or after the command.
    let before = cli(&["--json", "--notebook", &path, "info"]);
    let value = json(&before);
    assert_eq!(value["name"], "Basic Sample");
    assert_eq!(value["id"], "nb_basic_sample");
    assert_eq!(value["format"], 1);
    assert_eq!(value["sections"], 4);
    assert_eq!(value["groups"], 1);
    assert_eq!(value["pages"], 12);
    assert_eq!(value["warnings"].as_array().unwrap().len(), 0);
    let after = cli(&["info", "--json", &format!("--notebook={path}")]);
    assert_eq!(stdout(&after), stdout(&before));
}

#[test]
fn quiet_prints_nothing_for_info() {
    let nb = basic_copy();
    let quiet = cli(&["info", "--quiet", "--notebook", &path_arg(&nb)]);
    assert!(quiet.status.success());
    assert_eq!(stdout(&quiet), "");
}

#[test]
fn the_notebook_defaults_to_the_current_folder_when_it_is_one() {
    let nb = basic_copy();
    let output = cli_in(nb.path(), &["info", "--json"]);
    assert!(output.status.success());
    assert_eq!(json(&output)["pages"], 12);

    // A folder that is not a notebook is "not found" (exit 3), with JSON on request.
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(cli_in(empty.path(), &["info"]).status.code(), Some(3));
    let output = cli_in(empty.path(), &["info", "--json"]);
    assert_eq!(output.status.code(), Some(3));
    let value = json(&output);
    assert_eq!(value["error"], "not_found");
    assert_eq!(value["ok"], false);
}

#[test]
fn a_missing_notebook_folder_and_bad_global_options_have_exit_codes() {
    let parent = tempfile::tempdir().unwrap();
    let missing = parent.path().join("nope");
    let missing = missing.to_str().unwrap().to_owned();
    assert_eq!(
        cli(&["info", "--notebook", &missing]).status.code(),
        Some(3)
    );
    assert_eq!(cli(&["info", "--notebook"]).status.code(), Some(2));
    assert_eq!(cli(&["info", "--log-level", "loud"]).status.code(), Some(2));
    assert_eq!(cli(&["info", "--bogus"]).status.code(), Some(2));
    // A --cmd value that looks like a global option is not taken for one.
    let output = cli(&["run", "--cmd", "view.zoom={\"percent\":150}", "--quiet"]);
    assert!(output.status.success());
}
