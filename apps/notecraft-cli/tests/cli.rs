//! Runs the built `notecraft-cli` binary as a script would.
// Test code: clippy only exempts #[test] functions, not their helpers.
#![allow(clippy::unwrap_used)]

use std::process::{Command, Output};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_notecraft-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn version_and_help() {
    let version = cli(&["--version"]);
    assert!(version.status.success());
    assert!(stdout(&version).starts_with("notecraft-cli "));
    for args in [
        &["--help"][..],
        &[],
        &["run", "--help"],
        &["commands", "-h"],
    ] {
        let help = cli(args);
        assert!(help.status.success(), "{args:?}");
        assert!(stdout(&help).contains("USAGE:"), "{args:?}");
    }
}

#[test]
fn commands_list_every_command_as_text_and_json() {
    let text = stdout(&cli(&["commands"]));
    assert!(text
        .lines()
        .any(|l| l.starts_with("view.zoom ") && l.contains("ready")));
    let json: serde_json::Value =
        serde_json::from_str(&stdout(&cli(&["commands", "--json"]))).unwrap();
    let list = json.as_array().unwrap();
    assert!(list.len() > 100);
    let zoom = list.iter().find(|c| c["id"] == "view.zoom").unwrap();
    assert_eq!(zoom["status"], "ready");
    assert_eq!(zoom["args"][0]["name"], "percent");
    assert_eq!(zoom["args"][0]["type"], "integer");
}

#[test]
fn run_prints_one_result_per_command_and_stops_at_the_first_failure() {
    let output = cli(&[
        "run",
        "--sample",
        "--cmd",
        r#"view.zoom={"percent":150}"#,
        "--cmd",
        "format.bold",
        "--cmd",
        "view.zoom_in",
    ]);
    assert_eq!(output.status.code(), Some(4), "Coming Soon");
    let lines: Vec<serde_json::Value> = stdout(&output)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2, "stops after format.bold");
    assert_eq!(lines[0]["ok"], true);
    assert_eq!(lines[0]["value"]["percent"], 150);
    assert_eq!(lines[1]["error"], "coming_soon");
}

#[test]
fn bad_input_has_its_own_exit_codes() {
    assert_eq!(cli(&["run", "--cmd", "no.such"]).status.code(), Some(3));
    assert_eq!(
        cli(&["run", "--cmd", r#"view.zoom={"percent":"big"}"#])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(cli(&["run", "--cmd", "view.zoom={"]).status.code(), Some(2));
    assert_eq!(cli(&["run"]).status.code(), Some(2));
    assert_eq!(cli(&["frobnicate"]).status.code(), Some(2));
    let huge = format!("view.zoom={{\"x\":\"{}\"}}", "a".repeat(20_000));
    assert_eq!(cli(&["run", "--cmd", &huge]).status.code(), Some(6));
}
