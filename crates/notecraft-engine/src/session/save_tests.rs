#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::*;
use crate::cmd::Registry;
use crate::open::open_folder;

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

fn current_id(session: &Session) -> String {
    session.selection().page.clone().unwrap()
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
}

#[test]
fn a_page_whose_canvas_block_cannot_be_read_is_kept_and_not_saved_over() {
    let text = "Words\n\n```notecraft-canvas\n{ \"ink\": [ \n```\n";
    let dir = notebook(&[("Sec/A.notecraft", text)]);
    let mut session = open(&dir);
    let notice = session.current_page_notice().unwrap();
    assert!(notice.read_only, "{notice:?}");
    assert!(session.current_ink().is_none());
    assert_eq!(session.current_page_source().unwrap().text, text);
    let copy = dir
        .path()
        .join(".notecraft/history/Sec_A.notecraft/canvas-unreadable.notecraft");
    assert_eq!(std::fs::read_to_string(copy).unwrap(), text);
    let id = current_id(&session);
    assert_eq!(session.save_page(&id), Err(SaveError::ReadOnly));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Sec/A.notecraft")).unwrap(),
        text
    );
}

#[test]
fn ink_is_saved_with_a_minimal_diff_and_reloads() {
    let dir = notebook(&[("Sec/A.notecraft", BASIC)]);
    let mut session = open(&dir);
    let id = current_id(&session);
    assert_eq!(session.save_page(&id), Ok(SaveOutcome::Unchanged));
    run(&mut session, "draw.pen", json!({}));
    run(
        &mut session,
        "ink.add_stroke",
        json!({ "points": [[5, 6, 0.5], [7, 8, 0.25]] }),
    );
    assert_eq!(session.save_page(&id), Ok(SaveOutcome::Written));
    let saved = std::fs::read_to_string(dir.path().join("Sec/A.notecraft")).unwrap();
    let ink_at = BASIC.find("\"ink\": ").unwrap();
    assert_eq!(saved.get(..ink_at), BASIC.get(..ink_at));
    assert!(saved.ends_with("  \"z\": [\"c1\", \"c2\", \"sh1\", \"s1\", \"s2\"],\n  \"future\": { \"keep\": [\"me\", 1, null] }\n}\n```\n"));
    assert_eq!(session.save_page(&id), Ok(SaveOutcome::Unchanged));
    let again = open(&dir);
    assert_eq!(again.current_ink().unwrap().len(), 3);
    // The file keeps two decimal places, so compare what the file would store.
    let stored = |s: &Session| -> Vec<_> {
        s.current_ink()
            .unwrap()
            .strokes()
            .iter()
            .map(notecraft_storage::normalized)
            .collect()
    };
    assert_eq!(stored(&again), stored(&session));
}

#[test]
fn a_markdown_page_with_ink_is_promoted_on_save() {
    let dir = notebook(&[("Sec/A.md", "# Hi\n"), ("Sec/B.md", "other\n")]);
    let mut session = open(&dir);
    let id = current_id(&session);
    assert!(id.ends_with("Sec/A.md"), "{id}");
    // Text only: stays a `.md` file.
    session.set_page_source(&id, "# Hi there\n");
    assert_eq!(session.save_page(&id), Ok(SaveOutcome::Written));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("Sec/A.md")).unwrap(),
        "# Hi there\n"
    );
    run(&mut session, "draw.pen", json!({}));
    run(
        &mut session,
        "ink.add_stroke",
        json!({ "points": [[1, 2], [3, 4]] }),
    );
    let Ok(SaveOutcome::Promoted { new_id }) = session.save_page(&id) else {
        panic!("not promoted");
    };
    assert!(new_id.ends_with("Sec/A.notecraft"));
    assert!(!dir.path().join("Sec/A.md").exists());
    assert_eq!(current_id(&session), new_id);
    assert!(session
        .current_section()
        .unwrap()
        .pages
        .iter()
        .any(|p| p.id == new_id));
    assert_eq!(session.current_ink().unwrap().len(), 1);
    let text = std::fs::read_to_string(dir.path().join("Sec/A.notecraft")).unwrap();
    assert!(text.starts_with("# Hi there\n\n```notecraft-canvas\n"));
    // A fresh session reads it back, and the page list shows it as a canvas page.
    let again = open(&dir);
    assert!(again.current_page().unwrap().id.ends_with("A.notecraft"));
    assert_eq!(again.current_ink().unwrap().len(), 1);
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
    let text = std::fs::read_to_string(dir.path().join("Sec/A.md")).unwrap();
    assert!(text.contains("## Agenda") && text.contains("Remember to send"));
    assert!(!text.contains("notecraft:container") && !text.contains("notecraft-canvas"));
    assert!(session.current_ink().is_none());
    assert!(current_id(&session).ends_with("Sec/A.md"));
    // A markdown page has nothing to convert.
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
    assert_eq!(session.save_page("p-grocery"), Err(SaveError::NotSaveable));
}
