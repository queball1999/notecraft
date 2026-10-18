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

// Pictures and files with a place of their own (docs/markdown.md section 3.4).

fn current_page(s: &Setup) -> String {
    s.harness
        .state()
        .session()
        .selection()
        .page
        .clone()
        .unwrap()
}

fn objects(s: &Setup) -> Vec<notecraft_model::PlacedObject> {
    let id = current_page(s);
    s.harness.state().session().page_objects(&id).to_vec()
}

fn canvas_origin(s: &Setup) -> egui::Pos2 {
    s.harness.get_by_label("Canvas").rect().min
}

/// A click at a page point (zoom 100%, scrolled to the top).
fn click_page(s: &mut Setup, x: f32, y: f32) {
    let at = canvas_origin(s) + egui::vec2(x, y);
    s.harness.hover_at(at);
    s.harness.step();
    s.harness.drag_at(at);
    s.harness.step();
    s.harness.drop_at(at);
    s.harness.step();
    s.harness.step();
}

/// Drag from one screen point to another in small steps, one command at the end.
fn drag_between(s: &mut Setup, from: egui::Pos2, by: egui::Vec2) {
    s.harness.hover_at(from);
    s.harness.step();
    s.harness.drag_at(from);
    s.harness.step();
    for i in 1..=10 {
        s.harness.hover_at(from + by * (i as f32 / 10.0));
        s.harness.step();
    }
    s.harness.drop_at(from + by);
    s.harness.step();
    s.harness.step();
}

fn near(a: notecraft_core::Vec2, x: f32, y: f32) -> bool {
    (a.x - x).abs() <= 1.5 && (a.y - y).abs() <= 1.5
}

#[test]
fn insert_picture_lands_where_the_page_was_clicked() {
    let mut s = setup(None);
    click_page(&mut s, 700.0, 400.0);
    dispatch(&mut s, "insert.picture", Value::Null);
    let placed = objects(&s);
    assert_eq!(placed.len(), 1, "{placed:?}");
    assert!(near(placed[0].pos, 700.0, 400.0), "{:?}", placed[0].pos);
    assert_eq!(s.harness.state().picture_counts(), (1, 0));
    // The picture is drawn at its place: a button named for it, 64 by 48 points.
    let rect = s.harness.get_by_label("Picture photo").rect();
    assert!((rect.width() - 64.0).abs() < 1.0 && (rect.height() - 48.0).abs() < 1.0);
    let at = s.harness.state().screen_point_of(placed[0].pos).unwrap();
    assert!((rect.min.x - at.x).abs() < 1.5 && (rect.min.y - at.y).abs() < 1.5);
    // A second insert does not cover the first.
    dispatch(&mut s, "insert.file_attachment", Value::Null);
    let placed = objects(&s);
    assert_eq!(placed.len(), 2);
    assert!(near(placed[1].pos, 724.0, 424.0), "{:?}", placed[1].pos);
    // One undo per insert.
    dispatch(&mut s, "edit.undo", Value::Null);
    dispatch(&mut s, "edit.undo", Value::Null);
    assert!(objects(&s).is_empty());
    assert_eq!(page_text(&s), "# First\n\nHello.\n");
}

#[test]
fn a_click_inside_text_puts_the_picture_below_that_note() {
    let mut s = setup(None);
    click_page(&mut s, 100.0, 140.0);
    dispatch(&mut s, "insert.picture", Value::Null);
    let placed = objects(&s);
    assert_eq!(placed.len(), 1);
    // Left edge of the note, under its text.
    assert!((placed[0].pos.x - 48.0).abs() < 1.5, "{:?}", placed[0].pos);
    assert!(placed[0].pos.y > 140.0, "{:?}", placed[0].pos);
}

#[test]
fn with_no_click_a_picture_flows_with_the_text() {
    let mut s = setup(None);
    dispatch(&mut s, "insert.picture", Value::Null);
    assert!(objects(&s).is_empty());
    assert!(page_text(&s).contains("![photo](attachments/"));
}

#[test]
fn dragging_a_picture_moves_it_in_one_undo_step() {
    let mut s = setup(None);
    let args = serde_json::json!({
        "path": s.dir.path().join("photo.png").to_string_lossy(),
        "x": 600, "y": 300
    });
    dispatch(&mut s, "insert.picture", args);
    let before_text = page_text(&s);
    let rect = s.harness.get_by_label("Picture photo").rect();
    drag_between(&mut s, rect.center(), egui::vec2(-120.0, 90.0));
    let placed = objects(&s);
    assert_eq!(placed.len(), 1);
    assert!(near(placed[0].pos, 480.0, 390.0), "{:?}", placed[0].pos);
    assert_eq!(page_text(&s), before_text, "the text is not touched");
    // The whole drag is one step.
    let page = current_page(&s);
    assert_eq!(
        s.harness.state().session().history().undo_label(Some(&page)),
        Some("Move picture")
    );
    dispatch(&mut s, "edit.undo", Value::Null);
    assert!(near(objects(&s)[0].pos, 600.0, 300.0));
    dispatch(&mut s, "edit.redo", Value::Null);
    assert!(near(objects(&s)[0].pos, 480.0, 390.0));
}

#[test]
fn dragging_a_picture_out_of_its_text_gives_it_a_place() {
    let mut s = setup(None);
    dispatch(&mut s, "insert.picture", Value::Null);
    assert!(objects(&s).is_empty());
    let text = page_text(&s);
    let rect = s.harness.get_by_label("Picture photo").rect();
    drag_between(&mut s, rect.center(), egui::vec2(500.0, 100.0));
    let placed = objects(&s);
    assert_eq!(placed.len(), 1, "{placed:?}");
    assert!(placed[0].pos.x > 500.0);
    assert_eq!(page_text(&s), text);
    // It no longer takes room in the text, and it follows the pointer's place.
    let moved = s.harness.get_by_label("Picture photo").rect();
    assert!(moved.min.x > rect.min.x + 400.0);
}

#[test]
fn a_file_can_be_dragged_and_a_click_still_opens_it() {
    let mut s = setup(None);
    let args = serde_json::json!({
        "path": s.dir.path().join("budget.xlsx").to_string_lossy(),
        "x": 300, "y": 300
    });
    dispatch(&mut s, "insert.file_attachment", args);
    let rect = s.harness.get_by_label("Open budget.xlsx").rect();
    drag_between(&mut s, rect.center(), egui::vec2(40.0, 150.0));
    let placed = objects(&s);
    assert_eq!(placed.len(), 1);
    assert!(placed[0].kind == notecraft_model::ObjectKind::File);
    assert!(near(placed[0].pos, 340.0, 450.0), "{:?}", placed[0].pos);
    // Dragging did not open it.
    assert!(s.opened.lock().unwrap().is_empty());
    let center = s.harness.get_by_label("Open budget.xlsx").rect().center();
    s.harness.hover_at(center);
    s.harness.step();
    s.harness.drag_at(center);
    s.harness.step();
    s.harness.drop_at(center);
    s.harness.run_steps(4);
    let toasts: Vec<String> = s.harness.state().toasts().texts().map(str::to_owned).collect();
    assert_eq!(s.opened.lock().unwrap().len(), 1, "{center:?} {toasts:?}");
    assert_eq!(objects(&s).len(), 1);
    assert!(near(objects(&s)[0].pos, 340.0, 450.0));
}

#[test]
fn dropped_files_land_at_the_drop_point() {
    let mut s = setup(None);
    let at = canvas_origin(&s) + egui::vec2(700.0, 350.0);
    s.harness.hover_at(at);
    s.harness.step();
    let file = Dropped(s.dir.path().join("photo.png"));
    s.harness.input_mut().dropped_files.push(Arc::new(file));
    s.harness.run_steps(6);
    let placed = objects(&s);
    assert_eq!(placed.len(), 1, "{placed:?}");
    assert!(near(placed[0].pos, 700.0, 350.0), "{:?}", placed[0].pos);
}
