//! Tests for the text formatting commands (PR 1.6).

use serde_json::{json, Value};

use super::*;
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

fn select(session: &mut Session, from: usize, to: usize) {
    run(session, "text.set_caret", json!({ "at": from }));
    if to != from {
        run(
            session,
            "text.set_caret",
            json!({ "at": to, "extend": true }),
        );
    }
}

fn applied(value: &Value) -> bool {
    value.get("applied").and_then(Value::as_bool) == Some(true)
}

const FORMAT_IDS: [&str; 20] = [
    "format.font_family",
    "format.font_size",
    "format.bullets",
    "format.numbering",
    "format.outdent",
    "format.indent",
    "format.clear",
    "format.bold",
    "format.italic",
    "format.underline",
    "format.strikethrough",
    "format.subscript",
    "format.superscript",
    "format.highlight",
    "format.font_color",
    "format.align_left",
    "format.align_center",
    "format.align_right",
    "style.apply",
    "tag.todo",
];

#[test]
fn every_formatting_command_is_ready_and_undoable() {
    let registry = Registry::builtin();
    for id in FORMAT_IDS {
        let spec = registry.get(id).expect(id);
        assert_eq!(spec.status, Status::Ready, "{id}");
        assert!(spec.undoable, "{id}");
    }
}

#[test]
fn bold_wraps_the_selection_in_one_undo_step() {
    let mut s = session("one two three\n");
    select(&mut s, 4, 7);
    assert!(applied(&run(&mut s, "format.bold", Value::Null)));
    assert_eq!(text(&s), "one **two** three\n");
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "one two three\n");
    run(&mut s, "edit.redo", Value::Null);
    assert_eq!(text(&s), "one **two** three\n");
}

#[test]
fn a_caret_formats_the_word_at_the_caret() {
    let mut s = session("one two three\n");
    select(&mut s, 5, 5);
    run(&mut s, "format.italic", Value::Null);
    assert_eq!(text(&s), "one *two* three\n");
    run(&mut s, "format.italic", Value::Null);
    assert_eq!(text(&s), "one two three\n");
}

#[test]
fn each_application_is_its_own_undo_step() {
    let mut s = session("one two\n");
    select(&mut s, 0, 3);
    run(&mut s, "format.bold", Value::Null);
    run(&mut s, "format.italic", Value::Null);
    run(&mut s, "format.underline", Value::Null);
    assert_eq!(
        text(&s),
        "***<u>one</u>*** two
"
    );
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "***one*** two\n");
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "**one** two\n");
}

#[test]
fn only_the_touched_bytes_change() {
    let original = "# Title\r\n\r\n-  loose   list\r\n\r\ntrailing   word\r\n";
    let mut s = session(original);
    let at = original.find("word").expect("word");
    select(&mut s, at, at + 4);
    run(&mut s, "format.bold", Value::Null);
    assert_eq!(text(&s), original.replace("word", "**word**"));
}

#[test]
fn color_highlight_font_and_size_use_the_format_spec() {
    let mut s = session("hi\n");
    select(&mut s, 0, 2);
    run(&mut s, "format.font_color", json!({ "color": "#e53935" }));
    assert_eq!(text(&s), "<span style=\"color:#E53935\">hi</span>\n");
    run(&mut s, "format.font_color", json!({ "color": "none" }));
    assert_eq!(text(&s), "hi\n");
    run(&mut s, "format.highlight", Value::Null);
    assert_eq!(text(&s), "==hi==\n");
    run(&mut s, "format.highlight", json!({ "color": "#00ff00" }));
    assert_eq!(text(&s), "<mark style=\"background:#00FF00\">hi</mark>\n");
    run(&mut s, "format.highlight", json!({ "color": "none" }));
    run(&mut s, "format.font_size", json!({ "points": 14 }));
    assert_eq!(text(&s), "<span style=\"font-size:14pt\">hi</span>\n");
    run(
        &mut s,
        "format.font_family",
        json!({ "name": "Courier New" }),
    );
    assert!(text(&s).contains("font-family:Courier New"));
}

#[test]
fn bad_arguments_are_rejected() {
    let mut s = session("hi\n");
    select(&mut s, 0, 2);
    let registry = Registry::builtin();
    for (id, args) in [
        ("format.font_color", json!({ "color": "red" })),
        ("format.highlight", json!({ "color": "x" })),
        ("format.font_size", json!({ "points": 0 })),
        ("format.font_size", json!({ "points": 99999 })),
        ("format.font_family", json!({ "name": "a\"b" })),
        ("style.apply", json!({ "style": "heading9" })),
    ] {
        assert!(
            matches!(
                registry.run(&mut s, id, args),
                Err(CommandError::InvalidArgs { .. })
            ),
            "{id}"
        );
    }
    assert_eq!(text(&s), "hi\n");
}

#[test]
fn styles_and_lists() {
    let mut s = session("Title\n\nitem one\nitem two\n");
    select(&mut s, 2, 2);
    run(&mut s, "style.apply", json!({ "style": "heading2" }));
    assert_eq!(text(&s), "## Title\n\nitem one\nitem two\n");
    run(&mut s, "style.apply", json!({ "style": "normal" }));
    assert_eq!(text(&s), "Title\n\nitem one\nitem two\n");
    let start = text(&s).find("item one").expect("item");
    select(&mut s, start, start + 12);
    run(&mut s, "format.bullets", Value::Null);
    assert_eq!(text(&s), "Title\n\n- item one\n- item two\n");
    run(&mut s, "format.numbering", Value::Null);
    assert_eq!(text(&s), "Title\n\n1. item one\n2. item two\n");
    run(&mut s, "format.indent", Value::Null);
    assert_eq!(text(&s), "Title\n\n   1. item one\n   2. item two\n");
    run(&mut s, "format.outdent", Value::Null);
    run(&mut s, "tag.todo", Value::Null);
    assert_eq!(text(&s), "Title\n\n1. [ ] item one\n2. [ ] item two\n");
    run(&mut s, "tag.todo", Value::Null);
    assert_eq!(text(&s), "Title\n\nitem one\nitem two\n");
}

#[test]
fn alignment_and_clear() {
    let mut s = session("hello **world**\n");
    select(&mut s, 2, 2);
    run(&mut s, "format.align_center", Value::Null);
    assert_eq!(
        text(&s),
        "<!-- notecraft:align=center -->\nhello **world**\n"
    );
    run(&mut s, "format.align_right", Value::Null);
    assert!(text(&s).starts_with("<!-- notecraft:align=right -->\n"));
    run(&mut s, "format.align_left", Value::Null);
    assert_eq!(text(&s), "hello **world**\n");
    run(&mut s, "format.align_center", Value::Null);
    run(&mut s, "style.apply", json!({ "style": "heading1" }));
    run(&mut s, "format.clear", Value::Null);
    assert_eq!(text(&s), "hello world\n");
}

#[test]
fn nothing_happens_without_a_caret_or_in_frontmatter_and_markers() {
    let mut s = session("hi\n");
    assert!(!applied(&run(&mut s, "format.bold", Value::Null)));
    assert_eq!(text(&s), "hi\n");

    let page = "---\ntitle: Hello\n---\nbody\n";
    let mut s = session(page);
    select(&mut s, 0, page.len());
    run(&mut s, "format.bold", Value::Null);
    assert_eq!(text(&s), "---\ntitle: Hello\n---\n**body**\n");

    let page = "one\n\n<!-- notecraft:container id=c1 -->\ntwo\n";
    let mut s = session(page);
    select(&mut s, 0, page.len());
    run(&mut s, "format.bold", Value::Null);
    assert_eq!(
        text(&s),
        "**one**\n\n<!-- notecraft:container id=c1 -->\n**two**\n"
    );
}

#[test]
fn a_read_only_page_is_not_changed() {
    let mut s = session("hi\n");
    select(&mut s, 0, 2);
    s.set_page_notice(PAGE, "read only", true);
    assert!(!applied(&run(&mut s, "format.bold", Value::Null)));
    assert_eq!(text(&s), "hi\n");
}

#[test]
fn format_state_follows_the_caret() {
    let mut s = session("## **head** and <u>under</u>\n");
    assert_eq!(s.format_state(), Default::default());
    select(&mut s, 7, 7);
    let state = s.format_state();
    assert!(state.bold && !state.underline);
    assert_eq!(state.heading, 2);
    let at = text(&s).find("under").expect("under") + 2;
    select(&mut s, at, at);
    let state = s.format_state();
    assert!(state.underline && !state.bold);
}

#[test]
fn formatting_a_large_page_stays_bounded() {
    let line = "word ".repeat(40);
    let page = format!("{line}\n").repeat(2000);
    let mut s = session(&page);
    select(&mut s, 0, page.len());
    run(&mut s, "format.bold", Value::Null);
    run(&mut s, "format.bullets", Value::Null);
    assert!(text(&s).starts_with("- **word"));
}
