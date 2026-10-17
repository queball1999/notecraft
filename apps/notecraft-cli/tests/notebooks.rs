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

#[test]
fn text_prints_a_page_found_by_id_path_or_link_target() {
    let nb = basic_copy();
    let path = path_arg(&nb);
    let expected = "Monday\nWater the plants\nCall the plumber\n";
    for page in [
        "Journal/Monday.md",
        "Journal/Monday",
        "Monday",
        "monday",
        "nb_basic_sample/Journal/Monday.md",
    ] {
        let output = cli(&["text", page, "--notebook", &path]);
        assert!(output.status.success(), "{page}");
        assert_eq!(stdout(&output), expected, "{page}");
    }
    let output = cli(&["--json", "--notebook", &path, "text", "Monday"]);
    let value = json(&output);
    assert_eq!(value["page"], "nb_basic_sample/Journal/Monday.md");
    assert_eq!(value["path"], "Journal/Monday.md");
    assert_eq!(value["text"], expected);

    // A canvas page shows its text without the canvas block or the container marker.
    let sketch = stdout(&cli(&["text", "Sketch ideas", "--notebook", &path]));
    assert!(
        sketch.contains("A treehouse with a rope bridge"),
        "{sketch}"
    );
    assert!(!sketch.contains("notecraft-canvas"), "{sketch}");
    assert!(!sketch.contains("notecraft:container"), "{sketch}");
}

#[test]
fn text_errors_have_exit_codes() {
    let nb = basic_copy();
    let path = path_arg(&nb);
    assert_eq!(
        cli(&["text", "No such page", "--notebook", &path])
            .status
            .code(),
        Some(3)
    );
    assert_eq!(cli(&["text", "--notebook", &path]).status.code(), Some(2));
    assert_eq!(
        cli(&["text", "a", "b", "--notebook", &path]).status.code(),
        Some(2)
    );
    // Two pages share a name: ambiguous (exit 2), but a path picks one.
    std::fs::write(nb.path().join("Reference").join("Monday.md"), "# Other\n").unwrap();
    let both = cli(&["text", "Monday", "--notebook", &path]);
    assert_eq!(both.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&both.stderr).contains("more than one page"));
    let one = cli(&["text", "Reference/Monday", "--notebook", &path]);
    assert_eq!(stdout(&one), "Other\n");
}

#[test]
fn check_passes_on_a_clean_notebook() {
    let nb = basic_copy();
    let path = path_arg(&nb);
    let output = cli(&["check", "--notebook", &path]);
    assert!(output.status.success(), "{}", stdout(&output));
    assert!(stdout(&output).contains("Checked 12 pages: 0 problems"));
    let value = json(&cli(&["check", "--json", "--notebook", &path]));
    assert_eq!(value["ok"], true);
    assert_eq!(value["pages"], 12);
}

#[test]
fn check_finds_broken_links_missing_attachments_and_bad_canvas_blocks() {
    let nb = basic_copy();
    let path = path_arg(&nb);
    let journal = nb.path().join("Journal");
    std::fs::write(
        journal.join("Links.md"),
        "# Links\n\nGood [[Monday]] and [[Journal/Tuesday|Tue]] and `[[in code]]`.\n\n\
         Bad [[Nowhere]] and [[Nowhere]] again.\n\n\
         ![pic](attachments/gone.png) [file](attachments/a1b2c3-readme.txt) \
         [web](https://example.org/x) [gone](attachments/missing%20file.pdf)\n",
    )
    .unwrap();
    std::fs::write(
        journal.join("Broken ink.notecraft"),
        "Text\n\n```notecraft-canvas\n{ not json\n```\n",
    )
    .unwrap();

    let output = cli(&["check", "--notebook", &path]);
    assert_eq!(output.status.code(), Some(1));
    let text = stdout(&output);
    assert!(text.contains("Journal/Links.md: broken_link"), "{text}");
    assert_eq!(
        text.matches("[[Nowhere]]").count(),
        1,
        "once per page: {text}"
    );
    assert!(!text.contains("[[Monday]]"), "{text}");
    assert!(!text.contains("in code"), "{text}");
    assert!(text.contains("attachments/gone.png"), "{text}");
    assert!(text.contains("attachments/missing%20file.pdf"), "{text}");
    assert!(!text.contains("a1b2c3-readme"), "{text}");
    assert!(!text.contains("example.org"), "{text}");
    assert!(
        text.contains("Journal/Broken ink.notecraft: bad_canvas"),
        "{text}"
    );

    let value = json(&cli(&["check", "--json", "--notebook", &path]));
    assert_eq!(value["ok"], false);
    let kinds: Vec<&str> = value["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == "missing_attachment").count(),
        2
    );
    assert_eq!(kinds.iter().filter(|k| **k == "broken_link").count(), 1);
    assert_eq!(kinds.iter().filter(|k| **k == "bad_canvas").count(), 1);

    // Quiet keeps the problems and drops the summary; the exit code is the same.
    let quiet = cli(&["check", "--quiet", "--notebook", &path]);
    assert_eq!(quiet.status.code(), Some(1));
    assert!(!stdout(&quiet).contains("Checked"));
}

#[test]
fn check_reports_pages_that_cannot_be_read_as_text() {
    let nb = basic_copy();
    let path = path_arg(&nb);
    std::fs::write(
        nb.path().join("Journal").join("Bytes.md"),
        [0x66, 0xff, 0xfe, 0x67],
    )
    .unwrap();
    let value = json(&cli(&["check", "--json", "--notebook", &path]));
    let problems = value["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{value}");
    assert_eq!(problems[0]["kind"], "bad_text");
    assert_eq!(problems[0]["path"], "Journal/Bytes.md");
}

#[test]
fn an_ambiguous_link_is_a_warning_not_a_problem() {
    let nb = basic_copy();
    let path = path_arg(&nb);
    std::fs::write(nb.path().join("Reference").join("Monday.md"), "# Other\n").unwrap();
    let beta = nb.path().join("Projects").join("Beta");
    std::fs::write(beta.join("Pointer.md"), "See [[Monday]].\n").unwrap();
    // From the Journal section the link resolves to the page in the same section.
    std::fs::write(
        nb.path().join("Journal").join("Near.md"),
        "See [[Monday]].\n",
    )
    .unwrap();
    let output = cli(&["check", "--notebook", &path]);
    assert!(output.status.success(), "{}", stdout(&output));
    let text = stdout(&output);
    assert!(text.contains("warning: Projects/Beta/Pointer.md"), "{text}");
    assert!(!text.contains("Near.md"), "{text}");
}
