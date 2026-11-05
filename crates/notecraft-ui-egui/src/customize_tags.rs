//! The Customize Tags dialog (PR 2.5, extended in 2.7): the tag gallery as a list, and for the
//! tag picked its display name, icon, font color, and highlight color, as in OneNote's dialog.
//! Every tag, default or custom, can be modified (a default tag keeps its slug, so pages do not
//! change). Move Up and Move Down order the list, which sets Ctrl+1 to Ctrl+9. Delete takes a
//! custom tag away, or hides a default one (text already tagged keeps its icon); Reset gives a
//! modified default tag its built-in look; Reset Tags (after a confirm) puts the whole built-in
//! list back. The buttons run the `tag.custom_add`, `tag.edit`, `tag.custom_delete`, `tag.hide`,
//! `tag.move`, `tag.reset`, and `tag.reset_all` commands; the dialog itself only holds what is
//! being typed. It opens from `tag.customize`. Esc or Close closes it.

use egui::{Context, Id, RichText, ScrollArea, Sense};
use notecraft_engine::cmd::ids;
use notecraft_markdown::{TagDef, TagIcon, TagSet, MAX_CUSTOM_TAGS, MAX_TAG_NAME_CHARS};
use serde_json::json;

use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// Title of the dialog window, which is also what a screen reader announces.
pub const TITLE: &str = "Customize Tags";

/// What is being edited.
#[derive(Clone, Debug, PartialEq)]
enum Editing {
    /// Nothing picked.
    Nothing,
    /// A new tag.
    New,
    /// The tag with this slug.
    Tag(String),
}

/// The values typed so far.
#[derive(Clone, Debug, PartialEq)]
struct Draft {
    name: String,
    icon: TagIcon,
    font: Option<[u8; 3]>,
    highlight: Option<[u8; 3]>,
}

impl Draft {
    fn new() -> Self {
        Self {
            name: String::new(),
            icon: TagIcon::Dot,
            font: None,
            highlight: None,
        }
    }

    fn of(def: &TagDef) -> Self {
        Self {
            name: def.name.clone(),
            icon: def.icon,
            font: def.font_color.map(split),
            highlight: def.highlight.map(split),
        }
    }
}

fn split(rgb: u32) -> [u8; 3] {
    [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

fn color32(rgb: u32) -> egui::Color32 {
    let [r, g, b] = split(rgb);
    egui::Color32::from_rgb(r, g, b)
}

/// The dialog's state.
#[derive(Debug)]
pub(crate) struct CustomizeTags {
    open: bool,
    focus_pending: bool,
    editing: Editing,
    draft: Draft,
    /// Load the draft from this tag when the next frame has the new tag list (after a Reset).
    repick: Option<String>,
    /// Reset Tags asked for, waiting for the confirm.
    confirm_reset: bool,
}

impl CustomizeTags {
    pub(crate) fn new() -> Self {
        Self {
            open: false,
            focus_pending: false,
            editing: Editing::Nothing,
            draft: Draft::new(),
            repick: None,
            confirm_reset: false,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn open(&mut self) {
        if !self.open {
            self.open = true;
            self.focus_pending = true;
            self.editing = Editing::Nothing;
            self.confirm_reset = false;
            self.repick = None;
        }
    }

    fn pick(&mut self, set: &TagSet, slug: &str) {
        if let Some(def) = set.get(slug) {
            self.editing = Editing::Tag(def.slug.clone());
            self.draft = Draft::of(def);
        }
    }

    /// Draw the dialog if it is open. Returns the commands the buttons ran.
    pub(crate) fn show(&mut self, ctx: &Context, t: &ThemeTokens, set: &TagSet) -> Vec<Clicked> {
        let mut out = Vec::new();
        if !self.open {
            return out;
        }
        if let Some(slug) = self.repick.take() {
            self.pick(set, &slug);
        }
        // A tag deleted while it was picked is no longer picked.
        if let Editing::Tag(slug) = &self.editing {
            if set.get(slug).is_none() {
                self.editing = Editing::Nothing;
            }
        }
        let mut open = true;
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let focus = std::mem::take(&mut self.focus_pending);
        egui::Window::new(TITLE)
            .id(Id::new("customize_tags_dialog"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(560.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(230.0);
                        ui.set_max_width(230.0);
                        self.list(ui, t, set, &mut out);
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        ui.set_max_width(300.0);
                        self.editor(ui, t, set, &mut out);
                    });
                });
                ui.add_space(8.0);
                self.reset_all_row(ui, t, set, &mut out);
                ui.add_space(4.0);
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
        if close || !open {
            self.open = false;
        }
        out
    }

    fn list(&mut self, ui: &mut egui::Ui, t: &ThemeTokens, set: &TagSet, out: &mut Vec<Clicked>) {
        ui.label(RichText::new("Tags").strong().color(t.text));
        let mut picked = None;
        ScrollArea::vertical()
            .id_salt("customize_tags_list")
            .max_height(280.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for def in set.all() {
                    let selected = self.editing == Editing::Tag(def.slug.clone());
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::Vec2::splat(16.0), Sense::hover());
                        crate::icons::paint_tag(
                            ui.painter(),
                            rect,
                            def.icon,
                            color32(def.icon_color),
                        );
                        let r =
                            ui.selectable_label(selected, RichText::new(&def.name).color(t.text));
                        if r.clicked() {
                            picked = Some(def.slug.clone());
                        }
                    });
                }
            });
        if let Some(slug) = picked {
            self.pick(set, &slug);
        }
        // Move Up and Move Down act on the tag picked; the order is Ctrl+1 to Ctrl+9.
        let position = match &self.editing {
            Editing::Tag(slug) => set.all().iter().position(|d| d.slug == *slug),
            _ => None,
        };
        ui.horizontal(|ui| {
            let slug = position
                .and_then(|i| set.all().get(i))
                .map(|d| d.slug.clone());
            let up = position.is_some_and(|i| i > 0);
            let down = position.is_some_and(|i| i + 1 < set.all().len());
            for (label, enabled, direction) in [("Move Up", up, "up"), ("Move Down", down, "down")]
            {
                if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                    if let Some(slug) = &slug {
                        out.push(Clicked {
                            command: ids::TAG_MOVE,
                            args: json!({ "tag": slug, "direction": direction }),
                        });
                    }
                }
            }
        });
        ui.label(
            RichText::new("The first nine tags are Ctrl+1 to Ctrl+9.")
                .small()
                .color(t.text_muted),
        );
    }

    /// Reset Tags and its confirm, under the two columns.
    fn reset_all_row(
        &mut self,
        ui: &mut egui::Ui,
        t: &ThemeTokens,
        set: &TagSet,
        out: &mut Vec<Clicked>,
    ) {
        ui.horizontal(|ui| {
            if self.confirm_reset {
                ui.label(
                    RichText::new(
                        "Put the built-in tags back? Deleted tags return and default tags look as built. Your own tags stay.",
                    )
                    .color(t.text),
                );
                if ui.button("Yes, reset").clicked() {
                    out.push(Clicked {
                        command: ids::TAG_RESET_ALL,
                        args: serde_json::Value::Null,
                    });
                    self.confirm_reset = false;
                    self.editing = Editing::Nothing;
                }
                if ui.button("Cancel").clicked() {
                    self.confirm_reset = false;
                }
            } else {
                let changed = !set.hidden().is_empty()
                    || set.all().iter().any(|d| d.builtin && set.is_modified(&d.slug))
                    || set
                        .all()
                        .iter()
                        .filter(|d| d.builtin)
                        .map(|d| d.slug.as_str())
                        .ne(notecraft_markdown::default_tags()
                            .iter()
                            .map(|d| d.slug.as_str())
                            .filter(|slug| set.all().iter().any(|d| d.slug == *slug))
                            .collect::<Vec<_>>());
                if ui
                    .add_enabled(changed, egui::Button::new("Reset Tags"))
                    .on_hover_text("Put the built-in tag list back")
                    .clicked()
                {
                    self.confirm_reset = true;
                }
            }
        });
    }

    fn editor(&mut self, ui: &mut egui::Ui, t: &ThemeTokens, set: &TagSet, out: &mut Vec<Clicked>) {
        let picked = match &self.editing {
            Editing::Tag(slug) => set.get(slug),
            _ => None,
        };
        let builtin = picked.is_some_and(|d| d.builtin);
        match &self.editing {
            Editing::Nothing => {
                ui.label(
                    RichText::new("Pick a tag to change it, or add a new one.").color(t.text_muted),
                );
            }
            _ => {
                if let (Some(def), true) = (picked, builtin) {
                    ui.label(
                        RichText::new(format!(
                            "A default tag. It is stored as #{} on the page, whatever its name.",
                            def.slug
                        ))
                        .small()
                        .color(t.text_muted),
                    );
                }
                let label = ui.label(RichText::new("Display name").color(t.text));
                ui.add(
                    egui::TextEdit::singleline(&mut self.draft.name)
                        .char_limit(MAX_TAG_NAME_CHARS)
                        .desired_width(280.0),
                )
                .labelled_by(label.id);
                ui.add_space(4.0);
                ui.label(RichText::new("Icon").color(t.text));
                self.icon_grid(ui, t);
                ui.add_space(4.0);
                color_row(ui, t, "Font color", &mut self.draft.font);
                color_row(ui, t, "Highlight color", &mut self.draft.highlight);
            }
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let can_add = set.custom_count() < MAX_CUSTOM_TAGS;
            if ui
                .add_enabled(can_add, egui::Button::new("New Tag"))
                .clicked()
            {
                self.editing = Editing::New;
                self.draft = Draft::new();
            }
            if self.editing == Editing::Nothing {
                return;
            }
            let ready = !self.draft.name.trim().is_empty();
            if ui.add_enabled(ready, egui::Button::new("Save")).clicked() {
                self.save(out);
            }
            if let Editing::Tag(slug) = self.editing.clone() {
                // A custom tag is deleted; a default tag leaves the list (text keeps its icon).
                let tip = if builtin {
                    "Take this tag out of the list. Text already tagged keeps its icon."
                } else {
                    "Delete this tag. Text already tagged keeps its name."
                };
                if ui.button("Delete").on_hover_text(tip).clicked() {
                    out.push(Clicked {
                        command: if builtin {
                            ids::TAG_HIDE
                        } else {
                            ids::TAG_CUSTOM_DELETE
                        },
                        args: json!({ "tag": slug }),
                    });
                    self.editing = Editing::Nothing;
                }
                if builtin && set.is_modified(&slug) && ui.button("Reset").clicked() {
                    out.push(Clicked {
                        command: ids::TAG_RESET,
                        args: json!({ "tag": slug }),
                    });
                    self.repick = Some(slug);
                }
            }
        });
    }

    fn icon_grid(&mut self, ui: &mut egui::Ui, t: &ThemeTokens) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
            for icon in TagIcon::ALL {
                let (rect, response) =
                    ui.allocate_exact_size(egui::Vec2::splat(26.0), Sense::click());
                let chosen = self.draft.icon == icon;
                if chosen {
                    ui.painter()
                        .rect_filled(rect, 3.0, t.accent.gamma_multiply(0.25));
                }
                if response.hovered() {
                    ui.painter().rect_stroke(
                        rect,
                        3.0,
                        egui::Stroke::new(1.0, t.border),
                        egui::StrokeKind::Inside,
                    );
                }
                crate::icons::paint_tag(
                    ui.painter(),
                    rect.shrink(4.0),
                    icon,
                    color32(icon.color()),
                );
                let response = response.on_hover_text(icon.label());
                if response.clicked() {
                    self.draft.icon = icon;
                }
            }
        });
    }

    fn save(&mut self, out: &mut Vec<Clicked>) {
        let color = |c: Option<[u8; 3]>| c.map_or_else(|| "none".to_owned(), hex);
        let draft = &self.draft;
        match &self.editing {
            Editing::New => out.push(Clicked {
                command: ids::TAG_CUSTOM_ADD,
                args: json!({
                    "name": draft.name,
                    "icon": draft.icon.id(),
                    "font_color": color(draft.font),
                    "highlight": color(draft.highlight),
                }),
            }),
            Editing::Tag(slug) => out.push(Clicked {
                command: ids::TAG_EDIT,
                args: json!({
                    "tag": slug,
                    "name": draft.name,
                    "icon": draft.icon.id(),
                    "font_color": color(draft.font),
                    "highlight": color(draft.highlight),
                }),
            }),
            Editing::Nothing => {}
        }
        if self.editing == Editing::New {
            self.editing = Editing::Nothing;
        }
    }
}

/// A check box that turns a color on, and the color picker beside it.
fn color_row(ui: &mut egui::Ui, t: &ThemeTokens, label: &str, value: &mut Option<[u8; 3]>) {
    ui.horizontal(|ui| {
        let mut on = value.is_some();
        if ui
            .checkbox(&mut on, RichText::new(label).color(t.text))
            .changed()
        {
            *value = on.then_some([0xFF, 0xF1, 0x76]);
        }
        if let Some(rgb) = value {
            let _ = ui.color_edit_button_srgb(rgb);
        }
    });
}
