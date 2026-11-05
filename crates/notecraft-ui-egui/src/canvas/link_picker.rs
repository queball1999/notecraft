//! The page picker that opens when `[[` is typed (docs/markdown.md section 5.2): a list of the
//! pages of the notebook under the caret, filtered by what was typed after the brackets. The
//! session owns the picker (`link.picker.*` commands); this only draws what
//! `Session::link_picker_view` says and turns a click on a row into `link.picker.choose`.
//! The keys (Up, Down, Enter, Tab, Esc) are mapped in `keymap.rs`.

use egui::{Area, CornerRadius, Id, Order, Rect, Sense};
use notecraft_core::Vec2;
use notecraft_engine::cmd::{ids, Value};
use notecraft_engine::Session;

use super::flow::Flow;
use super::PageToScreen;
use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// Height of one row, in points.
const ROW: f32 = 40.0;
/// Rows shown before the list scrolls.
const VISIBLE_ROWS: f32 = 7.0;
/// Width of the list, in points.
const WIDTH: f32 = 320.0;

/// The accessible name of the list, which UI tests look up.
pub const LABEL: &str = "Page list";

/// Draw the picker under the caret while it is open.
pub(super) fn show(
    ui: &egui::Ui,
    session: &Session,
    flow: &Flow,
    map: PageToScreen,
    tokens: &ThemeTokens,
    clicked: &mut Vec<Clicked>,
) {
    let Some(view) = session.link_picker_view() else {
        return;
    };
    let Some(caret) = flow.caret else {
        return;
    };
    let below = map.pos(Vec2::new(caret.min.x, caret.max.y)) + egui::vec2(0.0, 4.0);
    let id = Id::new("notecraft.link_picker");
    let seen: Option<usize> = ui.data(|d| d.get_temp(id));
    ui.data_mut(|d| d.insert_temp(id, view.selected));
    let reveal = seen != Some(view.selected);
    let area = Area::new(id.with("area"))
        .order(Order::Foreground)
        .fixed_pos(below)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(WIDTH);
                list(ui, &view, reveal, tokens)
            })
        });
    area.response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, LABEL));
    if let Some(page) = area.inner.inner {
        let mut args = notecraft_engine::cmd::Map::new();
        args.insert("page".to_owned(), Value::from(page));
        clicked.push(Clicked {
            command: ids::LINK_PICKER_CHOOSE,
            args: Value::Object(args),
        });
    }
}

/// The rows. Returns the id of a clicked page.
fn list(
    ui: &mut egui::Ui,
    view: &notecraft_engine::links::LinkPickerView,
    reveal: bool,
    tokens: &ThemeTokens,
) -> Option<String> {
    if view.entries.is_empty() {
        ui.label(
            egui::RichText::new("No pages match.")
                .color(tokens.text_muted)
                .italics(),
        );
        return None;
    }
    let mut picked = None;
    ui.spacing_mut().item_spacing.y = 0.0;
    let height = (view.entries.len() as f32).min(VISIBLE_ROWS) * ROW;
    let mut scroll = egui::ScrollArea::vertical()
        .min_scrolled_height(height)
        .max_height(height)
        .auto_shrink([false, false]);
    if reveal {
        let top = view.selected as f32 * ROW;
        scroll = scroll.vertical_scroll_offset((top - height / 2.0 + ROW / 2.0).max(0.0));
    }
    scroll.show_rows(ui, ROW, view.entries.len(), |ui, range| {
        for position in range {
            let Some(entry) = view.entries.get(position) else {
                continue;
            };
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW), Sense::click());
            let selected = position == view.selected;
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    selected,
                    format!("{}, {}", entry.title, entry.place),
                )
            });
            let fill = if selected || response.hovered() {
                tokens.hover
            } else {
                egui::Color32::TRANSPARENT
            };
            ui.painter()
                .rect_filled(rect, CornerRadius::same(tokens.radius), fill);
            paint_row(ui, rect, entry, tokens);
            if response.clicked() {
                picked = Some(entry.page.clone());
            }
        }
    });
    picked
}

fn paint_row(
    ui: &egui::Ui,
    rect: Rect,
    entry: &notecraft_engine::links::PickerEntry,
    tokens: &ThemeTokens,
) {
    let painter = ui.painter_at(rect.intersect(ui.clip_rect()));
    let left = rect.min.x + 8.0;
    painter.text(
        egui::pos2(left, rect.min.y + 6.0),
        egui::Align2::LEFT_TOP,
        &entry.title,
        egui::FontId::proportional(14.0),
        ui.visuals().text_color(),
    );
    painter.text(
        egui::pos2(left, rect.min.y + 23.0),
        egui::Align2::LEFT_TOP,
        &entry.place,
        egui::FontId::proportional(11.0),
        tokens.text_muted,
    );
}
