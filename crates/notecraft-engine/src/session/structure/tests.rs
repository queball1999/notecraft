use std::fs;
use std::path::Path;

use notecraft_storage::index::Index;
use notecraft_storage::scan_notebook;

use super::*;
use crate::open::open_folder;

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A notebook with sections Work (A, B, C) and Home (D) and a group Projects (Alpha).
fn open(files: &[(&str, &str)]) -> (tempfile::TempDir, Session, String) {
    let dir = tempfile::TempDir::new().unwrap();
    for (rel, text) in files {
        write(dir.path(), rel, text);
    }
    let mut session = Session::new();
    let opened = open_folder(dir.path()).unwrap();
    let id = opened.notebook.id.clone();
    session.open_notebook(opened.notebook);
    session.set_notebook_root(&id, dir.path());
    (dir, session, id)
}

fn sample() -> (tempfile::TempDir, Session, String) {
    open(&[
        ("Work/A.md", "alpha"),
        ("Work/B.md", "beta"),
        ("Work/C.md", "gamma"),
        ("Home/D.md", "delta"),
        ("Projects/.section.toml", "kind = \"group\"\n"),
        ("Projects/Alpha/E.md", "echo"),
    ])
}

fn pages(session: &Session, section: &str) -> Vec<String> {
    session
        .current_notebook()
        .unwrap()
        .section(section)
        .map(|s| s.pages.iter().map(|p| p.title.clone()).collect())
        .unwrap_or_default()
}

fn top_names(session: &Session) -> Vec<String> {
    session
        .current_notebook()
        .unwrap()
        .sections
        .iter()
        .map(|n| match n {
            SectionNode::Section(s) => s.name.clone(),
            SectionNode::Group(g) => g.name.clone(),
        })
        .collect()
}

/// The tree in memory equals a fresh read of the folder, and the index agrees.
fn assert_matches_disk(dir: &Path, session: &Session) {
    let fresh = open_folder(dir).unwrap().notebook;
    assert_eq!(session.current_notebook().unwrap(), &fresh);
    let mut index = Index::open_in_memory().unwrap();
    index.refresh(dir).unwrap();
    let pages: Vec<String> = index
        .sidebar()
        .unwrap()
        .pages
        .iter()
        .map(|p| p.id.clone())
        .collect();
    let model: Vec<String> = fresh
        .all_sections()
        .iter()
        .flat_map(|s| s.pages.iter().map(|p| p.id.clone()))
        .collect();
    assert_eq!(pages, model);
}

#[test]
fn a_new_page_is_a_file_listed_last_and_undoes_and_redoes() {
    let (dir, mut s, nb) = sample();
    let section = format!("{nb}/Work");
    let id = s.create_page(&section, None).unwrap();
    assert_eq!(id, format!("{nb}/Work/Untitled Page.md"));
    assert!(dir.path().join("Work/Untitled Page.md").is_file());
    assert_eq!(pages(&s, &section), ["A", "B", "C", "Untitled Page"]);
    assert_eq!(s.selection().page.as_deref(), Some(id.as_str()));
    assert_matches_disk(dir.path(), &s);
    // A second one is numbered, never overwriting.
    let second = s.create_page(&section, Some("Untitled Page")).unwrap();
    assert!(second.ends_with("Untitled Page 2.md"));
    assert_eq!(s.undo().as_deref(), Some("New page"));
    assert_eq!(s.undo().as_deref(), Some("New page"));
    assert_eq!(pages(&s, &section), ["A", "B", "C"]);
    assert!(!dir.path().join("Work/Untitled Page.md").exists());
    assert_matches_disk(dir.path(), &s);
    assert_eq!(s.redo().as_deref(), Some("New page"));
    assert_eq!(pages(&s, &section), ["A", "B", "C", "Untitled Page"]);
    assert_matches_disk(dir.path(), &s);
}

#[test]
fn page_titles_are_made_safe_for_every_system() {
    let (dir, mut s, nb) = sample();
    let section = format!("{nb}/Work");
    let id = s.create_page(&section, Some("Plan: v2?")).unwrap();
    assert!(id.ends_with("Plan- v2-.md"), "{id}");
    assert!(dir.path().join("Work/Plan- v2-.md").is_file());
    let con = s.create_page(&section, Some("CON")).unwrap();
    assert!(con.ends_with("CON_.md"), "{con}");
}

#[test]
fn renaming_a_page_renames_its_file_and_keeps_what_belongs_to_it() {
    let (dir, mut s, nb) = sample();
    let old = format!("{nb}/Work/A.md");
    s.select_page_id(&old);
    assert!(s.load_current_page());
    s.edit_page_text(&old, 0, 0, "x", false);
    let new = s.rename_page(&old, "Agenda").unwrap();
    assert_eq!(new, format!("{nb}/Work/Agenda.md"));
    assert!(dir.path().join("Work/Agenda.md").is_file());
    assert!(!dir.path().join("Work/A.md").exists());
    assert_eq!(s.selection().page.as_deref(), Some(new.as_str()));
    assert_eq!(s.page_text(&new), Some("xalpha"));
    assert_eq!(s.page_text(&old), None);
    assert_matches_disk(dir.path(), &s);
    // Undo the rename, then the earlier text edit still undoes (history followed the id).
    assert_eq!(s.undo().as_deref(), Some("Rename page"));
    assert_eq!(s.selection().page.as_deref(), Some(old.as_str()));
    assert_eq!(s.page_text(&old), Some("xalpha"));
    assert_eq!(s.undo().as_deref(), Some("Edit text"));
    assert_eq!(s.page_text(&old), Some("alpha"));
    assert_matches_disk(dir.path(), &s);
}

#[test]
fn a_rename_never_replaces_another_page() {
    let (dir, mut s, nb) = sample();
    let b = format!("{nb}/Work/B.md");
    let new = s.rename_page(&b, "a").unwrap();
    assert!(new.ends_with("a 2.md"), "{new}");
    assert_eq!(
        fs::read_to_string(dir.path().join("Work/A.md")).unwrap(),
        "alpha"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("Work/a 2.md")).unwrap(),
        "beta"
    );
    // A case-only change is allowed.
    let c = format!("{nb}/Work/C.md");
    let new = s.rename_page(&c, "c").unwrap();
    assert!(new.ends_with("/c.md"), "{new}");
}

#[test]
fn deleting_a_page_moves_it_to_the_trash_and_undo_brings_it_back() {
    let (dir, mut s, nb) = sample();
    let section = format!("{nb}/Work");
    let b = format!("{nb}/Work/B.md");
    s.select_page_id(&b);
    s.delete_page(&b).unwrap();
    assert_eq!(pages(&s, &section), ["A", "C"]);
    assert!(!dir.path().join("Work/B.md").exists());
    assert_eq!(
        s.selection().page.as_deref(),
        Some(format!("{nb}/Work/C.md").as_str())
    );
    let trash = dir.path().join(".notecraft/trash");
    let kept: Vec<_> = fs::read_dir(&trash).unwrap().collect();
    assert_eq!(kept.len(), 1);
    let slot = kept[0].as_ref().unwrap().path();
    assert_eq!(fs::read_to_string(slot.join("B.md")).unwrap(), "beta");
    let note = fs::read_to_string(slot.join("origin.toml")).unwrap();
    assert!(note.contains("Work/B.md"), "{note}");
    assert_matches_disk(dir.path(), &s);
    assert_eq!(s.undo().as_deref(), Some("Delete page"));
    assert_eq!(pages(&s, &section), ["A", "B", "C"]);
    assert_eq!(
        fs::read_to_string(dir.path().join("Work/B.md")).unwrap(),
        "beta"
    );
    assert_eq!(
        fs::read_dir(&trash).unwrap().count(),
        0,
        "the empty slot is removed"
    );
    assert_matches_disk(dir.path(), &s);
    assert_eq!(s.redo().as_deref(), Some("Delete page"));
    assert_eq!(pages(&s, &section), ["A", "C"]);
}

#[test]
fn reordering_pages_writes_the_order_file() {
    let (dir, mut s, nb) = sample();
    let section = format!("{nb}/Work");
    s.move_page_to(&format!("{nb}/Work/C.md"), &section, 0)
        .unwrap();
    assert_eq!(pages(&s, &section), ["C", "A", "B"]);
    let toml = fs::read_to_string(dir.path().join("Work/.section.toml")).unwrap();
    assert!(
        toml.find("C.md").unwrap() < toml.find("A.md").unwrap(),
        "{toml}"
    );
    assert_matches_disk(dir.path(), &s);
    s.undo();
    assert_eq!(pages(&s, &section), ["A", "B", "C"]);
    assert_matches_disk(dir.path(), &s);
}

#[test]
fn moving_a_page_to_another_section_moves_the_file_and_copies_attachments() {
    let (dir, mut s, nb) = open(&[
        (
            "Work/A.md",
            "see ![x](attachments/3f9a2c-pic.png) and [f](attachments/b1-my%20file.pdf)",
        ),
        ("Work/B.md", "![x](attachments/3f9a2c-pic.png)"),
        ("Work/attachments/3f9a2c-pic.png", "PNG"),
        ("Work/attachments/b1-my file.pdf", "PDF"),
        ("Home/D.md", "d"),
    ]);
    let a = format!("{nb}/Work/A.md");
    s.select_page_id(&a);
    let new = s.move_page_to(&a, &format!("{nb}/Home"), 0).unwrap();
    assert_eq!(new, format!("{nb}/Home/A.md"));
    assert!(dir.path().join("Home/A.md").is_file());
    assert!(!dir.path().join("Work/A.md").exists());
    assert_eq!(
        fs::read_to_string(dir.path().join("Home/attachments/3f9a2c-pic.png")).unwrap(),
        "PNG"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("Home/attachments/b1-my file.pdf")).unwrap(),
        "PDF"
    );
    // Page B still uses the original.
    assert!(dir.path().join("Work/attachments/3f9a2c-pic.png").is_file());
    assert_eq!(pages(&s, &format!("{nb}/Home")), ["A", "D"]);
    assert_eq!(s.selection().page.as_deref(), Some(new.as_str()));
    assert_matches_disk(dir.path(), &s);
    s.undo();
    assert!(dir.path().join("Work/A.md").is_file());
    assert!(!dir.path().join("Home/A.md").exists());
    assert!(!dir.path().join("Home/attachments/3f9a2c-pic.png").exists());
    assert_matches_disk(dir.path(), &s);
    s.redo();
    assert!(dir.path().join("Home/attachments/3f9a2c-pic.png").is_file());
    assert_matches_disk(dir.path(), &s);
}

#[test]
fn a_page_moved_where_the_name_is_taken_gets_a_number() {
    let (dir, mut s, nb) = open(&[("Work/Note.md", "w"), ("Home/Note.md", "h")]);
    let new = s
        .move_page_to(&format!("{nb}/Work/Note.md"), &format!("{nb}/Home"), 5)
        .unwrap();
    assert!(new.ends_with("Home/Note 2.md"), "{new}");
    assert_eq!(
        fs::read_to_string(dir.path().join("Home/Note.md")).unwrap(),
        "h"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("Home/Note 2.md")).unwrap(),
        "w"
    );
}

#[test]
fn page_levels_are_limited_and_undoable() {
    let (dir, mut s, nb) = sample();
    let section = format!("{nb}/Work");
    let (a, b, c) = (
        format!("{nb}/Work/A.md"),
        format!("{nb}/Work/B.md"),
        format!("{nb}/Work/C.md"),
    );
    assert_eq!(
        s.set_page_level(&a, 1).unwrap(),
        0,
        "the first page stays a page"
    );
    assert_eq!(
        s.set_page_level(&b, 2).unwrap(),
        1,
        "one level below the page before"
    );
    assert_eq!(s.set_page_level(&c, 2).unwrap(), 2);
    let levels = |s: &Session| -> Vec<u8> {
        s.current_notebook()
            .unwrap()
            .section(&section)
            .unwrap()
            .pages
            .iter()
            .map(|p| p.level)
            .collect()
    };
    assert_eq!(levels(&s), [0, 1, 2]);
    // Promoting the middle page pulls the one below it up with it.
    s.set_page_level(&b, 0).unwrap();
    assert_eq!(levels(&s), [0, 0, 1]);
    assert_matches_disk(dir.path(), &s);
    s.undo();
    assert_eq!(levels(&s), [0, 1, 2]);
}

#[test]
fn sections_and_groups_are_created_renamed_colored_and_deleted() {
    let (dir, mut s, nb) = sample();
    let new = s.create_section(&nb, None, Some("Ideas")).unwrap();
    assert_eq!(new, format!("{nb}/Ideas"));
    assert!(dir.path().join("Ideas/.section.toml").is_file());
    assert_eq!(top_names(&s), ["Home", "Projects", "Work", "Ideas"]);
    assert_eq!(s.selection().section.as_deref(), Some(new.as_str()));
    assert_matches_disk(dir.path(), &s);

    let group = s.create_section_group(&nb, None, Some("Archive")).unwrap();
    let inner = s.create_section(&nb, Some(&group), None).unwrap();
    assert_eq!(inner, format!("{nb}/Archive/New Section"));
    assert_matches_disk(dir.path(), &s);

    s.select_section_id(&new);
    let renamed = s.rename_section(&new, "Brainstorm").unwrap();
    assert_eq!(renamed, format!("{nb}/Brainstorm"));
    assert!(dir.path().join("Brainstorm").is_dir() && !dir.path().join("Ideas").exists());
    assert_eq!(s.selection().section.as_deref(), Some(renamed.as_str()));
    // A taken name is numbered, never merged.
    let other = s.rename_section(&renamed, "work").unwrap();
    assert_eq!(other, format!("{nb}/work 2"));
    assert_matches_disk(dir.path(), &s);

    s.set_section_color(&other, 0x112233).unwrap();
    assert_eq!(
        s.current_notebook().unwrap().section(&other).unwrap().color,
        0x112233
    );
    assert_matches_disk(dir.path(), &s);

    s.delete_section(&other).unwrap();
    assert!(!dir.path().join("work 2").exists());
    assert!(s.current_notebook().unwrap().section(&other).is_none());
    assert!(s.selection().section.is_some());
    assert_matches_disk(dir.path(), &s);
    assert_eq!(s.undo().as_deref(), Some("Delete section"));
    assert!(dir.path().join("work 2/.section.toml").is_file());
    assert_eq!(s.undo().as_deref(), Some("Section color"));
    assert_eq!(s.undo().as_deref(), Some("Rename section"));
    assert_eq!(s.undo().as_deref(), Some("Rename section"));
    assert_matches_disk(dir.path(), &s);
}

#[test]
fn deleting_a_section_with_pages_keeps_them_in_the_trash() {
    let (dir, mut s, nb) = sample();
    let work = format!("{nb}/Work");
    s.delete_section(&work).unwrap();
    assert!(!dir.path().join("Work").exists());
    let kept = fs::read_dir(dir.path().join(".notecraft/trash"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(fs::read_to_string(kept.join("Work/B.md")).unwrap(), "beta");
    s.undo();
    assert_eq!(pages(&s, &work), ["A", "B", "C"]);
    assert_matches_disk(dir.path(), &s);
}

#[test]
fn sections_move_between_groups_and_reorder() {
    let (dir, mut s, nb) = sample();
    let work = format!("{nb}/Work");
    let projects = format!("{nb}/Projects");
    let moved = s.move_section_to(&work, Some(&projects), 0).unwrap();
    assert_eq!(moved, format!("{nb}/Projects/Work"));
    assert!(dir.path().join("Projects/Work/A.md").is_file());
    assert_eq!(pages(&s, &moved), ["A", "B", "C"]);
    assert_matches_disk(dir.path(), &s);
    // Out again to the top, first.
    let back = s.move_section_to(&moved, None, 0).unwrap();
    assert_eq!(back, work);
    assert_eq!(top_names(&s)[0], "Work");
    s.move_section_to(&work, None, 5).unwrap();
    assert_eq!(top_names(&s).last().map(String::as_str), Some("Work"));
    assert_matches_disk(dir.path(), &s);
    for _ in 0..3 {
        s.undo();
    }
    assert!(!dir.path().join("Projects/Work").exists());
    assert_matches_disk(dir.path(), &s);
    // A group cannot move into itself or its children.
    assert!(s
        .move_section_to(&projects, Some(&format!("{nb}/Projects/Alpha")), 0)
        .is_err());
    assert!(s.move_section_to(&projects, Some(&projects), 0).is_err());
}

#[test]
fn the_nesting_limit_is_kept() {
    let (_dir, mut s, nb) = sample();
    let mut parent: Option<String> = None;
    for _ in 0..8 {
        parent = Some(
            s.create_section_group(&nb, parent.as_deref(), Some("G"))
                .unwrap(),
        );
    }
    assert!(s
        .create_section_group(&nb, parent.as_deref(), Some("G"))
        .is_err());
    assert!(s.create_section(&nb, parent.as_deref(), Some("S")).is_err());
}

#[test]
fn a_notebook_is_renamed_recolored_and_closed_with_undo() {
    let (dir, mut s, nb) = sample();
    s.rename_notebook(&nb, "Field Notes").unwrap();
    assert_eq!(s.current_notebook().unwrap().name, "Field Notes");
    let toml = fs::read_to_string(dir.path().join(".notecraft/notebook.toml")).unwrap();
    assert!(toml.contains("Field Notes") && toml.contains(&nb), "{toml}");
    s.set_notebook_color(&nb, 0xAA5500).unwrap();
    assert_eq!(s.current_notebook().unwrap().color, 0xAA5500);
    assert_eq!(
        s.current_notebook().unwrap().id,
        nb,
        "the id stays the same"
    );
    assert!(s.rename_notebook(&nb, "  ").is_err());
    s.close_notebook(&nb).unwrap();
    assert!(s.notebooks().is_empty());
    assert!(
        dir.path().join("Work/A.md").is_file(),
        "closing leaves the files"
    );
    assert_eq!(s.undo().as_deref(), Some("Close notebook"));
    assert_eq!(s.current_notebook().unwrap().name, "Field Notes");
    assert_eq!(s.undo().as_deref(), Some("Notebook color"));
    assert_eq!(s.undo().as_deref(), Some("Rename notebook"));
    assert!(!dir.path().join(".notecraft/notebook.toml").exists());
    assert_matches_disk(dir.path(), &s);
}

#[test]
fn creating_a_notebook_makes_the_folder_and_its_settings_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let folder = dir.path().join("New Book");
    let mut s = Session::new();
    let id = s.create_notebook(&folder).unwrap();
    assert!(folder.join(".notecraft/notebook.toml").is_file());
    assert_eq!(s.current_notebook().unwrap().name, "New Book");
    assert_eq!(s.current_notebook().unwrap().id, id);
    // Creating again in the same folder keeps the notebook.
    assert_eq!(s.create_notebook(&folder).unwrap(), id);
    assert_eq!(s.notebooks().len(), 1);
    let section = s.create_section(&id, None, None).unwrap();
    s.create_page(&section, Some("First")).unwrap();
    assert_eq!(scan_notebook(&folder).unwrap().page_count(), 2);
    assert!(s
        .create_notebook(&folder.join(".notecraft/notebook.toml"))
        .is_err());
}

#[test]
fn a_step_that_cannot_finish_leaves_the_files_and_the_tree_as_they_were() {
    let (dir, mut s, nb) = sample();
    let section = format!("{nb}/Work");
    let before = s.clone();
    // Something else puts a file where the rename would go, after the plan is made.
    let plan_target = dir.path().join("Work/Zed.md");
    let ops = vec![
        Op::Fs(FsOp::Move {
            notebook: nb.clone(),
            from: "Work/A.md".into(),
            to: "Work/Zed.md".into(),
        }),
        Op::Fs(FsOp::Move {
            notebook: nb.clone(),
            from: "Work/B.md".into(),
            to: "Work/C.md".into(),
        }),
    ];
    let tx = ops
        .into_iter()
        .fold(Transaction::new(Scope::Notebook, "Test"), Transaction::with);
    assert!(!s.commit(tx), "the second move is refused");
    assert!(!plan_target.exists(), "the first move was reversed");
    assert_eq!(
        fs::read_to_string(dir.path().join("Work/A.md")).unwrap(),
        "alpha"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("Work/C.md")).unwrap(),
        "gamma"
    );
    assert_eq!(s.notebooks(), before.notebooks());
    assert!(!s.can_undo());
    assert_eq!(pages(&s, &section), ["A", "B", "C"]);
    // A path that leaves the notebook is refused too.
    let escape = Op::Fs(FsOp::Move {
        notebook: nb.clone(),
        from: "Work/A.md".into(),
        to: "../outside.md".into(),
    });
    assert!(!s.commit(Transaction::new(Scope::Notebook, "Test").with(escape)));
    assert!(dir.path().join("Work/A.md").is_file());
}

#[test]
fn an_undo_that_no_longer_fits_the_files_is_dropped_without_damage() {
    let (dir, mut s, nb) = sample();
    let a = format!("{nb}/Work/A.md");
    s.rename_page(&a, "Agenda").unwrap();
    // Someone puts a different file back where the undo would move the page.
    write(dir.path(), "Work/A.md", "someone else");
    assert_eq!(s.undo(), None);
    assert_eq!(
        fs::read_to_string(dir.path().join("Work/A.md")).unwrap(),
        "someone else"
    );
    assert!(dir.path().join("Work/Agenda.md").is_file());
    assert_matches_disk(dir.path(), &s);
}

#[test]
fn a_sample_notebook_cannot_be_edited_on_disk() {
    let mut s = Session::new();
    s.set_notebooks(crate::mock::notebooks());
    let section = s.current_section().unwrap().id.clone();
    assert!(s.create_page(&section, None).is_err());
    assert!(s
        .create_section(&s.current_notebook().unwrap().id.clone(), None, None)
        .is_err());
}

#[test]
fn attachment_links_are_found_leniently() {
    let found = attachment_links(
        "![a](attachments/x1-a.png) [b](attachments/y2-b%20c.pdf) attachments/../evil attachments/dir/f attachments/",
    );
    assert_eq!(found, ["x1-a.png", "y2-b c.pdf"]);
}

#[test]
fn attachment_links_decode_what_the_attachment_writer_encodes() {
    let name = "3f9a2c-photo (1) caf\u{e9}.png";
    let link = notecraft_storage::attachments::encode_link_path(&format!("attachments/{name}"));
    let page = format!("![p]({link}) [up](attachments/..%2Fsecret) [dir](attachments/a%2Fb)");
    assert_eq!(attachment_links(&page), [name]);
}

#[test]
fn a_new_section_starts_with_one_untitled_page_in_the_same_step() {
    let (dir, mut s, nb) = sample();
    let section = s.create_section(&nb, None, Some("Ideas")).unwrap();
    assert!(dir.path().join("Ideas/Untitled Page.md").is_file());
    assert_eq!(pages(&s, &section), ["Untitled Page"]);
    assert_eq!(
        s.selection().page.as_deref(),
        Some(format!("{section}/Untitled Page.md").as_str())
    );
    assert_matches_disk(dir.path(), &s);
    // One undo removes the section and its page; redo brings both back.
    assert_eq!(s.undo().as_deref(), Some("New section"));
    assert!(!dir.path().join("Ideas").exists());
    assert_matches_disk(dir.path(), &s);
    assert_eq!(s.redo().as_deref(), Some("New section"));
    assert_eq!(pages(&s, &section), ["Untitled Page"]);
    assert_matches_disk(dir.path(), &s);
    // A group has no page.
    let group = s.create_section_group(&nb, None, Some("Folder")).unwrap();
    assert!(!dir.path().join("Folder/Untitled Page.md").exists());
    assert!(s.current_notebook().unwrap().section(&group).is_none());
}

/// Work holds A, B (sub of A), C (sub of B), D2 and E2; Home holds H.
fn nested() -> (tempfile::TempDir, Session, String) {
    open(&[
        ("Work/A.md", "a"),
        ("Work/B.md", "b"),
        ("Work/C.md", "c"),
        ("Work/D2.md", "d"),
        ("Work/E2.md", "e"),
        (
            "Work/.section.toml",
            "kind = \"section\"\npages = [\"A.md\", { file = \"B.md\", level = 1 }, { file = \"C.md\", level = 2 }, \"D2.md\", \"E2.md\"]\n",
        ),
        ("Home/H.md", "h"),
    ])
}

fn levels(s: &Session, section: &str) -> Vec<(String, u8, bool)> {
    s.current_notebook()
        .unwrap()
        .section(section)
        .unwrap()
        .pages
        .iter()
        .map(|p| (p.title.clone(), p.level, p.collapsed))
        .collect()
}

#[test]
fn collapsing_subpages_is_saved_in_the_section_file_and_undoable() {
    let (dir, mut s, nb) = nested();
    let section = format!("{nb}/Work");
    let a = format!("{nb}/Work/A.md");
    assert!(s
        .set_subpages_collapsed(&format!("{nb}/Work/C.md"), true)
        .is_err());
    s.select_page_id(&format!("{nb}/Work/C.md"));
    s.set_subpages_collapsed(&a, true).unwrap();
    assert!(levels(&s, &section)[0].2);
    assert_eq!(s.selection().page.as_deref(), Some(a.as_str()));
    let text = fs::read_to_string(dir.path().join("Work/.section.toml")).unwrap();
    assert!(text.contains("collapsed = true"), "{text}");
    assert_matches_disk(dir.path(), &s);
    // A restart reads the state back.
    let reopened = open_folder(dir.path()).unwrap().notebook;
    assert!(reopened.section(&section).unwrap().pages[0].collapsed);
    assert_eq!(s.undo().as_deref(), Some("Collapse subpages"));
    assert!(!levels(&s, &section)[0].2);
    s.redo();
    s.set_subpages_collapsed(&a, false).unwrap();
    assert!(!levels(&s, &section)[0].2);
    let text = fs::read_to_string(dir.path().join("Work/.section.toml")).unwrap();
    assert!(!text.contains("collapsed"), "{text}");
}

#[test]
fn moving_a_parent_carries_its_subpages_as_one_undo_step() {
    let (dir, mut s, nb) = nested();
    let section = format!("{nb}/Work");
    let a = format!("{nb}/Work/A.md");
    s.move_page_to(&a, &section, 10).unwrap();
    assert_eq!(
        levels(&s, &section)
            .iter()
            .map(|(t, l, _)| (t.as_str(), *l))
            .collect::<Vec<_>>(),
        [("D2", 0), ("E2", 0), ("A", 0), ("B", 1), ("C", 2)]
    );
    assert_matches_disk(dir.path(), &s);
    assert_eq!(s.undo().as_deref(), Some("Move page"));
    assert_eq!(pages(&s, &section), ["A", "B", "C", "D2", "E2"]);
    assert_eq!(levels(&s, &section)[2].1, 2);
    // To another section, levels kept, one step.
    let home = format!("{nb}/Home");
    let new = s.move_page_to(&a, &home, 0).unwrap();
    assert_eq!(new, format!("{nb}/Home/A.md"));
    assert_eq!(pages(&s, &home), ["A", "B", "C", "H"]);
    assert_eq!(
        levels(&s, &home).iter().map(|x| x.1).collect::<Vec<_>>(),
        [0, 1, 2, 0]
    );
    assert_eq!(pages(&s, &section), ["D2", "E2"]);
    assert!(dir.path().join("Home/C.md").is_file());
    assert_matches_disk(dir.path(), &s);
    assert_eq!(s.undo().as_deref(), Some("Move page"));
    assert_eq!(pages(&s, &section), ["A", "B", "C", "D2", "E2"]);
    assert!(!dir.path().join("Home/B.md").exists());
    assert_matches_disk(dir.path(), &s);
}

#[test]
fn deleting_a_collapsed_parent_trashes_its_subpages_too() {
    let (dir, mut s, nb) = nested();
    let section = format!("{nb}/Work");
    let a = format!("{nb}/Work/A.md");
    // Expanded: only the page goes, and the first subpage is promoted so levels stay valid.
    s.delete_page(&a).unwrap();
    assert_eq!(levels(&s, &section)[0], ("B".to_owned(), 0, false));
    s.undo();
    s.set_subpages_collapsed(&a, true).unwrap();
    s.select_page_id(&a);
    s.delete_page(&a).unwrap();
    assert_eq!(pages(&s, &section), ["D2", "E2"]);
    for gone in ["A.md", "B.md", "C.md"] {
        assert!(!dir.path().join("Work").join(gone).exists());
    }
    assert_eq!(
        fs::read_dir(dir.path().join(".notecraft/trash"))
            .unwrap()
            .count(),
        3
    );
    assert_eq!(
        s.selection().page.as_deref(),
        Some(format!("{nb}/Work/D2.md").as_str())
    );
    assert_matches_disk(dir.path(), &s);
    assert_eq!(s.undo().as_deref(), Some("Delete page"));
    assert_eq!(pages(&s, &section), ["A", "B", "C", "D2", "E2"]);
    assert!(levels(&s, &section)[0].2, "still collapsed after undo");
    assert_matches_disk(dir.path(), &s);
}
