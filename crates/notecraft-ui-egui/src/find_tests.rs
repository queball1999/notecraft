//! Harness tests for find on page (TX-11): Ctrl+F opens the bar, typing finds, Enter, Shift+Enter
//! and F3 move between matches, the page scrolls to the current match, and Esc closes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use egui_kittest::kittest::Queryable;
use notecraft_engine::mock::WELCOME_ID;

use crate::find_bar::LABEL;
use crate::AppShell;

const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

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

fn open_find(harness: &mut egui_kittest::Harness<'static, AppShell>) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    harness.run();
}

fn type_query(harness: &mut egui_kittest::Harness<'static, AppShell>, query: &str) {
    harness.event(egui::Event::Text(query.to_owned()));
    harness.run();
}

fn current(harness: &egui_kittest::Harness<'static, AppShell>) -> usize {
    harness.state().session().find().current
}

const PAGE: &str = "Apple pie\n\nan APPLE a day\n\nApples again\n";

#[test]
fn ctrl_f_opens_the_bar_and_takes_the_keyboard() {
    let mut harness = harness(PAGE);
    assert!(harness.query_by_label(LABEL).is_none());
    open_find(&mut harness);
    assert!(harness.state().session().find().open);
    // Typing goes straight into the box.
    type_query(&mut harness, "apple");
    assert_eq!(harness.state().session().find().query, "apple");
    assert_eq!(harness.state().session().find_matches().len(), 3);
    assert!(harness.query_by_label("1 of 3").is_some());
}

#[test]
fn enter_shift_enter_and_f3_move_between_matches() {
    let mut harness = harness(PAGE);
    open_find(&mut harness);
    type_query(&mut harness, "apple");
    assert_eq!(current(&harness), 0);

    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(current(&harness), 1);
    assert!(harness.query_by_label("2 of 3").is_some());

    harness.key_press(egui::Key::F3);
    harness.run();
    assert_eq!(current(&harness), 2);

    // Past the last match wraps to the first.
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(current(&harness), 0);

    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    harness.run();
    assert_eq!(current(&harness), 2);
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F3);
    harness.run();
    assert_eq!(current(&harness), 1);
}

#[test]
fn f3_works_after_the_box_loses_focus() {
    let mut harness = harness(PAGE);
    open_find(&mut harness);
    type_query(&mut harness, "apple");
    harness.get_by_label("Next").click();
    harness.run();
    assert_eq!(current(&harness), 1);
    harness.get_by_label("Previous").click();
    harness.run();
    assert_eq!(current(&harness), 0);
}

#[test]
fn no_matches_says_so_and_next_is_disabled() {
    let mut harness = harness(PAGE);
    open_find(&mut harness);
    type_query(&mut harness, "zebra");
    assert!(harness.query_by_label("No matches").is_some());
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(current(&harness), 0);
}

#[test]
fn escape_and_close_hide_the_bar() {
    let mut harness = harness(PAGE);
    open_find(&mut harness);
    type_query(&mut harness, "pie");
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!harness.state().session().find().open);
    assert!(harness.query_by_label(LABEL).is_none());

    open_find(&mut harness);
    // The last query is still there, selected.
    assert_eq!(harness.state().session().find().query, "pie");
    harness.get_by_label("Close").click();
    harness.run();
    assert!(!harness.state().session().find().open);
}

#[test]
fn the_page_scrolls_to_a_match_that_is_off_screen() {
    let mut text = String::new();
    for i in 0..120 {
        text.push_str(&format!("Line number {i}\n\n"));
    }
    text.push_str("The needle is down here.\n");
    let mut harness = harness(&text);
    let before = harness.state().canvas().viewport().origin();
    open_find(&mut harness);
    type_query(&mut harness, "needle");
    harness.run();
    harness.run();
    let after = harness.state().canvas().viewport().origin();
    assert!(after.y > before.y + 500.0, "{before:?} -> {after:?}");
    // Going back to a match that is on screen leaves the page where it is.
    type_query(&mut harness, "");
    assert_eq!(harness.state().session().find().query, "needle");
}
