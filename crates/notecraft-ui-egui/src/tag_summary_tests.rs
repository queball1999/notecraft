//! Harness tests for the Tags Summary pane (LK-07): Home > Find Tags opens it, items are grouped,
//! a click opens the page at the paragraph, and Create Summary Page writes a page.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use egui_kittest::kittest::Queryable;
use serde_json::json;

use crate::AppShell;

const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// A notebook folder with tagged pages and its tags indexed: Work has Alpha (two To Do items,
/// one checked, and an idea) and Home has Gamma (a question).
fn tags_shell() -> (tempfile::TempDir, egui_kittest::Harness<'static, AppShell>) {
    let dir = tempfile::TempDir::new().unwrap();
    for (rel, text) in [
        (
            "Work/Alpha.md",
            "Intro line\n\n- [ ] call Sam\n- [x] pay rent\n\n#idea new logo\n",
        ),
        ("Home/Gamma.md", "#question why is the sky blue\n"),
    ] {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    notecraft_engine::tags_index::refresh_tag_index(dir.path()).unwrap();
    let mut shell = AppShell::empty();
    assert!(shell.open_path(dir.path()));
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
    (dir, harness)
}

fn page_id(harness: &egui_kittest::Harness<'static, AppShell>, title: &str) -> String {
    harness
        .state()
        .session()
        .notebooks()
        .iter()
        .flat_map(|nb| nb.all_sections())
        .flat_map(|s| s.pages.iter())
        .find(|p| p.title == title)
        .map(|p| p.id.clone())
        .unwrap()
}

fn select(harness: &mut egui_kittest::Harness<'static, AppShell>, title: &str) {
    let id = page_id(harness, title);
    harness
        .state_mut()
        .dispatch("nav.page.select", json!({ "id": id }));
    harness.run();
}

#[test]
fn find_tags_opens_the_pane_and_lists_items_by_tag() {
    let (_dir, mut harness) = tags_shell();
    select(&mut harness, "Alpha");
    assert!(!harness.state().session().tag_summary().open);
    harness.get_by_label("Find Tags").click();
    harness.run();
    assert!(harness.state().session().tag_summary().open);
    assert!(harness.query_by_label("Tags Summary").is_some());
    assert!(harness.query_by_label("To Do (2)").is_some());
    assert!(harness.query_by_label("Idea (1)").is_some());
    assert!(harness.query_by_label("call Sam").is_some());
    assert!(harness.query_by_label("pay rent").is_some());
    assert!(harness.query_by_label("new logo").is_some());
    // The default scope is the notebook, so Gamma's question is listed too.
    assert!(harness.query_by_label("why is the sky blue").is_some());
    assert!(harness.query_by_label("4 notes").is_some());
}

#[test]
fn scope_group_and_unchecked_change_the_list() {
    let (_dir, mut harness) = tags_shell();
    select(&mut harness, "Alpha");
    harness.state_mut().dispatch("tag.find", json!({}));
    harness
        .state_mut()
        .dispatch("tag.summary.set_scope", json!({ "scope": "section" }));
    harness.run();
    assert!(harness.query_by_label("3 notes").is_some());
    assert!(harness.query_by_label("why is the sky blue").is_none());
    harness
        .state_mut()
        .dispatch("tag.summary.set_group", json!({ "group": "title" }));
    harness.run();
    assert!(harness.query_by_label("Alpha (3)").is_some());
    harness.get_by_label("Show only unchecked items").click();
    harness.run();
    assert!(harness.state().session().tag_summary().only_unchecked);
    assert!(harness.query_by_label("pay rent").is_none());
    assert!(harness.query_by_label("2 notes").is_some());
}

#[test]
fn clicking_an_item_opens_its_page_with_the_caret_at_the_paragraph() {
    let (_dir, mut harness) = tags_shell();
    select(&mut harness, "Alpha");
    harness.state_mut().dispatch("tag.find", json!({}));
    harness.run();
    harness.get_by_label("why is the sky blue").click();
    harness.run();
    harness.run();
    let gamma = page_id(&harness, "Gamma");
    assert_eq!(
        harness.state().session().selection().page.as_deref(),
        Some(gamma.as_str())
    );
    assert_eq!(harness.state().session().text_cursor().unwrap().head, 0);
    // The second To Do item of Alpha: its text starts after the "- [x] " marker.
    harness.get_by_label("pay rent").click();
    harness.run();
    harness.run();
    let alpha = page_id(&harness, "Alpha");
    assert_eq!(
        harness.state().session().selection().page.as_deref(),
        Some(alpha.as_str())
    );
    let at = "Intro line\n\n- [ ] call Sam\n- [x] ".len();
    assert_eq!(harness.state().session().text_cursor().unwrap().head, at);
}

#[test]
fn create_summary_page_adds_a_page_and_close_hides_the_pane() {
    let (dir, mut harness) = tags_shell();
    select(&mut harness, "Alpha");
    harness.state_mut().dispatch("tag.find", json!({}));
    harness.run();
    harness.get_by_label("Create Summary Page").click();
    harness.run();
    let text = std::fs::read_to_string(dir.path().join("Work/Tags Summary.md")).unwrap();
    assert!(text.contains("- [ ] call Sam [[Alpha]]"), "{text}");
    assert!(
        text.contains("- #question why is the sky blue [[Gamma]]"),
        "{text}"
    );
    let made = page_id(&harness, "Tags Summary");
    assert_eq!(
        harness.state().session().selection().page.as_deref(),
        Some(made.as_str())
    );
    harness.get_by_label("Close").click();
    harness.run();
    assert!(!harness.state().session().tag_summary().open);
}
