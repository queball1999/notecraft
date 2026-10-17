//! The Draw tab's pens and highlighters. The list is data the user keeps (add, delete, color,
//! thickness; the app saves it in its settings), so these buttons are drawn from
//! [`Look::tools`] rather than from the spec. Each tool is a tall pen whose band and tip show
//! its color; the one in use sits raised. Clicking a tool picks it up (`draw.tool_select`);
//! clicking the tool in use opens its menu: a preview of the stroke, Thickness, Recent Colors,
//! Colors, More Colors, Eyedropper, and Delete Tool.

use craft_ui::buttons::IconButton;
use egui::{
    Color32, CornerRadius, Id, Popup, PopupCloseBehavior, Response, Sense, SetOpenCommand, Shape,
    Stroke, StrokeKind, Ui, WidgetInfo, WidgetType,
};
use notecraft_engine::cmd::{ids, Map, Value};
use notecraft_engine::{MAX_PEN_POINTS, MIN_PEN_POINTS};

use super::spec::Preview;
use super::{button, menus, Clicked, Look, ToolView};
use crate::icons::{self, ToolShape};
use crate::themes::{hex, mix};

/// Width of a tool button in the classic ribbon, in points.
const TOOL_WIDTH: f32 = 30.0;
/// Width of a tool button in the simplified ribbon.
pub(super) const COMPACT_WIDTH: f32 = 20.0;
/// Width of the tool menu, in points.
const MENU_WIDTH: f32 = 200.0;
/// Swatch size in the Colors grid, and the gap between swatches.
const CELL: f32 = 28.0;
const GAP: f32 = 4.0;
/// Swatches per row in the Colors grid, as in OneNote.
const COLUMNS: usize = 4;

/// The pen colors, four to a row: warm hues, cool hues, greens, then grays. Original values.
pub const PEN_COLORS: [(&str, u32); 16] = [
    ("Olive Brown", 0x7A5B14),
    ("Burnt Orange", 0xE36C09),
    ("Hot Pink", 0xE5185D),
    ("Red", 0xE81123),
    ("Lavender", 0x9B7FD1),
    ("Violet", 0xB146C2),
    ("Pink", 0xF06AAE),
    ("Light Blue", 0x3AA7E8),
    ("Teal Blue", 0x0A7EA4),
    ("Dark Teal", 0x00706A),
    ("Green", 0x3DAE49),
    ("Dark Green", 0x107C41),
    ("White", 0xFFFFFF),
    ("Light Gray", 0xBDBDBD),
    ("Gray", 0x7A7A7A),
    ("Black", 0x000000),
];

/// Thickness choices in points for a pen and for a highlighter.
pub const PEN_SIZES: [f32; 6] = [0.5, 1.0, 2.0, 3.5, 5.0, 8.0];
pub const HIGHLIGHTER_SIZES: [f32; 6] = [4.0, 6.0, 8.0, 12.0, 16.0, 24.0];

/// A name for a color: the palette's own, or the nearest basic hue.
pub fn color_name(rgb: u32) -> &'static str {
    if let Some((name, _)) = PEN_COLORS.iter().find(|(_, value)| *value == rgb) {
        return name;
    }
    let [_, r, g, b] = rgb.to_be_bytes();
    let (h, s, v) = egui::ecolor::hsv_from_rgb([
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ]);
    if v < 0.25 {
        return "Black";
    }
    if s < 0.15 {
        return if v > 0.85 { "White" } else { "Gray" };
    }
    let degrees = h * 360.0;
    match degrees {
        d if !(15.0..345.0).contains(&d) => "Red",
        d if d < 40.0 => "Orange",
        d if d < 70.0 => "Yellow",
        d if d < 160.0 => "Green",
        d if d < 200.0 => "Turquoise",
        d if d < 255.0 => "Blue",
        d if d < 290.0 => "Purple",
        _ => "Pink",
    }
}

/// The name of a tool, for its tooltip and its screen reader name: "Red Pen".
pub fn tool_label(view: ToolView) -> String {
    let [r, g, b, _] = view.color;
    let rgb = u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b);
    let kind = if view.highlighter {
        "Highlighter"
    } else {
        "Pen"
    };
    format!("{} {kind}", color_name(rgb))
}

/// The next thickness up or down from `width` in points: a quarter point at the thin end, a
/// half under 6, a point above. Kept within what the commands accept.
pub fn step_width(width: f32, thicker: bool) -> f32 {
    let step = if width < 2.0 {
        0.25
    } else if width < 6.0 {
        0.5
    } else {
        1.0
    };
    let next = if thicker { width + step } else { width - step };
    next.clamp(MIN_PEN_POINTS, MAX_PEN_POINTS)
}

fn color32([r, g, b, a]: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn rgb_of([r, g, b, _]: [u8; 4]) -> u32 {
    u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
}

fn object(pairs: &[(&str, Value)]) -> Value {
    let mut map = Map::new();
    for (name, value) in pairs {
        map.insert((*name).to_owned(), value.clone());
    }
    Value::Object(map)
}

fn eyedropper_id() -> Id {
    Id::new("draw_eyedropper")
}

/// Whether the Eyedropper is waiting for a click on the page.
pub(crate) fn eyedropper_active(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(eyedropper_id()))
        .unwrap_or(false)
}

/// Start or stop the Eyedropper. The canvas takes the next click on a stroke.
pub(crate) fn set_eyedropper(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|data| data.insert_temp(eyedropper_id(), on));
}

/// The command that picks up tool `index`.
fn select(index: usize) -> Clicked {
    Clicked {
        command: ids::DRAW_TOOL_SELECT,
        args: object(&[("index", Value::from(index))]),
    }
}

/// Draw every tool as a tall button, `height` tall.
pub(super) fn show(ui: &mut Ui, look: Look<'_>, height: f32, out: &mut Vec<Clicked>) {
    ui.spacing_mut().item_spacing.x = 2.0;
    for (index, view) in look.tools.iter().enumerate() {
        tool_button(ui, index, *view, look, egui::vec2(TOOL_WIDTH, height), out);
    }
}

/// Draw every tool as a narrow icon button, for the simplified ribbon.
pub(super) fn show_compact(ui: &mut Ui, look: Look<'_>, out: &mut Vec<Clicked>) {
    for (index, view) in look.tools.iter().enumerate() {
        let size = egui::vec2(COMPACT_WIDTH, button::SMALL_HEIGHT);
        tool_button(ui, index, *view, look, size, out);
    }
}

/// Width the compact tool buttons take, with their gaps.
pub(super) fn compact_width(look: Look<'_>) -> f32 {
    look.tools.len() as f32 * (COMPACT_WIDTH + 2.0)
}

/// The tools as menu rows that pick them up (More Options, when the row has no room).
pub(super) fn menu_rows(ui: &mut Ui, look: Look<'_>, out: &mut Vec<Clicked>) {
    for (index, view) in look.tools.iter().enumerate() {
        let current = look.selected_tool == Some(index);
        let label = tool_label(*view);
        if menus::choice(ui, &label, current, look).clicked() {
            out.push(select(index));
            ui.close();
        }
    }
}

fn tool_button(
    ui: &mut Ui,
    index: usize,
    view: ToolView,
    look: Look<'_>,
    size: egui::Vec2,
    out: &mut Vec<Clicked>,
) {
    let tokens = look.tokens;
    let selected = look.selected_tool == Some(index);
    let label = tool_label(view);
    let style = button::style(tokens);
    let compact = size.y <= button::SMALL_HEIGHT + 0.5;
    let response = IconButton::new(&label, size, &style)
        .selected(selected)
        .show(ui, |ui, rect, fg| {
            // The tool in use sits raised; the others rest lower.
            let lift = if compact {
                0.0
            } else if selected {
                -2.0
            } else {
                5.0
            };
            let inner = if compact {
                rect.shrink2(egui::vec2(2.0, 0.0))
            } else {
                rect.shrink2(egui::vec2(3.0, 5.0))
            };
            let icon = inner.translate(egui::vec2(0.0, lift));
            let shape = if view.highlighter {
                ToolShape::Highlighter
            } else {
                ToolShape::Pen
            };
            icons::paint_tool(
                ui.painter(),
                icon,
                shape,
                color32(view.color),
                tokens.canvas_bg,
                fg,
            );
        });
    let response = crate::a11y::plain_button(response);
    let clicked = response.clicked();
    if clicked && !selected {
        out.push(select(index));
    }
    // Clicking the tool in use opens its menu.
    let toggle = (clicked && selected).then_some(SetOpenCommand::Toggle);
    Popup::menu(&response)
        .open_memory(toggle)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| menu(ui, index, view, look, out));
    let tip = if selected {
        format!("{label}\nClick again for thickness and color")
    } else {
        label
    };
    let _ = response.on_hover_text(tip);
}

/// The menu of the tool in use.
fn menu(ui: &mut Ui, index: usize, view: ToolView, look: Look<'_>, out: &mut Vec<Clicked>) {
    let ctx = ui.ctx().clone();
    ui.set_min_width(MENU_WIDTH);
    ui.spacing_mut().item_spacing = egui::vec2(GAP, GAP);
    stroke_preview(ui, view, look);
    menus::heading(ui, "Thickness");
    thickness_row(ui, view, look, out);

    let recent: Vec<[u8; 4]> = menus::recent_colors(&ctx)
        .iter()
        .filter_map(|c| crate::color::parse(c))
        .take(COLUMNS * 2)
        .collect();
    if !recent.is_empty() {
        menus::heading(ui, "Recent Colors");
        let mut picked = None;
        ui.horizontal_wrapped(|ui| {
            for rgba in &recent {
                let name = crate::color::format_hex(*rgba);
                if swatch(ui, color32(*rgba), &name, *rgba == view.color, look).clicked() {
                    picked = Some(*rgba);
                }
            }
        });
        if let Some(rgba) = picked {
            pick_color(&ctx, rgba, out);
            ui.close();
            return;
        }
    }

    menus::heading(ui, "Colors");
    let mut picked = None;
    for chunk in PEN_COLORS.chunks(COLUMNS) {
        ui.horizontal(|ui| {
            for (name, rgb) in chunk {
                let rgba = [(rgb >> 16) as u8, (rgb >> 8) as u8, *rgb as u8, u8::MAX];
                let current = rgb_of(view.color) == *rgb;
                if swatch(ui, hex(*rgb), name, current, look).clicked() {
                    picked = Some(rgba);
                }
            }
        });
    }
    if let Some(rgba) = picked {
        pick_color(&ctx, rgba, out);
        ui.close();
        return;
    }

    ui.separator();
    if menus::row(ui, "More Colors...", Preview::None, look).clicked() {
        menus::open_colors_dialog(&ctx, ids::DRAW_COLOR, "color", view.color);
        ui.close();
    }
    if menus::row(ui, "Eyedropper", Preview::None, look)
        .on_hover_text("Click ink on the page to take its color")
        .clicked()
    {
        set_eyedropper(&ctx, true);
        ui.close();
    }
    if menus::row(ui, "Delete Tool", Preview::None, look).clicked() {
        out.push(Clicked {
            command: ids::DRAW_TOOL_DELETE,
            args: object(&[("index", Value::from(index))]),
        });
        ui.close();
    }
}

/// Run `draw.color` with `rgba` and remember it among the Recent Colors.
fn pick_color(ctx: &egui::Context, rgba: [u8; 4], out: &mut Vec<Clicked>) {
    let hex = crate::color::format_hex(rgba);
    menus::pick(ctx, ids::DRAW_COLOR, "color", &hex, out);
}

/// A short curve drawn with the tool's color and thickness, so a change shows at once.
fn stroke_preview(ui: &mut Ui, view: ToolView, look: Look<'_>) {
    let tokens = look.tokens;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(MENU_WIDTH, 40.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, "Stroke preview"));
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        CornerRadius::same(tokens.radius),
        mix(tokens.canvas_bg, tokens.text, 0.04),
    );
    let width = (view.width_pt * 96.0 / 72.0).clamp(1.0, 24.0);
    let mut color = color32(view.color);
    if view.highlighter {
        color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 140);
    }
    let (left, right) = (rect.min.x + 18.0, rect.max.x - 18.0);
    let points: Vec<egui::Pos2> = (0..=32)
        .map(|i| {
            let t = i as f32 / 32.0;
            let x = left + (right - left) * t;
            let y = rect.center().y + (t * std::f32::consts::TAU).sin() * 7.0;
            egui::pos2(x, y)
        })
        .collect();
    if let (Some(first), Some(last)) = (points.first().copied(), points.last().copied()) {
        painter.add(Shape::line(points, Stroke::new(width, color)));
        // Round ends, as the pen draws.
        painter.circle_filled(first, width / 2.0, color);
        painter.circle_filled(last, width / 2.0, color);
    }
}

/// The Thickness row: a minus button, the sizes as dots (the current one ringed), a plus.
fn thickness_row(ui: &mut Ui, view: ToolView, look: Look<'_>, out: &mut Vec<Clicked>) {
    let sizes = if view.highlighter {
        &HIGHLIGHTER_SIZES
    } else {
        &PEN_SIZES
    };
    let widest = sizes.last().copied().unwrap_or(1.0);
    let set = |width: f32, out: &mut Vec<Clicked>| {
        out.push(Clicked {
            command: ids::DRAW_THICKNESS,
            args: object(&[("width", Value::from(f64::from(width)))]),
        });
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        if step_button(ui, "Thinner", false, look).clicked() {
            set(step_width(view.width_pt, false), out);
        }
        for &width in sizes {
            let current = (width - view.width_pt).abs() < 1.0e-3;
            if size_dot(ui, width, widest, current, look).clicked() {
                set(width, out);
            }
        }
        if step_button(ui, "Thicker", true, look).clicked() {
            set(step_width(view.width_pt, true), out);
        }
    });
}

/// The minus or plus button of the Thickness row.
fn step_button(ui: &mut Ui, label: &str, plus: bool, look: Look<'_>) -> Response {
    let t = look.tokens;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(t.radius), t.hover);
    }
    let stroke = Stroke::new(1.5, ui.visuals().text_color());
    let c = rect.center();
    ui.painter().hline((c.x - 4.5)..=(c.x + 4.5), c.y, stroke);
    if plus {
        ui.painter().vline(c.x, (c.y - 4.5)..=(c.y + 4.5), stroke);
    }
    response.on_hover_text(label)
}

/// One size of the Thickness row: a dot as wide as the stroke it stands for, ringed when it is
/// the current size.
fn size_dot(ui: &mut Ui, width: f32, widest: f32, current: bool, look: Look<'_>) -> Response {
    let t = look.tokens;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::click());
    let name = format!("{width} pt");
    response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, current, &name));
    if response.hovered() || response.has_focus() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(t.radius), t.hover);
    }
    let diameter = 4.0 + 12.0 * (width / widest).clamp(0.0, 1.0);
    ui.painter()
        .circle_filled(rect.center(), diameter / 2.0, ui.visuals().text_color());
    if current {
        ui.painter()
            .circle_stroke(rect.center(), 10.0, Stroke::new(1.5, t.accent));
    }
    response.on_hover_text(name)
}

/// One color swatch; ringed when it is the tool's color. Its accessible name is `name`.
fn swatch(ui: &mut Ui, color: Color32, name: &str, current: bool, look: Look<'_>) -> Response {
    let t = look.tokens;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(CELL, CELL), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, true, current, name));
    let inner = rect.shrink(3.0);
    let round = CornerRadius::same(4);
    ui.painter().rect_filled(inner, round, color);
    ui.painter()
        .rect_stroke(inner, round, Stroke::new(1.0, t.border), StrokeKind::Inside);
    if current {
        ui.painter().rect_stroke(
            rect.shrink(0.5),
            CornerRadius::same(6),
            Stroke::new(2.0, t.accent),
            StrokeKind::Inside,
        );
    } else if response.hovered() || response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            CornerRadius::same(5),
            Stroke::new(1.0, mix(t.border, t.accent, 0.6)),
            StrokeKind::Inside,
        );
    }
    response.on_hover_text(name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn the_palette_is_four_by_four_and_distinct() {
        assert_eq!(PEN_COLORS.len(), COLUMNS * 4);
        let unique: BTreeSet<u32> = PEN_COLORS.iter().map(|(_, rgb)| *rgb).collect();
        assert_eq!(unique.len(), PEN_COLORS.len());
        let names: BTreeSet<&str> = PEN_COLORS.iter().map(|(name, _)| *name).collect();
        assert_eq!(names.len(), PEN_COLORS.len());
        assert!(PEN_COLORS.iter().all(|(_, rgb)| *rgb <= 0xFF_FF_FF));
    }

    #[test]
    fn tools_are_named_after_their_color() {
        let view = |highlighter, rgb: u32| ToolView {
            highlighter,
            color: [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255],
            width_pt: 1.0,
        };
        assert_eq!(tool_label(view(false, 0x000000)), "Black Pen");
        assert_eq!(tool_label(view(false, 0xE81123)), "Red Pen");
        assert_eq!(tool_label(view(true, 0xFFF176)), "Yellow Highlighter");
        assert_eq!(tool_label(view(true, 0x80DEEA)), "Turquoise Highlighter");
        assert_eq!(color_name(0x123456), "Blue");
        assert_eq!(color_name(0x808080), "Gray");
    }

    #[test]
    fn thickness_steps_stay_in_range() {
        assert_eq!(step_width(1.0, true), 1.25);
        assert_eq!(step_width(1.0, false), 0.75);
        assert_eq!(step_width(3.0, true), 3.5);
        assert_eq!(step_width(8.0, true), 9.0);
        assert_eq!(step_width(MIN_PEN_POINTS, false), MIN_PEN_POINTS);
        assert_eq!(step_width(MAX_PEN_POINTS, true), MAX_PEN_POINTS);
    }

    #[test]
    fn the_size_rows_are_in_the_range_the_commands_take() {
        for sizes in [PEN_SIZES, HIGHLIGHTER_SIZES] {
            assert!(sizes.windows(2).all(|w| w[0] < w[1]));
            assert!(sizes
                .iter()
                .all(|w| (MIN_PEN_POINTS..=MAX_PEN_POINTS).contains(w)));
        }
    }
}
