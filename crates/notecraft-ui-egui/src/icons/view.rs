//! Icons for the View tab.

use egui::Color32;

use super::Pen;

/// A window frame with a title bar.
fn window(p: &Pen, x0: f32, y0: f32, x1: f32, y1: f32) {
    p.outline(x0, y0, x1, y1);
    p.line(&[(x0, y0 + 2.5), (x1, y0 + 2.5)]);
}

/// A magnifying glass.
fn glass(p: &Pen) {
    p.circle(6.5, 6.5, 5.0);
    p.line_colored(&[(10.2, 10.2), (14.5, 14.5)], 1.5, p.fg);
}

pub(super) fn normal_view(p: &Pen) {
    window(p, 1.5, 2.0, 14.5, 14.0);
    p.fill(1.5, 4.5, 14.5, 6.5, p.fg);
    p.line(&[(5.0, 6.5), (5.0, 14.0)]);
}

pub(super) fn full_page(p: &Pen) {
    window(p, 1.5, 2.0, 14.5, 14.0);
    p.line(&[(4.0, 7.5), (4.0, 6.0), (5.5, 6.0)]);
    p.line(&[(12.0, 7.5), (12.0, 6.0), (10.5, 6.0)]);
    p.line(&[(4.0, 10.0), (4.0, 11.5), (5.5, 11.5)]);
    p.line(&[(12.0, 10.0), (12.0, 11.5), (10.5, 11.5)]);
}

pub(super) fn dock(p: &Pen) {
    p.outline(1.5, 2.0, 14.5, 14.0);
    p.fill(10.0, 2.0, 14.5, 14.0, p.fg);
    p.line(&[(4.0, 8.0), (8.5, 8.0)]);
    p.line(&[(6.5, 6.0), (8.5, 8.0), (6.5, 10.0)]);
}

pub(super) fn navigation(p: &Pen) {
    p.outline(1.5, 2.0, 14.5, 14.0);
    p.fill(1.5, 2.0, 6.0, 14.0, p.fg);
}

pub(super) fn page_color(p: &Pen, accent: Color32) {
    p.solid(
        &[(3.0, 1.5), (13.0, 1.5), (13.0, 14.5), (3.0, 14.5)],
        accent,
    );
    p.outline(3.0, 1.5, 13.0, 14.5);
}

pub(super) fn rule_lines(p: &Pen) {
    p.outline(2.5, 1.5, 13.5, 14.5);
    p.text_lines(2.5, 13.5, &[4.5, 7.0, 9.5, 12.0]);
    p.line(&[(5.0, 1.5), (5.0, 14.5)]);
}

pub(super) fn hide_title(p: &Pen) {
    p.outline(2.5, 1.5, 13.5, 14.5);
    p.dashed(&[(4.5, 4.5), (11.5, 4.5)]);
    p.text_lines(4.5, 11.5, &[8.0, 10.5, 13.0]);
}

pub(super) fn paper_size(p: &Pen) {
    p.outline(4.5, 1.5, 14.5, 14.5);
    p.line(&[(2.0, 1.5), (2.0, 14.5)]);
    p.line(&[(1.0, 3.0), (2.0, 1.5), (3.0, 3.0)]);
    p.line(&[(1.0, 13.0), (2.0, 14.5), (3.0, 13.0)]);
}

pub(super) fn zoom_in(p: &Pen) {
    glass(p);
    p.line(&[(4.0, 6.5), (9.0, 6.5)]);
    p.line(&[(6.5, 4.0), (6.5, 9.0)]);
}

pub(super) fn zoom_out(p: &Pen) {
    glass(p);
    p.line(&[(4.0, 6.5), (9.0, 6.5)]);
}

pub(super) fn zoom(p: &Pen) {
    glass(p);
}

pub(super) fn zoom_actual(p: &Pen) {
    p.outline(1.5, 3.0, 14.5, 13.0);
    p.text(8.0, 8.0, "1:1", 7.0, true);
}

pub(super) fn page_width(p: &Pen) {
    p.outline(4.0, 1.5, 12.0, 14.5);
    p.line(&[(1.0, 8.0), (15.0, 8.0)]);
    p.line(&[(2.5, 6.5), (1.0, 8.0), (2.5, 9.5)]);
    p.line(&[(13.5, 6.5), (15.0, 8.0), (13.5, 9.5)]);
}

pub(super) fn new_window(p: &Pen) {
    window(p, 4.0, 1.5, 14.5, 10.5);
    window(p, 1.5, 5.5, 11.5, 14.5);
}

pub(super) fn new_docked_window(p: &Pen) {
    window(p, 1.5, 2.0, 10.0, 14.0);
    p.outline(11.0, 2.0, 14.5, 14.0);
    p.fill(11.0, 2.0, 14.5, 4.5, p.fg);
}

pub(super) fn theme(p: &Pen, accent: Color32) {
    // A brush over a swatch.
    p.solid(&[(1.5, 9.5), (8.5, 9.5), (8.5, 14.5), (1.5, 14.5)], accent);
    p.outline(1.5, 9.5, 8.5, 14.5);
    p.line_colored(&[(14.5, 1.5), (9.0, 7.0)], 1.5, p.fg);
    p.polygon(
        &[(9.0, 7.0), (10.0, 8.0), (8.5, 10.5), (6.5, 9.5)],
        Color32::TRANSPARENT,
    );
}

pub(super) fn dark_mode(p: &Pen) {
    // A circle, half filled.
    p.circle(8.0, 8.0, 6.0);
    let points: Vec<(f32, f32)> = (0..=12)
        .map(|i| {
            let a = (90.0 + 180.0 * i as f32 / 12.0_f32).to_radians();
            (8.0 + 6.0 * a.cos(), 8.0 + 6.0 * a.sin())
        })
        .collect();
    p.solid(&points, p.fg);
}
