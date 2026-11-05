//! Tests for the page link commands (PR 2.2): following links, creating missing pages, and the
//! page picker.

use std::path::Path;

use serde_json::{json, Value};

use crate::cmd::Registry;
use crate::open::open_folder;
use crate::session::Session;

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

/// An open notebook folder with the given files. Returns the folder, the session, and the id
/// of the notebook.
fn open(files: &[(&str, &str)]) -> (tempfile::TempDir, Session, String) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    for (rel, text) in files {
        write(dir.path(), rel, text);
    }
    let mut session = Session::new();
    let opened = open_folder(dir.path()).expect("open");
    let id = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&id, dir.path());
    (dir, session, id)
}

fn run(session: &mut Session, id: &str, args: Value) -> Value {
    match Registry::builtin().run(session, id, args) {
        Ok(out) => out.value,
        Err(err) => panic!("{id} failed: {err}"),
    }
}

/// Show the page at `path` (relative to the notebook folder) and load its text.
fn show(session: &mut Session, notebook: &str, path: &str) -> String {
    let id = format!("{notebook}/{path}");
    run(session, "nav.page.select", json!({ "id": id }));
    session.load_current_page();
    id
}

fn current_path(session: &Session) -> String {
    session.current_page_path().expect("a page is shown")
}

fn text_of(session: &Session, page: &str) -> String {
    session.page_text(page).unwrap_or_default().to_owned()
}

fn type_str(session: &mut Session, text: &str) {
    for c in text.chars() {
        run(
            session,
            "text.insert",
            json!({ "text": c.to_string(), "typing": true }),
        );
    }
}

fn titles(session: &Session, section: &str) -> Vec<String> {
    session
        .current_notebook()
        .and_then(|nb| nb.section(&format!("{}/{section}", nb.id)).cloned())
        .map(|s| s.pages.iter().map(|p| p.title.clone()).collect())
        .unwrap_or_default()
}

fn notebook() -> (tempfile::TempDir, Session, String) {
    open(&[
        ("Work/Plan.md", "work plan\n"),
        ("Work/Notes.md", "work notes\n"),
        ("Home/Notes.md", "home notes\n"),
        ("Home/Groceries.md", "milk\n"),
        ("Home/Trip.md", "---\naliases: [Holiday]\n---\ntrip\n"),
    ])
}

// Following links

#[test]
fn following_a_link_shows_the_page_in_another_section() {
    let (_dir, mut session, nb) = notebook();
    show(&mut session, &nb, "Work/Plan.md");
    let out = run(
        &mut session,
        "link.follow",
        json!({ "target": "Groceries" }),
    );
    assert_eq!(out["created"], false);
    assert_eq!(out["page"], format!("{nb}/Home/Groceries.md"));
    assert_eq!(current_path(&session), "Home/Groceries.md");
    assert_eq!(
        session.selection().section.as_deref(),
        Some(format!("{nb}/Home").as_str())
    );
}

#[test]
fn a_link_by_path_or_case_or_alias_is_followed() {
    let (_dir, mut session, nb) = notebook();
    // Aliases come from the notebook index; the background refresh hands them over.
    session.remember_link_aliases(
        &nb,
        [(format!("{nb}/Home/Trip.md"), vec!["Holiday".to_owned()])].into(),
    );
    show(&mut session, &nb, "Work/Plan.md");
    for (target, path) in [
        ("Home/Trip", "Home/Trip.md"),
        ("groceries", "Home/Groceries.md"),
        ("Holiday", "Home/Trip.md"),
        // Step 2: relative to the section of the page the link is on.
        ("Notes", "Work/Notes.md"),
    ] {
        show(&mut session, &nb, "Work/Plan.md");
        run(&mut session, "link.follow", json!({ "target": target }));
        assert_eq!(current_path(&session), path, "{target}");
    }
}

#[test]
fn an_ambiguous_link_goes_to_the_closest_page() {
    let (_dir, mut session, nb) = open(&[
        ("G/.section.toml", "kind = \"group\"\n"),
        ("G/A/Main.md", "m"),
        ("G/B/Same.md", "near"),
        ("H/Same.md", "far"),
    ]);
    show(&mut session, &nb, "G/A/Main.md");
    let out = run(&mut session, "link.follow", json!({ "target": "Same" }));
    assert_eq!(out["ambiguous"], true);
    assert_eq!(current_path(&session), "G/B/Same.md");
    let resolved = {
        show(&mut session, &nb, "G/A/Main.md");
        session.resolve_page_link("Same")
    };
    assert!(resolved.ambiguous);
    let labels: Vec<&str> = resolved
        .candidates
        .iter()
        .map(|c| c.label.as_str())
        .collect();
    assert_eq!(labels, ["G/B/Same", "H/Same"]);
}

#[test]
fn an_unresolved_link_creates_the_page_in_the_current_section() {
    let (dir, mut session, nb) = notebook();
    show(&mut session, &nb, "Work/Plan.md");
    let out = run(&mut session, "link.follow", json!({ "target": "Ideas" }));
    assert_eq!(out["created"], true);
    assert_eq!(current_path(&session), "Work/Ideas.md");
    assert!(dir.path().join("Work/Ideas.md").exists());
    assert_eq!(
        titles(&session, "Work").last().map(String::as_str),
        Some("Ideas")
    );
    // The link now resolves, so following it again creates nothing.
    show(&mut session, &nb, "Work/Plan.md");
    let again = run(&mut session, "link.follow", json!({ "target": "Ideas" }));
    assert_eq!(again["created"], false);
    assert_eq!(current_path(&session), "Work/Ideas.md");
}

#[test]
fn a_missing_page_named_by_a_path_is_made_from_the_last_part() {
    let (dir, mut session, nb) = notebook();
    show(&mut session, &nb, "Work/Plan.md");
    run(
        &mut session,
        "link.follow",
        json!({ "target": "Nowhere/Deep Page" }),
    );
    assert!(dir.path().join("Work/Deep Page.md").exists());
}

#[test]
fn a_missing_page_named_with_an_existing_section_is_made_in_that_section() {
    let (dir, mut session, nb) = notebook();
    show(&mut session, &nb, "Work/Plan.md");
    let out = run(
        &mut session,
        "link.follow",
        json!({ "target": "home/Packing list" }),
    );
    assert_eq!(out["created"], true);
    assert!(dir.path().join("Home/Packing list.md").exists());
    assert_eq!(current_path(&session), "Home/Packing list.md");
}

#[test]
fn renaming_a_page_updates_links_and_the_setting_command_turns_it_off() {
    let (dir, mut session2, nb2) = open(&[
        ("Work/Plan.md", "work plan\n"),
        ("Home/Todo.md", "see [[Work/Plan]]\n"),
    ]);
    run(
        &mut session2,
        "page.rename",
        json!({ "id": format!("{nb2}/Work/Plan.md"), "name": "Roadmap" }),
    );
    let text = std::fs::read_to_string(dir.path().join("Home/Todo.md")).expect("read");
    assert_eq!(text, "see [[Roadmap|Work/Plan]]\n");
    let undone = run(&mut session2, "edit.undo", Value::Null);
    assert_eq!(undone["label"], "Rename page");
    // The page was opened meanwhile, so the undo edited its text; saving writes it.
    session2.save_dirty();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Home/Todo.md")).expect("read"),
        "see [[Work/Plan]]\n"
    );
    let out = run(&mut session2, "links.update_on_rename", json!({}));
    assert_eq!(out["enabled"], false);
    run(
        &mut session2,
        "page.rename",
        json!({ "id": format!("{nb2}/Work/Plan.md"), "name": "Roadmap" }),
    );
    let text = std::fs::read_to_string(dir.path().join("Home/Todo.md")).expect("read");
    assert_eq!(text, "see [[Work/Plan]]\n");
}

#[test]
fn creating_the_page_is_one_undo_step() {
    let (dir, mut session, nb) = notebook();
    show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "link.follow", json!({ "target": "Ideas" }));
    assert!(dir.path().join("Work/Ideas.md").exists());
    run(&mut session, "edit.undo", Value::Null);
    assert!(!dir.path().join("Work/Ideas.md").exists());
    assert!(!titles(&session, "Work").contains(&"Ideas".to_owned()));
    run(&mut session, "edit.redo", Value::Null);
    assert!(dir.path().join("Work/Ideas.md").exists());
}

#[test]
fn create_false_leaves_a_missing_link_alone() {
    let (dir, mut session, nb) = notebook();
    let id = show(&mut session, &nb, "Work/Plan.md");
    let out = run(
        &mut session,
        "link.follow",
        json!({ "target": "Ideas", "create": false }),
    );
    assert_eq!(out["page"], Value::Null);
    assert_eq!(out["created"], false);
    assert_eq!(session.selection().page.as_deref(), Some(id.as_str()));
    assert!(!dir.path().join("Work/Ideas.md").exists());
}

#[test]
fn follow_needs_a_target() {
    let (_dir, mut session, nb) = notebook();
    show(&mut session, &nb, "Work/Plan.md");
    for args in [
        json!({}),
        json!({ "target": "   " }),
        json!({ "target": 4 }),
    ] {
        assert!(Registry::builtin()
            .run(&mut session, "link.follow", args)
            .is_err());
    }
}

// The picker

#[test]
fn typing_double_brackets_opens_the_picker_and_filters_as_you_type() {
    let (_dir, mut session, nb) = notebook();
    let page = show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 10 }));
    assert!(session.link_picker_view().is_none());
    type_str(&mut session, "See [");
    assert!(
        session.link_picker_view().is_none(),
        "one bracket is not enough"
    );
    type_str(&mut session, "[");
    let view = session.link_picker_view().expect("picker");
    assert_eq!(view.query, "");
    // Every other page of the notebook; the page being edited is not offered.
    assert_eq!(view.entries.len(), 4);
    assert!(view.entries.iter().all(|e| e.page != page));
    type_str(&mut session, "not");
    let view = session.link_picker_view().expect("picker");
    let found: Vec<(&str, &str)> = view
        .entries
        .iter()
        .map(|e| (e.title.as_str(), e.place.as_str()))
        .collect();
    assert_eq!(found, [("Notes", "Home"), ("Notes", "Work")]);
    type_str(&mut session, "zzz");
    assert!(session
        .link_picker_view()
        .expect("picker")
        .entries
        .is_empty());
}

#[test]
fn a_longer_insert_with_double_brackets_opens_the_picker_too() {
    let (_dir, mut session, nb) = notebook();
    show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    // An IME commit or a fast paste-like insert arrives as one piece of text.
    run(&mut session, "text.insert", json!({ "text": "see [[Gro" }));
    let view = session.link_picker_view().expect("picker");
    assert_eq!(view.query, "Gro");
    assert_eq!(view.entries.len(), 1);
    // Brackets that are already finished do not open it.
    run(&mut session, "link.picker.close", json!({}));
    run(
        &mut session,
        "text.insert",
        json!({ "text": " and [[Done]]" }),
    );
    assert!(session.link_picker_view().is_none());
    // A second bracket typed next to an earlier one does.
    run(&mut session, "text.insert", json!({ "text": " [" }));
    assert!(session.link_picker_view().is_none());
    run(&mut session, "text.insert", json!({ "text": "[" }));
    assert!(session.link_picker_view().is_some());
}

#[test]
fn matches_start_with_titles_that_begin_like_the_text() {
    let (_dir, mut session, nb) = open(&[
        ("S/Main.md", "\n"),
        ("S/Weekly plan.md", "x"),
        ("S/Plan.md", "x"),
        ("S/Replanning.md", "x"),
    ]);
    show(&mut session, &nb, "S/Main.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    type_str(&mut session, "[[PLAN");
    let view = session.link_picker_view().expect("picker");
    let titles: Vec<&str> = view.entries.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, ["Plan", "Replanning", "Weekly plan"]);
}

#[test]
fn choosing_a_page_inserts_a_link_in_the_shortest_form() {
    let (_dir, mut session, nb) = notebook();
    let page = show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    type_str(&mut session, "[[Groc");
    let before = text_of(&session, &page);
    let view = session.link_picker_view().expect("picker");
    assert_eq!(view.entries.len(), 1);
    let out = run(&mut session, "link.picker.choose", json!({}));
    assert_eq!(out["applied"], true);
    assert_eq!(text_of(&session, &page), "[[Groceries]]work plan\n");
    // The caret is after the closing brackets, and the picker is gone.
    assert_eq!(session.text_cursor().map(|c| c.head), Some(13));
    assert!(session.link_picker().is_none());
    // One undo step puts back what was typed.
    run(&mut session, "edit.undo", Value::Null);
    assert_eq!(text_of(&session, &page), before);
}

#[test]
fn a_shared_name_is_inserted_with_its_section() {
    let (_dir, mut session, nb) = notebook();
    let page = show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    type_str(&mut session, "[[Notes");
    let other = format!("{nb}/Home/Notes.md");
    run(&mut session, "link.picker.choose", json!({ "page": other }));
    // From Work, `Notes` would mean Work/Notes (step 2), so the other one needs its path.
    assert_eq!(text_of(&session, &page), "[[Home/Notes]]work plan\n");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    type_str(&mut session, "[[Notes");
    let near = format!("{nb}/Work/Notes.md");
    run(&mut session, "link.picker.choose", json!({ "page": near }));
    assert!(text_of(&session, &page).starts_with("[[Notes]][[Home/Notes]]"));
    // Whatever was written resolves back to the page that was chosen.
    assert_eq!(
        session.resolve_page_link("Home/Notes").page,
        Some(format!("{nb}/Home/Notes.md"))
    );
}

#[test]
fn choosing_swallows_closing_brackets_that_follow_the_caret() {
    let (_dir, mut session, nb) = notebook();
    let page = show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    run(&mut session, "text.insert", json!({ "text": "]]" }));
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    type_str(&mut session, "[[Trip");
    assert_eq!(text_of(&session, &page), "[[Trip]]work plan\n");
    run(&mut session, "link.picker.choose", json!({}));
    assert_eq!(text_of(&session, &page), "[[Trip]]work plan\n");
    assert_eq!(session.text_cursor().map(|c| c.head), Some(8));
}

#[test]
fn the_highlight_moves_and_wraps() {
    let (_dir, mut session, nb) = notebook();
    let page = show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    type_str(&mut session, "[[");
    let count = session.link_picker_view().expect("picker").entries.len();
    assert_eq!(count, 4);
    run(&mut session, "link.picker.move", json!({ "delta": -1 }));
    assert_eq!(
        session.link_picker_view().expect("picker").selected,
        count - 1
    );
    run(&mut session, "link.picker.move", json!({ "delta": 1 }));
    assert_eq!(session.link_picker_view().expect("picker").selected, 0);
    run(&mut session, "link.picker.move", json!({ "delta": 2 }));
    let view = session.link_picker_view().expect("picker");
    assert_eq!(view.selected, 2);
    let picked = view.entries[2].page.clone();
    run(&mut session, "link.picker.choose", json!({}));
    let target = picked
        .rsplit('/')
        .next()
        .expect("name")
        .trim_end_matches(".md");
    assert!(text_of(&session, &page).contains(target));
}

#[test]
fn closing_the_picker_leaves_the_text() {
    let (_dir, mut session, nb) = notebook();
    let page = show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    type_str(&mut session, "[[Gro");
    let out = run(&mut session, "link.picker.close", json!({}));
    assert_eq!(out["applied"], true);
    assert!(session.link_picker_view().is_none());
    assert_eq!(text_of(&session, &page), "[[Growork plan\n");
    let again = run(&mut session, "link.picker.close", json!({}));
    assert_eq!(again["applied"], false);
    // Choosing with no picker does nothing.
    let out = run(&mut session, "link.picker.choose", json!({}));
    assert_eq!(out["applied"], false);
}

#[test]
fn the_picker_goes_away_when_the_link_is_finished_or_the_caret_leaves() {
    let (_dir, mut session, nb) = notebook();
    show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    type_str(&mut session, "[[Gro");
    assert!(session.link_picker_view().is_some());
    // Typing the closing brackets, a pipe, or a line break ends it.
    type_str(&mut session, "|");
    assert!(session.link_picker_view().is_none());
    // A click elsewhere closes it for good, even if the caret comes back.
    run(
        &mut session,
        "text.delete",
        json!({ "direction": "backward" }),
    );
    assert!(session.link_picker_view().is_some());
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    assert!(session.link_picker().is_none());
    run(&mut session, "text.set_caret", json!({ "at": 5 }));
    assert!(session.link_picker_view().is_none());
}

#[test]
fn deleting_the_brackets_closes_the_picker() {
    let (_dir, mut session, nb) = notebook();
    show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    type_str(&mut session, "[[");
    assert!(session.link_picker_view().is_some());
    run(
        &mut session,
        "text.delete",
        json!({ "direction": "backward" }),
    );
    assert!(session.link_picker_view().is_none());
}

#[test]
fn brackets_in_code_do_not_open_the_picker() {
    let (_dir, mut session, nb) = open(&[("S/Main.md", "```\ncode\n```\n"), ("S/Other.md", "x")]);
    show(&mut session, &nb, "S/Main.md");
    run(&mut session, "text.set_caret", json!({ "at": 6 }));
    type_str(&mut session, "[[");
    assert!(session.link_picker_view().is_none());
}

#[test]
fn the_open_command_types_the_brackets_when_they_are_missing() {
    let (_dir, mut session, nb) = notebook();
    let page = show(&mut session, &nb, "Work/Plan.md");
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    let out = run(&mut session, "link.picker.open", json!({}));
    assert_eq!(out["applied"], true);
    assert_eq!(text_of(&session, &page), "[[work plan\n");
    assert!(session.link_picker_view().is_some());
    // Typing the brackets is one undo step.
    run(&mut session, "edit.undo", Value::Null);
    assert_eq!(text_of(&session, &page), "work plan\n");
    run(&mut session, "edit.redo", Value::Null);
    // With brackets already before the caret nothing is typed.
    run(&mut session, "link.picker.close", json!({}));
    run(&mut session, "link.picker.open", json!({}));
    assert_eq!(text_of(&session, &page), "[[work plan\n");
    assert!(session.link_picker_view().is_some());
}

#[test]
fn a_page_that_cannot_be_edited_has_no_picker() {
    let (_dir, mut session, _nb) = notebook();
    // No page text was loaded, so there is nothing to edit.
    let out = run(&mut session, "link.picker.open", json!({}));
    assert_eq!(out["applied"], false);
    assert!(session.link_picker().is_none());
}
