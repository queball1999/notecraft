//! The bar above the page for questions the user must answer (PR 1.7): unsaved text found
//! from the last session (restore or discard), and a page that changed outside NoteCraft
//! while it had unsaved changes (keep mine or use the outside version). The buttons run
//! commands (`file.restore_unsaved`, `file.discard_unsaved`, `file.keep_my_version`,
//! `file.use_outside_version`); nothing here changes the session itself.

use egui::{Align, Layout, Rect, RichText, Ui};
use notecraft_engine::cmd::{ids, CommandId, Value};
use notecraft_engine::{PageEncoding, Session};

use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// Height of the bar when it shows.
pub const BAR_HEIGHT: f32 = 34.0;

/// What the bar asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    /// Unsaved text from the last session: how many pages.
    Recovery(usize),
    /// The page shown changed outside NoteCraft.
    Conflict,
    /// The ink and layout data of the page shown did not parse; the page is read-only until a
    /// fresh block replaces it (`page.repair_canvas`).
    CanvasUnreadable,
    /// The page file is not UTF-8 text; it is read-only until converted (`page.convert_encoding`).
    NotUtf8(PageEncoding),
}

/// The bars to show now, top to bottom: the global recovery question first, then the shown
/// page's own questions. A recovery question never hides a conflict.
pub fn all(session: &Session) -> Vec<Bar> {
    let mut bars = Vec::new();
    let pending = session.pending_recovery().len();
    if pending > 0 {
        bars.push(Bar::Recovery(pending));
    }
    if session.current_conflict().is_some() {
        bars.push(Bar::Conflict);
    }
    if let Some(encoding) = session.current_page_legacy_encoding() {
        bars.push(Bar::NotUtf8(encoding));
    }
    if session.can_repair_canvas() {
        bars.push(Bar::CanvasUnreadable);
    }
    bars
}

/// The room the bars take: a row each, or 0 with none.
pub fn height(session: &Session) -> f32 {
    all(session).len() as f32 * BAR_HEIGHT
}

/// The bar's message.
pub fn message(bar: Bar) -> String {
    match bar {
        Bar::Recovery(1) => {
            "NoteCraft found unsaved changes to 1 page from the last session.".to_owned()
        }
        Bar::Recovery(n) => {
            format!("NoteCraft found unsaved changes to {n} pages from the last session.")
        }
        Bar::Conflict => {
            "This page was changed outside NoteCraft. Your version is still open.".to_owned()
        }
        Bar::NotUtf8(encoding) => format!(
            "This page is not saved as UTF-8 text. NoteCraft read it as {} and opened it read-only.",
            encoding.label()
        ),
        Bar::CanvasUnreadable => {
            "The ink and layout of this page could not be read, so it is read-only.".to_owned()
        }
    }
}

fn buttons(bar: Bar) -> Vec<(&'static str, CommandId)> {
    match bar {
        Bar::CanvasUnreadable => vec![("Repair", ids::PAGE_REPAIR_CANVAS)],
        Bar::NotUtf8(_) => vec![("Convert to UTF-8", ids::PAGE_CONVERT_ENCODING)],
        Bar::Recovery(_) => vec![
            ("Restore", ids::FILE_RESTORE_UNSAVED),
            ("Discard", ids::FILE_DISCARD_UNSAVED),
        ],
        Bar::Conflict => vec![
            ("Keep my version", ids::FILE_KEEP_MY_VERSION),
            ("Use outside version", ids::FILE_USE_OUTSIDE_VERSION),
        ],
    }
}

/// Draw the bars in `rect` (as tall as [`height`]), one row each.
pub fn show(
    ui: &mut Ui,
    rect: Rect,
    session: &Session,
    tokens: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    for (row, bar) in all(session).into_iter().enumerate() {
        let top = rect.min.y + row as f32 * BAR_HEIGHT;
        let row_rect = Rect::from_min_size(
            egui::pos2(rect.min.x, top),
            egui::vec2(rect.width(), BAR_HEIGHT),
        );
        show_one(ui, row_rect, bar, tokens, out);
    }
}

fn show_one(ui: &mut Ui, rect: Rect, bar: Bar, tokens: &ThemeTokens, out: &mut Vec<Clicked>) {
    ui.painter().rect_filled(rect, 0.0, tokens.badge_bg);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::new(message(bar))
                    .size(13.0)
                    .color(tokens.badge_text),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(10.0);
                // Right to left, so the last entry draws leftmost.
                for (label, command) in buttons(bar).into_iter().rev() {
                    if ui.button(label).clicked() {
                        out.push(Clicked {
                            command,
                            args: Value::Null,
                        });
                    }
                }
            });
        });
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn no_bar_without_a_question() {
        let session = Session::new();
        assert!(all(&session).is_empty());
        assert_eq!(height(&session), 0.0);
    }

    #[test]
    fn messages_are_plain_ascii() {
        for bar in [
            Bar::Recovery(1),
            Bar::Recovery(3),
            Bar::Conflict,
            Bar::CanvasUnreadable,
            Bar::NotUtf8(PageEncoding::Windows1252),
            Bar::NotUtf8(PageEncoding::Utf16Le),
        ] {
            assert!(message(bar).is_ascii());
        }
    }

    /// A session whose page `Sec/A.md` has a conflict, plus unsaved text offered from the
    /// last session.
    pub(crate) fn conflict_and_recovery() -> (tempfile::TempDir, Session) {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("Sec")).unwrap();
        let file = dir.path().join("Sec/A.md");
        std::fs::write(&file, "one\n").unwrap();
        let mut session = Session::new();
        let opened = notecraft_engine::open::open_folder(dir.path()).unwrap();
        let id = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&id, dir.path());
        session.load_current_page();
        let page = session.selection().page.clone().unwrap();
        session.edit_page_text(&page, 0, 0, "mine ", false);
        std::fs::write(&file, "theirs\n").unwrap();
        session.save_page(&page);
        assert!(session.current_conflict().is_some());
        session.offer_recovery(vec![notecraft_engine::RecoveryEntry {
            path: file,
            text: "from the last session\n".to_owned(),
        }]);
        assert_eq!(session.pending_recovery().len(), 1);
        (dir, session)
    }

    #[test]
    fn recovery_and_conflict_bars_stack() {
        let (_dir, session) = conflict_and_recovery();
        assert_eq!(all(&session), vec![Bar::Recovery(1), Bar::Conflict]);
        assert_eq!(height(&session), 2.0 * BAR_HEIGHT);
    }

    #[test]
    fn both_bars_are_drawn_with_their_buttons() {
        use egui_kittest::kittest::Queryable;
        let (_dir, session) = conflict_and_recovery();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(900.0, 200.0))
            .build_ui_state(
                |ui, session: &mut Session| {
                    let tokens = ThemeTokens::for_variant(session.view().theme_preset, false);
                    let rect = Rect::from_min_size(
                        ui.max_rect().min,
                        egui::vec2(ui.max_rect().width(), height(session)),
                    );
                    show(ui, rect, session, &tokens, &mut Vec::new());
                },
                session,
            );
        harness.run();
        let _ = harness.get_by_label(&message(Bar::Recovery(1)));
        let _ = harness.get_by_label(&message(Bar::Conflict));
        let _ = harness.get_by_label("Restore");
        let _ = harness.get_by_label("Keep my version");
    }

    #[test]
    fn an_unreadable_canvas_block_shows_the_repair_bar() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("Sec")).unwrap();
        let text = "Words

```notecraft-canvas
{ \"ink\": [ 
```
";
        std::fs::write(dir.path().join("Sec/A.notecraft"), text).unwrap();
        let mut session = Session::new();
        let opened = notecraft_engine::open::open_folder(dir.path()).unwrap();
        let id = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&id, dir.path());
        session.load_current_page();
        assert_eq!(all(&session), vec![Bar::CanvasUnreadable]);
        assert!(height(&session) > 0.0);
        let labels = buttons(Bar::CanvasUnreadable);
        assert_eq!(labels, vec![("Repair", ids::PAGE_REPAIR_CANVAS)]);
    }

    #[test]
    fn a_non_utf8_page_shows_the_convert_bar() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("Sec")).unwrap();
        std::fs::write(dir.path().join("Sec/A.md"), b"caf\xe9\n").unwrap();
        let mut session = Session::new();
        let opened = notecraft_engine::open::open_folder(dir.path()).unwrap();
        let id = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&id, dir.path());
        session.load_current_page();
        assert_eq!(all(&session), vec![Bar::NotUtf8(PageEncoding::Windows1252)]);
        assert!(message(Bar::NotUtf8(PageEncoding::Windows1252)).contains("Windows-1252"));
        assert_eq!(
            buttons(Bar::NotUtf8(PageEncoding::Windows1252)),
            vec![("Convert to UTF-8", ids::PAGE_CONVERT_ENCODING)]
        );
    }
}
