//! Tests for the find on page commands (TX-11).

use serde_json::{json, Value};

use super::Registry;
use crate::session::Session;
use crate::undo::tests::session_with_pages;

fn session(text: &str) -> Session {
    let mut session = session_with_pages();
    session.set_page_source("p1", text);
    session
}

fn run(session: &mut Session, id: &str, args: Value) -> Value {
    match Registry::builtin().run(session, id, args) {
        Ok(out) => out.value,
        Err(err) => panic!("{id} failed: {err}"),
    }
}

fn num(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

const PAGE: &str = "Apple pie\n\nan APPLE a day\n\n| apple |\n|---|\n| x |\n";

#[test]
fn find_opens_the_bar_and_counts_matches_across_the_page() {
    let mut s = session(PAGE);
    let out = run(&mut s, "edit.find", json!({ "query": "apple" }));
    assert_eq!(out["open"], true);
    assert_eq!(num(&out, "count"), Some(3));
    assert_eq!(num(&out, "current"), Some(1));
    assert!(s.find().open);
}

#[test]
fn next_and_previous_wrap_around() {
    let mut s = session(PAGE);
    run(&mut s, "edit.find", json!({ "query": "apple" }));
    let seen: Vec<Option<i64>> = (0..4)
        .map(|_| num(&run(&mut s, "edit.find.next", json!({})), "current"))
        .collect();
    assert_eq!(seen, [Some(2), Some(3), Some(1), Some(2)]);
    let back = run(&mut s, "edit.find.previous", json!({}));
    assert_eq!(num(&back, "current"), Some(1));
    let wrapped = run(&mut s, "edit.find.previous", json!({}));
    assert_eq!(num(&wrapped, "current"), Some(3));
}

#[test]
fn a_new_query_goes_to_the_first_match_and_scrolls() {
    let mut s = session(PAGE);
    run(&mut s, "edit.find", json!({ "query": "apple" }));
    run(&mut s, "edit.find.next", json!({}));
    let scroll = s.find().scroll;
    let out = run(&mut s, "edit.find.set_query", json!({ "query": "day" }));
    assert_eq!(
        (num(&out, "count"), num(&out, "current")),
        (Some(1), Some(1))
    );
    assert!(s.find().scroll > scroll);
    let none = run(&mut s, "edit.find.set_query", json!({ "query": "zzz" }));
    assert_eq!(
        (num(&none, "count"), num(&none, "current")),
        (Some(0), Some(0))
    );
    // Next with no matches changes nothing and does not fail.
    let before = s.find().clone();
    run(&mut s, "edit.find.next", json!({}));
    assert_eq!(*s.find(), before);
}

#[test]
fn close_hides_the_bar_and_keeps_the_query() {
    let mut s = session(PAGE);
    run(&mut s, "edit.find", json!({ "query": "pie" }));
    let out = run(&mut s, "edit.find.close", json!({}));
    assert_eq!(out["open"], false);
    assert_eq!(s.find().query, "pie");
    let again = run(&mut s, "edit.find", json!({}));
    assert_eq!(again["open"], true);
    assert_eq!(num(&again, "count"), Some(1));
}

#[test]
fn opening_find_ends_text_editing_and_never_changes_the_page() {
    let mut s = session(PAGE);
    run(&mut s, "text.set_caret", json!({ "at": 3 }));
    assert!(s.text_cursor().is_some());
    let revision = s.revision();
    run(&mut s, "edit.find", json!({ "query": "a" }));
    assert!(s.text_cursor().is_none());
    assert!(s.revision() > revision);
    assert_eq!(s.page_text("p1"), Some(PAGE));
    assert!(!s.can_undo());
}

#[test]
fn a_huge_query_is_cut_and_markup_is_not_matched() {
    let mut s = session("**bold** text\n");
    let out = run(&mut s, "edit.find", json!({ "query": "*".repeat(5000) }));
    assert_eq!(num(&out, "count"), Some(0));
    assert!(s.find().query.chars().count() <= crate::find::MAX_QUERY_CHARS);
    let out = run(&mut s, "edit.find.set_query", json!({ "query": "BOLD t" }));
    assert_eq!(num(&out, "count"), Some(1));
}

#[test]
fn the_shortcuts_are_bound() {
    let map = crate::cmd::shortcuts::default_map();
    let bound = |text: &str, id: &str| {
        map.iter()
            .any(|b| b.text == text && b.command.as_str() == id)
    };
    assert!(bound("Ctrl+F", "edit.find"));
    assert!(bound("F3", "edit.find.next"));
    assert!(bound("Shift+F3", "edit.find.previous"));
}
