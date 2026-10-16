//! The Colors dialog's picker: a saturation and brightness square, a hue slider, and an alpha
//! slider. egui's own picker adds a row of R, G, B, A fields the dialog already has, so the
//! dialog draws these parts itself.

use egui::ecolor::Hsva;
use egui::epaint::{CircleShape, Mesh};
use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};

/// Side of the square and width of the sliders.
const SIZE: f32 = 180.0;
const SLIDER_HEIGHT: f32 = 14.0;
/// Mesh steps per side; enough for smooth gradients.
const STEPS: usize = 24;

/// Black or white, whichever reads better on `c`.
fn contrast(c: Color32) -> Color32 {
    let luma = 0.299 * f32::from(c.r()) + 0.587 * f32::from(c.g()) + 0.114 * f32::from(c.b());
    if luma > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// Draw the picker for `hsva`. Returns whether the user changed it.
pub(super) fn show(ui: &mut Ui, hsva: &mut Hsva, border: Color32) -> bool {
    // Kept as HSVA, so a grey keeps its hue and raising saturation brings it back.
    let mut changed = square(ui, hsva, border);
    changed |= slider(ui, "Hue", &mut hsva.h, border, |t| {
        Color32::from(Hsva::new(t, 1.0, 1.0, 1.0))
    });
    let opaque = Hsva { a: 1.0, ..*hsva };
    changed |= slider(ui, "Transparency", &mut hsva.a, border, |t| {
        Color32::from(Hsva { a: t, ..opaque })
    });
    changed
}

/// Saturation left to right, brightness bottom to top, at the current hue.
fn square(ui: &mut Ui, hsva: &mut Hsva, border: Color32) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(SIZE, SIZE), Sense::click_and_drag());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, "Saturation and brightness")
    });
    let mut changed = false;
    if let Some(pos) = response.interact_pointer_pos() {
        hsva.s = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        hsva.v = ((rect.bottom() - pos.y) / rect.height()).clamp(0.0, 1.0);
        changed = true;
    }
    let mut mesh = Mesh::default();
    for yi in 0..=STEPS {
        for xi in 0..=STEPS {
            let (s, v) = (xi as f32 / STEPS as f32, yi as f32 / STEPS as f32);
            let pos = Pos2::new(
                rect.left() + s * rect.width(),
                rect.bottom() - v * rect.height(),
            );
            mesh.colored_vertex(pos, Color32::from(Hsva::new(hsva.h, s, v, 1.0)));
            if xi < STEPS && yi < STEPS {
                let i = (yi * (STEPS + 1) + xi) as u32;
                let row = (STEPS + 1) as u32;
                mesh.add_triangle(i, i + 1, i + row);
                mesh.add_triangle(i + 1, i + row, i + row + 1);
            }
        }
    }
    let painter = ui.painter();
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
    let marker = Pos2::new(
        rect.left() + hsva.s * rect.width(),
        rect.bottom() - hsva.v * rect.height(),
    );
    let fill = Color32::from(Hsva { a: 1.0, ..*hsva });
    painter.add(CircleShape {
        center: marker,
        radius: 6.0,
        fill,
        stroke: Stroke::new(1.5, contrast(fill)),
    });
    changed
}

/// A horizontal slider over a gradient from `color_at(0)` to `color_at(1)`.
fn slider(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    border: Color32,
    color_at: impl Fn(f32) -> Color32,
) -> bool {
    let (rect, response): (Rect, Response) =
        ui.allocate_exact_size(egui::vec2(SIZE, SLIDER_HEIGHT), Sense::click_and_drag());
    let shown = *value;
    response.widget_info(|| egui::WidgetInfo::slider(true, f64::from(shown), label));
    let mut changed = false;
    if let Some(pos) = response.interact_pointer_pos() {
        *value = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        changed = true;
    }
    let painter = ui.painter();
    // A checkerboard under the gradient shows transparency.
    let cell = SLIDER_HEIGHT / 2.0;
    let mut x = rect.left();
    let mut column = 0;
    while x < rect.right() {
        for row in 0..2 {
            if (column + row) % 2 == 0 {
                let r = Rect::from_min_size(
                    Pos2::new(x, rect.top() + row as f32 * cell),
                    egui::vec2(cell.min(rect.right() - x), cell),
                );
                painter.rect_filled(r, 0.0, Color32::from_gray(160));
            }
        }
        x += cell;
        column += 1;
    }
    let mut mesh = Mesh::default();
    for i in 0..=STEPS {
        let t = i as f32 / STEPS as f32;
        let x = rect.left() + t * rect.width();
        let c = color_at(t);
        mesh.colored_vertex(Pos2::new(x, rect.top()), c);
        mesh.colored_vertex(Pos2::new(x, rect.bottom()), c);
        if i < STEPS {
            let k = (2 * i) as u32;
            mesh.add_triangle(k, k + 1, k + 2);
            mesh.add_triangle(k + 1, k + 2, k + 3);
        }
    }
    painter.add(Shape::mesh(mesh));
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
    let x = rect.left() + *value * rect.width();
    let thumb = Rect::from_center_size(
        Pos2::new(x, rect.center().y),
        egui::vec2(4.0, SLIDER_HEIGHT + 4.0),
    );
    let c = color_at(*value);
    painter.rect_filled(thumb, 1.0, c);
    painter.rect_stroke(
        thumb,
        1.0,
        Stroke::new(1.0, contrast(c)),
        StrokeKind::Outside,
    );
    changed
}
