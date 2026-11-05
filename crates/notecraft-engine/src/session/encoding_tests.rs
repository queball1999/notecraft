#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;

use super::*;
use crate::cmd::Registry;
use crate::open::open_folder;

/// Windows-1252 bytes for "caf\u{e9} \u{201c}ok\u{201d}\r\nline two\r\n".
const CP1252: &[u8] = b"caf\xe9 \x93ok\x94\r\nline two\r\n";
const CP1252_TEXT: &str = "caf\u{e9} \u{201c}ok\u{201d}\r\nline two\r\n";

fn utf16_le_bom(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    bytes
}

fn notebook(bytes: &[u8]) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("Sec/A.md");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
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

fn page(session: &Session) -> String {
    session.selection().page.clone().unwrap()
}

fn disk(dir: &tempfile::TempDir) -> Vec<u8> {
    std::fs::read(dir.path().join("Sec/A.md")).unwrap()
}

fn backups(dir: &tempfile::TempDir) -> Vec<std::path::PathBuf> {
    let folder = dir.path().join(".notecraft/history/Sec_A.md");
    match std::fs::read_dir(folder) {
        Ok(entries) => entries.map(|e| e.unwrap().path()).collect(),
        Err(_) => Vec::new(),
    }
}

fn check_opens_read_only_and_converts(original: &[u8], text: &str) {
    let dir = notebook(original);
    let mut session = open(&dir);
    let id = page(&session);
    assert_eq!(session.page_text(&id), Some(text), "decoded, not U+FFFD");
    let notice = session.current_page_notice().unwrap();
    assert!(notice.read_only);
    assert_eq!(
        notice.text,
        "This page is not saved as UTF-8 text, so NoteCraft opened it read-only."
    );
    assert!(session.can_convert_to_utf8());
    assert!(!session.page_is_editable(&id));

    // Typing does nothing and the file keeps its bytes.
    assert!(!session.edit_page_text(&id, 0, 0, "x", false));
    assert!(!session.is_page_dirty(&id));
    assert_eq!(session.save_page(&id), SaveOutcome::NotDirty);
    assert_eq!(disk(&dir), original);

    let out = Registry::builtin()
        .run(&mut session, "page.convert_encoding", json!({}))
        .unwrap()
        .value;
    assert_eq!(out["converted"], true, "{out}");
    assert_eq!(
        disk(&dir),
        text.as_bytes(),
        "UTF-8, same text and line endings"
    );
    let kept = backups(&dir);
    assert_eq!(kept.len(), 1);
    assert_eq!(std::fs::read(&kept[0]).unwrap(), original);
    let name = kept[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("original-") && name.ends_with(".bak"),
        "{name}"
    );

    assert_eq!(session.current_page_notice(), None);
    assert!(!session.can_convert_to_utf8());
    assert!(session.page_is_editable(&id));
    assert!(session.edit_page_text(&id, 0, 0, "x", false));
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(disk(&dir), format!("x{text}").as_bytes());
}

#[test]
fn a_windows_1252_page_opens_read_only_and_converts() {
    check_opens_read_only_and_converts(CP1252, CP1252_TEXT);
}

#[test]
fn a_utf16_page_with_a_bom_opens_read_only_and_converts() {
    check_opens_read_only_and_converts(&utf16_le_bom(CP1252_TEXT), CP1252_TEXT);
}

#[test]
fn a_valid_utf8_page_is_unaffected() {
    let dir = notebook("caf\u{e9}\n".as_bytes());
    let mut session = open(&dir);
    let id = page(&session);
    assert_eq!(session.current_page_notice(), None);
    assert!(!session.can_convert_to_utf8());
    assert!(session.edit_page_text(&id, 0, 0, "x", false));
    assert_eq!(session.save_page(&id), SaveOutcome::Saved);
    assert_eq!(disk(&dir), "xcaf\u{e9}\n".as_bytes());
    let out = Registry::builtin()
        .run(&mut session, "page.convert_encoding", json!({}))
        .unwrap()
        .value;
    assert_eq!(out["converted"], false);
    assert!(backups(&dir).is_empty());
}

#[test]
fn an_outside_edit_to_a_utf8_file_does_not_make_it_editable_when_it_is_not_utf8() {
    let dir = notebook(b"plain\n");
    let mut session = open(&dir);
    let id = page(&session);
    assert!(session.page_is_editable(&id));
    let path = dir.path().join("Sec/A.md");
    std::fs::write(&path, CP1252).unwrap();
    session.note_outside_change(&path);
    assert_eq!(session.page_text(&id), Some(CP1252_TEXT));
    assert!(
        session.current_page_notice().unwrap().read_only,
        "reload keeps read-only"
    );
    assert!(!session.page_is_editable(&id));
    assert!(session.can_convert_to_utf8());
    assert_eq!(disk(&dir), CP1252);
}

#[test]
fn unsaved_text_is_never_saved_over_a_file_that_turned_non_utf8() {
    let dir = notebook(b"plain\n");
    let mut session = open(&dir);
    let id = page(&session);
    assert!(session.edit_page_text(&id, 0, 0, "mine ", false));
    let path = dir.path().join("Sec/A.md");
    std::fs::write(&path, CP1252).unwrap();
    session.note_outside_change(&path);
    assert!(matches!(session.save_page(&id), SaveOutcome::Failed(_)));
    assert_eq!(disk(&dir), CP1252);
    assert_eq!(session.page_text(&id), Some("mine plain\n"));
    assert!(session.is_page_dirty(&id));
}

#[test]
fn recovery_is_not_offered_over_a_non_utf8_file() {
    let dir = notebook(CP1252);
    let mut session = open(&dir);
    let id = page(&session);
    let path = dir.path().join("Sec/A.md");
    let entry = notecraft_storage::RecoveryEntry {
        path,
        text: "recovered\n".to_owned(),
    };
    session.offer_recovery(vec![entry]);
    assert!(session.pending_recovery().is_empty());
    assert_eq!(session.restore_recovery(), 0);
    assert!(!session.page_is_editable(&id));
    assert_eq!(disk(&dir), CP1252);
}

#[test]
fn a_page_that_changed_since_it_was_read_is_not_converted() {
    let dir = notebook(CP1252);
    let mut session = open(&dir);
    std::fs::write(dir.path().join("Sec/A.md"), b"other \xe9\n").unwrap();
    assert_eq!(
        session.convert_current_page_to_utf8(),
        Utf8Outcome::Failed("the file changed outside NoteCraft".to_owned())
    );
    assert_eq!(disk(&dir), b"other \xe9\n");
    assert!(backups(&dir).is_empty());
    assert!(session.current_page_notice().unwrap().read_only);
}

#[test]
fn hostile_bytes_open_without_a_panic() {
    for bytes in [
        &[0xFFu8, 0xFE, 0x00][..],
        &[0xFE, 0xFF, 0xD8, 0x00, 0xD8],
        &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        &[0xC0, 0xAF, 0xE0, 0x80, 0x80, 0xF8],
    ] {
        let dir = notebook(bytes);
        let mut session = open(&dir);
        let id = page(&session);
        assert!(!session.edit_page_text(&id, 0, 0, "x", false) || session.page_is_editable(&id));
        let _ = session.convert_current_page_to_utf8();
    }
}

#[test]
fn ink_and_style_changes_are_refused_on_a_non_utf8_page() {
    let dir = notebook(CP1252);
    let mut session = open(&dir);
    let id = page(&session);
    let registry = Registry::builtin();
    let refused = registry.run(
        &mut session,
        "ink.add_stroke",
        json!({ "points": [[10.0, 10.0], [20.0, 20.0]] }),
    );
    assert!(format!("{refused:?}").contains("read-only"), "{refused:?}");
    assert_eq!(session.ink().layer(&id).map_or(0, |layer| layer.len()), 0);
    let style = crate::view::PageStyle::default();
    let tx = crate::undo::Transaction::new(crate::undo::Scope::Page(id.clone()), "Style").with(
        crate::undo::Op::SetPageStyle {
            page: id.clone(),
            before: style,
            after: style,
        },
    );
    assert!(!session.commit(tx));
    assert!(!session.can_undo());
    assert!(matches!(
        session.save_page(&id),
        SaveOutcome::NotDirty | SaveOutcome::ReadOnly
    ));
    assert_eq!(disk(&dir), CP1252);
    // After converting, the same edits are allowed.
    registry
        .run(&mut session, "page.convert_encoding", json!({}))
        .unwrap();
    registry
        .run(
            &mut session,
            "ink.add_stroke",
            json!({ "points": [[10.0, 10.0], [20.0, 20.0]] }),
        )
        .unwrap();
    assert_eq!(session.ink().layer(&id).map_or(0, |layer| layer.len()), 1);
}
