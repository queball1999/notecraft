//! Harness tests for pictures and file attachments: insert from a picker, drop, paste from the
//! clipboard, render, and open.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use egui_kittest::kittest::Queryable;
use notecraft_engine::cmd::Value;

use crate::{AppShell, FilePicker};

const WINDOW: egui::Vec2 = egui::vec2(1280.0, 800.0);

#[derive(Debug, Default, Clone)]
struct Fake {
    folder: Option<String>,
    picture: Option<String>,
    attachment: Option<String>,
    clipboard: Option<String>,
    opened: Arc<Mutex<Vec<String>>>,
}

impl FilePicker for Fake {
    fn pick_folder(&mut self, _title: &str) -> Option<String> {
        self.folder.clone()
    }
    fn pick_file(&mut self, _title: &str) -> Option<String> {
        None
    }
    fn pick_picture(&mut self, _title: &str) -> Option<String> {
        self.picture.clone()
    }
    fn pick_attachment(&mut self, _title: &str) -> Option<String> {
        self.attachment.clone()
    }
    fn clipboard_picture(&mut self) -> Option<String> {
        self.clipboard.clone()
    }
    fn open_in_default_app(&mut self, path: &str) -> bool {
        self.opened.lock().unwrap().push(path.to_owned());
        true
    }
}

#[derive(Debug)]
struct Dropped(PathBuf);

impl egui::DroppedFile for Dropped {
    fn path(&self) -> &Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

struct Setup {
    dir: tempfile::TempDir,
    harness: egui_kittest::Harness<'static, AppShell>,
    opened: Arc<Mutex<Vec<String>>>,
}

fn setup(clipboard: Option<&str>) -> Setup {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("Notes")).unwrap();
    std::fs::write(dir.path().join("Notes/First.md"), "# First\n\nHello.\n").unwrap();
    std::fs::write(dir.path().join("photo.png"), png(64, 48)).unwrap();
    std::fs::write(dir.path().join("broken.png"), b"\x89PNG\r\n\x1a\nnope").unwrap();
    std::fs::write(dir.path().join("budget.xlsx"), b"cells").unwrap();
    let path = |name: &str| Some(dir.path().join(name).to_string_lossy().into_owned());
    let opened = Arc::new(Mutex::new(Vec::new()));
    let fake = Fake {
        folder: Some(dir.path().to_string_lossy().into_owned()),
        picture: path("photo.png"),
        attachment: path("budget.xlsx"),
        clipboard: clipboard.and_then(path),
        opened: opened.clone(),
    };
    let shell = AppShell::empty().with_picker(Box::new(fake));
    let mut harness = egui_kittest::Harness::builder()
        .with_size(WINDOW)
        .build_ui_state(
            |ui, shell: &mut AppShell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
            },
            shell,
        );
    harness.run();
    harness.get_by_label("Open Notebook").click();
    harness.run();
    Setup {
        dir,
        harness,
        opened,
    }
}

fn page_text(s: &Setup) -> String {
    s.harness
        .state()
        .session()
        .current_page_source()
        .map(|p| p.text.to_owned())
        .unwrap_or_default()
}

fn dispatch(s: &mut Setup, id: &str, args: Value) {
    s.harness.state_mut().dispatch(id, args);
    s.harness.run_steps(6);
}

#[test]
fn insert_picture_copies_links_renders_and_undoes() {
    let mut s = setup(None);
    dispatch(&mut s, "insert.picture", Value::Null);
    let text = page_text(&s);
    assert!(text.contains("![photo](attachments/"), "{text}");
    assert!(s.dir.path().join("Notes/attachments").is_dir());
    assert_eq!(s.harness.state().picture_counts(), (1, 0));
    dispatch(&mut s, "edit.undo", Value::Null);
    assert_eq!(page_text(&s), "# First\n\nHello.\n");
    dispatch(&mut s, "edit.redo", Value::Null);
    assert!(page_text(&s).contains("![photo]("));
    assert_eq!(s.harness.state().picture_counts(), (1, 0));
}

#[test]
fn a_corrupt_picture_shows_a_placeholder_not_a_crash() {
    let mut s = setup(None);
    let broken = s
        .dir
        .path()
        .join("broken.png")
        .to_string_lossy()
        .into_owned();
    let mut args = notecraft_engine::cmd::Map::new();
    args.insert("path".to_owned(), Value::from(broken));
    dispatch(&mut s, "insert.picture", Value::Object(args));
    assert!(page_text(&s).contains("![broken]("));
    assert_eq!(s.harness.state().picture_counts(), (0, 1));
    // A missing file behind a link is a placeholder too (the broken one stays cached).
    let missing = "# T\n\n![gone](attachments/none.png)\n\n![web](https://example.org/a.png)\n";
    let id = s
        .harness
        .state()
        .session()
        .selection()
        .page
        .clone()
        .unwrap();
    s.harness.state_mut().set_page_source(&id, missing);
    s.harness.run_steps(6);
    assert_eq!(s.harness.state().picture_counts(), (0, 2));
}

#[test]
fn files_become_attachment_objects_and_open_in_the_default_app() {
    let mut s = setup(None);
    dispatch(&mut s, "insert.file_attachment", Value::Null);
    let text = page_text(&s);
    assert!(text.contains("[budget.xlsx](attachments/"), "{text}");
    // The object is on the canvas as a button named for the file.
    s.harness.get_by_label("Open budget.xlsx").click();
    s.harness.run_steps(6);
    let opened = s.opened.lock().unwrap().clone();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(opened[0].contains("attachments") && opened[0].ends_with("budget.xlsx"));
}

#[test]
fn dropped_files_insert_pictures_and_attachments() {
    let mut s = setup(None);
    for name in ["photo.png", "budget.xlsx"] {
        let file = Dropped(s.dir.path().join(name));
        s.harness.input_mut().dropped_files.push(Arc::new(file));
        s.harness.run_steps(6);
    }
    let text = page_text(&s);
    assert!(text.contains("![photo](attachments/"), "{text}");
    assert!(text.contains("[budget.xlsx](attachments/"), "{text}");
    // Two drops, two undo steps.
    dispatch(&mut s, "edit.undo", Value::Null);
    dispatch(&mut s, "edit.undo", Value::Null);
    assert_eq!(page_text(&s), "# First\n\nHello.\n");
}

#[test]
fn paste_as_picture_uses_the_clipboard_picture_and_removes_the_temp_file() {
    let mut s = setup(Some("photo.png"));
    dispatch(&mut s, "edit.paste_picture", Value::Null);
    assert!(page_text(&s).contains("![photo](attachments/"));
    // With no picture on the clipboard nothing changes and the user is told.
    let mut s = setup(None);
    dispatch(&mut s, "edit.paste_picture", Value::Null);
    assert_eq!(page_text(&s), "# First\n\nHello.\n");
    assert!(s
        .harness
        .state()
        .toasts()
        .texts()
        .any(|t| t.contains("no picture on the clipboard")));
}

fn key_v_released(modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::V,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers,
    }
}

#[test]
fn ctrl_v_with_a_picture_and_no_text_inserts_the_picture() {
    let mut s = setup(Some("photo.png"));
    s.harness.event(key_v_released(egui::Modifiers::COMMAND));
    s.harness.run_steps(6);
    let text = page_text(&s);
    assert!(text.contains("![photo](attachments/"), "{text}");
    dispatch(&mut s, "edit.undo", Value::Null);
    assert_eq!(page_text(&s), "# First\n\nHello.\n");
}

#[test]
fn ctrl_v_with_text_does_not_look_for_a_picture() {
    let mut s = setup(Some("photo.png"));
    s.harness.event(egui::Event::Paste("words".to_owned()));
    s.harness.event(key_v_released(egui::Modifiers::COMMAND));
    s.harness.run_steps(6);
    assert!(!page_text(&s).contains("![photo]"));
    // Nor does a key release without the command key.
    s.harness.event(key_v_released(egui::Modifiers::NONE));
    s.harness.run_steps(6);
    assert!(!page_text(&s).contains("![photo]"));
}

#[test]
fn an_svg_picture_is_inserted_drawn_and_resized() {
    let mut s = setup(None);
    let svg = s.dir.path().join("logo.svg");
    std::fs::write(
        &svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><rect width="80" height="40" fill="#336699"/></svg>"##,
    )
    .unwrap();
    let args = serde_json::json!({ "path": svg.to_string_lossy() });
    dispatch(&mut s, "insert.picture", args);
    let text = page_text(&s);
    assert!(text.contains("![logo](attachments/"), "{text}");
    assert_eq!(s.harness.state().picture_counts(), (1, 0));
    let link = text
        .split("![logo](")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .unwrap()
        .to_owned();
    let args = serde_json::json!({ "src": link, "width": 200 });
    dispatch(&mut s, "picture.resize", args);
    assert!(page_text(&s).contains("<!-- notecraft:width=200 -->"));
    assert_eq!(s.harness.state().picture_counts(), (1, 0));
    dispatch(&mut s, "edit.undo", Value::Null);
    assert!(!page_text(&s).contains("notecraft:width"));
}
