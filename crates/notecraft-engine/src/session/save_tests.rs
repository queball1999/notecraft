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
fn container_positions_load_from_the_canvas_block() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let session = open(&dir);
    let id = current_id(&session);
    let c1 = session.container(&id, "c1");
    assert_eq!(c1.pos, Some(notecraft_core::Vec2::new(48.0, 96.0)));
    assert_eq!(c1.width, 420.0);
    assert_eq!(session.page_containers(&id).len(), 2);
    assert!(!session.is_page_dirty(&id));
}

#[test]
fn a_moved_and_resized_container_is_saved_and_reloads() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    run(
        &mut session,
        "container.move",
        json!({ "id": "c1", "x": 100, "y": 150 }),
    );
    assert!(session.is_page_dirty(&id), "a move marks the page dirty");
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    run(
        &mut session,
        "container.resize",
        json!({ "id": "c2", "width": 250 }),
    );
    assert!(session.is_page_dirty(&id), "a resize marks the page dirty");
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    let file = read(&dir, "Sec/A.notecraft");
    assert!(file.contains("\"c1\": { \"x\": 100, \"y\": 150, \"w\": 420 }"));
    assert!(file.contains("\"c2\": { \"x\": 520, \"y\": 96, \"w\": 250 }"));
    // Ink, shapes, z, and unknown keys are untouched.
    assert!(file.contains("\"future\": { \"keep\": [\"me\", 1, null] }"));
    assert!(file.contains("\"sh1\""));

    let again = open(&dir);
    let id = current_id(&again);
    assert_eq!(
        again.container(&id, "c1").pos,
        Some(notecraft_core::Vec2::new(100.0, 150.0))
    );
    assert_eq!(again.container(&id, "c2").width, 250.0);
    assert_eq!(again.current_ink().unwrap().len(), 2);
}

#[test]
fn undoing_a_move_marks_the_page_dirty_and_saves_the_old_place() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    run(
        &mut session,
        "container.move",
        json!({ "id": "c1", "x": 100, "y": 150 }),
    );
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    run(&mut session, "edit.undo", json!({}));
    assert!(session.is_page_dirty(&id), "undo marks the page dirty");
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(read(&dir, "Sec/A.notecraft"), BASIC);
}

#[test]
fn a_markdown_page_with_a_positioned_container_is_promoted() {
    let dir = notebook(&[("Sec/A.md", "# Hi\n")]);
    let mut session = open(&dir);
    let id = current_id(&session);
    run(&mut session, "draw.type", json!({}));
    run(&mut session, "container.new", json!({ "x": 300, "y": 400 }));
    run(&mut session, "text.insert", json!({ "text": "typed" }));
    assert!(session.is_page_dirty(&id));
    let SaveOutcome::Promoted { new_id } = session.save_page(&id) else {
        panic!("not promoted");
    };
    assert!(new_id.ends_with("Sec/A.notecraft"));
    assert!(!dir.path().join("Sec/A.md").exists());
    let file = read(&dir, "Sec/A.notecraft");
    assert!(file.contains("\"containers\": {"), "{file}");
    assert!(file.contains("\"x\": 300, \"y\": 400"));
    assert!(file.contains("typed"));

    let again = open(&dir);
    let id = current_id(&again);
    assert_eq!(
        again.container(&id, "c1").pos,
        Some(notecraft_core::Vec2::new(300.0, 400.0))
    );
}

#[test]
fn a_markdown_page_without_positions_stays_markdown() {
    let dir = notebook(&[("Sec/A.md", "# Hi\n")]);
    let mut session = open(&dir);
    let id = current_id(&session);
    run(&mut session, "text.set_caret", json!({ "at": 0 }));
    run(&mut session, "text.insert", json!({ "text": "x" }));
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert!(dir.path().join("Sec/A.md").exists());
}

#[test]
fn convert_to_markdown_forgets_the_positions() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let mut session = open(&dir);
    run(
        &mut session,
        "page.convert_to_markdown",
        json!({ "confirm": true }),
    );
    let id = current_id(&session);
    assert!(session.page_containers(&id).is_empty());
}

#[test]
fn a_broken_canvas_block_can_be_repaired() {
    let text = "Words\n\n```notecraft-canvas\n{ \"ink\": [ \n```\n";
    let dir = notebook(&[("Sec/A.notecraft", text)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    assert!(session.can_repair_canvas());
    assert!(session.current_page_notice().unwrap().read_only);
    let out = run(&mut session, "page.repair_canvas", json!({}));
    assert_eq!(out["repaired"], true);
    assert_eq!(session.current_page_notice(), None);
    assert!(session.is_page_dirty(&id));
    assert!(!session.can_repair_canvas());
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    let file = read(&dir, "Sec/A.notecraft");
    assert!(file.starts_with("Words\n\n```notecraft-canvas\n{"));
    assert!(notecraft_storage::read_canvas(&file).problem.is_none());
    // The unreadable original is still under history.
    let copy = dir
        .path()
        .join(".notecraft/history/Sec_A.notecraft/canvas-unreadable.notecraft");
    assert_eq!(std::fs::read_to_string(copy).unwrap(), text);
    // Nothing left to repair.
    let out = run(&mut session, "page.repair_canvas", json!({}));
    assert_eq!(out["repaired"], false);
}

#[test]
fn a_newer_format_page_is_not_offered_a_repair() {
    let text = "Words\n\n```notecraft-canvas\n{ \"format\": 99 }\n```\n";
    let dir = notebook(&[("Sec/A.notecraft", text)]);
    let mut session = open(&dir);
    assert!(session.current_page_notice().unwrap().read_only);
    assert!(!session.can_repair_canvas());
    let out = run(&mut session, "page.repair_canvas", json!({}));
    assert_eq!(out["repaired"], false);
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

fn picture_notebook(page: &str, text: &str) -> tempfile::TempDir {
    let dir = notebook(&[(page, text)]);
    std::fs::write(dir.path().join("pic.png"), b"png").unwrap();
    std::fs::write(dir.path().join("doc.pdf"), b"pdf").unwrap();
    dir
}

fn source(dir: &tempfile::TempDir, name: &str) -> String {
    dir.path().join(name).to_string_lossy().into_owned()
}

#[test]
fn a_picture_placed_at_a_point_promotes_a_markdown_page_and_reloads() {
    let dir = picture_notebook("Sec/A.md", "# Hi\n");
    let mut session = open(&dir);
    let id = current_id(&session);
    let out = run(
        &mut session,
        "insert.picture",
        json!({ "path": source(&dir, "pic.png"), "x": 300, "y": 220.5 }),
    );
    let link = out["link"].as_str().unwrap().to_owned();
    let objects = session.page_objects(&id);
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].src, link);
    assert_eq!(objects[0].pos, notecraft_core::Vec2::new(300.0, 220.5));
    assert!(session.is_page_dirty(&id));
    let SaveOutcome::Promoted { new_id } = session.save_page(&id) else {
        panic!("not promoted");
    };
    assert!(!dir.path().join("Sec/A.md").exists());
    let file = read(&dir, "Sec/A.notecraft");
    assert!(file.starts_with("# Hi\n\n!["), "{file}");
    assert!(file.contains("\"images\": ["), "{file}");
    assert!(file.contains("\"x\": 300, \"y\": 220.5"), "{file}");

    let again = open(&dir);
    let page = current_id(&again);
    assert_eq!(page, new_id);
    assert_eq!(again.page_objects(&page).len(), 1);
    assert_eq!(
        again.page_objects(&page)[0].pos,
        notecraft_core::Vec2::new(300.0, 220.5)
    );
}

#[test]
fn an_insert_goes_before_the_canvas_block_and_undoes_in_one_step() {
    let dir = picture_notebook("Sec/A.notecraft", BASIC);
    let mut session = open(&dir);
    let id = current_id(&session);
    run(
        &mut session,
        "insert.file_attachment",
        json!({ "path": source(&dir, "doc.pdf"), "x": 40, "y": 50 }),
    );
    let text = session.page_text(&id).unwrap().to_owned();
    let block = text.find("```notecraft-canvas").unwrap();
    let link = text.find("[doc.pdf](attachments/").unwrap();
    assert!(link < block, "the file link is before the block");
    assert!(text.trim_end().ends_with("```"));
    assert_eq!(session.page_objects(&id).len(), 1);
    // One undo takes the text and the place out together.
    assert_eq!(session.undo().as_deref(), Some("Insert file"));
    assert_eq!(session.page_text(&id).unwrap(), BASIC);
    assert!(session.page_objects(&id).is_empty());
    assert!(session.redo().is_some());
    assert_eq!(session.page_objects(&id).len(), 1);
}

#[test]
fn moving_a_picture_is_one_undo_step_and_is_saved() {
    let dir = picture_notebook("Sec/A.md", "# Hi\n");
    let mut session = open(&dir);
    let id = current_id(&session);
    // No point: the picture flows with the text and has no place of its own.
    let out = run(
        &mut session,
        "insert.picture",
        json!({ "path": source(&dir, "pic.png") }),
    );
    let link = out["link"].as_str().unwrap().to_owned();
    assert!(session.page_objects(&id).is_empty());
    // Dragging it gives it a place.
    run(
        &mut session,
        "picture.move",
        json!({ "src": link, "x": 100, "y": 120 }),
    );
    assert_eq!(session.page_objects(&id).len(), 1);
    // A second drag moves the same object (one id, one entry).
    run(
        &mut session,
        "picture.move",
        json!({ "src": link, "x": 400.4, "y": -5 }),
    );
    let objects = session.page_objects(&id).to_vec();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].id, "i1");
    assert_eq!(objects[0].pos, notecraft_core::Vec2::new(400.4, 0.0));
    // The same place again makes no undo step.
    run(
        &mut session,
        "picture.move",
        json!({ "src": link, "x": 400.4, "y": 0 }),
    );
    assert_eq!(session.undo().as_deref(), Some("Move picture"));
    assert_eq!(
        session.page_objects(&id)[0].pos,
        notecraft_core::Vec2::new(100.0, 120.0)
    );
    assert_eq!(session.undo().as_deref(), Some("Move picture"));
    assert!(session.page_objects(&id).is_empty());
    assert!(session.redo().is_some());
    assert!(session.redo().is_some());
    assert_eq!(
        session.page_objects(&id)[0].pos,
        notecraft_core::Vec2::new(400.4, 0.0)
    );
    let SaveOutcome::Promoted { .. } = session.save_page(&id) else {
        panic!("not promoted");
    };
    assert!(read(&dir, "Sec/A.notecraft").contains("\"x\": 400.4, \"y\": 0"));
}

#[test]
fn the_second_copy_of_a_picture_gets_its_own_place() {
    let dir = picture_notebook("Sec/A.md", "x\n");
    let mut session = open(&dir);
    let id = current_id(&session);
    let args = |x: i32, y: i32| json!({ "path": source(&dir, "pic.png"), "x": x, "y": y });
    run(&mut session, "insert.picture", args(10, 10));
    run(&mut session, "insert.picture", args(200, 10));
    let objects = session.page_objects(&id);
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0].src, objects[1].src);
    assert_eq!((objects[0].nth, objects[1].nth), (0, 1));
    assert_eq!(
        (objects[0].id.as_str(), objects[1].id.as_str()),
        ("i1", "i2")
    );
}

#[test]
fn deleting_the_picture_text_drops_its_place_on_save() {
    let dir = picture_notebook("Sec/A.md", "x\n");
    let mut session = open(&dir);
    let id = current_id(&session);
    run(
        &mut session,
        "insert.picture",
        json!({ "path": source(&dir, "pic.png"), "x": 10, "y": 10 }),
    );
    let SaveOutcome::Promoted { new_id } = session.save_page(&id) else {
        panic!("not promoted");
    };
    // The user deletes the picture's line in a text editor.
    let text = session.page_text(&new_id).unwrap().to_owned();
    let start = text.find("![").unwrap();
    let end = start + text[start..].find('\n').unwrap() + 1;
    session.replace_page_text(&new_id, format!("{}{}", &text[..start], &text[end..]));
    session.mark_page_dirty(&new_id);
    assert_eq!(session.save_page(&new_id), SaveOutcome::Saved);
    let file = read(&dir, "Sec/A.notecraft");
    assert!(file.contains("\"images\": []"), "{file}");
    assert!(!file.contains("pic.png"), "{file}");
}

#[test]
fn moving_a_picture_that_is_not_on_the_page_fails_and_changes_nothing() {
    let dir = picture_notebook("Sec/A.md", "x\n");
    let mut session = open(&dir);
    for args in [
        json!({ "src": "attachments/none.png", "x": 1, "y": 1 }),
        json!({ "src": "attachments/none.png", "x": 1 }),
        json!({ "src": "a.png", "x": "1", "y": 2 }),
        json!({ "x": 1, "y": 1 }),
    ] {
        let err = Registry::builtin()
            .run(&mut session, "picture.move", args.clone())
            .unwrap_err();
        assert_eq!(err.code(), "invalid_args", "{args}");
    }
    let err = Registry::builtin()
        .run(
            &mut session,
            "insert.picture",
            json!({ "path": source(&dir, "pic.png"), "x": 5 }),
        )
        .unwrap_err();
    assert_eq!(err.code(), "invalid_args");
    assert!(!session.can_undo());
}

#[test]
fn convert_to_markdown_keeps_the_picture_links_and_forgets_the_places() {
    let dir = picture_notebook("Sec/A.md", "x\n");
    let mut session = open(&dir);
    let id = current_id(&session);
    run(
        &mut session,
        "insert.file_attachment",
        json!({ "path": source(&dir, "doc.pdf"), "x": 10, "y": 10 }),
    );
    let SaveOutcome::Promoted { new_id } = session.save_page(&id) else {
        panic!("not promoted");
    };
    run(
        &mut session,
        "page.convert_to_markdown",
        json!({ "confirm": true }),
    );
    let page = current_id(&session);
    assert_ne!(page, new_id);
    assert!(session.page_objects(&page).is_empty());
    assert!(session
        .page_text(&page)
        .unwrap()
        .contains("[doc.pdf](attachments/"));
    assert!(!session
        .page_text(&page)
        .unwrap()
        .contains("notecraft-canvas"));
}

#[test]
fn placed_objects_load_from_the_canvas_block_and_save_back_unchanged() {
    let text = include_str!("../../../../testdata/notecraft/placed-objects.notecraft");
    let dir = notebook(&[("Sec/A.notecraft", text)]);
    let session = open(&dir);
    let id = current_id(&session);
    let objects = session.page_objects(&id);
    assert_eq!(objects.len(), 3);
    assert_eq!(objects[1].nth, 1);
    assert_eq!(objects[2].kind, notecraft_model::ObjectKind::File);
    // Nothing changed, so saving rewrites the file byte for byte.
    let saved = session.file_text(&id).unwrap().unwrap();
    assert_eq!(saved, text);
}

fn cursor_of(session: &Session) -> (usize, usize) {
    let c = session.text_cursor().unwrap();
    (c.anchor, c.head)
}

#[test]
fn a_picture_goes_into_the_text_at_the_caret_in_one_undo_step() {
    let dir = picture_notebook("Sec/A.md", "hello world\n");
    let mut session = open(&dir);
    let id = current_id(&session);
    run(&mut session, "text.set_caret", json!({ "at": 5 }));
    let out = run(
        &mut session,
        "insert.picture",
        json!({ "path": source(&dir, "pic.png") }),
    );
    let link = out["link"].as_str().unwrap();
    let md = format!("![pic]({link})");
    assert_eq!(
        session.page_text(&id).unwrap(),
        format!("hello{md} world\n")
    );
    assert!(session.page_objects(&id).is_empty());
    // The caret sits right after the picture.
    assert_eq!(cursor_of(&session), (5 + md.len(), 5 + md.len()));
    assert_eq!(session.undo().as_deref(), Some("Insert picture"));
    assert_eq!(session.page_text(&id).unwrap(), "hello world\n");
    assert_eq!(cursor_of(&session), (5, 5));
    assert!(session.redo().is_some());
    assert!(session.page_text(&id).unwrap().starts_with("hello!["));
}

#[test]
fn an_inserted_picture_replaces_the_selection() {
    let dir = picture_notebook("Sec/A.md", "hello world\n");
    let mut session = open(&dir);
    let id = current_id(&session);
    run(&mut session, "text.set_caret", json!({ "at": 6 }));
    run(
        &mut session,
        "text.set_caret",
        json!({ "at": 11, "extend": true }),
    );
    let out = run(
        &mut session,
        "edit.paste_picture",
        json!({ "path": source(&dir, "pic.png") }),
    );
    let link = out["link"].as_str().unwrap();
    assert_eq!(
        session.page_text(&id).unwrap(),
        format!("hello ![pic]({link})\n")
    );
    assert_eq!(session.undo().as_deref(), Some("Insert picture"));
    assert_eq!(session.page_text(&id).unwrap(), "hello world\n");
}

#[test]
fn a_file_at_the_caret_becomes_a_paragraph_of_its_own() {
    let dir = picture_notebook("Sec/A.md", "one two\n\nnext\n");
    let mut session = open(&dir);
    let id = current_id(&session);
    run(&mut session, "text.set_caret", json!({ "at": 3 }));
    run(
        &mut session,
        "insert.file_attachment",
        json!({ "path": source(&dir, "doc.pdf") }),
    );
    let text = session.page_text(&id).unwrap().to_owned();
    assert!(
        text.starts_with("one\n\n[doc.pdf](attachments/") && text.contains(")\n\n two\n\nnext\n"),
        "{text:?}"
    );
    // The link alone on its line is a file object for the canvas.
    let slots = notecraft_markdown::objects::object_slots(&notecraft_markdown::parse(&text));
    assert!(slots
        .iter()
        .any(|(k, _)| *k == notecraft_model::ObjectKind::File));
    assert_eq!(session.undo().as_deref(), Some("Insert file"));
    assert_eq!(session.page_text(&id).unwrap(), "one two\n\nnext\n");
}

#[test]
fn an_inline_insert_keeps_the_canvas_block_last_and_the_markers_intact() {
    let text = include_str!("../../../../testdata/notecraft/placed-objects.notecraft");
    let dir = picture_notebook("Sec/A.notecraft", text);
    let mut session = open(&dir);
    let id = current_id(&session);
    let at = text.find("sit where").unwrap() + 3;
    run(&mut session, "text.set_caret", json!({ "at": at }));
    run(
        &mut session,
        "insert.picture",
        json!({ "path": source(&dir, "pic.png") }),
    );
    let after = session.page_text(&id).unwrap().to_owned();
    assert!(after.starts_with("<!-- notecraft:container id=c1 -->\n"));
    let picture = after.find("sit![pic](attachments/").unwrap();
    let block = after.find("```notecraft-canvas").unwrap();
    assert!(picture < block);
    assert!(after.trim_end().ends_with("```"));
    assert_eq!(read_canvas(&after).problem, None);
    assert_eq!(session.page_objects(&id).len(), 3);
    assert_eq!(session.undo().as_deref(), Some("Insert picture"));
    assert_eq!(session.page_text(&id).unwrap(), text);
}

#[test]
fn a_point_or_no_caret_keeps_the_other_placements() {
    let dir = picture_notebook("Sec/A.md", "hello\n");
    let mut session = open(&dir);
    let id = current_id(&session);
    run(&mut session, "text.set_caret", json!({ "at": 2 }));
    // An explicit point wins over the caret: free place, text appended.
    run(
        &mut session,
        "insert.picture",
        json!({ "path": source(&dir, "pic.png"), "x": 10, "y": 20 }),
    );
    assert_eq!(session.page_objects(&id).len(), 1);
    assert!(session.page_text(&id).unwrap().starts_with("hello\n\n!["));
    // No caret and no point: appended, flowing.
    session.clear_text_cursor();
    run(
        &mut session,
        "insert.file_attachment",
        json!({ "path": source(&dir, "doc.pdf") }),
    );
    assert_eq!(session.page_objects(&id).len(), 1);
    assert!(session.page_text(&id).unwrap().ends_with(")\n"));
}
