//! Tests for transactions, the history stacks, and the commands that drive them.

use notecraft_core::{InkColor, InkPoint, InkTool, Stroke};
use notecraft_model::{Notebook, PageRef, Section, SectionNode};
use serde_json::{json, Value};

use super::*;
use crate::cmd::{
    ids, CommandError, CommandId, CommandOutput, CommandSpec, Handler, Map, Registry,
};

fn stroke(id: &str, x: f32) -> Stroke {
    Stroke {
        id: id.into(),
        tool: InkTool::Pen,
        color: InkColor::BLACK,
        width: 2.0,
        points: vec![InkPoint::new(x, 0.0, 1.0), InkPoint::new(x + 5.0, 5.0, 1.0)],
    }
}

fn insert(page: &str, index: usize, stroke: Stroke) -> Op {
    Op::InsertStroke {
        page: page.into(),
        index,
        stroke,
    }
}

fn page_tx(page: &str, label: &str, op: Op) -> Transaction {
    Transaction::new(Scope::Page(page.into()), label).with(op)
}

pub(super) fn session_with_pages() -> Session {
    let section = Section {
        id: "s1".into(),
        name: "Notes".into(),
        color: 0,
        pages: vec![
            PageRef::new("p1", "One", 0),
            PageRef::new("p2", "Two", 0),
            PageRef::new("p3", "Three", 0),
        ],
    };
    let notebook = Notebook {
        id: "nb".into(),
        name: "Notebook".into(),
        color: 0,
        sections: vec![SectionNode::Section(section)],
    };
    let mut session = Session::new();
    session.set_notebooks(vec![notebook]);
    session
}

fn ids_on(session: &Session, page: &str) -> Vec<String> {
    session
        .ink()
        .layer(page)
        .map(|layer| layer.strokes().iter().map(|s| s.id.clone()).collect())
        .unwrap_or_default()
}

fn run(session: &mut Session, registry: &Registry, id: &str, args: Value) -> Value {
    match registry.run(session, id, args) {
        Ok(out) => out.value,
        Err(err) => panic!("{id} failed: {err}"),
    }
}

#[test]
fn a_committed_stroke_undoes_and_redoes() {
    let mut session = session_with_pages();
    let before = session.ink().clone();
    assert!(!session.can_undo() && !session.can_redo());

    assert!(session.commit(page_tx(
        "p1",
        "Ink stroke",
        insert("p1", 0, stroke("s1", 0.0))
    )));
    assert_eq!(ids_on(&session, "p1"), ["s1"]);
    assert!(session.can_undo() && !session.can_redo());
    assert_eq!(session.history().undo_label(Some("p1")), Some("Ink stroke"));

    assert_eq!(session.undo().as_deref(), Some("Ink stroke"));
    // Undo puts the ink back exactly as it was, down to the empty layer being gone.
    assert_eq!(*session.ink(), before);
    assert!(!session.can_undo() && session.can_redo());
    assert_eq!(session.history().redo_label(Some("p1")), Some("Ink stroke"));

    assert_eq!(session.redo().as_deref(), Some("Ink stroke"));
    assert_eq!(ids_on(&session, "p1"), ["s1"]);
    assert!(session.can_undo() && !session.can_redo());
    // Nothing more to redo or, after undoing twice, to undo.
    assert_eq!(session.redo(), None);
    assert!(session.undo().is_some());
    assert_eq!(session.undo(), None);
}

#[test]
fn undo_restores_paint_order() {
    let mut session = session_with_pages();
    for (i, id) in ["s1", "s2", "s3"].iter().enumerate() {
        let tx = page_tx("p1", "Ink stroke", insert("p1", i, stroke(id, i as f32)));
        assert!(session.commit(tx));
    }
    // Erase the middle stroke as one step, then undo it: it returns between its neighbors.
    let erase = Transaction::new(Scope::Page("p1".into()), "Erase").with(Op::DeleteStroke {
        page: "p1".into(),
        index: 1,
        stroke: stroke("s2", 1.0),
    });
    assert!(session.commit(erase));
    assert_eq!(ids_on(&session, "p1"), ["s1", "s3"]);
    assert_eq!(session.undo().as_deref(), Some("Erase"));
    assert_eq!(ids_on(&session, "p1"), ["s1", "s2", "s3"]);
    assert_eq!(session.redo().as_deref(), Some("Erase"));
    assert_eq!(ids_on(&session, "p1"), ["s1", "s3"]);
}

#[test]
fn a_new_step_clears_redo() {
    let mut session = session_with_pages();
    assert!(session.commit(page_tx("p1", "One", insert("p1", 0, stroke("s1", 0.0)))));
    assert!(session.undo().is_some());
    assert!(session.can_redo());
    assert!(session.commit(page_tx("p1", "Two", insert("p1", 0, stroke("s2", 0.0)))));
    assert!(!session.can_redo());
    assert_eq!(session.redo(), None);
}

#[test]
fn history_is_per_page() {
    let mut session = session_with_pages();
    assert!(session.commit(page_tx("p1", "On one", insert("p1", 0, stroke("s1", 0.0)))));
    assert!(session.commit(page_tx("p2", "On two", insert("p2", 0, stroke("s1", 0.0)))));
    // Page one is shown: only its step is undoable.
    assert_eq!(session.history().undo_label(Some("p1")), Some("On one"));
    assert_eq!(session.history().undo_label(Some("p2")), Some("On two"));
    assert_eq!(session.history().undo_label(Some("p3")), None);
    assert_eq!(session.undo().as_deref(), Some("On one"));
    assert!(ids_on(&session, "p1").is_empty());
    assert_eq!(ids_on(&session, "p2"), ["s1"]);
    assert_eq!(session.undo(), None);
}

#[test]
fn a_step_that_does_not_fit_is_reversed_and_not_recorded() {
    let mut session = session_with_pages();
    let before = session.clone();
    // The second operation names a stroke that is not there.
    let tx = Transaction::new(Scope::Page("p1".into()), "Bad")
        .with(insert("p1", 0, stroke("s1", 0.0)))
        .with(Op::DeleteStroke {
            page: "p1".into(),
            index: 0,
            stroke: stroke("nope", 0.0),
        });
    assert!(!session.commit(tx));
    assert_eq!(session, before);
}

#[test]
fn a_stale_step_is_dropped_instead_of_half_applied() {
    let mut session = session_with_pages();
    assert!(session.commit(page_tx("p1", "Add", insert("p1", 0, stroke("s1", 0.0)))));
    // Something outside the history removes the stroke.
    let layer = session.ink_mut().layers.get_mut("p1").unwrap();
    std::sync::Arc::make_mut(layer).remove("s1");
    assert_eq!(session.undo(), None);
    assert!(!session.can_undo() && !session.can_redo());
}

// Merging

#[test]
fn a_merged_transaction_undoes_with_the_previous_one() {
    let mut session = session_with_pages();
    assert!(session.commit(page_tx(
        "p1",
        "Ink stroke",
        insert("p1", 0, stroke("s1", 0.0))
    )));
    // One eraser drag: the first stroke starts a step, the next two join it.
    assert!(session.commit(page_tx("p1", "Erase", insert("p1", 1, stroke("s2", 1.0)))));
    let join = |id: &str, index: usize| {
        page_tx("p1", "Erase", insert("p1", index, stroke(id, 2.0))).merge_into_previous()
    };
    assert!(session.commit(join("s3", 2)));
    assert!(session.commit(join("s4", 3)));
    assert_eq!(ids_on(&session, "p1"), ["s1", "s2", "s3", "s4"]);
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 2);

    // One undo takes all three; the earlier step is untouched.
    assert_eq!(session.undo().as_deref(), Some("Erase"));
    assert_eq!(ids_on(&session, "p1"), ["s1"]);
    // One redo puts all three back, in order.
    assert_eq!(session.redo().as_deref(), Some("Erase"));
    assert_eq!(ids_on(&session, "p1"), ["s1", "s2", "s3", "s4"]);
    assert_eq!(session.undo().as_deref(), Some("Erase"));
    assert_eq!(session.undo().as_deref(), Some("Ink stroke"));
    assert!(ids_on(&session, "p1").is_empty());
}

#[test]
fn merging_with_no_previous_step_starts_one() {
    let mut session = session_with_pages();
    let tx = page_tx("p1", "Erase", insert("p1", 0, stroke("s1", 0.0))).merge_into_previous();
    assert!(session.commit(tx));
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 1);
    // It does not join a step of another page.
    let tx = page_tx("p2", "Erase", insert("p2", 0, stroke("s1", 0.0))).merge_into_previous();
    assert!(session.commit(tx));
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 1);
    assert_eq!(session.history().undo_steps(Some("p2")).len(), 1);
}

#[test]
fn a_merge_after_undo_starts_a_new_step() {
    let mut session = session_with_pages();
    assert!(session.commit(page_tx("p1", "A", insert("p1", 0, stroke("s1", 0.0)))));
    assert!(session.commit(page_tx("p1", "B", insert("p1", 1, stroke("s2", 0.0)))));
    assert!(session.undo().is_some());
    // The step it would have joined is in the redo stack now; the merge joins the one below.
    let tx = page_tx("p1", "C", insert("p1", 1, stroke("s3", 0.0))).merge_into_previous();
    assert!(session.commit(tx));
    assert!(!session.can_redo());
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 1);
    assert!(session.undo().is_some());
    assert!(ids_on(&session, "p1").is_empty());
}

// Caps

#[test]
fn a_stack_keeps_at_most_the_step_cap() {
    let mut history = History::with_limits(3, usize::MAX);
    for i in 0..10 {
        let tx = page_tx(
            "p1",
            &format!("Step {i}"),
            insert("p1", 0, stroke("s1", 0.0)),
        );
        history.record(tx);
    }
    let labels: Vec<String> = history
        .undo_steps(Some("p1"))
        .into_iter()
        .map(|step| step.label)
        .collect();
    assert_eq!(labels, ["Step 7", "Step 8", "Step 9"]);
}

#[test]
fn a_stack_keeps_at_most_the_byte_cap() {
    let one = insert("p1", 0, stroke("s1", 0.0)).bytes() + "Step 0".len();
    let mut history = History::with_limits(MAX_STEPS, one * 2 + one / 2);
    for i in 0..6 {
        let tx = page_tx(
            "p1",
            &format!("Step {i}"),
            insert("p1", 0, stroke("s1", 0.0)),
        );
        history.record(tx);
    }
    assert_eq!(history.undo_steps(Some("p1")).len(), 2);
    // The newest step is always kept, even when it alone is over the cap.
    let mut tiny = History::with_limits(MAX_STEPS, 1);
    tiny.record(page_tx("p1", "Big", insert("p1", 0, stroke("s1", 0.0))));
    assert_eq!(tiny.undo_steps(Some("p1")).len(), 1);
}

#[test]
fn the_default_caps_are_500_steps_and_64_mib() {
    assert_eq!(MAX_STEPS, 500);
    assert_eq!(MAX_BYTES, 64 * 1024 * 1024);
    let mut history = History::default();
    for _ in 0..(MAX_STEPS + 20) {
        history.record(page_tx("p1", "S", insert("p1", 0, stroke("s1", 0.0))));
    }
    assert_eq!(history.undo_steps(Some("p1")).len(), MAX_STEPS);
}

// Commands

#[test]
fn undo_and_redo_commands_drive_the_history() {
    let registry = Registry::builtin();
    let mut session = session_with_pages();
    // Nothing to undo: no error, no change, null result.
    assert_eq!(
        run(&mut session, &registry, "edit.undo", Value::Null),
        Value::Null
    );
    assert_eq!(
        run(&mut session, &registry, "edit.redo", Value::Null),
        Value::Null
    );
    assert_eq!(session.revision(), 0);

    let added = json!({ "points": [[1, 2], [3, 4]] });
    run(&mut session, &registry, "ink.add_stroke", added.clone());
    run(&mut session, &registry, "ink.add_stroke", added);
    assert_eq!(ids_on(&session, "p1"), ["s1", "s2"]);

    let out = run(&mut session, &registry, "edit.undo", Value::Null);
    assert_eq!(out, json!({ "label": "Ink stroke" }));
    assert_eq!(ids_on(&session, "p1"), ["s1"]);
    run(&mut session, &registry, "edit.redo", Value::Null);
    assert_eq!(ids_on(&session, "p1"), ["s1", "s2"]);
    // The ids stay stable across undo and redo, and a new stroke after an undo reuses the
    // freed id.
    run(&mut session, &registry, "edit.undo", Value::Null);
    let out = run(
        &mut session,
        &registry,
        "ink.add_stroke",
        json!({ "points": [[0, 0]] }),
    );
    assert_eq!(out, json!({ "id": "s2" }));
}

#[test]
fn undo_and_redo_report_a_change_in_the_revision() {
    let registry = Registry::builtin();
    let mut session = session_with_pages();
    run(
        &mut session,
        &registry,
        "ink.add_stroke",
        json!({ "points": [[1, 2]] }),
    );
    let after_add = session.revision();
    run(&mut session, &registry, "edit.undo", Value::Null);
    assert!(session.revision() > after_add);
}

#[test]
fn undo_and_redo_are_specced_with_shortcuts() {
    let registry = Registry::builtin();
    for (id, shortcut) in [(ids::EDIT_UNDO, "Ctrl+Z"), (ids::EDIT_REDO, "Ctrl+Y")] {
        let spec = registry.get(id.as_str()).unwrap();
        assert_eq!(spec.status, crate::cmd::Status::Ready);
        assert_eq!(spec.default_shortcut, Some(shortcut));
        // Undo is not itself a step.
        assert!(!spec.undoable);
    }
    let map = crate::cmd::shortcuts::default_map();
    let redo: Vec<&str> = map
        .iter()
        .filter(|b| b.command == ids::EDIT_REDO)
        .map(|b| b.text)
        .collect();
    assert!(redo.contains(&"Ctrl+Y") && redo.contains(&"Ctrl+Shift+Z"));
}

// The command guard

const TEST_RECORD_THEN_PANIC: CommandId = CommandId::new("test.record_then_panic");
const TEST_RECORD_THEN_FAIL: CommandId = CommandId::new("test.record_then_fail");

fn record_then_panic(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    session.commit(page_tx("p1", "Doomed", insert("p1", 0, stroke("s9", 0.0))));
    session.undo();
    panic!("test command panicked on purpose");
}

fn record_then_fail(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    session.commit(page_tx("p1", "Doomed", insert("p1", 0, stroke("s9", 0.0))));
    Err(CommandError::InvalidArgs {
        id: TEST_RECORD_THEN_FAIL,
        reason: "refused".to_owned(),
    })
}

#[test]
fn a_rolled_back_command_leaves_the_history_as_it_was() {
    let mut registry = Registry::builtin();
    let handlers: [(CommandId, Handler); 2] = [
        (TEST_RECORD_THEN_PANIC, record_then_panic),
        (TEST_RECORD_THEN_FAIL, record_then_fail),
    ];
    for (id, handler) in handlers {
        let spec = CommandSpec::new(id, "Test", "Record a step, then not finish.").ready();
        assert_eq!(registry.register(spec, Some(handler)), Ok(()));
    }
    let mut session = session_with_pages();
    run(
        &mut session,
        &registry,
        "ink.add_stroke",
        json!({ "points": [[1, 2]] }),
    );
    run(
        &mut session,
        &registry,
        "ink.add_stroke",
        json!({ "points": [[3, 4]] }),
    );
    run(&mut session, &registry, "edit.undo", Value::Null);
    let before = session.clone();
    assert!(before.can_undo() && before.can_redo());

    for id in ["test.record_then_panic", "test.record_then_fail"] {
        assert!(registry.run(&mut session, id, Value::Null).is_err());
        // Strokes, undo stack, and redo stack are all as they were.
        assert_eq!(session, before, "{id}");
        assert_eq!(session.history(), before.history(), "{id}");
    }
    // Undo and redo still work from there.
    assert_eq!(session.undo().as_deref(), Some("Ink stroke"));
    assert!(ids_on(&session, "p1").is_empty());
    assert_eq!(session.redo().as_deref(), Some("Ink stroke"));
    assert_eq!(session.redo().as_deref(), Some("Ink stroke"));
    assert_eq!(ids_on(&session, "p1"), ["s1", "s2"]);
}

#[test]
fn opening_other_notebooks_clears_the_history() {
    let mut session = session_with_pages();
    assert!(session.commit(page_tx("p1", "A", insert("p1", 0, stroke("s1", 0.0)))));
    session.set_notebooks(Vec::new());
    assert!(!session.can_undo());
    assert!(session.history().undo_steps(Some("p1")).is_empty());
}
