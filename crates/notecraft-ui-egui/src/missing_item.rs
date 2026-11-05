//! The "not found" dialog: shown when a recent notebook or page no longer exists on disk. It
//! says so and offers to remove the entry from the Recent list (`file.recent.remove`); the
//! app never shows sample notebooks in its place. Esc or Keep closes it.

use egui::{Context, Id, RichText};
use notecraft_engine::cmd::{ids, Map, Value};
use notecraft_engine::recent::{RecentItem, RecentKind};

use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// Title of the dialog window, which is also what a screen reader announces.
pub const TITLE: &str = "Not Found";

const WIDTH: f32 = 440.0;

/// The dialog's state: the missing recent entry it is about, while it is open.
#[derive(Debug, Default)]
pub(crate) struct MissingItem {
    item: Option<RecentItem>,
    /// Set when the dialog was just opened, so the first frame moves keyboard focus to the
    /// default button.
    focus_pending: bool,
}

impl MissingItem {
    pub(crate) fn open(&mut self, item: RecentItem) {
        self.item = Some(item);
        self.focus_pending = true;
    }

    /// The sentence the dialog shows for `item`.
    pub(crate) fn message(item: &RecentItem) -> String {
        let what = match item.kind {
            RecentKind::Notebook => "notebook folder",
            RecentKind::Page => "page file",
        };
        format!(
            "The {what} \"{}\" was not found. It may have been moved, renamed, or deleted.",
            item.name()
        )
    }

    /// Draw the dialog if it is open. Remove from Recent pushes `file.recent.remove` onto `out`.
    pub(crate) fn show(&mut self, ctx: &Context, t: &ThemeTokens, out: &mut Vec<Clicked>) {
        let Some(item) = self.item.clone() else {
            return;
        };
        let mut open = true;
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let focus = std::mem::take(&mut self.focus_pending);
        egui::Window::new(TITLE)
            .id(Id::new("missing_item_dialog"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                ui.add(egui::Label::new(RichText::new(Self::message(&item)).color(t.text)).wrap());
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(RichText::new(&item.path).color(t.text_muted).size(12.0))
                        .wrap(),
                );
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let keep = ui.add(egui::Button::new("Keep").min_size(egui::vec2(88.0, 28.0)));
                    if keep.clicked() {
                        close = true;
                    }
                    let remove = ui.add(
                        egui::Button::new(RichText::new("Remove from Recent").color(t.accent_text))
                            .fill(t.accent)
                            .min_size(egui::vec2(88.0, 28.0)),
                    );
                    if focus {
                        remove.request_focus();
                    }
                    if remove.clicked() {
                        let mut args = Map::new();
                        args.insert("path".to_owned(), Value::from(item.path.as_str()));
                        args.insert("kind".to_owned(), Value::from(item.kind.id()));
                        out.push(Clicked {
                            command: ids::FILE_RECENT_REMOVE,
                            args: Value::Object(args),
                        });
                        close = true;
                    }
                });
            });
        if close || !open {
            self.item = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_names_the_item_and_says_it_is_missing() {
        let page = RecentItem::new(RecentKind::Page, "/n/Plan.md").unwrap();
        let text = MissingItem::message(&page);
        assert!(
            text.contains("page file \"Plan.md\" was not found"),
            "{text}"
        );
        let notebook = RecentItem::new(RecentKind::Notebook, "/n/Work").unwrap();
        assert!(MissingItem::message(&notebook).contains("notebook folder \"Work\""));
    }
}
