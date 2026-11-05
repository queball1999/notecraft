#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::cmd::Registry;
use crate::open::open_folder;
use crate::session::{SaveOutcome, SaveState, TextCursor};
use crate::stamp::{page_id_from_bits, Clock, IdSource, Stamp};

/// 2026-11-05 15:07:00 UTC.
const T0: i64 = 1_793_891_220;

fn notebook(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    for (name, text) in files {
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir
}

/// A session with a clock that reads `secs` and ids that count up from 1.
fn open(dir: &tempfile::TempDir) -> (Session, Arc<AtomicI64>) {
    let mut session = Session::new();
    let opened = open_folder(dir.path()).unwrap();
    let id = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&id, dir.path());
    session.load_current_page();
    let secs = Arc::new(AtomicI64::new(T0));
    let clock_secs = Arc::clone(&secs);
    session.set_clock(Clock::new(move || {
        Stamp::from_unix(clock_secs.load(Ordering::Relaxed))
    }));
    let next = AtomicU64::new(1);
    session.set_id_source(IdSource::new(move |_| {
        Some(page_id_from_bits(next.fetch_add(1, Ordering::Relaxed)))
    }));
    (session, secs)
}

fn current(session: &Session) -> String {
    session.selection().page.clone().unwrap()
}

fn read(dir: &tempfile::TempDir, rel: &str) -> String {
    std::fs::read_to_string(dir.path().join(rel)).unwrap()
}

fn append(session: &mut Session, id: &str, text: &str) {
    let at = session.page_text(id).unwrap().find("\n```notecraft-canvas");
    let at = at.unwrap_or_else(|| session.page_text(id).unwrap().len());
    assert!(session.edit_page_text(id, at, 0, text, false));
}

const STAMPED: &str =
    "created: 2026-11-05T15:07:00Z\nmodified: 2026-11-05T15:07:00Z\nnotecraft:\n  id: pg_000000000001\n";

#[test]
fn the_first_save_adds_the_three_keys_and_keeps_the_rest_byte_for_byte() {
    let head = "---\n# kept comment\ntitle: Meeting notes\nmood: \"quoted: value\"\ntags: [a, b]\n";
    let text = format!("{head}---\nHello\n");
    let dir = notebook(&[("Sec/A.notecraft", &text)]);
    let (mut s, _) = open(&dir);
    let id = current(&s);
    append(&mut s, &id, "more\n");
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    let saved = read(&dir, "Sec/A.notecraft");
    assert!(
        saved.starts_with(&format!("{head}{STAMPED}---\nHello\nmore\n")),
        "{saved}"
    );
    assert!(!s.is_page_dirty(&id));
    // The session holds what the file holds.
    assert_eq!(s.page_text(&id), Some(saved.as_str()));
    // A page with no frontmatter gets a new block.
    let dir = notebook(&[("Sec/B.notecraft", "Plain\n")]);
    let (mut s, _) = open(&dir);
    let id = current(&s);
    append(&mut s, &id, "x\n");
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    let saved = read(&dir, "Sec/B.notecraft");
    assert!(
        saved.starts_with(&format!("---\n{STAMPED}---\nPlain\nx\n")),
        "{saved}"
    );
}

#[test]
fn later_saves_keep_the_id_and_created_and_update_modified() {
    let dir = notebook(&[("Sec/A.notecraft", "Hello\n")]);
    let (mut s, secs) = open(&dir);
    let id = current(&s);
    append(&mut s, &id, "one\n");
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    secs.store(T0 + 3_600, Ordering::Relaxed);
    append(&mut s, &id, "two\n");
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    let saved = read(&dir, "Sec/A.notecraft");
    assert!(
        saved.starts_with(
            "---\ncreated: 2026-11-05T15:07:00Z\nmodified: 2026-11-05T16:07:00Z\nnotecraft:\n  id: pg_000000000001\n---\nHello\none\ntwo\n"
        ),
        "{saved}"
    );
    // A save that finds the same content writes nothing and keeps modified.
    secs.store(T0 + 7_200, Ordering::Relaxed);
    s.mark_page_dirty(&id);
    assert_eq!(s.save_page(&id), SaveOutcome::Unchanged);
    assert_eq!(read(&dir, "Sec/A.notecraft"), saved);
    // A reopened page keeps its id, and the id source is not asked again.
    let (mut again, _) = open(&dir);
    let id = current(&again);
    append(&mut again, &id, "three\n");
    assert_eq!(again.save_page(&id), SaveOutcome::Saved);
    let saved = read(&dir, "Sec/A.notecraft");
    assert!(saved.contains("  id: pg_000000000001\n"), "{saved}");
    assert_eq!(saved.matches("id:").count(), 1);
}

#[test]
fn existing_values_are_kept_and_modified_alone_is_updated() {
    let text = "---\ncreated: 2020-01-02T03:04:05+01:00\nmodified: 2020-01-02\nnotecraft:\n  id: pg_MINE\n  paper: lined    # keep\n---\nHi\n";
    let dir = notebook(&[("Sec/A.notecraft", text)]);
    let (mut s, _) = open(&dir);
    let id = current(&s);
    append(&mut s, &id, "x\n");
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    let saved = read(&dir, "Sec/A.notecraft");
    assert!(
        saved.starts_with("---\ncreated: 2020-01-02T03:04:05+01:00\nmodified: 2026-11-05T15:07:00Z\nnotecraft:\n  id: pg_MINE\n  paper: lined    # keep\n---\nHi\nx\n"),
        "{saved}"
    );
}

#[test]
fn a_clock_with_an_offset_writes_the_offset() {
    let dir = notebook(&[("Sec/A.notecraft", "Hello\n")]);
    let (mut s, _) = open(&dir);
    s.set_clock(Clock::fixed(Stamp::new(2026, 11, 5, 9, 7).unwrap()).with_offset_minutes(-300));
    let id = current(&s);
    append(&mut s, &id, "x\n");
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    assert!(read(&dir, "Sec/A.notecraft").contains("created: 2026-11-05T09:07:00-05:00\n"));
}

#[test]
fn markdown_pages_stay_plain() {
    let dir = notebook(&[
        ("Sec/A.md", "Hello\n"),
        ("Sec/B.md", "---\ntitle: Has frontmatter\n---\nHi\n"),
    ]);
    let (mut s, _) = open(&dir);
    for page in ["Sec/A.md", "Sec/B.md"] {
        let id = format!("{}/{page}", s.notebooks()[0].id);
        s.select_page_by_id(&id);
        s.load_current_page();
        append(&mut s, &id, "x\n");
        assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    }
    assert_eq!(read(&dir, "Sec/A.md"), "Hello\nx\n");
    assert_eq!(
        read(&dir, "Sec/B.md"),
        "---\ntitle: Has frontmatter\n---\nHi\nx\n"
    );
}

#[test]
fn a_markdown_page_that_becomes_a_notecraft_page_gets_its_id() {
    let dir = notebook(&[("Sec/A.md", "---\ntitle: T\n---\n# Hi\n")]);
    let (mut s, _) = open(&dir);
    let id = current(&s);
    Registry::builtin()
        .run(&mut s, "draw.pen", json!({}))
        .unwrap();
    Registry::builtin()
        .run(
            &mut s,
            "ink.add_stroke",
            json!({ "points": [[1, 2], [3, 4]] }),
        )
        .unwrap();
    let SaveOutcome::Promoted { new_id } = s.save_page(&id) else {
        panic!("not promoted");
    };
    let saved = read(&dir, "Sec/A.notecraft");
    assert!(
        saved.starts_with(&format!("---\ntitle: T\n{STAMPED}---\n# Hi\n")),
        "{saved}"
    );
    assert_eq!(s.page_text(&new_id), Some(saved.as_str()));
    assert!(!s.is_page_dirty(&new_id));
}

#[test]
fn read_only_and_conflicting_pages_are_not_touched() {
    let newer = "Words\n\n```notecraft-canvas\n{ \"format\": 99 }\n```\n";
    let dir = notebook(&[("Sec/A.notecraft", newer), ("Sec/B.notecraft", "Mine\n")]);
    let (mut s, _) = open(&dir);
    let a = current(&s);
    assert!(s.current_page_notice().unwrap().read_only);
    s.mark_page_dirty(&a);
    assert_eq!(s.save_page(&a), SaveOutcome::ReadOnly);
    assert_eq!(read(&dir, "Sec/A.notecraft"), newer);

    let b = format!("{}/Sec/B.notecraft", s.notebooks()[0].id);
    s.select_page_by_id(&b);
    s.load_current_page();
    append(&mut s, &b, "unsaved\n");
    let theirs = "Theirs\n";
    std::fs::write(dir.path().join("Sec/B.notecraft"), theirs).unwrap();
    assert_eq!(s.save_page(&b), SaveOutcome::Conflict);
    assert_eq!(read(&dir, "Sec/B.notecraft"), theirs);
    assert_eq!(s.save_page(&b), SaveOutcome::Conflict);
    assert_eq!(read(&dir, "Sec/B.notecraft"), theirs);
}

#[test]
fn our_own_write_is_not_an_outside_change_and_does_not_make_the_page_dirty() {
    let dir = notebook(&[("Sec/A.notecraft", "Hello\n")]);
    let (mut s, secs) = open(&dir);
    let id = current(&s);
    for n in 0..3 {
        secs.store(T0 + n * 120, Ordering::Relaxed);
        append(&mut s, &id, "x\n");
        assert_eq!(s.save_page(&id), SaveOutcome::Saved);
        // The watcher reports our write.
        assert!(s.note_outside_change(&dir.path().join("Sec/A.notecraft")));
        assert_eq!(s.save_state(), SaveState::default());
        assert!(!s.is_page_dirty(&id));
        assert!(s.journal_candidates().is_empty());
        assert!(s.current_conflict().is_none());
    }
    // The text on screen is what the file holds, so an outside edit is compared fairly.
    let saved = read(&dir, "Sec/A.notecraft");
    assert_eq!(s.page_text(&id), Some(saved.as_str()));
    let outside = saved.replace("Hello", "Howdy");
    std::fs::write(dir.path().join("Sec/A.notecraft"), &outside).unwrap();
    s.note_outside_change(&dir.path().join("Sec/A.notecraft"));
    assert_eq!(s.page_text(&id), Some(outside.as_str()));
    assert!(!s.is_page_dirty(&id));
}

#[test]
fn the_journal_text_carries_no_stamp_and_a_restored_page_gets_one_on_save() {
    let dir = notebook(&[("Sec/A.notecraft", "Hello\n")]);
    let (mut s, _) = open(&dir);
    let id = current(&s);
    append(&mut s, &id, "x\n");
    let journal = s.journal_candidates();
    assert_eq!(journal.len(), 1);
    assert!(!journal[0].3.contains("created:"), "{}", journal[0].3);
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    // Dirty again with the same content: the journal text is the saved file.
    s.mark_page_dirty(&id);
    assert_eq!(s.journal_candidates()[0].3, read(&dir, "Sec/A.notecraft"));
}

#[test]
fn the_caret_and_undo_follow_the_lines_the_first_save_adds() {
    let dir = notebook(&[("Sec/A.notecraft", "Hello\n")]);
    let (mut s, _) = open(&dir);
    let id = current(&s);
    s.set_text_cursor(&id, TextCursor::caret(5));
    assert!(s.edit_page_text(&id, 5, 0, " there", true));
    s.set_text_cursor(&id, TextCursor::caret(11));
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    let text = s.page_text(&id).unwrap().to_owned();
    let caret = s.text_cursor().unwrap();
    assert_eq!(text.get(caret.head - 6..caret.head), Some(" there"));
    // Undo still finds the typed text.
    assert!(s.undo().is_some());
    let undone = s.page_text(&id).unwrap();
    assert!(
        undone.contains("Hello\n") && !undone.contains("there"),
        "{undone}"
    );
    assert!(undone.starts_with("---\ncreated:"));
    s.redo();
    assert!(s.page_text(&id).unwrap().contains("Hello there\n"));
    // The undone and redone edits are unsaved edits like any other, and the id stays.
    assert!(s.is_page_dirty(&id));
    assert_eq!(s.save_page(&id), SaveOutcome::Unchanged);
    assert!(s.undo().is_some());
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    let saved = read(&dir, "Sec/A.notecraft");
    assert!(saved.contains("  id: pg_000000000001\n") && saved.contains("Hello\n"));
    assert_eq!(saved.matches("id:").count(), 1);
}

#[test]
fn without_a_clock_or_ids_the_file_is_written_as_before() {
    let dir = notebook(&[("Sec/A.notecraft", "Hello\n")]);
    let (mut s, _) = open(&dir);
    s.set_clock(Clock::new(|| None));
    s.set_id_source(IdSource::new(|_| None));
    let id = current(&s);
    append(&mut s, &id, "x\n");
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    assert!(!read(&dir, "Sec/A.notecraft").contains("created:"));
    // An id source with no clock still writes the id.
    s.set_id_source(IdSource::new(|_| Some("pg_TESTONLY".to_owned())));
    append(&mut s, &id, "y\n");
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    let saved = read(&dir, "Sec/A.notecraft");
    assert!(
        saved.starts_with("---\nnotecraft:\n  id: pg_TESTONLY\n---\nHello\n"),
        "{saved}"
    );
    assert!(!saved.contains("created:"));
}

#[test]
fn an_unwritable_frontmatter_is_left_alone() {
    let flow = "---\nnotecraft: {paper: lined}\n---\nHi\n";
    let dir = notebook(&[("Sec/A.notecraft", flow)]);
    let (mut s, _) = open(&dir);
    let id = current(&s);
    append(&mut s, &id, "x\n");
    assert_eq!(s.save_page(&id), SaveOutcome::Saved);
    assert!(read(&dir, "Sec/A.notecraft").starts_with(flow));
    assert!(!read(&dir, "Sec/A.notecraft").contains("created:"));
}

#[test]
fn the_shift_leaves_the_end_of_a_page_in_front_of_a_new_canvas_block() {
    let old = "---\nt: 1\n---\nBody\n";
    let new = "---\nt: 1\ncreated: x\n---\nBody\n\n```notecraft-canvas\n{}\n```\n";
    let shift = OffsetShift::between(old, new);
    let body_in_new = new.find("Body").unwrap();
    assert_eq!(shift.apply(0), 0);
    assert_eq!(shift.apply(old.find("Body").unwrap()), body_in_new);
    assert_eq!(shift.apply(old.find("Body").unwrap() + 2), body_in_new + 2);
    assert_eq!(shift.apply(old.len()), body_in_new + "Body\n".len());
}
