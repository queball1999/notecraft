//! Unsaved edits follow structure edits, survive re-reads, and are never written over a file
//! that went missing without asking.

use super::*;
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

fn notebook() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("temp dir");
    for (rel, text) in [("Sec/A.md", "one\n"), ("Zoo/B.md", "two\n")] {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
    }
    dir
}

fn read(dir: &tempfile::TempDir, rel: &str) -> String {
    std::fs::read_to_string(dir.path().join(rel)).expect("read")
}

/// The page `Sec/A.md`, shown.
fn shown(session: &Session) -> String {
    let id = session.selection().page.clone().expect("a page is shown");
    assert!(id.ends_with("Sec/A.md"), "{id}");
    id
}

#[test]
fn an_edit_survives_a_rename_and_saves_to_the_new_file() {
    let dir = notebook();
    let mut session = open(&dir);
    let id = shown(&session);
    assert!(session.edit_page_text(&id, 0, 0, "mine ", false));
    let new_id = session.rename_page(&id, "Renamed").expect("rename");
    assert!(session.is_page_dirty(&new_id));
    assert!(!session.is_page_dirty(&id));
    // The watcher reports the old and the new file.
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    session.note_outside_change(&dir.path().join("Sec/Renamed.md"));
    assert!(session.is_page_dirty(&new_id));
    assert!(session
        .save_dirty()
        .iter()
        .all(|(_, o)| *o == SaveOutcome::Saved));
    assert_eq!(read(&dir, "Sec/Renamed.md"), "mine one\n");
    assert!(!dir.path().join("Sec/A.md").exists());
    assert!(!session.has_unsaved_changes());
}

#[test]
fn a_rename_queues_the_journal_cleanup_of_the_old_path() {
    let dir = notebook();
    let mut session = open(&dir);
    let id = shown(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    session.rename_page(&id, "Renamed").expect("rename");
    let clean = session.take_journal_clean();
    assert!(clean.iter().any(|p| p.ends_with("A.md")), "{clean:?}");
    let candidates = session.journal_candidates();
    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].1.ends_with("Renamed.md"));
}

#[test]
fn an_edit_survives_a_move_to_another_section() {
    let dir = notebook();
    let mut session = open(&dir);
    let id = shown(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    let other = session
        .notebooks()
        .iter()
        .flat_map(|nb| nb.all_sections())
        .find(|s| s.id.ends_with("Zoo"))
        .map(|s| s.id.clone())
        .expect("the other section");
    let new_id = session.move_page_to(&id, &other, 0).expect("move");
    assert!(new_id.ends_with("Zoo/A.md"), "{new_id}");
    assert!(session.is_page_dirty(&new_id));
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    session.save_dirty();
    assert_eq!(read(&dir, "Zoo/A.md"), "mine one\n");
    assert!(!session.has_unsaved_changes());
}

#[test]
fn an_edit_survives_delete_and_undo() {
    let dir = notebook();
    let mut session = open(&dir);
    let id = shown(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    session.delete_page(&id).expect("delete");
    session.undo();
    assert!(session.is_page_dirty(&id), "the id is back and dirty");
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(read(&dir, "Sec/A.md"), "mine one\n");
}

#[test]
fn reopening_a_notebook_keeps_the_text_of_a_page_that_did_not_save() {
    let dir = notebook();
    let mut session = open(&dir);
    let id = shown(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    std::fs::write(dir.path().join("Sec/A.md"), "theirs\n").expect("outside edit");
    let nb = session.selection().notebook.clone().expect("nb");
    session.set_notebook_root(&nb, dir.path());
    assert!(session.is_page_dirty(&id), "still unsaved");
    assert_eq!(session.page_text(&id), Some("mine one\n"));
    assert_eq!(read(&dir, "Sec/A.md"), "theirs\n");
    assert_eq!(session.save_page(&id), SaveOutcome::Conflict);
}

#[test]
fn a_page_deleted_outside_gets_a_notice_and_is_not_recreated() {
    let dir = notebook();
    let mut session = open(&dir);
    let id = shown(&session);
    std::fs::remove_file(dir.path().join("Sec/A.md")).expect("remove");
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    let notice = session.current_page_notice().expect("a notice");
    assert!(notice.text.contains("moved or deleted"), "{}", notice.text);
    // Editing on, then saving, asks first.
    session.edit_page_text(&id, 0, 0, "mine ", false);
    assert_eq!(session.save_page(&id), SaveOutcome::Conflict);
    assert!(!dir.path().join("Sec/A.md").exists());
    assert!(session.current_conflict().is_some());
    assert!(session.resolve_current_conflict(true));
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(read(&dir, "Sec/A.md"), "mine one\n");
}

#[test]
fn discarding_the_edits_of_a_missing_page_leaves_no_file() {
    let dir = notebook();
    let mut session = open(&dir);
    let id = shown(&session);
    session.edit_page_text(&id, 0, 0, "mine ", false);
    std::fs::remove_file(dir.path().join("Sec/A.md")).expect("remove");
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    assert!(
        session.current_conflict().is_some(),
        "dirty and gone asks at once"
    );
    assert!(session.resolve_current_conflict(false));
    assert!(!session.is_page_dirty(&id));
    assert!(!dir.path().join("Sec/A.md").exists());
}

#[test]
fn a_late_read_of_an_older_own_write_does_not_revert_the_page() {
    let dir = notebook();
    let mut session = open(&dir);
    session.set_background_reads(true);
    let id = shown(&session);
    session.edit_page_text(&id, 0, 0, "A ", false);
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    session.note_outside_change(&dir.path().join("Sec/A.md"));
    let requests = session.take_read_requests();
    assert_eq!(requests.len(), 1);
    // The read of save A is out; auto-save writes B before it comes back.
    let stale = notecraft_storage::read_page_file(&requests[0].path);
    session.edit_page_text(&id, 0, 0, "B ", false);
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    session.finish_read(&id, ReadPurpose::Outside, stale);
    assert_eq!(session.page_text(&id), Some("B A one\n"));
    assert!(!session.is_page_dirty(&id));
    assert_eq!(session.current_conflict(), None);
}
