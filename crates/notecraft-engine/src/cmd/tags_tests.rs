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
        "tag.edit",
        "tag.hide",
        "tag.move",
        "tag.reset",
        "tag.reset_all",
        "tag.apply_nth",
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
fn ctrl_number_shortcuts_run_the_nth_tag_of_the_list() {
    let map = default_map();
    for n in 1..=9 {
        let key = format!("Ctrl+{n}");
        let binding = map.iter().find(|b| b.text == key).expect("bound");
        assert_eq!(binding.command.as_str(), "tag.apply_nth", "{key}");
        assert_eq!(binding.args, json!({ "number": n }), "{key}");
    }
    // By default the numbers go to the first nine default tags, Ctrl+1 being To Do.
    let mut s = session("Hello\n");
    select(&mut s, 2, 2);
    run(&mut s, "tag.apply_nth", json!({ "number": 2 }));
    assert_eq!(text(&s), "#important Hello\n");
    run(&mut s, "tag.apply_nth", json!({ "number": 1 }));
    assert_eq!(text(&s), "- [ ] #important Hello\n");
    let registry = Registry::builtin();
    for number in [0, 10] {
        assert!(matches!(
            registry.run(&mut s, "tag.apply_nth", json!({ "number": number })),
            Err(CommandError::InvalidArgs { .. })
        ));
    }
}

#[test]
fn the_shortcut_numbers_follow_the_list_order() {
    let mut s = session("Hello\n");
    select(&mut s, 2, 2);
    // Move Idea to the top; hide To Do. Ctrl+1 is now Idea, Ctrl+2 is Important.
    for _ in 0..10 {
        run(
            &mut s,
            "tag.move",
            json!({ "tag": "idea", "direction": "up" }),
        );
    }
    run(&mut s, "tag.hide", json!({ "tag": "todo" }));
    let first: Vec<&str> = s
        .tags()
        .all()
        .iter()
        .take(3)
        .map(|t| t.slug.as_str())
        .collect();
    assert_eq!(first, ["idea", "important", "question"]);
    run(&mut s, "tag.apply_nth", json!({ "number": 1 }));
    assert_eq!(text(&s), "#idea Hello\n");
    run(&mut s, "tag.apply_nth", json!({ "number": 2 }));
    assert_eq!(text(&s), "#idea #important Hello\n");
    // Down moves it back one place at a time; the ends stay put.
    run(
        &mut s,
        "tag.move",
        json!({ "tag": "idea", "direction": "down" }),
    );
    assert_eq!(s.tags().shortcut(2).map(|t| t.slug.as_str()), Some("idea"));
    run(
        &mut s,
        "tag.move",
        json!({ "tag": "important", "direction": "up" }),
    );
    run(
        &mut s,
        "tag.move",
        json!({ "tag": "important", "direction": "up" }),
    );
    assert_eq!(
        s.tags().shortcut(1).map(|t| t.slug.as_str()),
        Some("important")
    );
    let registry = Registry::builtin();
    assert!(matches!(
        registry.run(
            &mut s,
            "tag.move",
            json!({ "tag": "idea", "direction": "left" })
        ),
        Err(CommandError::InvalidArgs { .. })
    ));
    assert!(matches!(
        registry.run(
            &mut s,
            "tag.move",
            json!({ "tag": "nope", "direction": "up" })
        ),
        Err(CommandError::InvalidArgs { .. })
    ));
}

#[test]
fn a_default_tag_can_be_restyled_hidden_and_reset_without_touching_pages() {
    let mut s = session("#idea Plan\n");
    run(
        &mut s,
        "tag.edit",
        json!({ "tag": "idea", "name": "Thought", "icon": "heart", "font_color": "#112233", "highlight": "#FFF176" }),
    );
    let tag = s.tags().get("idea").expect("kept slug");
    assert_eq!(
        (tag.name.as_str(), tag.icon, tag.font_color, tag.highlight),
        (
            "Thought",
            notecraft_markdown::TagIcon::Heart,
            Some(0x11_22_33),
            Some(0xFF_F1_76)
        )
    );
    assert!(s.tags().is_modified("idea"));
    assert_eq!(text(&s), "#idea Plan\n");
    // A name another tag has is refused.
    let registry = Registry::builtin();
    assert!(matches!(
        registry.run(
            &mut s,
            "tag.edit",
            json!({ "tag": "question", "name": "thought" })
        ),
        Err(CommandError::InvalidArgs { .. })
    ));

    run(&mut s, "tag.reset", json!({ "tag": "idea" }));
    assert!(!s.tags().is_modified("idea"));
    assert_eq!(s.tags().get("idea").map(|t| t.name.as_str()), Some("Idea"));

    // Hidden: gone from the list and the numbers, still known to the page.
    run(&mut s, "tag.hide", json!({ "tag": "important" }));
    assert!(s.tags().all().iter().all(|t| t.slug != "important"));
    assert_eq!(
        s.tags().shortcut(2).map(|t| t.slug.as_str()),
        Some("question")
    );
    assert!(s.tags().get("important").is_some(), "text keeps its icon");
    // A hidden tag can still be applied by id (a script), and found by name.
    assert!(registry
        .run(&mut s, "tag.hide", json!({ "tag": "important" }))
        .is_err());
    // Custom tags are deleted, not hidden.
    run(&mut s, "tag.custom_add", json!({ "name": "Errand" }));
    assert!(registry
        .run(&mut s, "tag.hide", json!({ "tag": "errand" }))
        .is_err());

    run(
        &mut s,
        "tag.edit",
        json!({ "tag": "question", "name": "Query" }),
    );
    run(
        &mut s,
        "tag.move",
        json!({ "tag": "idea", "direction": "up" }),
    );
    run(&mut s, "tag.reset_all", Value::Null);
    let slugs: Vec<&str> = s.tags().all().iter().map(|t| t.slug.as_str()).collect();
    assert_eq!(slugs.len(), 30);
    assert_eq!(&slugs[..3], ["todo", "important", "question"]);
    assert_eq!(slugs.last().copied(), Some("errand"), "custom tags stay");
    assert!(s.tags().hidden().is_empty());
    assert_eq!(
        s.tags().get("question").map(|t| t.name.as_str()),
        Some("Question")
    );
}
