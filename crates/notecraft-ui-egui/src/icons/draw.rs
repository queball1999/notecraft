//! Icons for the Draw tab.

use egui::Color32;

use super::Pen;

pub(super) fn type_tool(p: &Pen) {
    // A text cursor (I-beam).
    p.line(&[(8.0, 2.5), (8.0, 13.5)]);
    p.line(&[(5.5, 2.0), (6.5, 2.5), (9.5, 2.5), (10.5, 2.0)]);
    p.line(&[(5.5, 14.0), (6.5, 13.5), (9.5, 13.5), (10.5, 14.0)]);
}

pub(super) fn lasso(p: &Pen) {
    p.dashed(&[
        (8.0, 2.0),
        (12.5, 3.5),
        (14.0, 7.0),
        (12.0, 10.0),
        (7.5, 10.5),
        (3.0, 9.0),
        (2.0, 5.5),
        (4.5, 2.8),
        (8.0, 2.0),
    ]);
    p.line(&[(5.0, 10.0), (4.0, 12.5), (5.5, 14.5)]);
}

pub(super) fn pan_hand(p: &Pen) {
    // An open hand: palm and four fingers, thumb to the side.
    p.line(&[
        (4.5, 9.0),
        (4.5, 12.0),
        (6.5, 14.5),
        (11.0, 14.5),
        (12.5, 12.0),
        (12.5, 6.0),
    ]);
    for (x, top) in [(5.5_f32, 4.0_f32), (7.8, 2.5), (10.1, 3.0), (12.5, 5.0)] {
        p.line(&[(x, 10.0), (x, top)]);
    }
    p.line(&[(4.5, 9.0), (2.5, 7.0), (2.0, 8.5), (4.0, 11.5)]);
}

pub(super) fn pen(p: &Pen, accent: Color32) {
    // A pen held at an angle, with a short stroke of ink.
    p.polygon(
        &[(11.0, 1.5), (14.5, 5.0), (6.0, 13.5), (2.5, 10.0)],
        Color32::TRANSPARENT,
    );
    p.line(&[(2.5, 10.0), (1.5, 14.5), (6.0, 13.5)]);
    p.line_colored(&[(8.0, 15.0), (14.5, 15.0)], 1.5, accent);
}

pub(super) fn highlighter(p: &Pen, accent: Color32) {
    // A broad marker over a wide stroke.
    p.polygon(
        &[(9.5, 1.5), (14.0, 6.0), (8.0, 12.0), (3.5, 7.5)],
        Color32::TRANSPARENT,
    );
    p.line(&[(3.5, 7.5), (2.0, 11.0), (4.5, 11.5), (8.0, 12.0)]);
    p.fill(1.5, 13.0, 14.5, 15.0, accent);
}

pub(super) fn eraser(p: &Pen) {
    p.polygon(
        &[(9.5, 2.0), (14.5, 7.0), (8.0, 13.5), (3.0, 8.5)],
        Color32::TRANSPARENT,
    );
    p.line(&[(6.0, 5.5), (11.0, 10.5)]);
    p.line(&[(1.5, 14.5), (14.5, 14.5)]);
}

pub(super) fn color(p: &Pen, accent: Color32) {
    // A paint palette with a thumb hole and color wells.
    p.circle(8.0, 8.0, 6.5);
    p.circle(10.5, 11.0, 1.4);
    p.dot(5.0, 6.0, 1.2);
    p.dot(8.0, 4.5, 1.2);
    p.solid(
        &[(10.0, 5.0), (12.0, 5.0), (12.0, 7.0), (10.0, 7.0)],
        accent,
    );
}

pub(super) fn thickness(p: &Pen) {
    p.line_colored(&[(2.0, 3.5), (14.0, 3.5)], 0.75, p.fg);
    p.line_colored(&[(2.0, 7.5), (14.0, 7.5)], 1.5, p.fg);
    p.line_colored(&[(2.0, 12.5), (14.0, 12.5)], 3.0, p.fg);
}

pub(super) fn ruler(p: &Pen) {
    p.polygon(
        &[(1.5, 11.0), (11.0, 1.5), (14.5, 5.0), (5.0, 14.5)],
        Color32::TRANSPARENT,
    );
    for i in 0..5 {
        let t = 3.0 + 2.0 * i as f32;
        let (x, y) = (t, 12.5 - t + 2.5);
        let len = if i % 2 == 0 { 2.0 } else { 1.2 };
        p.line(&[(x, y), (x + len, y + len)]);
    }
}

pub(super) fn shapes(p: &Pen) {
    p.circle(5.0, 5.0, 3.5);
    p.outline(8.0, 8.5, 14.5, 14.5);
    p.polygon(
        &[(4.5, 9.0), (8.0, 14.5), (1.0, 14.5)],
        Color32::TRANSPARENT,
    );
}

pub(super) fn delete(p: &Pen) {
    p.line_colored(&[(3.5, 3.5), (12.5, 12.5)], 1.5, p.fg);
    p.line_colored(&[(12.5, 3.5), (3.5, 12.5)], 1.5, p.fg);
}

pub(super) fn arrange(p: &Pen) {
    // Two overlapping sheets, the front one filled.
    p.outline(1.5, 1.5, 9.5, 9.5);
    p.fill(6.5, 6.5, 14.5, 14.5, p.fg);
}

pub(super) fn rotate(p: &Pen) {
    p.arc(8.0, 8.5, 5.5, 200.0, 470.0);
    p.line(&[(12.0, 1.5), (13.5, 3.5), (11.0, 4.5)]);
}

/// A loose hand-drawn squiggle on the left half.
fn squiggle(p: &Pen) {
    p.line(&[(1.0, 9.0), (2.5, 6.0), (4.0, 9.5), (5.5, 5.5), (7.0, 10.0)]);
    p.line(&[(7.5, 8.0), (9.0, 8.0)]);
    p.line(&[(8.0, 6.8), (9.0, 8.0), (8.0, 9.2)]);
}

pub(super) fn ink_to_shape(p: &Pen) {
    squiggle(p);
    p.outline(10.0, 5.0, 15.0, 11.0);
}

pub(super) fn ink_to_text(p: &Pen) {
    squiggle(p);
    p.text(12.5, 8.0, "T", 9.0, true);
}

pub(super) fn ink_to_math(p: &Pen) {
    squiggle(p);
    p.line(&[
        (14.5, 5.0),
        (10.5, 5.0),
        (12.5, 8.0),
        (10.5, 11.0),
        (14.5, 11.0),
    ]);
}
