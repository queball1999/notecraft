//! Harness tests for page links (LK-02): clicking a link follows it, a missing page is created,
//! and typing `[[` opens the page picker. Synthetic pointer and key events drive the real
//! window; the session is read back for what happened.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use egui_kittest::kittest::Queryable;
use notecraft_engine::cmd::Value;
use serde_json::json;

use crate::canvas::link_picker::LABEL as PICKER;
use crate::{AppShell, FilePicker};

const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

#[derive(Debug, Default, Clone)]
struct Fake {
    folder: Option<String>,
}

impl FilePicker for Fake {
    fn pick_folder(&mut self, _title: &str) -> Option<String> {
        self.folder.clone()
    }
    fn pick_file(&mut self, _title: &str) -> Option<String> {
        None
    }
}

struct Setup {
    dir: tempfile::TempDir,
    harness: egui_kittest::Harness<'static, AppShell>,
    notebook: String,
}

/// A notebook folder with the given files, opened in the app, with `show` selected.
fn setup(files: &[(&str, &str)], show: &str) -> Setup {
    let dir = tempfile::TempDir::new().unwrap();
    for (rel, text) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let fake = Fake {
        folder: Some(dir.path().to_string_lossy().into_owned()),
    };
    let shell = AppShell::empty().with_picker(Box::new(fake));
    let mut harness = egui_kittest::Harness::builder()
        .with_size(WINDOW)
        .build_ui_state(
            |ui, shell: &mut AppShell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
            },
            shell,
        );
    harness.run();
    harness.get_by_label("Open Notebook").click();
    harness.run();
    let notebook = harness
        .state()
        .session()
        .selection()
        .notebook
        .clone()
        .unwrap();
    let mut s = Setup {
        dir,
        harness,
        notebook,
    };
    select(&mut s, show);
    s
}

fn select(s: &mut Setup, path: &str) {
    let id = format!("{}/{path}", s.notebook);
    s.harness
        .state_mut()
        .dispatch("nav.page.select", json!({ "id": id }));
    s.harness.run_steps(8);
}

fn current_path(s: &Setup) -> String {
    s.harness
        .state()
        .session()
        .current_page_path()
        .unwrap_or_default()
}

fn page_text(s: &Setup) -> String {
    s.harness
        .state()
        .session()
        .current_page_source()
        .map(|p| p.text.to_owned())
        .unwrap_or_default()
}

fn click_page(s: &mut Setup, x: f32, y: f32) {
    let at = s.harness.get_by_label("Canvas").rect().min + egui::vec2(x, y);
    s.harness.hover_at(at);
    s.harness.step();
    s.harness.drag_at(at);
    s.harness.step();
    s.harness.drop_at(at);
    s.harness.step();
    s.harness.step();
}

fn type_text(s: &mut Setup, text: &str) {
    s.harness.event(egui::Event::Text(text.to_owned()));
    s.harness.step();
    s.harness.step();
}

fn exists(s: &Setup, rel: &str) -> bool {
    Path::new(s.dir.path()).join(rel).exists()
}

/// The first line of text starts at the top left of the text column: x 48, y 124.
const ON_FIRST_WORD: (f32, f32) = (62.0, 134.0);

#[test]
fn clicking_a_link_shows_the_page_it_points_to() {
    let mut s = setup(
        &[
            ("Notes/First.md", "[[Second]] and more\n"),
            ("Notes/Second.md", "the second page\n"),
        ],
        "Notes/First.md",
    );
    click_page(&mut s, ON_FIRST_WORD.0, ON_FIRST_WORD.1);
    assert_eq!(current_path(&s), "Notes/Second.md");
    assert!(page_text(&s).contains("the second page"));
    // A click on a link does not start editing the page it was on.
    assert!(s.harness.state().session().text_cursor().is_none());
}

#[test]
fn clicking_a_link_in_another_section_changes_the_section() {
    let mut s = setup(
        &[
            ("Notes/First.md", "[[Other/Deep|read this]]\n"),
            ("Other/Deep.md", "deep\n"),
        ],
        "Notes/First.md",
    );
    click_page(&mut s, ON_FIRST_WORD.0, ON_FIRST_WORD.1);
    assert_eq!(current_path(&s), "Other/Deep.md");
    let section = s.harness.state().session().selection().section.clone();
    assert_eq!(section, Some(format!("{}/Other", s.notebook)));
}

#[test]
fn clicking_a_link_to_a_missing_page_creates_it_and_undo_removes_it() {
    let mut s = setup(
        &[("Notes/First.md", "[[Brand new]] is not there\n")],
        "Notes/First.md",
    );
    assert!(!exists(&s, "Notes/Brand new.md"));
    click_page(&mut s, ON_FIRST_WORD.0, ON_FIRST_WORD.1);
    assert!(exists(&s, "Notes/Brand new.md"));
    assert_eq!(current_path(&s), "Notes/Brand new.md");
    s.harness.state_mut().dispatch("edit.undo", Value::Null);
    s.harness.run_steps(6);
    assert!(!exists(&s, "Notes/Brand new.md"));
    assert_eq!(current_path(&s), "Notes/First.md");
}

#[test]
fn an_ambiguous_link_opens_the_closest_page() {
    let mut s = setup(
        &[
            ("Notes/First.md", "[[Dup]]\n"),
            ("Far/Dup.md", "far\n"),
            ("Notes/Sub.md", "x\n"),
            ("Near/Dup.md", "near\n"),
        ],
        "Notes/First.md",
    );
    click_page(&mut s, ON_FIRST_WORD.0, ON_FIRST_WORD.1);
    // Both are one folder away from Notes; the shorter path wins the tie, then the name.
    assert_eq!(current_path(&s), "Far/Dup.md");
}

#[test]
fn a_link_in_the_block_with_the_caret_is_raw_text_and_does_not_follow() {
    let mut s = setup(
        &[
            ("Notes/First.md", "[[Second]] and more\n"),
            ("Notes/Second.md", "two\n"),
        ],
        "Notes/First.md",
    );
    // Put the caret in the block (far to the right of the link, past its end).
    click_page(&mut s, 400.0, 134.0);
    assert!(s.harness.state().session().text_cursor().is_some());
    assert_eq!(current_path(&s), "Notes/First.md");
    // The block shows its source now, so a click on the brackets edits.
    click_page(&mut s, ON_FIRST_WORD.0, ON_FIRST_WORD.1);
    assert_eq!(current_path(&s), "Notes/First.md");
}

#[test]
fn typing_double_brackets_opens_the_picker_and_enter_inserts_a_link() {
    let mut s = setup(
        &[
            ("Notes/Plain.md", "plain\n"),
            ("Notes/Second.md", "two\n"),
            ("Notes/Third.md", "three\n"),
        ],
        "Notes/Plain.md",
    );
    assert!(s.harness.query_by_label(PICKER).is_none());
    click_page(&mut s, 300.0, 134.0);
    type_text(&mut s, "[");
    assert!(s.harness.query_by_label(PICKER).is_none());
    type_text(&mut s, "[");
    assert!(s.harness.query_by_label(PICKER).is_some());
    // Typing filters the list.
    type_text(&mut s, "Sec");
    assert!(s.harness.query_by_label("Second, Notes").is_some());
    assert!(s.harness.query_by_label("Third, Notes").is_none());
    s.harness.key_press(egui::Key::Enter);
    s.harness.run_steps(4);
    assert!(page_text(&s).contains("[[Second]]"), "{}", page_text(&s));
    assert!(s.harness.query_by_label(PICKER).is_none());
    // The link is a real one: it shows as a link, and Undo takes it out as one step.
    s.harness.state_mut().dispatch("edit.undo", Value::Null);
    s.harness.run_steps(4);
    assert!(page_text(&s).contains("[[Sec"));
    assert!(!page_text(&s).contains("[[Second]]"));
}

#[test]
fn the_arrow_keys_move_the_highlight_and_a_click_chooses() {
    let mut s = setup(
        &[
            ("Notes/Plain.md", "plain\n"),
            ("Notes/Second.md", "two\n"),
            ("Notes/Third.md", "three\n"),
        ],
        "Notes/Plain.md",
    );
    click_page(&mut s, 300.0, 134.0);
    type_text(&mut s, "[[");
    assert_eq!(
        s.harness
            .state()
            .session()
            .link_picker_view()
            .unwrap()
            .selected,
        0
    );
    s.harness.key_press(egui::Key::ArrowDown);
    s.harness.run_steps(3);
    assert_eq!(
        s.harness
            .state()
            .session()
            .link_picker_view()
            .unwrap()
            .selected,
        1
    );
    s.harness.key_press(egui::Key::ArrowUp);
    s.harness.run_steps(3);
    assert_eq!(
        s.harness
            .state()
            .session()
            .link_picker_view()
            .unwrap()
            .selected,
        0
    );
    // A click on a row inserts that page.
    s.harness.get_by_label("Third, Notes").click();
    s.harness.run_steps(4);
    assert!(page_text(&s).contains("[[Third]]"), "{}", page_text(&s));
    assert!(s.harness.query_by_label(PICKER).is_none());
}

#[test]
fn escape_closes_the_picker_and_keeps_what_was_typed() {
    let mut s = setup(
        &[("Notes/Plain.md", "plain\n"), ("Notes/Second.md", "two\n")],
        "Notes/Plain.md",
    );
    click_page(&mut s, 300.0, 134.0);
    type_text(&mut s, "[[Se");
    assert!(s.harness.query_by_label(PICKER).is_some());
    s.harness.key_press(egui::Key::Escape);
    s.harness.run_steps(4);
    assert!(s.harness.query_by_label(PICKER).is_none());
    assert!(page_text(&s).contains("[[Se"));
    // Typing on does not bring it back.
    type_text(&mut s, "c");
    assert!(s.harness.query_by_label(PICKER).is_none());
}

#[test]
fn a_picker_with_no_match_says_so() {
    let mut s = setup(
        &[("Notes/Plain.md", "plain\n"), ("Notes/Second.md", "two\n")],
        "Notes/Plain.md",
    );
    click_page(&mut s, 300.0, 134.0);
    type_text(&mut s, "[[zzz");
    assert!(s.harness.query_by_label("No pages match.").is_some());
    // Enter with nothing to choose leaves the text and the picker alone.
    s.harness.key_press(egui::Key::Enter);
    s.harness.run_steps(3);
    assert!(page_text(&s).contains("[[zzz"));
}
