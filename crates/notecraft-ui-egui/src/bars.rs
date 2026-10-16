//! The bar above the page for questions the user must answer (PR 1.7): unsaved text found
//! from the last session (restore or discard), and a page that changed outside NoteCraft
//! while it had unsaved changes (keep mine or use the outside version). The buttons run
//! commands (`file.restore_unsaved`, `file.discard_unsaved`, `file.keep_my_version`,
//! `file.use_outside_version`); nothing here changes the session itself.

use egui::{Align, Layout, Rect, RichText, Ui};
use notecraft_engine::cmd::{ids, CommandId, Value};
use notecraft_engine::Session;

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
}

/// The bar to show now, if any. Recovery comes first.
pub fn current(session: &Session) -> Option<Bar> {
    let pending = session.pending_recovery().len();
    if pending > 0 {
        Some(Bar::Recovery(pending))
    } else if session.current_conflict().is_some() {
        Some(Bar::Conflict)
    } else {
        None
    }
}

/// The room the bar takes: its height, or 0 with no bar.
pub fn height(session: &Session) -> f32 {
    if current(session).is_some() {
        BAR_HEIGHT
    } else {
        0.0
    }
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
    }
}

fn buttons(bar: Bar) -> [(&'static str, CommandId); 2] {
    match bar {
        Bar::Recovery(_) => [
            ("Restore", ids::FILE_RESTORE_UNSAVED),
            ("Discard", ids::FILE_DISCARD_UNSAVED),
        ],
        Bar::Conflict => [
            ("Keep my version", ids::FILE_KEEP_MY_VERSION),
            ("Use outside version", ids::FILE_USE_OUTSIDE_VERSION),
        ],
    }
}

/// Draw the bar in `rect` (as tall as [`height`]).
pub fn show(
    ui: &mut Ui,
    rect: Rect,
    session: &Session,
    tokens: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    let Some(bar) = current(session) else {
        return;
    };
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
mod tests {
    use super::*;

    #[test]
    fn no_bar_without_a_question() {
        let session = Session::new();
        assert_eq!(current(&session), None);
        assert_eq!(height(&session), 0.0);
    }

    #[test]
    fn messages_are_plain_ascii() {
        for bar in [Bar::Recovery(1), Bar::Recovery(3), Bar::Conflict] {
            assert!(message(bar).is_ascii());
        }
    }
}
