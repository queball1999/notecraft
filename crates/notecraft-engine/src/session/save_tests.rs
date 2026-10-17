#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use serde_json::json;

use super::*;
use crate::cmd::Registry;
use crate::io::SessionIo;
use crate::open::open_folder;
use crate::session::SaveState;

const BASIC: &str = include_str!("../../../../testdata/notecraft/ink-basic.notecraft");

fn notebook(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    for (name, text) in files {
        let path = dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    dir
}

fn open(dir: &tempfile::TempDir) -> Session {
    let mut session = Session::new();
    let opened = open_folder(dir.path()).unwrap();
    let id = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&id, dir.path());
    session.load_current_page();
    session
}

fn run(session: &mut Session, id: &str, args: serde_json::Value) -> serde_json::Value {
    let out = Registry::builtin().run(session, id, args).unwrap();
    out.value
}

fn draw(session: &mut Session, points: serde_json::Value) {
    run(session, "draw.pen", json!({}));
    run(session, "ink.add_stroke", json!({ "points": points }));
}

fn current_id(session: &Session) -> String {
    session.selection().page.clone().unwrap()
}

fn read(dir: &tempfile::TempDir, rel: &str) -> String {
    std::fs::read_to_string(dir.path().join(rel)).unwrap()
}

fn pump_until(io: &mut SessionIo, session: &mut Session, done: impl Fn(&Session) -> bool) {
    let start = Instant::now();
    while !done(session) {
        assert!(start.elapsed() < Duration::from_secs(10), "timed out");
        io.pump(session, Instant::now(), true);
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn io_for(journal: &tempfile::TempDir) -> SessionIo {
    SessionIo::new(Some(journal.path().join("recovery")), || {})
        .with_debounce(Duration::from_millis(100))
}

#[test]
fn a_notecraft_page_shows_its_text_and_strokes() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let session = open(&dir);
    assert_eq!(session.current_page_source().unwrap().text, BASIC);
    assert_eq!(session.current_ink().unwrap().len(), 2);
    assert_eq!(session.current_page_notice(), None);
    let visible = session
        .current_page_source()
        .unwrap()
        .visible_text()
        .to_owned();
    assert!(visible.contains("Remember to send the deck"));
    assert!(!visible.contains("notecraft-canvas"));
    assert!(!session.is_page_dirty(&current_id(&session)));
}

#[test]
fn a_page_whose_canvas_block_cannot_be_read_is_kept_and_not_saved_over() {
    let text = "Words\n\n```notecraft-canvas\n{ \"ink\": [ \n```\n";
    let dir = notebook(&[("Sec/A.notecraft", text)]);
    let mut session = open(&dir);
    let notice = session.current_page_notice().unwrap();
    assert!(notice.read_only, "{notice:?}");
    assert!(session.current_ink().is_none());
    let copy = dir
        .path()
        .join(".notecraft/history/Sec_A.notecraft/canvas-unreadable.notecraft");
    assert_eq!(std::fs::read_to_string(copy).unwrap(), text);
    let id = current_id(&session);
    session.mark_page_dirty(&id);
    assert_eq!(session.save_page(&id), SaveOutcome::ReadOnly);
    assert_eq!(read(&dir, "Sec/A.notecraft"), text);
}

#[test]
fn ink_is_saved_with_a_minimal_diff_and_reloads() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    assert_eq!(session.save_page(&id), SaveOutcome::NotDirty);
    draw(&mut session, json!([[5, 6, 0.5], [7, 8, 0.25]]));
    assert!(session.is_page_dirty(&id), "a stroke marks the page dirty");
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert!(!session.is_page_dirty(&id));
    let saved = read(&dir, "Sec/A.notecraft");
    let ink_at = BASIC.find("\"ink\": ").unwrap();
    assert_eq!(saved.get(..ink_at), BASIC.get(..ink_at));
    assert!(saved.ends_with("  \"z\": [\"c1\", \"c2\", \"sh1\", \"s1\", \"s2\"],\n  \"future\": { \"keep\": [\"me\", 1, null] }\n}\n```\n"));
    let again = open(&dir);
    assert_eq!(again.current_ink().unwrap().len(), 3);
    let stored = |s: &Session| -> Vec<_> {
        s.current_ink()
            .unwrap()
            .strokes()
            .iter()
            .map(notecraft_storage::normalized)
            .collect()
    };
    assert_eq!(stored(&again), stored(&session));
    // Undoing the stroke and saving removes it again.
    run(&mut session, "edit.undo", json!({}));
    assert!(session.is_page_dirty(&id));
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(open(&dir).current_ink().unwrap().len(), 2);
}

#[test]
fn a_markdown_page_with_ink_is_promoted_on_save() {
    let dir = notebook(&[("Sec/A.md", "# Hi\n"), ("Sec/B.md", "other\n")]);
    let mut session = open(&dir);
    let id = current_id(&session);
    assert!(id.ends_with("Sec/A.md"), "{id}");
    draw(&mut session, json!([[1, 2], [3, 4]]));
    let SaveOutcome::Promoted { new_id } = session.save_page(&id) else {
        panic!("not promoted");
    };
    assert!(new_id.ends_with("Sec/A.notecraft"));
    assert!(!dir.path().join("Sec/A.md").exists());
    assert_eq!(current_id(&session), new_id);
    assert!(!session.is_page_dirty(&new_id) && !session.is_page_dirty(&id));
    assert!(session
        .current_section()
        .unwrap()
        .pages
        .iter()
        .any(|p| p.id == new_id));
    assert_eq!(session.current_ink().unwrap().len(), 1);
    assert!(read(&dir, "Sec/A.notecraft").starts_with("# Hi\n\n```notecraft-canvas\n"));
    // The watcher events for the two files change nothing and make no conflict.
    session.note_outside_change(&dir.path().join("Sec/A.notecraft"));
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    assert_eq!(session.save_state(), SaveState::default());
    // A second stroke saves to the same new file.
    draw(&mut session, json!([[9, 9]]));
    assert_eq!(session.save_page(&new_id), SaveOutcome::Saved);
    let again = open(&dir);
    assert!(again.current_page().unwrap().id.ends_with("A.notecraft"));
    assert_eq!(again.current_ink().unwrap().len(), 2);
}

#[test]
fn a_stroke_on_a_markdown_page_autosaves_as_notecraft_through_the_io_worker() {
    let dir = notebook(&[("Sec/A.md", "# Hi\n")]);
    let journal = tempfile::TempDir::new().unwrap();
    let mut session = Session::new();
    let opened = open_folder(dir.path()).unwrap();
    let nb = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&nb, dir.path());
    let mut io = io_for(&journal);
    pump_until(&mut io, &mut session, |s| s.current_page_source().is_some());
    draw(&mut session, json!([[1, 2], [3, 4]]));
    let t0 = Instant::now();
    io.pump(&mut session, t0, true);
    io.pump(&mut session, t0 + Duration::from_secs(31), true);
    assert!(dir.path().join("Sec/A.notecraft").exists());
    assert!(!dir.path().join("Sec/A.md").exists());
    assert!(!session.has_unsaved_changes());
    assert_eq!(session.current_ink().unwrap().len(), 1);
    assert!(read(&dir, "Sec/A.notecraft").contains("\"ink\""));
}

#[test]
fn an_outside_change_to_a_page_with_unsaved_strokes_is_a_conflict() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    draw(&mut session, json!([[5, 6], [7, 8]]));
    let theirs = BASIC.replace("Remember to send", "Do not forget to send");
    std::fs::write(dir.path().join("Sec/A.notecraft"), &theirs).unwrap();
    assert_eq!(session.save_page(&id), SaveOutcome::Conflict);
    assert_eq!(read(&dir, "Sec/A.notecraft"), theirs);
    assert!(session.is_page_dirty(&id));
    assert_eq!(session.save_state().conflicts, 1);
    // Keeping mine writes the strokes over the outside version.
    assert!(session.resolve_current_conflict(true));
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(read(&dir, "Sec/A.notecraft").matches("\"tool\"").count(), 3);
}

#[test]
fn taking_the_outside_version_reloads_its_strokes() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let mut session = open(&dir);
    draw(&mut session, json!([[5, 6], [7, 8]]));
    assert_eq!(session.current_ink().unwrap().len(), 3);
    std::fs::write(
        dir.path().join("Sec/A.notecraft"),
        "Plain now\n\n```notecraft-canvas\n{ \"ink\": [] }\n```\n",
    )
    .unwrap();
    let id = current_id(&session);
    assert_eq!(session.save_page(&id), SaveOutcome::Conflict);
    assert!(session.resolve_current_conflict(false));
    assert!(session.current_ink().is_none());
    assert!(!session.is_page_dirty(&id));
}

#[test]
fn recovery_after_a_crash_brings_the_strokes_back() {
    let dir = notebook(&[("Sec/A.md", "# Hi\n")]);
    let journal = tempfile::TempDir::new().unwrap();
    {
        let mut session = Session::new();
        let opened = open_folder(dir.path()).unwrap();
        let nb = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&nb, dir.path());
        let mut io = io_for(&journal);
        pump_until(&mut io, &mut session, |s| s.current_page_source().is_some());
        draw(&mut session, json!([[1, 2], [3, 4]]));
        let now = Instant::now();
        io.pump(&mut session, now, true);
        io.pump(&mut session, now + Duration::from_secs(6), true);
        // Crash: no flush, nothing saved.
    }
    assert!(!dir.path().join("Sec/A.notecraft").exists());
    let entries = notecraft_storage::Journal::new(journal.path().join("recovery")).load_all();
    assert_eq!(entries.len(), 1, "the stroke reached the journal");

    let mut session = Session::new();
    let opened = open_folder(dir.path()).unwrap();
    let nb = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&nb, dir.path());
    session.load_current_page();
    assert!(session.current_ink().is_none());
    session.offer_recovery(entries);
    assert_eq!(session.pending_recovery().len(), 1);
    assert_eq!(session.restore_recovery(), 1);
    assert_eq!(session.current_ink().unwrap().len(), 1);
    let id = current_id(&session);
    assert!(matches!(
        session.save_page(&id),
        SaveOutcome::Promoted { .. }
    ));
}

#[test]
fn convert_to_markdown_asks_first_then_drops_ink_and_markers() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let mut session = open(&dir);
    let out = run(&mut session, "page.convert_to_markdown", json!({}));
    assert_eq!(out["needs_confirm"], true);
    assert_eq!(out["warning"], CONVERT_WARNING);
    assert!(dir.path().join("Sec/A.notecraft").exists());
    let out = run(
        &mut session,
        "page.convert_to_markdown",
        json!({ "confirm": true }),
    );
    assert_eq!(out["converted"], true);
    assert!(!dir.path().join("Sec/A.notecraft").exists());
    let text = read(&dir, "Sec/A.md");
    assert!(text.contains("## Agenda") && text.contains("Remember to send"));
    assert!(!text.contains("notecraft:container") && !text.contains("notecraft-canvas"));
    assert!(session.current_ink().is_none());
    assert!(current_id(&session).ends_with("Sec/A.md"));
    assert!(!session.has_unsaved_changes());
    let out = run(
        &mut session,
        "page.convert_to_markdown",
        json!({ "confirm": true }),
    );
    assert_eq!(out["converted"], false);
}

#[test]
fn mock_pages_are_not_saveable() {
    let mut session = Session::new();
    session.set_notebooks(crate::mock::notebooks());
    session.mark_page_dirty("p-grocery");
    assert_eq!(session.save_page("p-grocery"), SaveOutcome::NotDirty);
}

#[test]
fn typing_keeps_working_after_the_page_is_promoted_under_the_caret() {
    let dir = notebook(&[("Sec/A.md", "# Hi\n")]);
    let mut session = open(&dir);
    let id = current_id(&session);
    draw(&mut session, json!([[1, 2], [3, 4]]));
    run(&mut session, "draw.type", json!({}));
    run(&mut session, "container.new", json!({ "x": 300, "y": 400 }));
    assert!(session.text_cursor().is_some());
    // Auto-save promotes the page while the caret is in the new container.
    let SaveOutcome::Promoted { new_id } = session.save_page(&id) else {
        panic!("not promoted");
    };
    assert!(
        session.text_cursor().is_some(),
        "the caret moves with the page"
    );
    let pos = session.container(&new_id, "c1").pos;
    assert!(pos.is_some(), "the container keeps its place");
    run(&mut session, "text.insert", json!({ "text": "typed" }));
    assert!(session.page_text(&new_id).unwrap().contains("-->\ntyped"));
}
