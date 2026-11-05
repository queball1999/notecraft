//! The Customize Tags dialog (PR 2.5): the tag gallery as a list, and for a custom tag its
//! name, icon, font color, and highlight color. Add, change, and delete run the
//! `tag.custom_add`, `tag.custom_edit`, and `tag.custom_delete` commands; the dialog itself
//! only holds what is being typed. It opens from `tag.customize`. Esc or Close closes it.

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
}

impl CustomizeTags {
    pub(crate) fn new() -> Self {
        Self {
            open: false,
            focus_pending: false,
            editing: Editing::Nothing,
            draft: Draft::new(),
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
                        self.list(ui, t, set);
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(300.0);
                        ui.set_max_width(300.0);
                        self.editor(ui, t, set, &mut out);
                    });
                });
                ui.add_space(8.0);
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

    fn list(&mut self, ui: &mut egui::Ui, t: &ThemeTokens, set: &TagSet) {
        ui.label(RichText::new("Tags").strong().color(t.text));
        let mut picked = None;
        ScrollArea::vertical()
            .id_salt("customize_tags_list")
            .max_height(300.0)
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
                        let text = if def.builtin {
                            RichText::new(&def.name).color(t.text_muted)
                        } else {
                            RichText::new(&def.name).color(t.text)
                        };
                        let r = ui.selectable_label(selected, text);
                        if r.clicked() {
                            picked = Some(def.slug.clone());
                        }
                    });
                }
            });
        if let Some(slug) = picked {
            self.pick(set, &slug);
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui, t: &ThemeTokens, set: &TagSet, out: &mut Vec<Clicked>) {
        let builtin = match &self.editing {
            Editing::Tag(slug) => set.get(slug).is_some_and(|d| d.builtin),
            _ => false,
        };
        match &self.editing {
            Editing::Nothing => {
                ui.label(
                    RichText::new("Pick a tag to see it, or add a new one.").color(t.text_muted),
                );
            }
            Editing::Tag(slug) if builtin => {
                ui.label(
                    RichText::new(format!(
                        "This is one of the default tags. It is stored as #{slug} on the page."
                    ))
                    .color(t.text_muted),
                );
            }
            _ => {
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
            if builtin || self.editing == Editing::Nothing {
                return;
            }
            let ready = !self.draft.name.trim().is_empty();
            if ui.add_enabled(ready, egui::Button::new("Save")).clicked() {
                self.save(out);
            }
            if let Editing::Tag(slug) = self.editing.clone() {
                if ui.button("Delete").clicked() {
                    out.push(Clicked {
                        command: ids::TAG_CUSTOM_DELETE,
                        args: json!({ "tag": slug }),
                    });
                    self.editing = Editing::Nothing;
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
                command: ids::TAG_CUSTOM_EDIT,
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
