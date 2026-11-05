//! The status bar under the page: which page of the section is shown, and the zoom, which
//! opens a zoom menu when clicked. The save state (Saved or Unsaved changes) sits beside it (PR 1.7).

use egui::{Align, Layout, RichText, Ui};
use notecraft_engine::cmd::{ids, CommandId, Value};
use notecraft_engine::Session;

use crate::ribbon::spec::{self, ZOOM_LEVELS};
use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// The zoom menu's step entries, above the fixed levels.
const ZOOM_STEPS: [(&str, CommandId); 4] = [
    ("Zoom In", ids::VIEW_ZOOM_IN),
    ("Zoom Out", ids::VIEW_ZOOM_OUT),
    ("100%", ids::VIEW_ZOOM_ACTUAL),
    ("Page Width", ids::VIEW_ZOOM_PAGE_WIDTH),
];

/// "Page 3 of 5" for the page shown, or `None` with no page shown.
pub fn page_position(session: &Session) -> Option<String> {
    let page = session.selection().page.as_deref()?;
    let pages = &session.current_section()?.pages;
    let index = pages.iter().position(|p| p.id == page)?;
    Some(format!("Page {} of {}", index + 1, pages.len()))
}

/// Longest save failure reason shown in the status bar, in characters.
const MAX_REASON: usize = 60;

/// The id of the page shown, for asking the IO layer about its save error.
pub fn shown_page(session: &Session) -> Option<&str> {
    session.selection().page.as_deref()
}

/// The save state text: nothing for a sample page, else "Cannot save: <reason>" when the
/// last save of the shown page failed (`save_error`), a conflict notice for a page changed
/// outside NoteCraft, "Unsaved changes", or "Saved".
pub fn save_label(session: &Session, save_error: Option<&str>) -> Option<String> {
    let page = shown_page(session)?;
    session.page_file_path(page)?;
    if let Some(reason) = save_error {
        let mut short: String = reason.chars().take(MAX_REASON).collect();
        if reason.chars().count() > MAX_REASON {
            short.push_str("...");
        }
        return Some(format!("Cannot save: {short}"));
    }
    if session.current_conflict().is_some() {
        return Some("Not saved: changed outside NoteCraft".to_owned());
    }
    let state = session.save_state();
    Some(
        if state.conflicts > 0 || state.unsaved > 0 {
            "Unsaved changes"
        } else {
            "Saved"
        }
        .to_owned(),
    )
}

/// The zoom button's text, for example "Zoom 100%".
pub fn zoom_label(session: &Session) -> String {
    format!("Zoom {}%", session.view().zoom_percent)
}

/// Draw the status bar. Nothing shows while no notebook is open.
/// `save_error` is why the last save of the shown page failed, if it did.
pub fn show(
    ui: &mut Ui,
    session: &Session,
    save_error: Option<&str>,
    tokens: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    if session.notebooks().is_empty() {
        return;
    }
    ui.horizontal_centered(|ui| {
        ui.add_space(8.0);
        if let Some(position) = page_position(session) {
            ui.label(RichText::new(position).size(12.0).color(tokens.ribbon_text));
        }
        if let Some(saved) = save_label(session, save_error) {
            ui.add_space(16.0);
            ui.label(RichText::new(saved).size(12.0).color(tokens.ribbon_text));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(8.0);
            let label = RichText::new(zoom_label(session))
                .size(12.0)
                .color(tokens.ribbon_text);
            ui.menu_button(label, |ui| {
                for (label, command) in ZOOM_STEPS {
                    if ui.button(label).clicked() {
                        out.push(Clicked {
                            command,
                            args: Value::Null,
                        });
                        ui.close();
                    }
                }
                ui.separator();
                for item in ZOOM_LEVELS {
                    if ui.button(item.label).clicked() {
                        out.push(Clicked {
                            command: item.command,
                            args: spec::args_value(item.args),
                        });
                        ui.close();
                    }
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_position_counts_within_the_section() {
        let mut session = Session::new();
        assert_eq!(page_position(&session), None);
        session.set_notebooks(notecraft_engine::mock::notebooks());
        let first = page_position(&session).unwrap_or_default();
        assert!(first.starts_with("Page 1 of "), "{first}");
        assert_eq!(zoom_label(&session), "Zoom 100%");
    }

    #[test]
    fn a_sample_page_has_no_save_label() {
        let mut session = Session::new();
        session.set_notebooks(notecraft_engine::mock::notebooks());
        assert_eq!(save_label(&session, Some("disk full")), None);
    }

    #[test]
    fn the_save_label_names_a_failure_a_conflict_or_changes() {
        let (_dir, mut session) = crate::bars::tests::conflict_and_recovery();
        assert_eq!(
            save_label(&session, Some("disk full")).as_deref(),
            Some("Cannot save: disk full"),
            "a failure wins"
        );
        let long = "x".repeat(200);
        let label = save_label(&session, Some(&long)).unwrap_or_default();
        assert!(label.ends_with("..."));
        assert!(label.len() < 90, "{label}");
        let label = save_label(&session, None).unwrap_or_default();
        assert!(label.contains("changed outside NoteCraft"), "{label}");
        assert!(label.is_ascii());
        // Settle the conflict by keeping the outside text; the label no longer names one.
        assert!(session.resolve_current_conflict(false));
        assert_eq!(save_label(&session, None).as_deref(), Some("Saved"));
    }

    #[test]
    fn a_dirty_page_without_trouble_says_unsaved_changes() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("Sec")).unwrap();
        std::fs::write(dir.path().join("Sec/A.md"), "one\n").unwrap();
        let mut session = Session::new();
        let opened = notecraft_engine::open::open_folder(dir.path()).unwrap();
        let id = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&id, dir.path());
        session.load_current_page();
        assert_eq!(save_label(&session, None).as_deref(), Some("Saved"));
        let page = session.selection().page.clone().unwrap();
        session.edit_page_text(&page, 0, 0, "x", false);
        assert_eq!(
            save_label(&session, None).as_deref(),
            Some("Unsaved changes")
        );
    }
}
