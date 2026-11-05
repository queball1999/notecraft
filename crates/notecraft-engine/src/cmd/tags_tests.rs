//! Tests for the note tag commands (PR 2.5).

use serde_json::{json, Value};

use super::shortcuts::default_map;
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

#[test]
fn the_tag_commands_are_ready() {
    let registry = Registry::builtin();
    for id in [
        "tag.apply",
        "tag.remove",
        "tag.customize",
        "tag.custom_add",
        "tag.custom_edit",
        "tag.custom_delete",
    ] {
        let spec = registry.get(id).expect(id);
        assert_eq!(spec.status, Status::Ready, "{id}");
    }
    assert!(registry.get("tag.apply").expect("spec").undoable);
    assert!(registry.get("tag.remove").expect("spec").undoable);
}

#[test]
fn applying_a_tag_toggles_it_in_one_undo_step() {
    let mut s = session("Buy milk\n\nCall Ada\n");
    select(&mut s, 3, 3);
    run(&mut s, "tag.apply", json!({ "tag": "important" }));
    assert_eq!(text(&s), "#important Buy milk\n\nCall Ada\n");
    run(&mut s, "tag.apply", json!({ "tag": "important" }));
    assert_eq!(text(&s), "Buy milk\n\nCall Ada\n");
    run(&mut s, "tag.apply", json!({ "tag": "question" }));
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "Buy milk\n\nCall Ada\n");
    run(&mut s, "edit.redo", Value::Null);
    assert_eq!(text(&s), "#question Buy milk\n\nCall Ada\n");
}

#[test]
fn a_selection_tags_every_paragraph_as_one_step() {
    let mut s = session("one\n\ntwo\n\n- three\n");
    let end = text(&s).len();
    select(&mut s, 0, end);
    run(&mut s, "tag.apply", json!({ "tag": "idea" }));
    assert_eq!(text(&s), "#idea one\n\n#idea two\n\n- #idea three\n");
    run(&mut s, "edit.undo", Value::Null);
    assert_eq!(text(&s), "one\n\ntwo\n\n- three\n");
}

#[test]
fn to_do_is_the_check_box() {
    let mut s = session("Buy milk\n");
    select(&mut s, 2, 2);
    run(&mut s, "tag.apply", json!({ "tag": "todo" }));
    assert_eq!(text(&s), "- [ ] Buy milk\n");
    run(&mut s, "tag.apply", json!({ "tag": "todo" }));
    assert_eq!(text(&s), "Buy milk\n");
}

#[test]
fn remove_takes_known_tags_but_not_other_words() {
    let mut s = session("#important #question #nottag keep\n\n- [ ] #idea task\n");
    let end = text(&s).len();
    select(&mut s, 0, end);
    run(&mut s, "tag.remove", Value::Null);
    assert_eq!(text(&s), "#nottag keep\n\ntask\n");
    run(&mut s, "edit.undo", Value::Null);
    assert!(text(&s).starts_with("#important #question"));
}

#[test]
fn unknown_tags_and_bad_arguments_are_errors() {
    let mut s = session("x\n");
    select(&mut s, 0, 0);
    let registry = Registry::builtin();
    for args in [json!({ "tag": "nope" }), json!({ "tag": 3 }), json!({})] {
        assert!(
            matches!(
                registry.run(&mut s, "tag.apply", args),
                Err(CommandError::InvalidArgs { .. })
            ),
            "bad args are refused"
        );
    }
    assert_eq!(text(&s), "x\n");
}

#[test]
fn nothing_happens_without_a_caret() {
    let mut s = session("x\n");
    let out = run(&mut s, "tag.apply", json!({ "tag": "idea" }));
    assert_eq!(out, json!({ "applied": false }));
    assert_eq!(text(&s), "x\n");
}

#[test]
fn custom_tags_can_be_added_changed_applied_and_deleted() {
    let mut s = session("Read this\n");
    let out = run(
        &mut s,
        "tag.custom_add",
        json!({ "name": "Read Later", "icon": "book", "font_color": "#112233", "highlight": "#FFF176" }),
    );
    assert_eq!(out, json!({ "tag": "read-later" }));
    let tag = s.tags().get("read-later").expect("added");
    assert_eq!(
        (tag.font_color, tag.highlight),
        (Some(0x11_22_33), Some(0xFF_F1_76))
    );

    select(&mut s, 2, 2);
    run(&mut s, "tag.apply", json!({ "tag": "read-later" }));
    assert_eq!(text(&s), "#read-later Read this\n");

    run(
        &mut s,
        "tag.custom_edit",
        json!({ "tag": "read-later", "name": "Reading", "icon": "star", "font_color": "none" }),
    );
    let tag = s.tags().get("read-later").expect("kept slug");
    assert_eq!(tag.name, "Reading");
    assert_eq!(tag.font_color, None);
    assert_eq!(tag.highlight, Some(0xFF_F1_76));
    assert_eq!(text(&s), "#read-later Read this\n", "pages are untouched");

    run(&mut s, "tag.custom_delete", json!({ "tag": "read-later" }));
    assert!(s.tags().get("read-later").is_none());
    assert_eq!(text(&s), "#read-later Read this\n");

    let registry = Registry::builtin();
    for (id, args) in [
        ("tag.custom_add", json!({ "name": "Idea" })),
        ("tag.custom_add", json!({ "name": "  " })),
        ("tag.custom_add", json!({ "name": "x", "icon": "nope" })),
        (
            "tag.custom_add",
            json!({ "name": "x", "font_color": "red" }),
        ),
        (
            "tag.custom_edit",
            json!({ "tag": "idea", "name": "Thought" }),
        ),
        ("tag.custom_delete", json!({ "tag": "idea" })),
    ] {
        assert!(
            matches!(
                registry.run(&mut s, id, args.clone()),
                Err(CommandError::InvalidArgs { .. })
            ),
            "{id} {args}"
        );
    }
}

#[test]
fn ctrl_number_shortcuts_run_the_first_nine_tags() {
    let map = default_map();
    let args_of = |text: &str| {
        map.iter()
            .find(|b| b.text == text)
            .map(|b| (b.command.as_str().to_owned(), b.args.clone()))
    };
    assert_eq!(
        args_of("Ctrl+1"),
        Some(("tag.todo".to_owned(), Value::Null))
    );
    assert_eq!(
        args_of("Ctrl+0"),
        Some(("tag.remove".to_owned(), Value::Null))
    );
    let session = Session::new();
    for (n, slug) in notecraft_markdown::shortcut_slugs().enumerate().skip(1) {
        let key = format!("Ctrl+{}", n + 1);
        let (command, args) = args_of(&key).expect("bound");
        assert_eq!(command, "tag.apply", "{key}");
        assert_eq!(args.get("tag").and_then(Value::as_str), Some(slug), "{key}");
        assert!(session.tags().get(slug).is_some());
    }
}
