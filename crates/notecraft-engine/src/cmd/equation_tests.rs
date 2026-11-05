//! Tests for `insert.equation` (PR 2.7).

use serde_json::{json, Value};

use super::*;
use crate::undo::tests::session_with_pages;

const PAGE: &str = "p1";

fn session(text: &str) -> Session {
    let mut session = session_with_pages();
    session.set_page_source(PAGE, text);
    session
}

fn run(session: &mut Session, id: &str, args: Value) -> Result<Value, CommandError> {
    Registry::builtin().run(session, id, args).map(|o| o.value)
}

fn text(session: &Session) -> String {
    session.page_text(PAGE).unwrap_or_default().to_owned()
}

fn select(session: &mut Session, anchor: usize, head: usize) {
    run(session, "text.set_caret", json!({ "at": anchor })).expect("caret");
    if head != anchor {
        run(
            session,
            "text.set_caret",
            json!({ "at": head, "extend": true }),
        )
        .expect("extend");
    }
}

fn selected(session: &Session) -> String {
    let c = session.text_cursor().expect("cursor");
    text(session)
        .get(c.anchor.min(c.head)..c.anchor.max(c.head))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn the_command_is_ready_and_has_its_shortcut() {
    let registry = Registry::builtin();
    let spec = registry.get("insert.equation").expect("registered");
    assert_eq!(spec.default_shortcut, Some("Alt+="));
    assert!(registry.validate("insert.equation", &json!({})).is_ok());
}

#[test]
fn without_a_formula_a_selected_placeholder_is_inserted() {
    let mut s = session("Area: \n");
    select(&mut s, 6, 6);
    run(&mut s, "insert.equation", json!({})).expect("insert");
    assert_eq!(text(&s), "Area: $x$\n");
    assert_eq!(selected(&s), "x");
    // Typing replaces the placeholder.
    run(&mut s, "text.insert", json!({ "text": "\\pi" })).expect("type");
    assert_eq!(text(&s), "Area: $\\pi$\n");
}

#[test]
fn a_selection_becomes_the_formula() {
    let mut s = session("so a^2 holds\n");
    select(&mut s, 3, 6);
    run(&mut s, "insert.equation", json!({})).expect("insert");
    assert_eq!(text(&s), "so $a^2$ holds\n");
    assert_eq!(selected(&s), "a^2");
}

#[test]
fn a_given_formula_replaces_the_selection_and_can_be_a_display_formula() {
    let mut s = session("x\n");
    select(&mut s, 0, 1);
    run(
        &mut s,
        "insert.equation",
        json!({ "latex": "\\frac{a}{b}", "display": true }),
    )
    .expect("insert");
    assert_eq!(text(&s), "$$\\frac{a}{b}$$\n");
    assert_eq!(selected(&s), "\\frac{a}{b}");
}

#[test]
fn the_page_reads_the_inserted_text_as_a_formula() {
    let mut s = session("\n");
    select(&mut s, 0, 0);
    run(&mut s, "insert.equation", json!({ "latex": "e^{i\\pi}" })).expect("insert");
    let doc = notecraft_markdown::parse(&text(&s));
    let found = doc.blocks.iter().any(|b| match &b.kind {
        notecraft_markdown::BlockKind::Paragraph { runs } => runs
            .iter()
            .any(|r| r.math == notecraft_markdown::Math::Inline && r.text == "e^{i\\pi}"),
        _ => false,
    });
    assert!(found, "{:?}", text(&s));
}

#[test]
fn inserting_an_equation_is_one_undo_step() {
    let mut s = session("a b\n");
    select(&mut s, 1, 1);
    run(&mut s, "insert.equation", json!({})).expect("insert");
    assert_ne!(text(&s), "a b\n");
    run(&mut s, "edit.undo", Value::Null).expect("undo");
    assert_eq!(text(&s), "a b\n");
    run(&mut s, "edit.redo", Value::Null).expect("redo");
    assert_eq!(text(&s), "a$x$ b\n");
}

#[test]
fn a_selection_that_cannot_be_a_formula_is_left_alone() {
    let mut s = session("cost $5 or more\n");
    select(&mut s, 5, 8);
    run(&mut s, "insert.equation", json!({})).expect("insert");
    assert_eq!(text(&s), "cost $5 $x$or more\n");
    assert_eq!(s.text_cursor().map(|c| c.is_caret()), Some(false));
}

#[test]
fn bad_formulas_are_refused_and_change_nothing() {
    let mut s = session("page\n");
    select(&mut s, 0, 0);
    let long = "x".repeat(4097);
    for args in [
        json!({ "latex": long }),
        json!({ "latex": "a$b" }),
        json!({ "latex": "   " }),
        json!({ "latex": 5 }),
        json!({ "latex": "a\nb" }),
    ] {
        assert!(
            run(&mut s, "insert.equation", args.clone()).is_err(),
            "{args}"
        );
    }
    assert_eq!(text(&s), "page\n");
    // An escaped dollar sign is fine, and so is the longest formula.
    run(&mut s, "insert.equation", json!({ "latex": "a\\$b" })).expect("escaped");
    let max = "x".repeat(4096);
    run(&mut s, "insert.equation", json!({ "latex": max })).expect("longest");
}
