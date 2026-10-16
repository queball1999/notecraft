//! Tests for the text editing and container commands (PR 1.5).

use serde_json::{json, Value};

use super::*;
use crate::session::TextCursor;
use crate::undo::tests::session_with_pages;

const PAGE: &str = "p1";

fn session(text: &str) -> Session {
    let mut session = session_with_pages();
    session.set_page_source(PAGE, text);
    session
}

fn run(session: &mut Session, id: &str, args: Value) -> Value {
    match Registry::builtin().run(session, id, args) {
        Ok(out) => out.value,
        Err(err) => panic!("{id} failed: {err}"),
    }
}

fn text(session: &Session) -> String {
    session.page_text(PAGE).unwrap_or_default().to_owned()
}

fn caret(session: &mut Session, at: usize) {
    run(session, "text.set_caret", json!({ "at": at }));
}

fn type_str(session: &mut Session, s: &str) {
    for c in s.chars() {
        run(
            session,
            "text.insert",
            json!({ "text": c.to_string(), "typing": true }),
        );
    }
}

fn cursor(session: &Session) -> TextCursor {
    session.text_cursor().expect("a caret")
}

#[test]
fn typing_inserts_at_the_caret_and_moves_it() {
    let mut s = session("Hello\n");
    caret(&mut s, 5);
    type_str(&mut s, ", you");
    assert_eq!(text(&s), "Hello, you\n");
    assert_eq!(cursor(&s), TextCursor::caret(10));
    assert!(s.is_page_dirty(PAGE));
}

#[test]
fn typing_undoes_one_word_at_a_time() {
    let mut s = session("");
    type_str(&mut s, "one two three");
    assert_eq!(text(&s), "one two three");
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "one two ");
    assert_eq!(cursor(&s), TextCursor::caret(8));
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "one ");
    run(&mut s, "edit.redo", Value::Null);
    assert_eq!(text(&s), "one two ");
    assert_eq!(cursor(&s), TextCursor::caret(8));
}

#[test]
fn only_touched_bytes_change() {
    let original = "# Title\r\n\r\n-  loose   list\r\n\r\ntrailing   \r\n";
    let mut s = session(original);
    caret(&mut s, original.len() - 5);
    type_str(&mut s, "X");
    let edited = text(&s);
    assert_eq!(edited.len(), original.len() + 1);
    assert_eq!(edited.replace('X', ""), original);
}

#[test]
fn selection_moves_with_shift_and_replaces_on_typing() {
    let mut s = session("hello world");
    caret(&mut s, 0);
    run(
        &mut s,
        "text.move",
        json!({ "motion": "word_right", "extend": true }),
    );
    assert_eq!(cursor(&s), TextCursor { anchor: 0, head: 6 });
    run(
        &mut s,
        "text.move",
        json!({ "motion": "right", "extend": true }),
    );
    assert_eq!(cursor(&s).head, 7);
    // Left without Shift collapses to the start of the selection.
    run(&mut s, "text.move", json!({ "motion": "left" }));
    assert_eq!(cursor(&s), TextCursor::caret(0));
    run(
        &mut s,
        "text.move",
        json!({ "motion": "end", "extend": true }),
    );
    run(&mut s, "text.insert", json!({ "text": "bye" }));
    assert_eq!(text(&s), "bye");
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "hello world");
}

#[test]
fn double_and_triple_click_select_a_word_and_a_line() {
    let mut s = session("one two\nthree\n");
    run(
        &mut s,
        "text.set_caret",
        json!({ "at": 5, "select": "word" }),
    );
    assert_eq!(cursor(&s), TextCursor { anchor: 4, head: 7 });
    run(
        &mut s,
        "text.set_caret",
        json!({ "at": 9, "select": "line" }),
    );
    assert_eq!(
        cursor(&s),
        TextCursor {
            anchor: 8,
            head: 13
        }
    );
    run(&mut s, "text.set_caret", json!({ "at": 1 }));
    run(&mut s, "text.set_caret", json!({ "at": 6, "extend": true }));
    assert_eq!(cursor(&s), TextCursor { anchor: 1, head: 6 });
}

#[test]
fn backspace_and_delete_work_by_character_and_word() {
    let mut s = session("alpha beta");
    caret(&mut s, 10);
    run(&mut s, "text.delete", json!({ "direction": "backward" }));
    assert_eq!(text(&s), "alpha bet");
    run(
        &mut s,
        "text.delete",
        json!({ "direction": "backward", "unit": "word" }),
    );
    assert_eq!(text(&s), "alpha ");
    caret(&mut s, 0);
    run(&mut s, "text.delete", json!({ "direction": "forward" }));
    assert_eq!(text(&s), "lpha ");
    run(&mut s, "edit.delete", Value::Null);
    assert_eq!(text(&s), "pha ");
    // Backspace at the start does nothing.
    caret(&mut s, 0);
    let out = run(&mut s, "text.delete", json!({ "direction": "backward" }));
    assert_eq!(out, json!({ "applied": false }));
}

#[test]
fn backspace_runs_undo_together() {
    let mut s = session("abcdef");
    caret(&mut s, 6);
    for _ in 0..3 {
        run(&mut s, "text.delete", json!({ "direction": "backward" }));
    }
    assert_eq!(text(&s), "abc");
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "abcdef");
}

#[test]
fn enter_makes_paragraphs_and_lists_continue() {
    let mut s = session("Title");
    caret(&mut s, 5);
    run(&mut s, "text.enter", json!({}));
    assert_eq!(text(&s), "Title\n\n");
    type_str(&mut s, "- one");
    run(&mut s, "text.enter", json!({}));
    assert_eq!(text(&s), "Title\n\n- one\n- ");
    type_str(&mut s, "two");
    run(&mut s, "text.enter", json!({}));
    run(&mut s, "text.enter", json!({}));
    assert_eq!(text(&s), "Title\n\n- one\n- two\n");
    // The page still reads as the paragraph and a two item list.
    let doc = notecraft_markdown::parse(&text(&s));
    assert_eq!(doc.blocks.len(), 2);
}

#[test]
fn enter_follows_the_pages_line_endings_and_code_fences() {
    let mut s = session("a\r\n");
    caret(&mut s, 1);
    run(&mut s, "text.enter", json!({ "soft": true }));
    assert_eq!(text(&s), "a\r\n\r\n");
    let mut s = session("```\ncode\n```\n");
    caret(&mut s, 8);
    run(&mut s, "text.enter", json!({}));
    assert_eq!(text(&s), "```\ncode\n\n```\n");
}

#[test]
fn tab_indents_and_outdents_list_items() {
    let mut s = session("- a\n- b\n");
    caret(&mut s, 7);
    run(&mut s, "text.tab", json!({}));
    assert_eq!(text(&s), "- a\n  - b\n");
    assert_eq!(cursor(&s), TextCursor::caret(9));
    run(&mut s, "text.tab", json!({ "shift": true }));
    assert_eq!(text(&s), "- a\n- b\n");
    // Outside a list, Tab at the start of a line is ignored.
    let mut s = session("plain");
    caret(&mut s, 0);
    let out = run(&mut s, "text.tab", json!({}));
    assert_eq!(out, json!({ "applied": false }));
}

#[test]
fn copy_cut_and_paste_use_markdown_or_plain_text() {
    let mut s = session("say **hi** now\n");
    run(&mut s, "text.set_caret", json!({ "at": 4 }));
    run(
        &mut s,
        "text.set_caret",
        json!({ "at": 10, "extend": true }),
    );
    assert_eq!(
        run(&mut s, "edit.copy", Value::Null),
        json!({ "text": "**hi**" })
    );
    let cut = run(&mut s, "edit.cut", Value::Null);
    assert_eq!(cut, json!({ "text": "**hi**" }));
    assert_eq!(text(&s), "say  now\n");
    run(&mut s, "edit.paste", json!({ "text": "**hi**\r\nthere" }));
    assert_eq!(text(&s), "say **hi**\nthere now\n");
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "say  now\n");
    // Keep Text Only drops the markdown.
    run(
        &mut s,
        "edit.paste_text_only",
        json!({ "text": "**bold** and [link](http://x.y)" }),
    );
    assert_eq!(text(&s), "say bold and link now\n");
    // Nothing selected: copy and cut return null; paste with no text does nothing.
    assert_eq!(run(&mut s, "edit.copy", Value::Null), Value::Null);
    assert_eq!(run(&mut s, "edit.cut", Value::Null), Value::Null);
    assert_eq!(
        run(&mut s, "edit.paste", json!({})),
        json!({ "applied": false })
    );
    let plain = run(&mut s, "edit.select_all", Value::Null);
    assert_eq!(plain, json!({ "applied": true }));
}

#[test]
fn select_all_takes_the_container_then_the_page() {
    let mut s = session("one\n\n<!-- notecraft:container id=c1 -->\ntwo\n");
    caret(&mut s, 2);
    run(&mut s, "edit.select_all", Value::Null);
    assert_eq!(cursor(&s).range(), 0..5);
    run(&mut s, "edit.select_all", Value::Null);
    let len = text(&s).len();
    assert_eq!(cursor(&s).range(), 0..len);
}

#[test]
fn the_caret_stays_off_marker_lines_and_the_frontmatter() {
    let src = "---\ntitle: T\n---\nbody\n\n<!-- notecraft:container id=c1 -->\nnext\n";
    let mut s = session(src);
    caret(&mut s, 2);
    assert_eq!(cursor(&s).head, 17);
    let marker = src.find("<!--").expect("marker");
    caret(&mut s, marker + 5);
    assert_eq!(cursor(&s).head, src.find("next").expect("next"));
    // Backspace at the start of a container does not eat the marker.
    let out = run(&mut s, "text.delete", json!({ "direction": "backward" }));
    assert_eq!(out, json!({ "applied": false }));
    assert_eq!(text(&s), src);
    // Typing at the end of the previous container keeps the marker on its own line.
    caret(&mut s, marker);
    type_str(&mut s, "x");
    assert!(text(&s).contains("x\n<!-- notecraft:container id=c1 -->\nnext"));
}

#[test]
fn a_selection_cannot_cut_a_marker_in_half() {
    let src = "aaa\n\n<!-- notecraft:container id=c1 -->\nbbb\n";
    let mut s = session(src);
    run(&mut s, "text.set_caret", json!({ "at": 1 }));
    // Select across the whole marker: it goes with the text.
    run(
        &mut s,
        "text.set_caret",
        json!({ "at": 40, "extend": true }),
    );
    run(&mut s, "text.delete", json!({ "direction": "backward" }));
    assert_eq!(text(&s), "abbb\n");
    // A selection end inside a marker line snaps to the edge of the marker, so a marker is
    // taken whole or not at all.
    let mut s = session(src);
    s.set_text_cursor(
        PAGE,
        TextCursor {
            anchor: 1,
            head: 12,
        },
    );
    run(&mut s, "text.insert", json!({ "text": "x" }));
    assert_eq!(text(&s), "axbbb\n");
}

#[test]
fn read_only_pages_are_not_edited() {
    let mut s = session("text");
    s.set_page_notice(PAGE, "read only", true);
    caret(&mut s, 0);
    let out = run(&mut s, "text.insert", json!({ "text": "x" }));
    assert_eq!(out, json!({ "applied": false }));
    assert_eq!(text(&s), "text");
    assert!(!s.edit_page_text(PAGE, 0, 0, "x", false));
    assert!(!s.is_page_dirty(PAGE));
}

#[test]
fn a_new_container_adds_a_marker_and_takes_the_caret() {
    let mut s = session("Existing\n");
    let out = run(&mut s, "container.new", json!({ "x": 200, "y": 150 }));
    assert_eq!(out, json!({ "id": "c1" }));
    assert_eq!(text(&s), "Existing\n\n<!-- notecraft:container id=c1 -->\n");
    assert_eq!(cursor(&s), TextCursor::caret(text(&s).len()));
    let c = s.container(PAGE, "c1");
    assert_eq!(c.pos, Some(notecraft_core::Vec2::new(200.0, 150.0)));
    type_str(&mut s, "note");
    assert!(text(&s).ends_with("-->\nnote"));
    // One undo takes back the typing; another the whole container.
    run(&mut s, "edit.undo", Value::Null);
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "Existing\n");
    assert!(s.page_containers(PAGE).is_empty());
    run(&mut s, "edit.redo", Value::Null);
    assert_eq!(
        s.container(PAGE, "c1").pos,
        Some(notecraft_core::Vec2::new(200.0, 150.0))
    );
}

#[test]
fn clicking_away_from_an_empty_container_removes_it() {
    let mut s = session("Existing\n");
    run(&mut s, "container.new", json!({ "x": 10, "y": 10 }));
    // Leave it by clicking in the first container.
    run(&mut s, "text.set_caret", json!({ "at": 2 }));
    assert_eq!(text(&s), "Existing\n");
    assert_eq!(cursor(&s), TextCursor::caret(2));
    // Another click on empty canvas replaces an empty container with a new one.
    run(&mut s, "container.new", json!({ "x": 10, "y": 10 }));
    run(&mut s, "container.new", json!({ "x": 50, "y": 60 }));
    assert_eq!(text(&s).matches("notecraft:container").count(), 1);
    // A container with text stays.
    type_str(&mut s, "keep");
    run(&mut s, "text.set_caret", json!({ "at": 0 }));
    assert!(text(&s).contains("keep"));
}

#[test]
fn containers_move_and_resize_with_undo() {
    let mut s = session("Text\n");
    run(
        &mut s,
        "container.move",
        json!({ "id": "c0", "x": 30.5, "y": 70 }),
    );
    assert_eq!(
        s.container(PAGE, "c0").pos,
        Some(notecraft_core::Vec2::new(30.5, 70.0))
    );
    run(
        &mut s,
        "container.resize",
        json!({ "id": "c0", "width": 10 }),
    );
    assert_eq!(
        s.container(PAGE, "c0").width,
        notecraft_model::container::MIN_WIDTH
    );
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(
        s.container(PAGE, "c0").width,
        notecraft_model::container::DEFAULT_WIDTH
    );
    run(&mut s, "edit.undo", Value::Null);
    assert!(s.container(PAGE, "c0").in_flow());
    // Text is untouched by any of this, and an unknown id is an error.
    assert_eq!(text(&s), "Text\n");
    let err = Registry::builtin()
        .run(
            &mut s,
            "container.move",
            json!({ "id": "nope", "x": 1, "y": 1 }),
        )
        .expect_err("unknown container");
    assert_eq!(err.code(), "invalid_args");
    assert!(!s.is_page_dirty("p3"));
}

#[test]
fn bad_arguments_are_errors_not_panics() {
    let mut s = session("x");
    let reg = Registry::builtin();
    for (id, args) in [
        ("text.set_caret", json!({ "at": -1 })),
        ("text.set_caret", json!({ "at": 0, "select": "page" })),
        ("text.move", json!({ "motion": "up" })),
        ("text.delete", json!({ "direction": "sideways" })),
        ("text.insert", json!({})),
        ("container.new", json!({ "x": "a", "y": 1 })),
    ] {
        assert!(reg.run(&mut s, id, args).is_err(), "{id}");
    }
    // Odd offsets are snapped, never panic.
    let mut s = session("\u{e9}\u{1F44D}");
    for at in [0, 1, 2, 3, 4, 5, 6, 999, usize::MAX >> 1] {
        run(&mut s, "text.set_caret", json!({ "at": at }));
        run(&mut s, "text.insert", json!({ "text": "z" }));
        run(
            &mut s,
            "text.delete",
            json!({ "direction": "backward", "unit": "word" }),
        );
    }
}

#[test]
fn unsaved_edits_are_flagged_until_saved() {
    let mut s = session("a");
    assert!(!s.is_page_dirty(PAGE));
    caret(&mut s, 1);
    type_str(&mut s, "b");
    assert_eq!(s.dirty_pages().collect::<Vec<_>>(), [PAGE]);
    s.mark_page_saved(PAGE);
    assert!(!s.is_page_dirty(PAGE));
    run(&mut s, "edit.undo", Value::Null);
    assert!(s.is_page_dirty(PAGE));
    // Reading text from disk is not an edit.
    s.set_page_source(PAGE, "fresh");
    assert!(!s.is_page_dirty(PAGE));
}
