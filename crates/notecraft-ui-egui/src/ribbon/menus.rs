//! What opens under a ribbon button: a list of items (with color or stroke previews), a color
//! picker (swatch grid, "no color", and a custom color editor with hex, rgb(), rgba(), and
//! R, G, B, A fields), or a table size grid.

use egui::color_picker::{color_picker_hsva_2d, show_color, Alpha};
use egui::ecolor::Hsva;
use egui::{
    Color32, CornerRadius, FontId, Id, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo,
    WidgetType,
};
use notecraft_engine::cmd::{CommandId, Map, Value};

use super::spec::{Button, ColorMenu, Item, Menu, Palette, Preview, STANDARD_COLORS};
use super::{Clicked, Look};
use crate::color::{self, Rgba8};
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

/// A section heading inside a menu, as in Ribbon Display Options.
pub(super) fn heading(ui: &mut Ui, text: &str) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ROW_WIDTH, ROW_HEIGHT), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
    ui.painter().text(
        egui::pos2(rect.min.x + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        FontId::proportional(13.0),
        ui.visuals().strong_text_color(),
    );
}

/// One of several choices, with a check mark in front of the current one. Screen readers
/// hear a radio button, checked or not.
pub(super) fn choice(ui: &mut Ui, label: &str, checked: bool, look: Look<'_>) -> Response {
    let t = look.tokens;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ROW_WIDTH, ROW_HEIGHT), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, checked, label));
    let painter = ui.painter_at(rect);
    if response.hovered() || response.has_focus() {
        painter.rect_filled(rect, CornerRadius::same(t.radius), t.hover);
    }
    let text = ui.visuals().text_color();
    if checked {
        let c = egui::pos2(rect.min.x + 15.0, rect.center().y);
        let points = vec![
            c + egui::vec2(-4.5, 0.0),
            c + egui::vec2(-1.5, 3.0),
            c + egui::vec2(4.5, -3.5),
        ];
        painter.add(egui::Shape::line(points, Stroke::new(1.5, text)));
    }
    painter.text(
        egui::pos2(rect.min.x + 30.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        text,
    );
    response
}

/// A ribbon button as a menu entry (the simplified ribbon's More Options menu): a plain
/// command runs, and a button with a menu opens the same menu as a submenu. A split button's
/// submenu starts with its own command.
pub(super) fn button_item(ui: &mut Ui, button: &Button, look: Look<'_>, out: &mut Vec<Clicked>) {
    match button.menu {
        Menu::None => {
            if row(ui, button.label, Preview::None, look).clicked() {
                out.push(Clicked::new(button.command, button.args));
                ui.close();
            }
        }
        Menu::Split(list) | Menu::Dropdown(list) => {
            ui.menu_button(button.label, |ui| {
                if button.menu.is_split() && row(ui, button.label, Preview::None, look).clicked() {
                    out.push(Clicked::new(button.command, button.args));
                    ui.close();
                }
                items(ui, list, look, out);
            });
        }
        Menu::Colors(menu) => {
            ui.menu_button(button.label, |ui| {
                colors(ui, button.command, &menu, look, out)
            });
        }
        Menu::TableGrid => {
            ui.menu_button(button.label, |ui| table_grid(ui, button.command, look, out));
        }
        Menu::Combo(combo) => {
            ui.menu_button(button.label, |ui| items(ui, combo.items, look, out));
        }
    }
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
#[derive(Clone, Debug)]
struct Custom {
    /// The color being edited, as HSVA so dragging hue and saturation is lossless.
    hsva: Hsva,
    /// The text in the color field; may be half typed.
    text: String,
    /// The text could not be read as a color.
    text_invalid: bool,
}

impl Custom {
    fn new(rgba: Rgba8) -> Self {
        Self {
            hsva: Hsva::from_srgba_unmultiplied(rgba),
            text: color::format_hex(rgba),
            text_invalid: false,
        }
    }

    fn rgba(&self) -> Rgba8 {
        self.hsva.to_srgba_unmultiplied()
    }

    /// Set the color from RGBA and show it in the text field.
    fn set(&mut self, rgba: Rgba8) {
        self.hsva = Hsva::from_srgba_unmultiplied(rgba);
        self.text = color::format_hex(rgba);
        self.text_invalid = false;
    }
}

/// The custom color editor: a picker with alpha, a field that takes `#RRGGBB`,
/// `#RRGGBBAA`, `rgb()`, or `rgba()`, and R, G, B, A number fields. All four stay in sync.
fn custom_editor(ui: &mut Ui, custom: &mut Custom, look: Look<'_>) {
    // Inputs need a visible box even in presets whose buttons have none.
    let t = look.tokens;
    let visuals = ui.visuals_mut();
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, t.border);
    visuals.widgets.inactive.weak_bg_fill = t.canvas_bg;
    visuals.extreme_bg_color = t.canvas_bg;
    let before = custom.rgba();
    if color_picker_hsva_2d(ui, &mut custom.hsva, Alpha::OnlyBlend) {
        custom.text = color::format_hex(custom.rgba());
        custom.text_invalid = false;
    }

    ui.horizontal(|ui| {
        ui.label("Color");
        let field = ui.add(
            egui::TextEdit::singleline(&mut custom.text)
                .desired_width(150.0)
                .char_limit(color::MAX_INPUT)
                .hint_text("#RRGGBBAA or rgba(r, g, b, a)"),
        );
        if field.changed() {
            match color::parse(&custom.text) {
                Some(rgba) => {
                    custom.hsva = Hsva::from_srgba_unmultiplied(rgba);
                    custom.text_invalid = false;
                }
                None => custom.text_invalid = true,
            }
        }
        if field.lost_focus() && !custom.text_invalid {
            custom.text = color::format_hex(custom.rgba());
        }
    });
    if custom.text_invalid {
        ui.label(
            egui::RichText::new("Not a color: use #RRGGBB, #RRGGBBAA, rgb(), or rgba()")
                .small()
                .color(look.tokens.text_muted),
        );
    }

    let mut rgba = custom.rgba();
    let mut changed = false;
    ui.horizontal(|ui| {
        for (name, value) in ["R", "G", "B", "A"].iter().zip(rgba.iter_mut()) {
            ui.label(*name);
            changed |= ui
                .add(egui::DragValue::new(value).range(0..=255).speed(1.0))
                .changed();
        }
    });
    if changed && rgba != before {
        custom.set(rgba);
    }
}

/// Most Recent Colors kept.
const RECENT_MAX: usize = 10;
/// Width of a color menu: ten swatches and their gaps.
const PALETTE_WIDTH: f32 = 10.0 * SWATCH + 9.0 * 3.0;

fn recent_id() -> Id {
    Id::new("ribbon_recent_colors")
}

fn last_id(command: CommandId) -> Id {
    Id::new(("ribbon_last_color", command.as_str()))
}

fn dialog_id() -> Id {
    Id::new("ribbon_colors_dialog")
}

/// The Recent Colors, newest first, as `#RRGGBB` or `#RRGGBBAA`.
pub(crate) fn recent_colors(ctx: &egui::Context) -> Vec<String> {
    ctx.data(|data| data.get_temp::<Vec<String>>(recent_id()))
        .unwrap_or_default()
}

/// Restore the Recent Colors (from settings); entries that are not colors are dropped.
pub(crate) fn set_recent_colors(ctx: &egui::Context, colors: &[String]) {
    let mut list: Vec<String> = Vec::new();
    for color in colors
        .iter()
        .filter_map(|c| color::parse(c))
        .map(color::format_hex)
    {
        if !list.contains(&color) && list.len() < RECENT_MAX {
            list.push(color);
        }
    }
    ctx.data_mut(|data| data.insert_temp(recent_id(), list));
}

/// The color a split color button applies and shows as its bar: the last one picked for
/// `command`, or the menu's default.
pub(super) fn last_color(ctx: &egui::Context, command: CommandId, menu: &ColorMenu) -> Color32 {
    let [r, g, b, a] = ctx
        .data(|data| data.get_temp::<String>(last_id(command)))
        .and_then(|hex| color::parse(&hex))
        .unwrap_or_else(|| {
            let [_, r, g, b] = menu.default.to_be_bytes();
            [r, g, b, u8::MAX]
        });
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// Record a pick: it becomes the command's last color and the newest Recent Color.
fn remember(ctx: &egui::Context, command: CommandId, value: &str) {
    let Some(rgba) = color::parse(value) else {
        return;
    };
    let hex = color::format_hex(rgba);
    let mut recent = recent_colors(ctx);
    recent.retain(|c| *c != hex);
    recent.insert(0, hex.clone());
    recent.truncate(RECENT_MAX);
    ctx.data_mut(|data| {
        data.insert_temp(recent_id(), recent);
        data.insert_temp(last_id(command), hex);
    });
}

/// Run `command` with `value` as its color argument, remembering real colors.
fn pick(ctx: &egui::Context, command: CommandId, arg: &str, value: &str, out: &mut Vec<Clicked>) {
    remember(ctx, command, value);
    out.push(Clicked {
        command,
        args: object(&[(arg, Value::from(value))]),
    });
}

/// The command a split color button's main part runs: its last color.
pub(super) fn apply_last(
    ctx: &egui::Context,
    command: CommandId,
    menu: &ColorMenu,
    out: &mut Vec<Clicked>,
) {
    let c = last_color(ctx, command, menu);
    let hex = color::format_hex(c.to_srgba_unmultiplied());
    pick(ctx, command, menu.arg, &hex, out);
}

/// A small caption over a block of swatches.
fn caption(ui: &mut Ui, text: &str, look: Look<'_>) {
    ui.label(
        egui::RichText::new(text)
            .size(11.0)
            .color(look.tokens.text_muted),
    );
}

/// A row of swatches; returns the color clicked.
fn swatch_row(ui: &mut Ui, colors: &[Color32], look: Look<'_>) -> Option<Color32> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for &c in colors {
            if swatch(ui, c, look).clicked() {
                picked = Some(c);
            }
        }
    });
    picked
}

/// A color picker for `command`, laid out as in OneNote: "no color" (first, or last for the
/// highlighter), the swatches (Theme Colors and Standard Colors, or one grid), Recent Colors,
/// and More Colors, which opens the Colors dialog. A pick runs the command and closes the
/// menu.
pub(super) fn colors(
    ui: &mut Ui,
    command: CommandId,
    menu: &ColorMenu,
    look: Look<'_>,
    out: &mut Vec<Clicked>,
) {
    let ctx = ui.ctx().clone();
    ui.set_min_width(PALETTE_WIDTH);
    let none_row = |ui: &mut Ui, out: &mut Vec<Clicked>| {
        if let Some((label, value)) = menu.none {
            if row(ui, label, Preview::None, look).clicked() {
                out.push(Clicked {
                    command,
                    args: object(&[(menu.arg, Value::from(value))]),
                });
                ui.close();
            }
        }
    };
    if !menu.none_last && menu.none.is_some() {
        none_row(ui, out);
        ui.separator();
    }
    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
    let columns = menu.columns.max(1);
    let rows: Vec<Vec<Color32>> = menu
        .colors
        .chunks(columns)
        .map(|chunk| chunk.iter().map(|&rgb| hex(rgb)).collect())
        .collect();
    let mut picked = None;
    match menu.palette {
        Palette::Theme => {
            caption(ui, "Theme Colors", look);
            for (index, colors) in rows.iter().enumerate() {
                if index == 1 {
                    // The hues stand apart from their tints and shades, as in Office.
                    ui.add_space(3.0);
                }
                picked = picked.or(swatch_row(ui, colors, look));
            }
            caption(ui, "Standard Colors", look);
            let standard: Vec<Color32> = STANDARD_COLORS.iter().map(|&rgb| hex(rgb)).collect();
            picked = picked.or(swatch_row(ui, &standard, look));
        }
        Palette::Grid => {
            for colors in &rows {
                picked = picked.or(swatch_row(ui, colors, look));
            }
        }
    }
    let recent: Vec<Color32> = recent_colors(&ctx)
        .iter()
        .filter_map(|c| color::parse(c))
        .map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a))
        .collect();
    if !recent.is_empty() {
        caption(ui, "Recent Colors", look);
        picked = picked.or(swatch_row(ui, &recent, look));
    }
    if let Some(c) = picked {
        let hex = color::format_hex(c.to_srgba_unmultiplied());
        pick(&ctx, command, menu.arg, &hex, out);
        ui.close();
        return;
    }
    if menu.none_last && menu.none.is_some() {
        ui.separator();
        none_row(ui, out);
    }
    if menu.custom {
        ui.separator();
        if row(ui, "More Colors...", Preview::None, look).clicked() {
            let current = last_color(&ctx, command, menu).to_srgba_unmultiplied();
            let dialog = Dialog {
                command,
                arg: menu.arg,
                current,
                custom: Custom::new(current),
                tab: DialogTab::Standard,
            };
            ctx.data_mut(|data| data.insert_temp(dialog_id(), Some(dialog)));
            ui.close();
        }
    }
}

/// Which page of the Colors dialog shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DialogTab {
    Standard,
    Custom,
}

/// The Colors dialog (More Colors): the command it sets, the color it had, and the new one.
#[derive(Clone, Debug)]
struct Dialog {
    command: CommandId,
    arg: &'static str,
    current: Rgba8,
    custom: Custom,
    tab: DialogTab,
}

/// The Standard page's swatches: twelve hues from light to dark in six rows, then greys.
/// Computed, not copied from any palette.
fn standard_swatches() -> Vec<Vec<Color32>> {
    let mut rows: Vec<Vec<Color32>> = [0.85_f32, 0.72, 0.6, 0.48, 0.36, 0.24]
        .iter()
        .map(|&value| {
            (0..12)
                .map(|step| {
                    let hue = step as f32 / 12.0;
                    let saturation = if value > 0.7 { 0.45 } else { 0.85 };
                    Color32::from(Hsva::new(hue, saturation, value + 0.15, 1.0))
                })
                .collect()
        })
        .collect();
    rows.push(
        (0..12)
            .map(|step| {
                let v = (255.0 * (1.0 - step as f32 / 11.0)).round() as u8;
                Color32::from_rgb(v, v, v)
            })
            .collect(),
    );
    rows
}

/// Draw the Colors dialog if More Colors opened it. OK runs its command with the new color.
pub(super) fn colors_dialog(ctx: &egui::Context, look: Look<'_>, out: &mut Vec<Clicked>) {
    let Some(mut dialog) = ctx
        .data(|data| data.get_temp::<Option<Dialog>>(dialog_id()))
        .flatten()
    else {
        return;
    };
    let mut done: Option<bool> = None;
    let mut open = true;
    egui::Window::new("Colors")
        .id(dialog_id().with("window"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (tab, label) in [
                    (DialogTab::Standard, "Standard"),
                    (DialogTab::Custom, "Custom"),
                ] {
                    if ui.selectable_label(dialog.tab == tab, label).clicked() {
                        dialog.tab = tab;
                    }
                }
            });
            ui.separator();
            match dialog.tab {
                DialogTab::Standard => {
                    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                    for row in standard_swatches() {
                        if let Some(c) = swatch_row(ui, &row, look) {
                            dialog.custom.set(c.to_srgba_unmultiplied());
                        }
                    }
                }
                DialogTab::Custom => custom_editor(ui, &mut dialog.custom, look),
            }
            ui.separator();
            ui.horizontal(|ui| {
                // New over Current, as in Office.
                ui.vertical(|ui| {
                    ui.label(format!("New {}", color::format_hex(dialog.custom.rgba())));
                    show_color(
                        ui,
                        Color32::from(dialog.custom.hsva),
                        egui::vec2(48.0, 18.0),
                    );
                    let [r, g, b, a] = dialog.current;
                    show_color(
                        ui,
                        Color32::from_rgba_unmultiplied(r, g, b, a),
                        egui::vec2(48.0, 18.0),
                    );
                    ui.label("Current");
                });
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    let ok =
                        egui::Button::new(egui::RichText::new("OK").color(look.tokens.accent_text))
                            .fill(look.tokens.accent);
                    if ui.add_enabled(!dialog.custom.text_invalid, ok).clicked() {
                        done = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        done = Some(false);
                    }
                });
            });
        });
    if ctx.input(|input| input.key_pressed(egui::Key::Escape)) || !open {
        done = done.or(Some(false));
    }
    if done == Some(true) {
        let hex = color::format_hex(dialog.custom.rgba());
        pick(ctx, dialog.command, dialog.arg, &hex, out);
    }
    let keep = done.is_none().then_some(dialog);
    ctx.data_mut(|data| {
        data.insert_temp(dialog_id(), keep);
    });
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
