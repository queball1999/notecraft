//! An in-ribbon gallery, as OneNote's Styles and Tags: a bordered box showing two entries
//! drawn in their own style, small up and down arrows that scroll through the entries a row
//! at a time, and a More arrow that opens the full list. Clicking an entry runs its command
//! with its arguments. In the simplified ribbon a gallery is its dropdown button instead
//! (`Gallery::button`).

use craft_ui::buttons::IconButton;
use egui::{
    Align2, Color32, CornerRadius, FontId, Id, Popup, PopupCloseBehavior, Rect, Response, Sense,
    SetOpenCommand, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
};
use notecraft_engine::cmd::Status;

use super::spec::{ArgValue, Gallery, GalleryKind, Item};
use super::{button, keytips, menus, Clicked, KeyFrame, Look};
use crate::icons;
use notecraft_markdown::TagDef;

/// Width of the arrow column.
const ARROW_WIDTH: f32 = 16.0;
/// Height of each of the three arrows (up, down, More).
const ARROW_HEIGHT: f32 = 22.0;
/// Height of the whole gallery.
const HEIGHT: f32 = ARROW_HEIGHT * 3.0;
/// Space between the box and the arrows.
const GAP: f32 = 2.0;

/// One entry of a gallery: an item of the spec, or a tag of the gallery list (the Tags gallery
/// is built from the session's tags each frame, so renamed, hidden, and moved tags show).
#[derive(Clone, Copy)]
enum Entry<'a> {
    Item(&'static Item),
    Tag(&'a TagDef),
}

impl Entry<'_> {
    fn label(&self) -> &str {
        match self {
            Self::Item(item) => item.label,
            Self::Tag(def) => &def.name,
        }
    }

    fn clicked(&self) -> Clicked {
        match self {
            Self::Item(item) => Clicked::new(item.command, item.args),
            Self::Tag(def) => apply_tag(def),
        }
    }
}

fn apply_tag(def: &TagDef) -> Clicked {
    Clicked {
        command: notecraft_engine::cmd::ids::TAG_APPLY,
        args: serde_json::json!({ "tag": def.slug }),
    }
}

/// Draw `gallery` and record a click on an entry or on an item of its More list.
pub(super) fn show(
    ui: &mut Ui,
    gallery: &'static Gallery,
    look: Look<'_>,
    keys: &KeyFrame,
    out: &mut Vec<Clicked>,
) {
    let tokens = look.tokens;
    let entries: Vec<Entry<'_>> = if gallery.kind == GalleryKind::Tags {
        look.tags.iter().map(Entry::Tag).collect()
    } else {
        gallery.entries().iter().map(Entry::Item).collect()
    };
    let visible = gallery.visible.clamp(1, entries.len().max(1));
    let last_first = entries.len().saturating_sub(visible);
    let state = Id::new(("ribbon_gallery", gallery.button.label));
    let mut first = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(state))
        .unwrap_or(0)
        .min(last_first);

    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(gallery.width + GAP + ARROW_WIDTH, HEIGHT),
        Sense::hover(),
    );
    let frame = Rect::from_min_size(rect.min, egui::vec2(gallery.width, HEIGHT));
    ui.painter()
        .rect_filled(frame, CornerRadius::same(tokens.radius), tokens.canvas_bg);
    ui.painter().rect_stroke(
        frame,
        CornerRadius::same(tokens.radius),
        Stroke::new(1.0, tokens.border),
        StrokeKind::Inside,
    );

    let style = button::style(tokens);
    let row_height = (HEIGHT - 2.0) / visible as f32;
    for (row, item) in entries.iter().skip(first).take(visible).enumerate() {
        let cell = Rect::from_min_size(
            frame.min + egui::vec2(1.0, 1.0 + row as f32 * row_height),
            egui::vec2(gallery.width - 2.0, row_height),
        );
        let index = first + row;
        let response = place(ui, cell, |ui| {
            IconButton::new(item.label(), cell.size(), &style).show(ui, |ui, rect, color| {
                paint_entry(ui, rect, *item, index, color);
            })
        });
        let response = crate::a11y::plain_button(response);
        if response.clicked() {
            out.push(item.clicked());
        }
        let _ = response.on_hover_text(entry_tip(*item, look));
    }

    let arrows = rect.min.x + gallery.width + GAP;
    let slot = |n: f32| {
        Rect::from_min_size(
            egui::pos2(arrows, rect.min.y + n * ARROW_HEIGHT),
            egui::vec2(ARROW_WIDTH, ARROW_HEIGHT),
        )
    };
    let name = gallery.button.label;
    let up = arrow(ui, slot(0.0), &format!("{name} up"), Arrow::Up, look);
    let down = arrow(ui, slot(1.0), &format!("{name} down"), Arrow::Down, look);
    if up.clicked() {
        first = first.saturating_sub(visible);
    }
    if down.clicked() {
        first = (first + visible).min(last_first);
    }
    ui.ctx().data_mut(|data| data.insert_temp(state, first));

    let more = arrow(ui, slot(2.0), &format!("More {name}"), Arrow::More, look);
    if let Some(code) = keys.tip(&gallery.button) {
        keytips::paint_tip(ui, more.rect.center_bottom(), code, look);
    }
    let open = more.clicked() || keys.pressed(&gallery.button);
    let toggle = open.then_some(SetOpenCommand::Toggle);
    Popup::menu(&more)
        .open_memory(toggle)
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            if gallery.kind == GalleryKind::Tags {
                tag_menu(ui, gallery.items(), look, out);
            } else {
                menus::items(ui, gallery.items(), look, out);
            }
        });
    let _ = more.on_hover_text(format!("More {name}"));
}

/// Run `add` in a child ui limited to `cell`.
fn place(ui: &mut Ui, cell: Rect, add: impl FnOnce(&mut Ui) -> Response) -> Response {
    ui.scope_builder(UiBuilder::new().max_rect(cell), add).inner
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Arrow {
    Up,
    Down,
    /// A down arrow over a bar: the full list.
    More,
}

/// One of the small arrow buttons at the right of the box.
fn arrow(ui: &mut Ui, cell: Rect, name: &str, kind: Arrow, look: Look<'_>) -> Response {
    let style = button::style(look.tokens);
    let response = place(ui, cell, |ui| {
        IconButton::new(name, cell.size(), &style).show(ui, |ui, rect, color| {
            let c = rect.center();
            let (bar, tip) = match kind {
                Arrow::Up => (false, [(-3.5, 1.5), (0.0, -2.0), (3.5, 1.5)]),
                Arrow::Down => (false, [(-3.5, -1.5), (0.0, 2.0), (3.5, -1.5)]),
                Arrow::More => (true, [(-3.5, -0.5), (0.0, 3.0), (3.5, -0.5)]),
            };
            let points = tip.iter().map(|&(x, y)| c + egui::vec2(x, y)).collect();
            let painter = ui.painter();
            painter.add(egui::Shape::line(points, Stroke::new(1.2, color)));
            if bar {
                painter.hline(
                    (c.x - 3.5)..=(c.x + 3.5),
                    c.y - 3.5,
                    Stroke::new(1.2, color),
                );
            }
        })
    });
    crate::a11y::plain_button(response)
}

/// The tooltip of an entry: its label and what its command does.
fn entry_tip(entry: Entry<'_>, look: Look<'_>) -> String {
    let command = match entry {
        Entry::Item(item) => item.command,
        Entry::Tag(_) => notecraft_engine::cmd::ids::TAG_APPLY,
    };
    let spec = look.registry.get(command.as_str());
    let mut tip = entry.label().to_owned();
    if let Some(spec) = spec {
        tip.push('\n');
        tip.push_str(spec.description);
    }
    if spec.is_none_or(|spec| spec.status == Status::ComingSoon) {
        tip.push_str("\nComing Soon");
    }
    tip
}

/// The text size and weight a style entry is drawn in, from its `style` argument.
fn style_look(item: &Item) -> (f32, bool) {
    let style = item
        .args
        .iter()
        .find(|arg| arg.name == "style")
        .and_then(|arg| match arg.value {
            ArgValue::Str(s) => Some(s),
            _ => None,
        })
        .unwrap_or("");
    match style {
        "title" => (22.0, true),
        "heading1" => (19.0, true),
        "heading2" => (16.0, true),
        "heading3" => (14.5, true),
        "heading4" | "heading5" | "heading6" => (13.0, true),
        _ => (13.0, false),
    }
}

fn rgb(value: u32) -> Color32 {
    Color32::from_rgb(
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    )
}

/// One row of the Tags list: an icon and a label. True when clicked.
fn tag_row(ui: &mut Ui, label: &str, paint: impl FnOnce(&egui::Painter, Rect)) -> bool {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
        paint(ui.painter(), rect);
        ui.add(egui::Button::new(label).frame(false)).clicked()
    })
    .inner
}

/// The Tags list: the tags of the gallery in their order, then Customize Tags and Remove Tag.
/// Shared by the classic ribbon's More menu and the simplified ribbon's Tags button.
pub(super) fn tag_menu(ui: &mut Ui, items: &[Item], look: Look<'_>, out: &mut Vec<Clicked>) {
    ui.spacing_mut().item_spacing.y = 1.0;
    egui::ScrollArea::vertical()
        .id_salt("tag_menu_list")
        .max_height(420.0)
        .show(ui, |ui| {
            for def in look.tags {
                let (icon, color) = (def.icon, rgb(def.icon_color));
                if tag_row(ui, &def.name, |p, r| icons::paint_tag(p, r, icon, color)) {
                    out.push(apply_tag(def));
                    ui.close();
                }
            }
        });
    ui.separator();
    for item in items
        .iter()
        .filter(|item| item.command != notecraft_engine::cmd::ids::TAG_APPLY)
    {
        if tag_row(ui, item.label, |_, _| {}) {
            out.push(Clicked::new(item.command, item.args));
            ui.close();
        }
    }
}

/// Draw one entry inside its button.
fn paint_entry(ui: &Ui, rect: Rect, entry: Entry<'_>, index: usize, color: Color32) {
    let painter = ui.painter_at(rect);
    match entry {
        Entry::Item(item) => {
            let (size, heavy) = style_look(item);
            let font = FontId::proportional(size);
            let pos = egui::pos2(rect.min.x + 8.0, rect.center().y);
            painter.text(pos, Align2::LEFT_CENTER, item.label, font.clone(), color);
            if heavy {
                painter.text(
                    pos + egui::vec2(0.6, 0.0),
                    Align2::LEFT_CENTER,
                    item.label,
                    font,
                    color,
                );
            }
        }
        Entry::Tag(def) => {
            let icon = Rect::from_min_size(
                egui::pos2(rect.min.x + 6.0, rect.center().y - 8.0),
                Vec2::splat(16.0),
            );
            icons::paint_tag(&painter, icon, def.icon, rgb(def.icon_color));
            // The first nine tags of the list have Ctrl+1 to Ctrl+9, as in OneNote.
            let text = if index < 9 {
                format!("{} (Ctrl+{})", def.name, index + 1)
            } else {
                def.name.clone()
            };
            painter.text(
                egui::pos2(icon.max.x + 6.0, rect.center().y),
                Align2::LEFT_CENTER,
                text,
                FontId::proportional(12.0),
                color,
            );
        }
    }
}
