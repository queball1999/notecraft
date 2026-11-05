//! Harness tests for the note tags UI (PR 2.5): the Customize Tags dialog, Ctrl+number
//! shortcuts, and the Tags gallery.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use egui_kittest::kittest::{NodeT, Queryable};
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

fn open_dialog(harness: &mut egui_kittest::Harness<'static, AppShell>) {
    run(harness, "tag.customize", serde_json::Value::Null);
    assert!(harness.state().customize_tags_open());
}

fn click(harness: &mut egui_kittest::Harness<'static, AppShell>, label: &str) {
    harness.get_by_label(label).click_accesskit();
    harness.run();
}

fn disabled(harness: &egui_kittest::Harness<'static, AppShell>, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().is_disabled()
}

#[test]
fn a_default_tag_can_be_renamed_restyled_and_reset_in_the_dialog() {
    let mut harness = harness("Hello\n");
    open_dialog(&mut harness);
    assert!(
        harness.query_by_label("Reset").is_none(),
        "nothing to reset yet"
    );
    click(&mut harness, "Idea");
    assert!(harness.query_by_label("Save").is_some());

    // Type a new name over the old one, pick another icon, and save.
    harness
        .get_all_by_label_contains("Display name")
        .last()
        .expect("name box")
        .focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    harness.event(egui::Event::Text("Thought".to_owned()));
    harness.run();
    click(&mut harness, "Save");
    let tag = harness
        .state()
        .tag_set()
        .get("idea")
        .expect("kept slug")
        .clone();
    assert_eq!(tag.name, "Thought");
    assert!(harness.state().tag_set().is_modified("idea"));
    assert!(
        harness.query_by_label("Thought").is_some(),
        "the list shows the new name"
    );

    // Reset brings the built-in look back.
    click(&mut harness, "Reset");
    let tag = harness.state().tag_set().get("idea").expect("tag").clone();
    assert_eq!((tag.name.as_str(), tag.icon), ("Idea", TagIcon::Bulb));
    assert!(harness.query_by_label("Reset").is_none());
}

#[test]
fn delete_hides_a_default_tag_and_reset_tags_brings_it_back_after_a_confirm() {
    let mut harness = harness("#password Secret\n");
    open_dialog(&mut harness);
    click(&mut harness, "Password");
    click(&mut harness, "Delete");
    let set = harness.state().tag_set();
    assert!(set.all().iter().all(|t| t.slug != "password"));
    assert_eq!(set.hidden().len(), 1);
    assert!(set.get("password").is_some(), "pages keep its icon");
    assert!(harness.query_by_label("Password").is_none());

    // Reset Tags asks first; Cancel changes nothing.
    click(&mut harness, "Reset Tags");
    assert_eq!(harness.state().tag_set().hidden().len(), 1);
    click(&mut harness, "Cancel");
    assert_eq!(harness.state().tag_set().hidden().len(), 1);
    click(&mut harness, "Reset Tags");
    click(&mut harness, "Yes, reset");
    assert!(harness.state().tag_set().hidden().is_empty());
    assert!(harness.query_by_label("Password").is_some());
}

#[test]
fn move_up_and_move_down_reorder_the_list_and_the_ctrl_number_shortcuts() {
    let mut harness = harness("Hello\n");
    open_dialog(&mut harness);
    assert!(disabled(&harness, "Move Up"), "nothing picked");
    click(&mut harness, "Question");
    let order = |h: &egui_kittest::Harness<'static, AppShell>| -> Vec<String> {
        h.state()
            .tag_set()
            .all()
            .iter()
            .take(3)
            .map(|t| t.slug.clone())
            .collect()
    };
    click(&mut harness, "Move Up");
    assert_eq!(order(&harness), ["todo", "question", "important"]);
    click(&mut harness, "Move Down");
    assert_eq!(order(&harness), ["todo", "important", "question"]);
    click(&mut harness, "Move Up");
    click(&mut harness, "Move Up");
    assert_eq!(order(&harness), ["question", "todo", "important"]);
    assert!(disabled(&harness, "Move Up"), "at the top");

    // Close the dialog; Ctrl+1 now applies Question.
    harness.key_press(egui::Key::Escape);
    harness.run();
    run(
        &mut harness,
        "text.set_caret",
        serde_json::json!({ "at": 2 }),
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num1);
    harness.run();
    assert_eq!(text(&harness), "#question Hello\n");
}

#[test]
fn the_ribbon_gallery_shows_edited_names_and_leaves_out_hidden_tags() {
    let mut harness = harness("Hello\n");
    run(
        &mut harness,
        "tag.edit",
        serde_json::json!({ "tag": "important", "name": "Urgent" }),
    );
    run(
        &mut harness,
        "tag.hide",
        serde_json::json!({ "tag": "todo" }),
    );
    assert!(harness.query_by_label("Urgent").is_some());
    assert!(harness.query_by_label("Important").is_none());
    assert!(harness.query_by_label("To Do").is_none());
    run(
        &mut harness,
        "text.set_caret",
        serde_json::json!({ "at": 2 }),
    );
    harness.get_by_label("Urgent").click();
    harness.run();
    assert_eq!(text(&harness), "#important Hello\n");
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
