#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use notecraft_canvas::{PaperSize, RuleLines};
use serde_json::json;

use super::*;
use crate::cmd::Registry;
use crate::open::open_folder;
use crate::session::SaveOutcome;

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

fn run(session: &mut Session, id: &str, args: serde_json::Value) {
    Registry::builtin().run(session, id, args).unwrap();
}

fn current_id(session: &Session) -> String {
    session.selection().page.clone().unwrap()
}

fn read(dir: &tempfile::TempDir, rel: &str) -> String {
    std::fs::read_to_string(dir.path().join(rel)).unwrap()
}

fn style_everything(session: &mut Session) {
    run(session, "view.rule_lines", json!({ "style": "wide" }));
    run(
        session,
        "view.rule_line_color",
        json!({ "color": "#336699" }),
    );
    run(session, "view.page_color", json!({ "color": "#FFF8E1" }));
    run(session, "view.paper_size", json!({ "size": "a4" }));
}

fn check_styled(session: &Session) {
    let style = session.page_style();
    assert_eq!(style.rule_lines, RuleLines::Wide);
    assert_eq!(style.rule_color, Some([0x33, 0x66, 0x99, 255]));
    assert_eq!(style.color, Some([0xFF, 0xF8, 0xE1, 255]));
    assert_eq!(style.paper_size, PaperSize::A4);
}

#[test]
fn page_style_is_saved_and_reloads_for_markdown_and_canvas_pages() {
    for (file, text) in [
        ("Sec/A.md", "Plain page\n"),
        ("Sec/A.notecraft", "Plain page\n"),
    ] {
        let dir = notebook(&[(file, text)]);
        let mut session = open(&dir);
        let id = current_id(&session);
        assert!(!session.is_page_dirty(&id));
        style_everything(&mut session);
        assert!(session.is_page_dirty(&id), "a style change is unsaved");
        assert_eq!(session.save_page(&id), SaveOutcome::Saved);
        assert!(!session.is_page_dirty(&id));
        let saved = read(&dir, file);
        assert!(saved.starts_with("---\nnotecraft:\n  paper: lined\n  paper_spacing: wide\n"));
        assert!(saved.contains("  paper_size: a4\n"));
        assert!(saved.contains("  rule_color: \"#336699\"\n"));
        assert!(saved.contains("  color: \"#FFF8E1\"\n"));
        assert!(saved.ends_with("---\nPlain page\n") || saved.contains("---\nPlain page\n"));
        let again = open(&dir);
        check_styled(&again);
    }
}

#[test]
fn a_page_with_an_unchanged_style_is_saved_byte_for_byte() {
    let text =
        "---\ntitle: T\nnotecraft:\n  paper: grid   # my grid\n  color: \"#FFF8E1\"\n---\nBody\n";
    let dir = notebook(&[("Sec/A.md", text)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    assert_eq!(session.page_style().rule_lines, RuleLines::GridMedium);
    // Setting what is already there changes nothing; an explicit save does not rewrite it.
    run(
        &mut session,
        "view.page_color",
        json!({ "color": "#FFF8E1" }),
    );
    assert!(!session.is_page_dirty(&id));
    session.mark_page_dirty(&id);
    session.save_page(&id);
    assert_eq!(read(&dir, "Sec/A.md"), text);
    // Without a style at all, a plain page stays plain.
    let dir = notebook(&[("Sec/B.md", "Just text\n")]);
    let mut session = open(&dir);
    let id = current_id(&session);
    session.mark_page_dirty(&id);
    session.save_page(&id);
    assert_eq!(read(&dir, "Sec/B.md"), "Just text\n");
}

#[test]
fn only_the_style_lines_change_in_a_page_with_other_frontmatter() {
    let text = "---\ntitle: T\ntags: [a, b]\nnotecraft:\n  id: pg_1    # stable\n  paper: lined\ncreated: 2026-10-14\n---\nBody\r\n\nMore\n";
    let dir = notebook(&[("Sec/A.md", text)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    assert_eq!(session.page_style().rule_lines, RuleLines::College);
    run(
        &mut session,
        "view.rule_lines",
        json!({ "style": "grid_large" }),
    );
    run(
        &mut session,
        "view.page_color",
        json!({ "color": "#102030" }),
    );
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(
        read(&dir, "Sec/A.md"),
        "---\ntitle: T\ntags: [a, b]\nnotecraft:\n  id: pg_1    # stable\n  paper: grid\n  paper_spacing: large\n  color: \"#102030\"\ncreated: 2026-10-14\n---\nBody\r\n\nMore\n"
    );
}

#[test]
fn going_back_to_the_default_removes_the_keys() {
    let text = "---\ntitle: T\nnotecraft:\n  paper: lined\n  paper_spacing: narrow\n  color: \"#FFF8E1\"\n---\nBody\n";
    let dir = notebook(&[("Sec/A.md", text)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    run(&mut session, "view.rule_lines", json!({ "style": "none" }));
    run(&mut session, "view.page_color", json!({ "color": "none" }));
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    // The map is empty now, so it goes too; the rest of the block stays.
    assert_eq!(read(&dir, "Sec/A.md"), "---\ntitle: T\n---\nBody\n");
    let again = open(&dir);
    assert_eq!(again.page_style(), PageStyle::default());
}

#[test]
fn bad_values_are_ignored_and_left_alone() {
    let text = "---\nnotecraft:\n  paper: stripey\n  paper_spacing: 9\n  paper_size: tabloid\n  color: red\n  rule_color: \"#12\"\n---\nBody\n";
    let dir = notebook(&[("Sec/A.md", text)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    assert_eq!(session.page_style(), PageStyle::default());
    // Unknown spacing on a known paper falls back to its default spacing.
    assert_eq!(
        style_from_text("---\nnotecraft:\n  paper: grid\n  paper_spacing: huge\n---\n").rule_lines,
        RuleLines::GridMedium
    );
    session.mark_page_dirty(&id);
    session.save_page(&id);
    assert_eq!(
        read(&dir, "Sec/A.md"),
        text,
        "ignored values are kept as written"
    );
    // Changing one key leaves the others as they were.
    run(&mut session, "view.paper_size", json!({ "size": "legal" }));
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(
        read(&dir, "Sec/A.md"),
        text.replace("tabloid", "legal"),
        "only the touched key changed"
    );
    // Hostile frontmatter never panics.
    for hostile in [
        "---\nnotecraft: {paper: lined}\n---\n",
        "---\nnotecraft: 5\n---\n",
    ] {
        let style = PageStyle {
            rule_lines: RuleLines::Wide,
            ..PageStyle::default()
        };
        assert_eq!(text_with_style(hostile, &style), hostile);
    }
}

#[test]
fn undo_and_redo_keep_the_dirty_state_right() {
    let dir = notebook(&[("Sec/A.md", "Body\n")]);
    let mut session = open(&dir);
    let id = current_id(&session);
    run(
        &mut session,
        "view.rule_lines",
        json!({ "style": "college" }),
    );
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert!(!session.is_page_dirty(&id));
    run(&mut session, "edit.undo", json!({}));
    assert!(session.is_page_dirty(&id), "undo is an unsaved change");
    assert_eq!(session.page_style(), PageStyle::default());
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(read(&dir, "Sec/A.md"), "---\n---\nBody\n");
    run(&mut session, "edit.redo", json!({}));
    assert!(session.is_page_dirty(&id), "redo is an unsaved change");
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(
        read(&dir, "Sec/A.md"),
        "---\nnotecraft:\n  paper: lined\n---\nBody\n"
    );
}

#[test]
fn a_new_page_gets_the_paper_key_when_rule_lines_are_always_on() {
    let dir = notebook(&[("Sec/A.md", "Body\n")]);
    let mut session = open(&dir);
    let section = session.selection().section.clone().unwrap();
    let plain = session.create_page(&section, Some("Plain")).unwrap();
    assert!(!plain.is_empty());
    assert_eq!(read(&dir, "Sec/Plain.md"), "");
    session.set_default_rule_lines(RuleLines::Narrow);
    session.create_page(&section, Some("Ruled")).unwrap();
    assert_eq!(
        read(&dir, "Sec/Ruled.md"),
        "---\nnotecraft:\n  paper: lined\n  paper_spacing: narrow\n---\n"
    );
    let mut again = open(&dir);
    let ruled = again
        .current_section()
        .unwrap()
        .pages
        .iter()
        .find(|p| p.title == "Ruled")
        .unwrap()
        .id
        .clone();
    again.selection_mut().page = Some(ruled);
    again.load_current_page();
    assert_eq!(again.page_style().rule_lines, RuleLines::Narrow);
}

#[test]
fn mock_pages_keep_their_style_for_the_session_only() {
    use notecraft_model::{Notebook, PageRef, Section, SectionNode};
    let section = Section {
        id: "s1".into(),
        name: "Notes".into(),
        color: 0,
        pages: vec![PageRef::new("p1", "One", 0)],
    };
    let mut session = Session::new();
    session.set_notebooks(vec![Notebook {
        id: "nb".into(),
        name: "Notebook".into(),
        color: 0,
        sections: vec![SectionNode::Section(section)],
    }]);
    run(&mut session, "view.rule_lines", json!({ "style": "wide" }));
    assert_eq!(session.page_style().rule_lines, RuleLines::Wide);
    let id = current_id(&session);
    assert!(!session.is_page_dirty(&id));
    assert_eq!(session.save_page(&id), SaveOutcome::NotDirty);
}
