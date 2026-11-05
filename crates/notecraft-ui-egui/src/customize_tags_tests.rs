//! Harness tests for the note tags UI (PR 2.5): the Customize Tags dialog, Ctrl+number
//! shortcuts, and the Tags gallery.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use egui_kittest::kittest::Queryable;
use notecraft_engine::mock::WELCOME_ID;
use notecraft_markdown::TagIcon;

use crate::AppShell;

const WINDOW: egui::Vec2 = egui::vec2(1400.0, 900.0);

fn harness(text: &str) -> egui_kittest::Harness<'static, AppShell> {
    let mut shell = AppShell::new();
    shell.set_page_source(WELCOME_ID, text);
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
    harness
}

fn text(harness: &egui_kittest::Harness<'static, AppShell>) -> String {
    harness
        .state()
        .session()
        .page_text(WELCOME_ID)
        .unwrap_or_default()
        .to_owned()
}

fn run(harness: &mut egui_kittest::Harness<'static, AppShell>, id: &str, args: serde_json::Value) {
    assert!(harness.state_mut().dispatch(id, args), "{id} failed");
    harness.run();
}

#[test]
fn customize_tags_opens_lists_adds_edits_and_deletes() {
    let mut harness = harness("Hello\n");
    run(&mut harness, "tag.customize", serde_json::Value::Null);
    assert!(harness.state().customize_tags_open());
    assert!(harness.query_by_label("Question").is_some());
    assert!(harness.query_by_label("Idea").is_some());

    // Add through the dialog's own button, name typed into the box.
    harness.get_by_label("New Tag").click_accesskit();
    harness.run();
    harness
        .get_all_by_label_contains("Display name")
        .last()
        .expect("name box")
        .focus();
    harness.run();
    harness.event(egui::Event::Text("Errand".to_owned()));
    harness.run();
    harness.get_by_label("Save").click_accesskit();
    harness.run();
    let set = harness.state().tag_set();
    assert_eq!(set.custom_count(), 1);
    assert_eq!(set.get("errand").map(|t| t.icon), Some(TagIcon::Dot));

    // Pick it and delete it.
    harness.get_by_label("Errand").click_accesskit();
    harness.run();
    harness.get_by_label("Delete").click_accesskit();
    harness.run();
    assert_eq!(harness.state().tag_set().custom_count(), 0);

    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!harness.state().customize_tags_open());
}

#[test]
fn a_default_tag_has_no_edit_buttons() {
    let mut harness = harness("Hello\n");
    run(&mut harness, "tag.customize", serde_json::Value::Null);
    harness.get_by_label("Question").click();
    harness.run();
    assert!(harness.query_by_label("Save").is_none());
    assert!(harness.query_by_label("Delete").is_none());
}

#[test]
fn ctrl_number_shortcuts_tag_the_paragraph_at_the_caret() {
    let mut harness = harness("Hello\n");
    run(
        &mut harness,
        "text.set_caret",
        serde_json::json!({ "at": 2 }),
    );
    let command = egui::Modifiers::COMMAND;
    harness.key_press_modifiers(command, egui::Key::Num2);
    harness.run();
    assert_eq!(text(&harness), "#important Hello\n");
    harness.key_press_modifiers(command, egui::Key::Num3);
    harness.run();
    assert_eq!(text(&harness), "#important #question Hello\n");
    harness.key_press_modifiers(command, egui::Key::Num2);
    harness.run();
    assert_eq!(text(&harness), "#question Hello\n");
    harness.key_press_modifiers(command, egui::Key::Num0);
    harness.run();
    assert_eq!(text(&harness), "Hello\n");
    harness.key_press_modifiers(command, egui::Key::Num1);
    harness.run();
    assert_eq!(text(&harness), "- [ ] Hello\n");
    // One undo step each.
    run(&mut harness, "edit.undo", serde_json::Value::Null);
    assert_eq!(text(&harness), "Hello\n");
}

#[test]
fn the_tags_gallery_lists_the_default_tags_and_applies_one() {
    let mut harness = harness("Hello\n");
    run(
        &mut harness,
        "text.set_caret",
        serde_json::json!({ "at": 2 }),
    );
    // The gallery shows the first entries with their shortcuts.
    assert!(harness.query_by_label("To Do").is_some());
    assert!(harness.query_by_label("Important").is_some());
    harness.get_by_label("Important").click();
    harness.run();
    assert_eq!(text(&harness), "#important Hello\n");
}
