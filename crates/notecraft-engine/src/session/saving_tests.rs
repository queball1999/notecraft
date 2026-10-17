use std::path::Path;

use serde_json::{json, Value};

use super::*;
use crate::cmd::Registry;
use crate::open::open_folder;

fn open(dir: &tempfile::TempDir) -> Session {
    let mut session = Session::new();
    let opened = open_folder(dir.path()).expect("scan");
    let id = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&id, dir.path());
    session.load_current_page();
    session
}

fn notebook(text: &str) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let path = dir.path().join("Sec/A.md");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
    dir
}

fn page(session: &Session) -> String {
    session.selection().page.clone().expect("a page is shown")
}

fn file(dir: &tempfile::TempDir) -> String {
    std::fs::read_to_string(dir.path().join("Sec/A.md")).expect("read")
}

fn run(session: &mut Session, id: &str) -> Result<Value, &'static str> {
    Registry::builtin()
        .run(session, id, Value::Null)
        .map(|o| o.value)
        .map_err(|e| e.code())
}

#[test]
fn text_edits_undo_and_redo_mark_the_page_dirty_and_save_writes_them() {
    let dir = notebook("hello\n");
    let mut session = open(&dir);
    let id = page(&session);
    assert!(!session.has_unsaved_changes());
    assert!(session.edit_page_text(&id, 5, 0, " world", false));
    assert!(session.is_page_dirty(&id));
    assert_eq!(session.save_state().unsaved, 1);
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(file(&dir), "hello world\n");
    assert!(!session.is_page_dirty(&id));
    assert_eq!(session.save_page(&id), SaveOutcome::NotDirty);

    session.undo();
    assert!(session.is_page_dirty(&id), "undo is an edit");
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(file(&dir), "hello\n");
    session.redo();
    assert!(session.is_page_dirty(&id), "redo is an edit");
}

#[test]
fn a_stroke_marks_the_page_dirty_and_saving_it_promotes_the_page() {
    let dir = notebook("x\n");
    let mut session = open(&dir);
    let id = page(&session);
    let args = json!({ "points": [[0.0, 0.0], [10.0, 10.0]] });
    Registry::builtin()
        .run(&mut session, "ink.add_stroke", args)
        .expect("stroke");
    assert!(session.is_page_dirty(&id));
    // A stroke on a markdown page is saved with the page, which becomes a `.notecraft` page.
    let nb = id.split('/').next().unwrap_or_default().to_owned();
    assert_eq!(
        session.save_page(&id),
        SaveOutcome::Promoted {
            new_id: format!("{nb}/Sec/A.notecraft")
        }
    );
    assert!(!session.is_page_dirty(&id));
    assert!(!dir.path().join("Sec/A.md").exists());
}

#[test]
fn a_read_only_page_is_never_written() {
    let dir = notebook("x\n");
    let mut session = open(&dir);
    let id = page(&session);
    session.page_notices.insert(
        id.clone(),
        PageNotice {
            text: "cut short".to_owned(),
            read_only: true,
        },
    );
    session.edit_page_text(&id, 0, 0, "y", false);
    assert!(session.journal_candidates().is_empty());
    assert_eq!(session.save_page(&id), SaveOutcome::ReadOnly);
    assert_eq!(file(&dir), "x\n");
}

#[test]
fn pages_without_a_file_are_never_dirty() {
    let mut session = Session::new();
    session.set_notebooks(crate::mock::notebooks());
    session.set_page_source(crate::mock::WELCOME_ID, "hi");
    session.edit_page_text(crate::mock::WELCOME_ID, 0, 0, "x", false);
    assert!(!session.has_unsaved_changes());
}

#[test]
fn a_save_does_not_overwrite_a_file_that_changed_underneath() {
    let dir = notebook("one\n");
    let mut session = open(&dir);
    let id = page(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    std::fs::write(dir.path().join("Sec/A.md"), "theirs\n").expect("outside edit");
    assert_eq!(session.save_page(&id), SaveOutcome::Conflict);
    assert_eq!(file(&dir), "theirs\n");
    assert_eq!(session.current_conflict(), Some("theirs\n"));
    // Still blocked while the conflict is open.
    assert_eq!(session.save_page(&id), SaveOutcome::Conflict);

    run(&mut session, "file.keep_my_version").expect("keep");
    assert_eq!(session.current_conflict(), None);
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(file(&dir), "mine one\n");
}

#[test]
fn taking_the_outside_version_replaces_the_text_and_drops_undo_steps() {
    let dir = notebook("one\n");
    let mut session = open(&dir);
    let id = page(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    std::fs::write(dir.path().join("Sec/A.md"), "theirs\n").expect("outside edit");
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    assert_eq!(session.current_conflict(), Some("theirs\n"));
    assert_eq!(session.page_text(&id), Some("mine one\n"), "user text kept");
    assert!(session.is_page_dirty(&id));

    run(&mut session, "file.use_outside_version").expect("take");
    assert_eq!(session.page_text(&id), Some("theirs\n"));
    assert!(!session.is_page_dirty(&id));
    assert!(!session.can_undo());
    assert_eq!(
        run(&mut session, "file.keep_my_version"),
        Err("invalid_args")
    );
}

#[test]
fn an_outside_edit_to_a_clean_page_reloads_it_and_drops_stale_undo_steps() {
    let dir = notebook("one\n");
    let mut session = open(&dir);
    let id = page(&session);
    session.edit_page_text(&id, 0, 0, "x", false);
    session.save_page(&id);
    assert!(session.can_undo());
    let rev = session.current_page_source().expect("text").revision;

    // Our own write coming back from the watcher changes nothing.
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    assert_eq!(session.current_page_source().expect("text").revision, rev);
    assert!(session.can_undo());

    std::fs::write(dir.path().join("Sec/A.md"), "new\n").expect("outside edit");
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    assert_eq!(session.page_text(&id), Some("new\n"));
    assert!(!session.can_undo(), "stale steps are dropped");
    assert!(!session.is_page_dirty(&id));
    assert_eq!(session.current_conflict(), None);
}

#[test]
fn an_outside_edit_equal_to_the_unsaved_text_just_adopts_it() {
    let dir = notebook("one\n");
    let mut session = open(&dir);
    let id = page(&session);
    session.edit_page_text(&id, 0, 0, "x", false);
    std::fs::write(dir.path().join("Sec/A.md"), "xone\n").expect("outside edit");
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    assert!(!session.is_page_dirty(&id));
    assert_eq!(session.current_conflict(), None);
}

#[test]
fn background_reads_are_queued_and_answered() {
    let dir = notebook("one\n");
    let mut session = Session::new();
    session.set_background_reads(true);
    let opened = open_folder(dir.path()).expect("scan");
    let nb = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&nb, dir.path());
    assert!(session.load_current_page());
    assert!(session.is_loading_current_page());
    assert_eq!(session.current_page_source(), None);
    assert!(!session.load_current_page(), "queued once");
    let requests = session.take_read_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].purpose, ReadPurpose::Load);
    let result = notecraft_storage::read_page_file(&requests[0].path);
    session.finish_read(&requests[0].page_id, ReadPurpose::Load, result);
    assert!(!session.is_loading_current_page());
    assert_eq!(session.current_page_source().expect("text").text, "one\n");
}

#[test]
fn recovery_is_offered_only_when_it_differs_and_can_be_restored_or_discarded() {
    let dir = notebook("on disk\n");
    let mut session = open(&dir);
    let id = page(&session);
    let path = dir.path().join("Sec/A.md");
    let same = notecraft_storage::RecoveryEntry {
        path: path.clone(),
        text: "on disk\n".to_owned(),
    };
    assert_eq!(session.offer_recovery(vec![same]), vec![path.clone()]);
    assert!(session.pending_recovery().is_empty());

    let newer = notecraft_storage::RecoveryEntry {
        path: path.clone(),
        text: "unsaved\n".to_owned(),
    };
    let elsewhere = notecraft_storage::RecoveryEntry {
        path: Path::new("/not/open/B.md").to_path_buf(),
        text: "x".to_owned(),
    };
    assert!(session
        .offer_recovery(vec![newer.clone(), elsewhere])
        .is_empty());
    assert_eq!(session.pending_recovery().len(), 1);

    let out = run(&mut session, "file.restore_unsaved").expect("restore");
    assert_eq!(out["restored"], 1);
    assert_eq!(session.page_text(&id), Some("unsaved\n"));
    assert!(session.is_page_dirty(&id));
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(file(&dir), "unsaved\n");

    session.offer_recovery(vec![notecraft_storage::RecoveryEntry {
        text: "again\n".to_owned(),
        ..newer
    }]);
    let out = run(&mut session, "file.discard_unsaved").expect("discard");
    assert_eq!(out["discarded"], 1);
    assert!(session.pending_recovery().is_empty());
    assert!(session.take_journal_clean().contains(&path));
    assert_eq!(file(&dir), "unsaved\n");
}
