//! Runs the built `notecraft-cli` binary for `search` and `index` (PR 2.11) and `check` link
//! resolution on small temp notebooks.
// Test code: clippy only exempts #[test] functions, not their helpers.
#![allow(clippy::unwrap_used)]

use std::path::Path;
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

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_str(&stdout(output)).unwrap()
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn notebook() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        root,
        "Work/Kayak.md",
        "# Kayak trip\n\nkayak kayak kayak packing list #todo\n",
    );
    write(
        root,
        "Work/Notes.md",
        "# Notes\n\nA long page that mentions a kayak once among many other words.\n\n- [ ] buy paddle #todo\n",
    );
    write(
        root,
        "Home/Garden.md",
        "# Garden\n\nkayak storage shed #idea\n",
    );
    write(root, "Home/Plain.md", "# Plain\n\nNothing relevant here.\n");
    dir
}

fn path(dir: &tempfile::TempDir) -> String {
    dir.path().to_str().unwrap().to_owned()
}

fn paths(value: &serde_json::Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["path"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn search_ranks_the_best_match_first_and_json_has_a_shape() {
    let nb = notebook();
    let value = json(&cli(&["search", "kayak", "--json", &path(&nb)]));
    assert_eq!(value["ok"], true);
    assert_eq!(value["query"], "kayak");
    assert_eq!(value["truncated"], false);
    assert_eq!(value["count"], 3);
    let hits = paths(&value, "hits");
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0], "Work/Kayak.md", "{hits:?}");
    let first = &value["hits"][0];
    assert_eq!(first["kind"], "page");
    assert_eq!(first["title"], "Kayak");
    assert!(first["page_id"]
        .as_str()
        .unwrap()
        .ends_with("/Work/Kayak.md"));
    assert!(first["rank"].is_number());
    assert!(first["snippet"].as_str().unwrap().contains("kayak"));
    assert!(first["highlights"].is_array());

    let text = stdout(&cli(&["search", "kayak", &path(&nb)]));
    assert!(text.starts_with("Work/Kayak.md  Kayak\n"), "{text}");
    assert!(text.contains("    "), "{text}");
}

#[test]
fn search_limit_bounds_the_hits_and_marks_truncation() {
    let nb = notebook();
    let value = json(&cli(&[
        "search",
        "kayak",
        "--limit",
        "2",
        "--json",
        "--notebook",
        &path(&nb),
    ]));
    assert_eq!(value["count"], 2);
    assert_eq!(value["truncated"], true);
    assert_eq!(
        cli(&["search", "kayak", "--limit", "0", &path(&nb)])
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn search_tag_filters_pages_and_a_bare_tag_lists_items() {
    let nb = notebook();
    let p = path(&nb);
    let value = json(&cli(&["search", "kayak", "--tag", "#todo", "--json", &p]));
    let mut hits = paths(&value, "hits");
    hits.sort();
    assert_eq!(hits, ["Work/Kayak.md", "Work/Notes.md"]);
    assert_eq!(value["hits"][0]["tags"][0], "todo");

    let none = json(&cli(&["search", "kayak", "--tag", "nosuch", "--json", &p]));
    assert_eq!(none["count"], 0);

    let items = json(&cli(&[
        "search",
        "--tag",
        "todo",
        "--json",
        "--notebook",
        &p,
    ]));
    assert_eq!(items["count"], 2);
    let first = &items["items"][0];
    assert_eq!(first["tag"], "todo");
    assert!(first["path"].as_str().unwrap().starts_with("Work/"));
    let checked: Vec<bool> = items["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["checked"].as_bool())
        .collect();
    assert_eq!(checked, [false, false]);

    let text = stdout(&cli(&["search", "--tag", "idea", "--notebook", &p]));
    assert!(text.contains("Home/Garden.md"), "{text}");
    assert!(text.contains("#idea"), "{text}");
}

#[test]
fn search_scope_and_errors() {
    let nb = notebook();
    let p = path(&nb);
    let value = json(&cli(&[
        "search",
        "kayak",
        "--scope",
        "section",
        "--in",
        "Home",
        "--json",
        "--notebook",
        &p,
    ]));
    assert_eq!(paths(&value, "hits"), ["Home/Garden.md"]);
    let items = json(&cli(&[
        "search",
        "--tag",
        "todo",
        "--scope",
        "section",
        "--in",
        "Home",
        "--json",
        "--notebook",
        &p,
    ]));
    assert_eq!(items["count"], 0);
    assert_eq!(
        cli(&[
            "search",
            "kayak",
            "--scope",
            "section",
            "--in",
            "Nope",
            "--notebook",
            &p
        ])
        .status
        .code(),
        Some(3)
    );
    assert_eq!(
        cli(&["search", "kayak", "--scope", "section", "--notebook", &p])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(cli(&["search", "--notebook", &p]).status.code(), Some(2));
    let empty = cli(&["search", "zzzzqq", &p]);
    assert!(empty.status.success());
    assert!(stdout(&empty).contains("No matches."));
    assert_eq!(
        cli(&["search", "x", "/no/such/folder/here"]).status.code(),
        Some(3)
    );
}

#[test]
fn index_updates_and_rebuilds() {
    let nb = notebook();
    let p = path(&nb);
    let db = nb.path().join(".notecraft/index.sqlite");
    assert!(!db.exists());
    let first = json(&cli(&["index", "--json", &p]));
    assert_eq!(first["ok"], true);
    assert_eq!(first["pages"], 4);
    assert_eq!(first["tagged_items"], 3);
    assert_eq!(first["tags"], 2);
    assert_eq!(first["links"], 0);
    assert!(db.is_file());

    write(
        nb.path(),
        "Home/Link.md",
        "See [[Garden]] and [[Missing]].\n",
    );
    let text = stdout(&cli(&["index", &p]));
    assert!(text.starts_with("Updated"), "{text}");
    assert!(text.contains("5 pages"), "{text}");
    assert!(text.contains("2 links (1 unresolved)"), "{text}");

    std::fs::write(&db, b"not a database").unwrap();
    let rebuilt = json(&cli(&["index", "--rebuild", "--json", "--notebook", &p]));
    assert_eq!(rebuilt["rebuilt"], true);
    assert_eq!(rebuilt["pages"], 5);
    assert_eq!(rebuilt["links"], 2);
    assert_eq!(rebuilt["indexed_text"], 5);
    assert!(stdout(&cli(&["index", "--rebuild", &p])).starts_with("Rebuilt"));

    assert_eq!(cli(&["index", "--nope"]).status.code(), Some(2));
    assert_eq!(cli(&["index", &p, "--notebook", &p]).status.code(), Some(2));
}

#[test]
fn search_sees_edits_without_a_separate_index_run() {
    let nb = notebook();
    let p = path(&nb);
    assert_eq!(json(&cli(&["search", "zebra", "--json", &p]))["count"], 0);
    write(nb.path(), "Home/Zoo.md", "zebra crossing\n");
    assert_eq!(json(&cli(&["search", "zebra", "--json", &p]))["count"], 1);
}

#[test]
fn check_separates_ambiguous_missing_and_alias_links() {
    let nb = tempfile::tempdir().unwrap();
    let root = nb.path();
    write(root, "A/Plan.md", "# A plan\n");
    write(root, "B/Plan.md", "# B plan\n");
    write(
        root,
        "C/Other.md",
        "---\naliases: [Nickname]\n---\n# Other\n",
    );
    write(
        root,
        "C/Source.md",
        "[[Plan]] [[Nickname]] [[Ghost]] `[[in code]]`\n",
    );
    let value = json(&cli(&[
        "check",
        "--json",
        "--notebook",
        root.to_str().unwrap(),
    ]));
    let problems = value["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 1, "{value}");
    assert_eq!(problems[0]["kind"], "broken_link");
    assert!(problems[0]["message"].as_str().unwrap().contains("Ghost"));
    let warnings = value["warnings"].as_array().unwrap();
    assert_eq!(warnings.len(), 1, "{value}");
    let warning = warnings[0].as_str().unwrap();
    assert!(
        warning.contains("[[Plan]]") && warning.contains("A/Plan.md"),
        "{warning}"
    );
}
