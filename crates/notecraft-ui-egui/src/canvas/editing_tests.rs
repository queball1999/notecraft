//! Typing, selection, and container tests with the egui_kittest harness: synthetic pointer and
//! keyboard events drive the real window, and the page text is read back from the session.
//! No live window or OS input is involved.

use egui_kittest::kittest::Queryable;
use notecraft_engine::cmd::Value;
use serde_json::json;

use crate::app_shell::AppShell;

type Harness = egui_kittest::Harness<'static, AppShell>;

const PAGE: &str = notecraft_engine::mock::WELCOME_ID;

fn harness(text: &str) -> Harness {
    let mut shell = AppShell::new();
    shell.set_page_source(PAGE, text);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
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

fn text(harness: &Harness) -> String {
    harness
        .state()
        .session()
        .page_text(PAGE)
        .unwrap_or_default()
        .to_owned()
}

fn canvas_origin(harness: &Harness) -> egui::Pos2 {
    harness.get_by_label("Canvas").rect().min
}

/// A click at a page point (zoom 100%, scrolled to the top).
fn click_page(harness: &mut Harness, x: f32, y: f32) {
    let at = canvas_origin(harness) + egui::vec2(x, y);
    harness.hover_at(at);
    harness.step();
    harness.drag_at(at);
    harness.step();
    harness.drop_at(at);
    harness.step();
    harness.step();
}

fn type_text(harness: &mut Harness, s: &str) {
    harness.event(egui::Event::Text(s.to_owned()));
    harness.step();
}

#[test]
fn clicking_text_then_typing_edits_the_page_and_undo_restores_it() {
    let mut harness = harness("Hello world\n");
    // The first line is at the top of the text column.
    click_page(&mut harness, 120.0, 135.0);
    assert!(harness.state().session().text_cursor().is_some());
    type_text(&mut harness, "X");
    let edited = text(&harness);
    assert!(edited.contains('X'));
    assert_eq!(edited.len(), "Hello world\n".len() + 1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();
    assert_eq!(text(&harness), "Hello world\n");
    assert!(harness.state().session().is_page_dirty(PAGE));
}

#[test]
fn keys_move_select_and_delete() {
    let mut harness = harness("one two\n");
    click_page(&mut harness, 300.0, 135.0);
    harness.key_press(egui::Key::Home);
    harness.run();
    assert_eq!(
        harness.state().session().text_cursor().map(|c| c.head),
        Some(0)
    );
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::ArrowRight);
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::ArrowRight);
    harness.run();
    let cursor = harness.state().session().text_cursor().expect("caret");
    assert_eq!((cursor.anchor, cursor.head), (0, 2));
    harness.key_press(egui::Key::Delete);
    harness.run();
    assert_eq!(text(&harness), "e two\n");
    harness.key_press(egui::Key::End);
    harness.key_press(egui::Key::Backspace);
    harness.run();
    assert_eq!(text(&harness), "e tw\n");
}

#[test]
fn enter_and_ime_commit_insert_text() {
    let mut harness = harness("a\n");
    click_page(&mut harness, 300.0, 135.0);
    harness.key_press(egui::Key::Home);
    harness.event(egui::Event::Ime(egui::ImeEvent::Preedit {
        text: "ni".to_owned(),
        active_range_chars: None,
    }));
    harness.step();
    // A composition is shown but is not in the page yet.
    assert_eq!(text(&harness), "a\n");
    harness.event(egui::Event::Ime(egui::ImeEvent::Commit(
        "\u{4f60}".to_owned(),
    )));
    harness.step();
    harness.run();
    assert_eq!(text(&harness), "\u{4f60}a\n");
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert!(text(&harness).starts_with("\u{4f60}\n\n"));
}

#[test]
fn clipboard_events_copy_cut_and_paste() {
    let mut harness = harness("alpha beta\n");
    click_page(&mut harness, 300.0, 135.0);
    harness.key_press(egui::Key::Home);
    for _ in 0..5 {
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::ArrowRight);
    }
    harness.run();
    harness.event(egui::Event::Cut);
    harness.run();
    assert_eq!(text(&harness), " beta\n");
    harness.event(egui::Event::Paste("**x**".to_owned()));
    harness.run();
    assert_eq!(text(&harness), "**x** beta\n");
}

#[test]
fn clicking_empty_canvas_makes_a_container_there() {
    let mut harness = harness("Hello\n");
    click_page(&mut harness, 300.0, 500.0);
    let session = harness.state().session();
    let source = session.page_text(PAGE).unwrap_or_default();
    assert!(
        source.contains("<!-- notecraft:container id=c1 -->"),
        "{source:?}"
    );
    let container = session.container(PAGE, "c1");
    let pos = container.pos.expect("placed");
    assert!(
        (pos.x - 300.0).abs() < 2.0 && (pos.y - 500.0).abs() < 2.0,
        "{pos:?}"
    );
    type_text(&mut harness, "note");
    assert!(text(&harness).ends_with("-->\nnote"));
}

#[test]
fn dragging_the_top_bar_moves_a_container_and_its_edge_resizes_it() {
    let mut harness = harness("Hello\n");
    click_page(&mut harness, 300.0, 500.0);
    type_text(&mut harness, "box");
    harness.run();
    let origin = canvas_origin(&harness);
    // The bar sits just above the container.
    let bar = origin + egui::vec2(350.0, 500.0 - 8.0);
    harness.hover_at(bar);
    harness.step();
    harness.drag_at(bar);
    harness.step();
    for i in 1..=10 {
        harness.hover_at(bar + egui::vec2(-8.0 * i as f32, 3.0 * i as f32));
        harness.step();
    }
    harness.drop_at(bar + egui::vec2(-80.0, 30.0));
    harness.step();
    harness.step();
    let pos = harness
        .state()
        .session()
        .container(PAGE, "c1")
        .pos
        .expect("placed");
    assert!(
        (pos.x - 220.0).abs() < 6.0 && (pos.y - 530.0).abs() < 6.0,
        "{pos:?}"
    );
    // Resize by the right edge.
    let width = harness.state().session().container(PAGE, "c1").width;
    let edge = origin + egui::vec2(pos.x + width + 3.0, pos.y + 6.0);
    harness.hover_at(edge);
    harness.step();
    harness.drag_at(edge);
    harness.step();
    for i in 1..=6 {
        harness.hover_at(edge + egui::vec2(-10.0 * i as f32, 0.0));
        harness.step();
    }
    harness.drop_at(edge + egui::vec2(-60.0, 0.0));
    harness.step();
    harness.step();
    let narrower = harness.state().session().container(PAGE, "c1").width;
    assert!(
        (width - narrower - 60.0).abs() < 8.0,
        "{width} -> {narrower}"
    );
    // Both are undoable.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();
    assert_eq!(harness.state().session().container(PAGE, "c1").width, width);
}

#[test]
fn a_read_only_page_ignores_typing() {
    let mut harness = harness("fixed\n");
    harness
        .state_mut()
        .set_page_notice(PAGE, "Cut short.", true);
    harness.run();
    click_page(&mut harness, 300.0, 135.0);
    type_text(&mut harness, "X");
    assert_eq!(text(&harness), "fixed\n");
    let _ = (json!({}), Value::Null);
}
