//! The What's New dialog: the notes for the running version, with older versions folded
//! below. It opens from `help.whats_new` (the Help tab, the About page) and by itself at the
//! first launch after an update (the app decides; see `notecraft::prefs`).
//!
//! Like the other dialogs it is a centered window: Esc or Close closes it, and keyboard
//! focus starts on Close so Enter or Space dismisses it.

use egui::{CollapsingHeader, Context, Id, RichText, ScrollArea};
use notecraft_engine::whats_new::{Entry, Item, ReleaseNotes};

use crate::themes::ThemeTokens;

/// Title of the dialog window, which is also what a screen reader announces.
pub const TITLE: &str = "What's New";

/// Widest the dialog text gets, in points.
const WIDTH: f32 = 520.0;
/// Tallest the notes area gets before it scrolls, in points.
const NOTES_HEIGHT: f32 = 380.0;

/// The dialog's state: whether it is open and the notes it shows.
#[derive(Debug)]
pub(crate) struct WhatsNew {
    open: bool,
    /// Set when the dialog was just opened, so the first frame moves keyboard focus to Close.
    focus_pending: bool,
    notes: ReleaseNotes,
}

impl WhatsNew {
    pub(crate) fn new() -> Self {
        Self::with_notes(ReleaseNotes::bundled())
    }

    pub(crate) fn with_notes(notes: ReleaseNotes) -> Self {
        Self {
            open: false,
            focus_pending: false,
            notes,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn open(&mut self) {
        if !self.open {
            self.open = true;
            self.focus_pending = true;
        }
    }

    /// Draw the dialog if it is open. `version` is the running app version.
    pub(crate) fn show(&mut self, ctx: &Context, t: &ThemeTokens, version: &str) {
        if !self.open {
            return;
        }
        let mut open = true;
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let focus = std::mem::take(&mut self.focus_pending);
        let has_current = self.notes.entry(version).is_some();
        egui::Window::new(TITLE)
            .id(Id::new("whats_new_dialog"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(WIDTH);
                ui.label(
                    RichText::new(format!("NoteCraft {}", version.trim()))
                        .size(20.0)
                        .color(t.text),
                );
                ui.add_space(6.0);
                let output = ScrollArea::vertical()
                    .id_salt("whats_new_scroll")
                    .max_height(NOTES_HEIGHT)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if self.notes.is_empty() {
                            ui.label(
                                RichText::new("No release notes are available.").color(t.text),
                            );
                        }
                        for (index, entry) in self.notes.entries().iter().enumerate() {
                            let current = entry.version == version.trim();
                            let open_now = current || (index == 0 && !has_current);
                            entry_ui(ui, entry, open_now, current, t);
                        }
                    });
                crate::a11y::name_scroll_bars(ui.ctx(), output.id);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let button = egui::Button::new(RichText::new("Close").color(t.accent_text))
                            .fill(t.accent)
                            .min_size(egui::vec2(88.0, 28.0));
                        let response = ui.add(button);
                        if focus {
                            response.request_focus();
                        }
                        if response.clicked() {
                            close = true;
                        }
                    });
                });
            });
        if close || !open {
            self.open = false;
        }
    }
}

fn entry_ui(ui: &mut egui::Ui, entry: &Entry, open: bool, current: bool, t: &ThemeTokens) {
    let title = if current {
        format!("Version {} (this version)", entry.version)
    } else {
        format!("Version {}", entry.version)
    };
    CollapsingHeader::new(RichText::new(title).size(16.0).color(t.text))
        .id_salt(("whats_new_entry", entry.version.as_str()))
        .default_open(open)
        .show(ui, |ui| {
            for item in &entry.items {
                match item {
                    Item::Heading(text) => {
                        ui.add_space(6.0);
                        ui.label(RichText::new(text).strong().color(t.text));
                    }
                    Item::Bullet(text) => {
                        ui.horizontal_top(|ui| {
                            ui.add_space(8.0);
                            ui.label(RichText::new("-").color(t.text_muted));
                            ui.add(egui::Label::new(RichText::new(text).color(t.text)).wrap());
                        });
                    }
                    Item::Text(text) => {
                        ui.add(egui::Label::new(RichText::new(text).color(t.text)).wrap());
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppInfo, AppShell};
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable;
    use notecraft_engine::cmd::Status;

    fn harness(shell: AppShell) -> egui_kittest::Harness<'static, AppShell> {
        egui_kittest::Harness::builder()
            .with_size(egui::vec2(1280.0, 900.0))
            .build_ui_state(
                |ui, shell: &mut AppShell| {
                    shell.logic(ui.ctx());
                    shell.ui(ui);
                },
                shell,
            )
    }

    fn versioned(version: &str) -> AppShell {
        AppShell::new().with_about(AppInfo {
            version: version.to_owned(),
            ..AppInfo::default()
        })
    }

    #[test]
    fn the_dialog_is_closed_until_asked_for() {
        let mut harness = harness(AppShell::new());
        harness.run();
        assert!(!harness.state().whats_new_open());
        assert!(harness
            .query_by_label("No release notes are available.")
            .is_none());
        assert!(harness.query_by_label(TITLE).is_none());
    }

    #[test]
    fn help_whats_new_is_a_ready_command_that_opens_the_dialog() {
        let mut shell = versioned("0.0.2");
        let spec = shell.registry().get("help.whats_new");
        assert_eq!(spec.map(|spec| spec.status), Some(Status::Ready));
        assert!(shell.dispatch("help.whats_new", serde_json::Value::Null));
        assert!(shell.toasts().is_empty(), "no Coming Soon toast");
        assert!(shell.whats_new_open());
        let mut harness = harness(shell);
        harness.run();
        let _ = harness.get_by_label("Version 0.0.2 (this version)");
        let _ = harness.get_by_label("NoteCraft 0.0.2");
        // The older version is folded away.
        assert!(harness
            .query_by_label_contains("The first preview")
            .is_none());
    }

    #[test]
    fn older_versions_open_when_their_header_is_clicked() {
        // With 0.0.1 as the running version, 0.0.2 sits folded at the top.
        let mut shell = versioned("0.0.1");
        shell.open_whats_new();
        let mut harness = harness(shell);
        harness.run();
        assert!(harness
            .query_by_label_contains("opens and saves real notebooks")
            .is_none());
        harness.get_by_label("Version 0.0.2").click();
        harness.run();
        let _ = harness.get_by_label_contains("opens and saves real notebooks");
    }

    #[test]
    fn the_help_tab_button_opens_it() {
        let mut harness = harness(AppShell::new());
        harness.run();
        harness.get_by_label("Help").click();
        harness.run();
        harness
            .get_by_role_and_label(Role::Button, "What's New")
            .click();
        harness.run();
        assert!(harness.state().whats_new_open());
        assert!(harness.state().toasts().is_empty());
    }

    #[test]
    fn escape_and_close_dismiss_it_and_close_has_focus() {
        let mut shell = AppShell::new();
        shell.open_whats_new();
        let mut harness = harness(shell);
        harness.run();
        assert!(harness.state().whats_new_open());
        assert!(harness
            .get_by_role_and_label(Role::Button, "Close")
            .is_focused());
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(!harness.state().whats_new_open());

        harness.state_mut().open_whats_new();
        harness.run();
        harness.get_by_role_and_label(Role::Button, "Close").click();
        harness.run();
        assert!(!harness.state().whats_new_open());
    }

    #[test]
    fn it_opens_over_the_start_page_and_the_about_page() {
        let mut shell = AppShell::empty();
        shell.open_whats_new();
        let mut harness = harness(shell);
        harness.run();
        let _ = harness.get_by_role_and_label(Role::Button, "Close");

        let mut shell = AppShell::new();
        assert!(shell.dispatch("file.about", serde_json::Value::Null));
        let mut harness = self::harness(shell);
        harness.run();
        harness
            .get_by_role_and_label(Role::Button, "What's New")
            .click();
        harness.run();
        assert!(harness.state().whats_new_open());
    }

    #[test]
    fn a_version_without_notes_shows_the_newest_open() {
        let mut shell = versioned("9.9.9");
        shell.open_whats_new();
        let mut harness = harness(shell);
        harness.run();
        let _ = harness.get_by_label("NoteCraft 9.9.9");
        let _ = harness.get_by_label("Version 0.0.2");
    }

    #[test]
    fn empty_notes_say_so() {
        let mut dialog = WhatsNew::with_notes(ReleaseNotes::default());
        dialog.open();
        assert!(dialog.is_open());
        let tokens = ThemeTokens::for_variant(crate::themes::Preset::default(), false);
        let mut harness = egui_kittest::Harness::new_ui_state(
            |ui, dialog: &mut WhatsNew| dialog.show(ui.ctx(), &tokens, "1.0.0"),
            dialog,
        );
        harness.run();
        let _ = harness.get_by_label("No release notes are available.");
        assert!(harness.state().is_open());
    }
}
