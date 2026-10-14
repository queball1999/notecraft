//! What opens under a ribbon button: a list of items (with color or stroke previews), a color
//! picker (swatch grid, "no color", and a full custom color picker), or a table size grid.

use egui::color_picker::{color_picker_color32, Alpha};
use egui::{
    Color32, CornerRadius, FontId, Id, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo,
    WidgetType,
};
use notecraft_engine::cmd::{CommandId, Map, Value};

use super::spec::{ColorMenu, Item, Preview};
use super::{Clicked, Look};
use crate::themes::{hex, mix};

const ROW_HEIGHT: f32 = 24.0;
const ROW_WIDTH: f32 = 200.0;
const SWATCH: f32 = 18.0;
/// Largest table the grid offers, in columns and rows.
const TABLE_COLUMNS: usize = 10;
const TABLE_ROWS: usize = 8;
const CELL: f32 = 18.0;

/// `#RRGGBB` for a color.
pub(super) fn hex_string(color: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r(), color.g(), color.b())
}

fn object(pairs: &[(&str, Value)]) -> Value {
    let mut map = Map::new();
    for (name, value) in pairs {
        map.insert((*name).to_owned(), value.clone());
    }
    Value::Object(map)
}

/// A clickable row with an optional preview, as wide as a menu.
fn row(ui: &mut Ui, label: &str, preview: Preview, look: Look<'_>) -> Response {
    let t = look.tokens;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ROW_WIDTH, ROW_HEIGHT), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    let painter = ui.painter_at(rect);
    if response.hovered() || response.has_focus() {
        painter.rect_filled(rect, CornerRadius::same(t.radius), t.hover);
    }
    let text = ui.visuals().text_color();
    let mut x = rect.min.x + 8.0;
    match preview {
        Preview::None => {}
        Preview::Color(rgb) => {
            let swatch = Rect::from_center_size(
                egui::pos2(x + 7.0, rect.center().y),
                egui::vec2(14.0, 14.0),
            );
            painter.rect_filled(swatch, CornerRadius::same(7), hex(rgb));
            painter.rect_stroke(
                swatch,
                CornerRadius::same(7),
                Stroke::new(1.0, t.border),
                StrokeKind::Inside,
            );
            x += 22.0;
        }
        Preview::Width(points) => {
            let width = points.clamp(0.5, 8.0);
            painter.hline((x)..=(x + 28.0), rect.center().y, Stroke::new(width, text));
            x += 36.0;
        }
    }
    painter.text(
        egui::pos2(x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        text,
    );
    response
}

/// A plain list of items. Picking one closes the menu.
pub(super) fn items(ui: &mut Ui, items: &[Item], look: Look<'_>, out: &mut Vec<Clicked>) {
    ui.spacing_mut().item_spacing.y = 1.0;
    for item in items {
        if row(ui, item.label, item.preview, look).clicked() {
            out.push(Clicked::new(item.command, item.args));
            ui.close();
        }
    }
}

/// One color swatch. Its accessible name is the color's hex value.
fn swatch(ui: &mut Ui, color: Color32, look: Look<'_>) -> Response {
    let t = look.tokens;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(SWATCH, SWATCH), Sense::click());
    let name = hex_string(color);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(2), color);
    let outline = if response.hovered() || response.has_focus() {
        Stroke::new(2.0, t.accent)
    } else {
        Stroke::new(1.0, mix(t.border, color, 0.3))
    };
    painter.rect_stroke(rect, CornerRadius::same(2), outline, StrokeKind::Inside);
    response.on_hover_text(name)
}

/// State of the custom color part of a picker, kept in egui memory per menu.
#[derive(Clone, Copy, Debug)]
struct Custom {
    open: bool,
    color: Color32,
}

/// A color picker for `command`. Picking a swatch, "no color", or applying a custom color
/// runs the command with the color and closes the menu.
pub(super) fn colors(
    ui: &mut Ui,
    command: CommandId,
    menu: &ColorMenu,
    look: Look<'_>,
    out: &mut Vec<Clicked>,
) {
    let run = |out: &mut Vec<Clicked>, value: &str| {
        out.push(Clicked {
            command,
            args: object(&[(menu.arg, Value::from(value))]),
        });
    };
    if let Some((label, value)) = menu.none {
        if row(ui, label, Preview::None, look).clicked() {
            run(out, value);
            ui.close();
        }
        ui.separator();
    }
    let columns = menu.columns.max(1);
    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
    for chunk in menu.colors.chunks(columns) {
        ui.horizontal(|ui| {
            for &rgb in chunk {
                if swatch(ui, hex(rgb), look).clicked() {
                    run(out, &hex_string(hex(rgb)));
                    ui.close();
                }
            }
        });
    }
    if !menu.custom {
        return;
    }
    ui.separator();
    let id = Id::new(("ribbon_custom_color", command.as_str()));
    let mut custom = ui
        .data(|data| data.get_temp::<Custom>(id))
        .unwrap_or(Custom {
            open: false,
            color: menu.colors.first().map_or(Color32::GRAY, |&rgb| hex(rgb)),
        });
    if row(ui, "More Colors", Preview::None, look).clicked() {
        custom.open = !custom.open;
    }
    if custom.open {
        color_picker_color32(ui, &mut custom.color, Alpha::Opaque);
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 18.0), Sense::hover());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(2), custom.color);
            ui.label(hex_string(custom.color));
            let apply =
                egui::Button::new(egui::RichText::new("Apply").color(look.tokens.accent_text))
                    .fill(look.tokens.accent);
            if ui.add(apply).clicked() {
                run(out, &hex_string(custom.color));
                custom.open = false;
                ui.close();
            }
        });
    }
    ui.data_mut(|data| data.insert_temp(id, custom));
}

/// A grid of cells: hovering picks columns and rows, clicking inserts that table. "Insert
/// Table" below runs the command with no size (the size dialog).
pub(super) fn table_grid(ui: &mut Ui, command: CommandId, look: Look<'_>, out: &mut Vec<Clicked>) {
    let t = look.tokens;
    let size = egui::vec2(TABLE_COLUMNS as f32 * CELL, TABLE_ROWS as f32 * CELL);
    let label_rect = ui
        .allocate_exact_size(egui::vec2(size.x, 20.0), Sense::hover())
        .0;
    let (grid, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Table size"));
    let hovered = response.hover_pos().map(|pos| {
        let cell = (pos - grid.min) / CELL;
        // In range by construction: the pointer is inside the grid.
        let columns = (cell.x.floor() as usize + 1).clamp(1, TABLE_COLUMNS);
        let rows = (cell.y.floor() as usize + 1).clamp(1, TABLE_ROWS);
        (columns, rows)
    });
    let painter = ui.painter();
    for row in 0..TABLE_ROWS {
        for column in 0..TABLE_COLUMNS {
            let cell = Rect::from_min_size(
                grid.min + egui::vec2(column as f32 * CELL, row as f32 * CELL),
                egui::vec2(CELL, CELL),
            )
            .shrink(1.5);
            let picked = hovered.is_some_and(|(columns, rows)| column < columns && row < rows);
            let fill = if picked {
                mix(t.accent, t.canvas_bg, 0.6)
            } else {
                t.canvas_bg
            };
            painter.rect_filled(cell, CornerRadius::ZERO, fill);
            let stroke = if picked { t.accent } else { t.border };
            painter.rect_stroke(
                cell,
                CornerRadius::ZERO,
                Stroke::new(1.0, stroke),
                StrokeKind::Inside,
            );
        }
    }
    let caption = hovered.map_or_else(
        || "Choose a size".to_owned(),
        |(columns, rows)| format!("{columns} x {rows} Table"),
    );
    painter.text(
        label_rect.left_center(),
        egui::Align2::LEFT_CENTER,
        caption,
        FontId::proportional(13.0),
        ui.visuals().text_color(),
    );
    if let (true, Some((columns, rows))) = (response.clicked(), hovered) {
        out.push(Clicked {
            command,
            args: object(&[
                ("rows", Value::from(rows as u64)),
                ("columns", Value::from(columns as u64)),
            ]),
        });
        ui.close();
    }
    ui.separator();
    if row(ui, "Insert Table", Preview::None, look).clicked() {
        out.push(Clicked {
            command,
            args: Value::Null,
        });
        ui.close();
    }
}
