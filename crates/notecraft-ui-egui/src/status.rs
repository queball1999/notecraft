//! The status bar under the page: which page of the section is shown, and the zoom, which
//! opens a zoom menu when clicked. Save state joins when auto-save lands (Phase 1).

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

/// The zoom button's text, for example "Zoom 100%".
pub fn zoom_label(session: &Session) -> String {
    format!("Zoom {}%", session.view().zoom_percent)
}

/// Draw the status bar. Nothing shows while no notebook is open.
pub fn show(ui: &mut Ui, session: &Session, tokens: &ThemeTokens, out: &mut Vec<Clicked>) {
    if session.notebooks().is_empty() {
        return;
    }
    ui.horizontal_centered(|ui| {
        ui.add_space(8.0);
        if let Some(position) = page_position(session) {
            ui.label(RichText::new(position).size(12.0).color(tokens.ribbon_text));
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
}
