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

pub(crate) fn session_with_pages() -> Session {
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

// Typing

/// Type `text` one character at a time at the end of page p1, as a keyboard would.
fn type_chars(session: &mut Session, text: &str) {
    for ch in text.chars() {
        let at = session.page_text("p1").unwrap_or_default().len();
        assert!(session.edit_page_text("p1", at, 0, &ch.to_string(), true));
    }
}

fn text(session: &Session) -> String {
    session.page_text("p1").unwrap_or_default().to_owned()
}

#[test]
fn typing_a_word_is_one_step() {
    let mut session = session_with_pages();
    type_chars(&mut session, "hello");
    assert_eq!(text(&session), "hello");
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 1);
    assert_eq!(session.undo().as_deref(), Some("Typing"));
    assert_eq!(text(&session), "");
    assert_eq!(session.redo().as_deref(), Some("Typing"));
    assert_eq!(text(&session), "hello");
}

#[test]
fn a_new_word_starts_a_new_step() {
    let mut session = session_with_pages();
    // A word and the space after it are one step; the next word is another.
    type_chars(&mut session, "hi yo there");
    let steps = session.history().undo_steps(Some("p1")).len();
    assert_eq!(steps, 3, "hi_ yo_ there");
    session.undo();
    assert_eq!(text(&session), "hi yo ");
    session.undo();
    assert_eq!(text(&session), "hi ");
    session.undo();
    assert_eq!(text(&session), "");
    assert_eq!(session.undo(), None);
    // Redo brings the words back one step at a time.
    session.redo();
    assert_eq!(text(&session), "hi ");
}

#[test]
fn typing_somewhere_else_starts_a_new_step() {
    let mut session = session_with_pages();
    type_chars(&mut session, "abc");
    // The caret jumps to the start.
    assert!(session.edit_page_text("p1", 0, 0, "x", true));
    assert!(session.edit_page_text("p1", 1, 0, "y", true));
    assert_eq!(text(&session), "xyabc");
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 2);
    session.undo();
    assert_eq!(text(&session), "abc");
}

#[test]
fn another_kind_of_step_ends_a_typing_run() {
    let mut session = session_with_pages();
    type_chars(&mut session, "ab");
    assert!(session.commit(page_tx(
        "p1",
        "Ink stroke",
        insert("p1", 0, stroke("s1", 0.0))
    )));
    type_chars(&mut session, "cd");
    assert_eq!(text(&session), "abcd");
    let labels: Vec<String> = session
        .history()
        .undo_steps(Some("p1"))
        .into_iter()
        .map(|step| step.label)
        .collect();
    assert_eq!(labels, ["Typing", "Ink stroke", "Typing"]);
    // Edits that are not typing never coalesce, even when they touch.
    assert!(session.edit_page_text("p1", 4, 0, "e", false));
    assert!(session.edit_page_text("p1", 5, 0, "f", false));
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 5);
}

#[test]
fn typing_after_an_undo_does_not_extend_the_older_step() {
    let mut session = session_with_pages();
    type_chars(&mut session, "ab");
    assert!(session.commit(page_tx(
        "p1",
        "Ink stroke",
        insert("p1", 0, stroke("s1", 0.0))
    )));
    session.undo();
    // The "ab" step is on top again, and the caret is right after it, but typing is new.
    type_chars(&mut session, "c");
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 2);
    session.undo();
    assert_eq!(text(&session), "ab");
}

#[test]
fn backspace_and_delete_runs_coalesce() {
    let mut session = session_with_pages();
    session.set_page_source("p1", "hello world");
    // Backspace three times from the end of "hello".
    for at in [4, 3, 2] {
        assert!(session.edit_page_text("p1", at, 1, "", true));
    }
    assert_eq!(text(&session), "he world");
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 1);
    session.undo();
    assert_eq!(text(&session), "hello world");
    session.redo();
    assert_eq!(text(&session), "he world");

    // Delete three times at the same spot.
    for _ in 0..3 {
        assert!(session.edit_page_text("p1", 2, 1, "", true));
    }
    assert_eq!(text(&session), "herld");
    session.undo();
    assert_eq!(text(&session), "he world");
    session.undo();
    assert_eq!(text(&session), "hello world");
    assert!(!session.can_undo());
}

#[test]
fn typing_and_deleting_do_not_merge_with_each_other() {
    let mut session = session_with_pages();
    type_chars(&mut session, "abc");
    assert!(session.edit_page_text("p1", 2, 1, "", true));
    assert_eq!(session.history().undo_steps(Some("p1")).len(), 2);
}

#[test]
fn multi_byte_text_round_trips_and_bad_offsets_are_refused() {
    let mut session = session_with_pages();
    session.set_page_source("p1", "caf\u{e9}!");
    // The e-acute is two bytes, at 3..5. An offset inside it is refused.
    assert!(!session.edit_page_text("p1", 4, 0, "x", true));
    assert!(!session.edit_page_text("p1", 3, 1, "", true));
    assert!(!session.edit_page_text("p1", 99, 0, "x", true));
    assert!(!session.edit_page_text("p1", 3, 99, "", true));
    // Nothing to change.
    assert!(!session.edit_page_text("p1", 0, 0, "", true));
    assert!(!session.can_undo());
    assert!(session.edit_page_text("p1", 3, 2, "e", false));
    assert_eq!(text(&session), "cafe!");
    session.undo();
    assert_eq!(text(&session), "caf\u{e9}!");
}

#[test]
fn a_typing_run_is_bounded() {
    let mut session = session_with_pages();
    type_chars(&mut session, &"a".repeat(1300));
    let steps = session.history().undo_steps(Some("p1")).len();
    assert!((3..=4).contains(&steps), "{steps} steps");
    while session.undo().is_some() {}
    assert_eq!(text(&session), "");
}

#[test]
fn a_text_step_that_no_longer_matches_is_dropped() {
    let mut session = session_with_pages();
    type_chars(&mut session, "abc");
    // The file is reloaded from disk with other text.
    session.set_page_source("p1", "something else");
    assert_eq!(session.undo(), None);
    assert_eq!(text(&session), "something else");
    assert!(!session.can_undo());
}

#[test]
fn text_and_ink_undo_in_the_order_they_happened() {
    let mut session = session_with_pages();
    type_chars(&mut session, "hi");
    assert!(session.commit(page_tx(
        "p1",
        "Ink stroke",
        insert("p1", 0, stroke("s1", 0.0))
    )));
    assert_eq!(session.undo().as_deref(), Some("Ink stroke"));
    assert_eq!(session.undo().as_deref(), Some("Typing"));
    assert_eq!(session.redo().as_deref(), Some("Typing"));
    assert_eq!(session.redo().as_deref(), Some("Ink stroke"));
}

// Notebook-level steps

fn two_sections() -> Session {
    let section = |id: &str, pages: &[&str]| {
        SectionNode::Section(Section {
            id: id.into(),
            name: id.into(),
            color: 0,
            pages: pages.iter().map(|p| PageRef::new(*p, *p, 0)).collect(),
        })
    };
    let notebook = Notebook {
        id: "nb".into(),
        name: "Notebook".into(),
        color: 0,
        sections: vec![section("s1", &["p1", "p2", "p3"]), section("s2", &["p4"])],
    };
    let mut session = Session::new();
    session.set_notebooks(vec![notebook]);
    session
}

fn pages_of(session: &Session, section: &str) -> Vec<String> {
    session
        .current_notebook()
        .and_then(|nb| nb.section(section))
        .map(|s| s.pages.iter().map(|p| p.id.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn moving_a_page_undoes_as_one_notebook_step() {
    let mut session = two_sections();
    let before = session.notebooks().to_vec();
    assert!(session.move_page("p2", "s2", 0));
    assert_eq!(pages_of(&session, "s1"), ["p1", "p3"]);
    assert_eq!(pages_of(&session, "s2"), ["p2", "p4"]);
    assert_eq!(session.history().undo_label(Some("p1")), Some("Move page"));

    assert_eq!(session.undo().as_deref(), Some("Move page"));
    assert_eq!(session.notebooks(), before.as_slice());
    assert_eq!(session.redo().as_deref(), Some("Move page"));
    assert_eq!(pages_of(&session, "s2"), ["p2", "p4"]);
}

#[test]
fn reordering_inside_a_section_undoes() {
    let mut session = two_sections();
    assert!(session.move_page("p1", "s1", 99));
    assert_eq!(pages_of(&session, "s1"), ["p2", "p3", "p1"]);
    assert!(session.move_page("p3", "s1", 0));
    assert_eq!(pages_of(&session, "s1"), ["p3", "p2", "p1"]);
    session.undo();
    assert_eq!(pages_of(&session, "s1"), ["p2", "p3", "p1"]);
    session.undo();
    assert_eq!(pages_of(&session, "s1"), ["p1", "p2", "p3"]);
}

#[test]
fn a_move_that_does_not_fit_changes_nothing() {
    let mut session = two_sections();
    let before = session.clone();
    assert!(!session.move_page("nope", "s2", 0));
    assert!(!session.move_page("p1", "nope", 0));
    // The raw operation refuses an index past the end instead of clamping it.
    let far = Op::MovePage {
        notebook: "nb".into(),
        page: "p1".into(),
        from: Place {
            section: "s1".into(),
            index: 0,
        },
        to: Place {
            section: "s2".into(),
            index: 5,
        },
    };
    assert!(!session.commit(Transaction::new(Scope::Notebook, "Far").with(far)));
    assert_eq!(session, before);
    assert!(!session.can_undo());
}

#[test]
fn a_cross_page_command_undoes_as_one_step() {
    // A rename with link updates: the move plus edits to the text of two other pages.
    let mut session = two_sections();
    session.set_page_source("p2", "see [[One]]");
    session.set_page_source("p3", "also [[One]]");
    let tx = Transaction::new(Scope::Notebook, "Rename page")
        .with(Op::MovePage {
            notebook: "nb".into(),
            page: "p1".into(),
            from: Place {
                section: "s1".into(),
                index: 0,
            },
            to: Place {
                section: "s2".into(),
                index: 1,
            },
        })
        .with(Op::ReplaceText {
            page: "p2".into(),
            at: 6,
            removed: "One".into(),
            inserted: "Uno".into(),
        })
        .with(Op::ReplaceText {
            page: "p3".into(),
            at: 7,
            removed: "One".into(),
            inserted: "Uno".into(),
        });
    assert!(session.commit(tx));
    assert_eq!(session.page_text("p2"), Some("see [[Uno]]"));
    assert_eq!(session.page_text("p3"), Some("also [[Uno]]"));
    assert_eq!(pages_of(&session, "s2"), ["p4", "p1"]);

    // One undo reverts all three, whichever page is shown.
    session.selection_mut().page = Some("p3".into());
    assert_eq!(session.undo().as_deref(), Some("Rename page"));
    assert_eq!(session.page_text("p2"), Some("see [[One]]"));
    assert_eq!(session.page_text("p3"), Some("also [[One]]"));
    assert_eq!(pages_of(&session, "s1"), ["p1", "p2", "p3"]);
    assert!(!session.can_undo());
    assert_eq!(session.redo().as_deref(), Some("Rename page"));
    assert_eq!(session.page_text("p3"), Some("also [[Uno]]"));
}

#[test]
fn a_notebook_step_with_a_bad_operation_is_all_or_nothing() {
    let mut session = two_sections();
    session.set_page_source("p2", "text");
    let before = session.clone();
    let tx = Transaction::new(Scope::Notebook, "Rename page")
        .with(Op::MovePage {
            notebook: "nb".into(),
            page: "p1".into(),
            from: Place {
                section: "s1".into(),
                index: 0,
            },
            to: Place {
                section: "s2".into(),
                index: 0,
            },
        })
        .with(Op::ReplaceText {
            page: "p2".into(),
            at: 0,
            removed: "wrong".into(),
            inserted: "x".into(),
        });
    assert!(!session.commit(tx));
    assert_eq!(pages_of(&session, "s1"), ["p1", "p2", "p3"]);
    assert_eq!(session.page_text("p2"), before.page_text("p2"));
    assert!(!session.can_undo());
}

#[test]
fn page_and_notebook_steps_undo_in_the_order_they_happened() {
    let mut session = two_sections();
    session.selection_mut().page = Some("p1".into());
    type_chars(&mut session, "hi");
    assert!(session.move_page("p3", "s2", 0));
    assert!(session.commit(page_tx(
        "p1",
        "Ink stroke",
        insert("p1", 0, stroke("s1", 0.0))
    )));
    let order: Vec<String> = session
        .history()
        .undo_steps(Some("p1"))
        .into_iter()
        .map(|step| step.label)
        .collect();
    assert_eq!(order, ["Typing", "Move page", "Ink stroke"]);

    assert_eq!(session.undo().as_deref(), Some("Ink stroke"));
    assert_eq!(session.undo().as_deref(), Some("Move page"));
    assert_eq!(session.undo().as_deref(), Some("Typing"));
    assert_eq!(session.undo(), None);
    assert_eq!(session.redo().as_deref(), Some("Typing"));
    assert_eq!(session.redo().as_deref(), Some("Move page"));
    assert_eq!(session.redo().as_deref(), Some("Ink stroke"));
}

#[test]
fn a_notebook_step_is_available_from_every_page() {
    let mut session = two_sections();
    session.selection_mut().page = Some("p1".into());
    assert!(session.move_page("p3", "s2", 0));
    session.selection_mut().page = Some("p4".into());
    assert!(session.can_undo());
    session.selection_mut().page = None;
    assert!(session.can_undo());
    assert_eq!(session.undo().as_deref(), Some("Move page"));
    assert!(session.can_redo());
    // A page edit afterwards does not clear the notebook's redo stack.
    session.selection_mut().page = Some("p1".into());
    type_chars(&mut session, "x");
    assert!(session.can_redo());
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

// Every Ready command

/// How to run each Ready undoable command in a test. A PR that makes another command
/// undoable and Ready adds a row here, or `every_ready_undoable_command_has_an_undo_case`
/// fails.
const UNDO_CASES: &[(&str, &str)] = &[
    ("ink.add_stroke", r#"{ "points": [[1, 2], [30, 40, 0.5]] }"#),
    (
        "ink.erase",
        r#"{ "points": [[500, 100]], "radius": 10, "mode": "point" }"#,
    ),
    ("view.rule_lines", r#"{ "style": "college" }"#),
    ("view.page_color", r##"{ "color": "#DCEBF7" }"##),
    ("view.paper_size", r#"{ "size": "a4" }"#),
    ("text.insert", r#"{ "text": "X" }"#),
    ("text.delete", r#"{ "direction": "backward" }"#),
    ("text.enter", "{}"),
    ("text.tab", "{}"),
    ("edit.cut", "{}"),
    ("edit.delete", "{}"),
    ("edit.paste", r#"{ "text": "pasted" }"#),
    ("edit.paste_text_only", r#"{ "text": "**plain**" }"#),
    ("container.new", r#"{ "x": 40, "y": 60 }"#),
    ("container.move", r#"{ "id": "c0", "x": 40, "y": 60 }"#),
    ("container.resize", r#"{ "id": "c0", "width": 300 }"#),
    ("format.bold", "{}"),
    ("format.italic", "{}"),
    ("format.underline", "{}"),
    ("format.strikethrough", "{}"),
    ("format.subscript", "{}"),
    ("format.superscript", "{}"),
    ("format.highlight", "{}"),
    ("format.font_color", r##"{ "color": "#E53935" }"##),
    ("format.font_family", r#"{ "name": "Courier New" }"#),
    ("format.font_size", r#"{ "points": 14 }"#),
    ("format.bullets", "{}"),
    ("format.numbering", "{}"),
    ("format.indent", "{}"),
    ("format.outdent", "{}"),
    ("format.clear", "{}"),
    ("format.align_left", "{}"),
    ("format.align_center", "{}"),
    ("format.align_right", "{}"),
    ("style.apply", r#"{ "style": "heading2" }"#),
    ("tag.todo", "{}"),
];

/// Cases that need page text other than "one" for the command to change something.
const SOURCES: &[(&str, &str)] = &[
    ("format.outdent", "  - one"),
    ("format.indent", "- one"),
    ("format.clear", "**one**"),
    (
        "format.align_left",
        "<!-- notecraft:align=center -->
one",
    ),
];

/// Cases that need a selection first: the whole text of the page is selected.
const NEEDS_SELECTION: &[&str] = &[
    "edit.cut",
    "edit.delete",
    "format.bold",
    "format.italic",
    "format.underline",
    "format.strikethrough",
    "format.subscript",
    "format.superscript",
    "format.highlight",
    "format.font_color",
    "format.font_family",
    "format.font_size",
    "format.bullets",
    "format.numbering",
    "format.indent",
    "format.outdent",
    "format.clear",
    "format.align_left",
    "format.align_center",
    "format.align_right",
    "style.apply",
    "tag.todo",
];

type Document = (
    Vec<Notebook>,
    crate::InkState,
    crate::PageStyle,
    Option<String>,
    Option<String>,
    Vec<notecraft_model::Container>,
);

/// What undo has to restore: everything in the document, not the view or the history.
fn document(session: &Session) -> Document {
    (
        session.notebooks().to_vec(),
        session.ink().clone(),
        session.style_of("p1"),
        session.page_text("p1").map(str::to_owned),
        session.page_text("p2").map(str::to_owned),
        session.page_containers("p1"),
    )
}

#[test]
fn every_ready_undoable_command_has_an_undo_case() {
    let registry = Registry::builtin();
    for spec in registry.specs() {
        let ready = spec.status == crate::cmd::Status::Ready;
        if ready && spec.undoable {
            let id = spec.id.as_str();
            let covered = UNDO_CASES
                .iter()
                .chain(crate::cmd::structure::UNDO_CASES)
                .any(|(case, _)| *case == id);
            assert!(
                covered,
                "{id} is Ready and undoable but has no row in UNDO_CASES"
            );
        }
    }
    for (id, _) in UNDO_CASES.iter().chain(crate::cmd::structure::UNDO_CASES) {
        let spec = registry.get(id).unwrap();
        assert!(spec.undoable, "{id} is in UNDO_CASES but not undoable");
    }
}

#[test]
fn every_undoable_command_undoes_and_redoes_exactly() {
    let registry = Registry::builtin();
    for (id, args) in UNDO_CASES {
        let args: Value = serde_json::from_str(args).unwrap();
        let mut session = session_with_pages();
        let source = SOURCES
            .iter()
            .find(|(case, _)| case == id)
            .map_or("one", |(_, text)| *text);
        session.set_page_source("p1", source);
        session.set_page_source("p2", "two");
        if NEEDS_SELECTION.contains(id) {
            run(&mut session, &registry, "edit.select_all", Value::Null);
        }
        // A line to erase for ink.erase, put there without an undo step.
        let line = Stroke {
            points: vec![
                InkPoint::new(0.0, 100.0, 1.0),
                InkPoint::new(1000.0, 100.0, 1.0),
            ],
            ..stroke("s1", 0.0)
        };
        assert!(insert("p1", 0, line).apply(&mut session));
        let before = document(&session);

        run(&mut session, &registry, id, args.clone());
        let after = document(&session);
        assert_ne!(after, before, "{id} changed nothing");
        assert!(session.can_undo(), "{id} left no undo step");
        assert!(!session.can_redo(), "{id}");

        run(&mut session, &registry, "edit.undo", Value::Null);
        assert_eq!(document(&session), before, "{id}: undo");
        assert!(!session.can_undo() && session.can_redo(), "{id}");

        run(&mut session, &registry, "edit.redo", Value::Null);
        assert_eq!(document(&session), after, "{id}: redo");

        // Twice over, and a long chain: undo all the way, redo all the way.
        run(&mut session, &registry, id, args.clone());
        run(&mut session, &registry, id, args.clone());
        let end = document(&session);
        while session.can_undo() {
            run(&mut session, &registry, "edit.undo", Value::Null);
        }
        assert_eq!(document(&session), before, "{id}: undo all");
        while session.can_redo() {
            run(&mut session, &registry, "edit.redo", Value::Null);
        }
        assert_eq!(document(&session), end, "{id}: redo all");
    }
}

#[test]
fn commands_that_are_not_undoable_leave_the_history_alone() {
    let registry = Registry::builtin();
    let args = |id: &str| -> Value {
        match id {
            "nav.page.select" | "nav.section.select" | "nav.notebook.select" => Value::Null,
            _ => Value::Null,
        }
    };
    for spec in registry.specs() {
        let id = spec.id.as_str();
        let runnable = spec.status == crate::cmd::Status::Ready
            && !spec.undoable
            && spec.args_schema.iter().all(|arg| !arg.required)
            && !id.starts_with("file.")
            && !id.starts_with("edit.");
        if !runnable {
            continue;
        }
        let mut session = session_with_pages();
        run(
            &mut session,
            &registry,
            "ink.add_stroke",
            json!({ "points": [[1, 2]] }),
        );
        let history = session.history().clone();
        let result = registry.run(&mut session, id, args(id));
        assert!(result.is_ok(), "{id}: {result:?}");
        assert_eq!(*session.history(), history, "{id} touched the history");
    }
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

#[test]
fn reopening_a_notebook_folder_drops_stale_text_steps() {
    use crate::open::open_folder;
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("Sec")).unwrap();
    std::fs::write(dir.path().join("Sec/A.md"), "# Hello\n").unwrap();
    let opened = open_folder(dir.path()).unwrap();
    let id = opened.notebook.id.clone();
    let mut session = Session::new();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&id, dir.path());
    assert!(session.load_current_page());
    let page = session.selection().page.clone().unwrap();

    assert!(session.edit_page_text(&page, 0, 0, "x", false));
    assert!(session.can_undo());
    // The folder is opened again with other text on disk: the page is read again.
    std::fs::write(dir.path().join("Sec/A.md"), "# Changed outside\n").unwrap();
    session.set_notebook_root(&id, dir.path());
    assert!(session.load_current_page());
    assert_eq!(session.page_text(&page), Some("# Changed outside\n"));
    // The old step no longer fits: it is dropped and the text is untouched.
    assert_eq!(session.undo(), None);
    assert_eq!(session.page_text(&page), Some("# Changed outside\n"));
    assert!(!session.can_undo());
}
