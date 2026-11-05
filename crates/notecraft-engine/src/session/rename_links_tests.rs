//! Links through renames and moves: one undo step restores names and links.

use super::*;
use crate::open::open_folder;
use crate::undo::{Scope, Transaction};

fn write(dir: &tempfile::TempDir, rel: &str, text: &str) {
    let path = dir.path().join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

fn read(dir: &tempfile::TempDir, rel: &str) -> String {
    std::fs::read_to_string(dir.path().join(rel)).expect("read")
}

fn open(dir: &tempfile::TempDir) -> (Session, String) {
    let mut session = Session::new();
    let opened = open_folder(dir.path()).expect("scan");
    let id = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&id, dir.path());
    (session, id)
}

fn notebook() -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("temp dir");
    write(&dir, "Work/Plan.md", "# Plan\n");
    write(
        &dir,
        "Home/Notes.md",
        "see [[Plan]] and [[Plan|the plan]] and [[Work/Plan]]\n\n`[[Plan]]`\n",
    );
    write(&dir, "Work/Other.md", "back to [[Plan]]\nno links here\n");
    dir
}

#[test]
fn a_rename_rewrites_links_across_sections_and_one_undo_restores_everything() {
    let dir = notebook();
    let (mut s, nb) = open(&dir);
    let before_home = read(&dir, "Home/Notes.md");
    let before_work = read(&dir, "Work/Other.md");
    let new = s
        .rename_page(&format!("{nb}/Work/Plan.md"), "Roadmap")
        .expect("rename");
    assert_eq!(new, format!("{nb}/Work/Roadmap.md"));
    assert_eq!(
        read(&dir, "Home/Notes.md"),
        "see [[Roadmap]] and [[Roadmap|the plan]] and [[Roadmap|Work/Plan]]\n\n`[[Plan]]`\n"
    );
    assert_eq!(
        read(&dir, "Work/Other.md"),
        "back to [[Roadmap]]\nno links here\n"
    );
    let report = s.take_link_update_report().expect("report");
    assert_eq!((report.links, report.pages), (4, 2));
    assert_eq!(s.undo().as_deref(), Some("Rename page"));
    assert_eq!(read(&dir, "Home/Notes.md"), before_home);
    assert_eq!(read(&dir, "Work/Other.md"), before_work);
    assert!(dir.path().join("Work/Plan.md").is_file());
    assert_eq!(s.redo().as_deref(), Some("Rename page"));
    assert_eq!(
        read(&dir, "Work/Other.md"),
        "back to [[Roadmap]]\nno links here\n"
    );
    assert!(dir.path().join("Work/Roadmap.md").is_file());
}

#[test]
fn moving_a_page_into_a_section_with_the_same_name_keeps_links_on_it() {
    let dir = notebook();
    write(&dir, "Archive/Plan.md", "an older plan");
    let (mut s, nb) = open(&dir);
    let before = read(&dir, "Home/Notes.md");
    s.move_page_to(&format!("{nb}/Work/Plan.md"), &format!("{nb}/Archive"), 99)
        .expect("move");
    // The moved page is called "Plan 2" there, so the links name it.
    assert_eq!(
        read(&dir, "Home/Notes.md"),
        "see [[Plan 2]] and [[Plan 2|the plan]] and [[Plan 2|Work/Plan]]\n\n`[[Plan]]`\n"
    );
    assert_eq!(s.undo().as_deref(), Some("Move page"));
    assert_eq!(read(&dir, "Home/Notes.md"), before);
}

#[test]
fn a_section_rename_rewrites_path_links_only() {
    let dir = notebook();
    let (mut s, nb) = open(&dir);
    s.rename_section(&format!("{nb}/Work"), "Projects")
        .expect("rename");
    assert_eq!(
        read(&dir, "Home/Notes.md"),
        "see [[Plan]] and [[Plan|the plan]] and [[Plan|Work/Plan]]\n\n`[[Plan]]`\n"
    );
    s.undo();
    assert!(read(&dir, "Home/Notes.md").contains("[[Work/Plan]]"));
    assert!(dir.path().join("Work/Plan.md").is_file());
}

#[test]
fn open_unsaved_pages_are_edited_in_memory_and_keep_their_text() {
    let dir = notebook();
    let (mut s, nb) = open(&dir);
    let home = format!("{nb}/Home/Notes.md");
    s.set_page_source(&home, "mine [[Plan]]\n");
    assert!(s.edit_page_text(&home, 0, 0, "typed ", false));
    let before = read(&dir, "Home/Notes.md");
    s.rename_page(&format!("{nb}/Work/Plan.md"), "Roadmap")
        .expect("rename");
    assert_eq!(s.page_text(&home), Some("typed mine [[Roadmap]]\n"));
    assert!(s.is_page_dirty(&home));
    // The file on disk is not touched until the page saves.
    assert_eq!(read(&dir, "Home/Notes.md"), before);
    s.undo();
    assert_eq!(s.page_text(&home), Some("typed mine [[Plan]]\n"));
}

#[test]
fn read_only_pages_are_skipped_and_reported() {
    let dir = notebook();
    let bytes = b"caf\xe9 [[Plan]]\n".to_vec();
    std::fs::write(dir.path().join("Home/Legacy.md"), &bytes).expect("write");
    let (mut s, nb) = open(&dir);
    s.rename_page(&format!("{nb}/Work/Plan.md"), "Roadmap")
        .expect("rename");
    assert_eq!(
        std::fs::read(dir.path().join("Home/Legacy.md")).expect("read"),
        bytes
    );
    let report = s.take_link_update_report().expect("report");
    assert_eq!(report.skipped.len(), 1);
    assert!(report
        .message()
        .expect("message")
        .contains("could not be changed"));
    assert_eq!(report.links, 4);
}

#[test]
fn the_setting_turns_updates_off() {
    let dir = notebook();
    let (mut s, nb) = open(&dir);
    assert!(s.link_update_on_rename());
    s.set_link_update_on_rename(false);
    let before = read(&dir, "Home/Notes.md");
    s.rename_page(&format!("{nb}/Work/Plan.md"), "Roadmap")
        .expect("rename");
    assert_eq!(read(&dir, "Home/Notes.md"), before);
    assert!(s.take_link_update_report().is_none());
}

#[test]
fn ambiguity_created_by_a_rename_keeps_links_on_their_page() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    write(&dir, "A/Home.md", "[[Target]]\n");
    write(&dir, "A/Target.md", "t");
    write(&dir, "B/Other.md", "o");
    let (mut s, nb) = open(&dir);
    // Renaming the other page to Target makes the name ambiguous; the closest (same section)
    // still wins, so nothing changes.
    s.rename_page(&format!("{nb}/B/Other.md"), "Target")
        .expect("rename");
    assert_eq!(read(&dir, "A/Home.md"), "[[Target]]\n");
    // Moving the first Target away leaves the plain name to the page in B; the link keeps
    // its page by naming it.
    s.move_page_to(&format!("{nb}/A/Target.md"), &format!("{nb}/B"), 9)
        .expect("move");
    assert_eq!(read(&dir, "A/Home.md"), "[[Target 2]]\n");
}

#[test]
fn a_file_that_changed_after_planning_is_never_written_over() {
    let dir = notebook();
    let (mut s, nb) = open(&dir);
    let fix = s
        .link_fix(
            &nb,
            &[("Work/Plan.md".to_owned(), "Work/Roadmap.md".to_owned())],
        )
        .expect("fix");
    let changed = "changed outside [[Plan]] plus more text\n";
    write(&dir, "Home/Notes.md", changed);
    let tx = fix
        .ops
        .into_iter()
        .fold(Transaction::new(Scope::Notebook, "x"), Transaction::with);
    assert!(!s.commit(tx));
    assert_eq!(read(&dir, "Home/Notes.md"), changed);
}

/// Move `from` to `to` on disk and read the notebook again, as the refresh does after the
/// watcher reported it.
fn outside_rename(dir: &tempfile::TempDir, s: &mut Session, from: &str, to: &str) {
    std::fs::rename(dir.path().join(from), dir.path().join(to)).expect("rename");
    let opened = open_folder(dir.path()).expect("scan");
    s.open_notebook(opened.notebook);
}

#[test]
fn an_outside_rename_offers_to_fix_links_and_one_undo_takes_it_back() {
    let dir = notebook();
    let (mut s, nb) = open(&dir);
    let before = read(&dir, "Home/Notes.md");
    outside_rename(&dir, &mut s, "Work/Plan.md", "Work/Roadmap.md");
    let moves = vec![("Work/Plan.md".to_owned(), "Work/Roadmap.md".to_owned())];
    s.consider_outside_renames(&nb, moves.clone());
    assert_eq!(s.link_fix_offer().map(|o| o.links), Some(4));
    // Not now closes it and changes nothing.
    assert!(s.dismiss_link_fix());
    assert_eq!(read(&dir, "Home/Notes.md"), before);
    s.consider_outside_renames(&nb, moves);
    let report = s.fix_links_after_rename().expect("report");
    assert_eq!((report.links, report.pages), (4, 2));
    assert!(s.link_fix_offer().is_none());
    assert_eq!(
        read(&dir, "Home/Notes.md"),
        "see [[Roadmap]] and [[Roadmap|the plan]] and [[Roadmap|Work/Plan]]\n\n`[[Plan]]`\n"
    );
    assert_eq!(s.undo().as_deref(), Some("Update links"));
    assert_eq!(read(&dir, "Home/Notes.md"), before);
}

#[test]
fn nothing_is_offered_when_no_link_breaks_or_the_links_are_fixed() {
    let dir = notebook();
    write(&dir, "Work/Lonely.md", "alone");
    let (mut s, nb) = open(&dir);
    // No moves (pages without ids): nothing.
    s.consider_outside_renames(&nb, Vec::new());
    assert!(s.link_fix_offer().is_none());
    // A page nothing links to.
    outside_rename(&dir, &mut s, "Work/Lonely.md", "Work/Alone.md");
    s.consider_outside_renames(
        &nb,
        vec![("Work/Lonely.md".to_owned(), "Work/Alone.md".to_owned())],
    );
    assert!(s.link_fix_offer().is_none());
    // A rename NoteCraft made itself already fixed its links.
    s.rename_page(&format!("{nb}/Work/Plan.md"), "Roadmap")
        .expect("rename");
    s.consider_outside_renames(
        &nb,
        vec![("Work/Plan.md".to_owned(), "Work/Roadmap.md".to_owned())],
    );
    assert!(s.link_fix_offer().is_none());
}

#[test]
fn with_the_setting_off_an_in_app_rename_leaves_an_offer() {
    let dir = notebook();
    let (mut s, nb) = open(&dir);
    s.set_link_update_on_rename(false);
    s.rename_page(&format!("{nb}/Work/Plan.md"), "Roadmap")
        .expect("rename");
    s.consider_outside_renames(
        &nb,
        vec![("Work/Plan.md".to_owned(), "Work/Roadmap.md".to_owned())],
    );
    assert_eq!(s.link_fix_offer().map(|o| o.links), Some(4));
}
