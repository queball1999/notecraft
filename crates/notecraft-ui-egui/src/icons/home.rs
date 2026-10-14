//! Icons for the File backstage and the Home tab.

use egui::Color32;

use super::Pen;

pub(super) fn back(p: &Pen) {
    p.circle(8.0, 8.0, 6.5);
    p.line(&[(11.0, 8.0), (5.0, 8.0)]);
    p.line(&[(7.5, 5.5), (5.0, 8.0), (7.5, 10.5)]);
}

pub(super) fn paste(p: &Pen) {
    // A clipboard with a sheet on it.
    p.outline(2.5, 3.0, 11.5, 14.5);
    p.fill(5.0, 1.5, 9.0, 4.0, p.fg);
    p.polygon(
        &[(7.5, 7.0), (14.0, 7.0), (14.0, 15.0), (7.5, 15.0)],
        Color32::TRANSPARENT,
    );
    p.text_lines(9.0, 12.5, &[9.5, 11.5, 13.5]);
}

pub(super) fn cut(p: &Pen) {
    // Two blades crossing above two finger rings.
    p.line(&[(4.5, 2.0), (10.5, 10.0)]);
    p.line(&[(11.5, 2.0), (5.5, 10.0)]);
    p.circle(4.5, 12.5, 2.2);
    p.circle(11.5, 12.5, 2.2);
}

pub(super) fn copy(p: &Pen) {
    p.outline(2.0, 2.0, 10.0, 11.0);
    p.polygon(
        &[(6.0, 5.0), (14.0, 5.0), (14.0, 14.5), (6.0, 14.5)],
        Color32::TRANSPARENT,
    );
    p.text_lines(8.0, 12.0, &[8.0, 10.0, 12.0]);
}

pub(super) fn format_painter(p: &Pen) {
    // A paint roller: the roller, its arm, and the handle.
    p.outline(2.0, 2.0, 12.0, 6.0);
    p.line(&[
        (12.0, 4.0),
        (14.0, 4.0),
        (14.0, 8.0),
        (8.0, 8.0),
        (8.0, 10.0),
    ]);
    p.fill(7.0, 10.0, 9.0, 15.0, p.fg);
}

pub(super) fn font(p: &Pen) {
    p.text(5.5, 9.0, "A", 12.0, false);
    p.text(12.0, 10.5, "a", 9.0, false);
}

pub(super) fn font_size(p: &Pen) {
    p.text(5.0, 9.5, "A", 11.0, false);
    p.line(&[(11.0, 3.0), (11.0, 13.0)]);
    p.line(&[(9.5, 4.5), (11.0, 3.0), (12.5, 4.5)]);
    p.line(&[(9.5, 11.5), (11.0, 13.0), (12.5, 11.5)]);
}

pub(super) fn bullets(p: &Pen) {
    for y in [4.0, 8.0, 12.0] {
        p.dot(3.0, y, 1.2);
    }
    p.text_lines(6.0, 14.0, &[4.0, 8.0, 12.0]);
}

pub(super) fn numbering(p: &Pen) {
    p.text(3.0, 4.5, "1", 5.0, false);
    p.text(3.0, 8.5, "2", 5.0, false);
    p.text(3.0, 12.5, "3", 5.0, false);
    p.text_lines(6.0, 14.0, &[4.5, 8.5, 12.5]);
}

pub(super) fn outdent(p: &Pen) {
    p.text_lines(2.0, 14.0, &[3.0, 13.0]);
    p.text_lines(8.0, 14.0, &[6.5, 9.5]);
    p.line(&[(6.0, 8.0), (2.0, 8.0)]);
    p.line(&[(4.0, 6.0), (2.0, 8.0), (4.0, 10.0)]);
}

pub(super) fn indent(p: &Pen) {
    p.text_lines(2.0, 14.0, &[3.0, 13.0]);
    p.text_lines(8.0, 14.0, &[6.5, 9.5]);
    p.line(&[(2.0, 8.0), (6.0, 8.0)]);
    p.line(&[(4.0, 6.0), (6.0, 8.0), (4.0, 10.0)]);
}

pub(super) fn clear_formatting(p: &Pen, accent: Color32) {
    p.text(6.0, 7.0, "A", 11.0, false);
    // An eraser tip across the corner.
    p.line_colored(&[(9.0, 14.0), (14.5, 8.5)], 3.0, accent);
}

pub(super) fn bold(p: &Pen) {
    p.text(8.0, 8.5, "B", 13.0, true);
}

pub(super) fn italic(p: &Pen) {
    // A slanted stem with serifs.
    p.line(&[(9.5, 2.5), (6.5, 13.5)]);
    p.line(&[(7.5, 2.5), (12.0, 2.5)]);
    p.line(&[(4.0, 13.5), (8.5, 13.5)]);
}

pub(super) fn underline(p: &Pen) {
    p.line(&[(4.5, 2.0), (4.5, 8.5)]);
    p.line(&[(11.5, 2.0), (11.5, 8.5)]);
    p.line(&[
        (4.5, 8.5),
        (5.5, 11.0),
        (8.0, 11.8),
        (10.5, 11.0),
        (11.5, 8.5),
    ]);
    p.line(&[(3.0, 14.5), (13.0, 14.5)]);
}

pub(super) fn strikethrough(p: &Pen) {
    p.text(8.0, 8.0, "ab", 11.0, false);
    p.line(&[(2.0, 8.5), (14.0, 8.5)]);
}

pub(super) fn subscript(p: &Pen) {
    p.text(6.0, 7.0, "x", 11.0, false);
    p.text(12.5, 12.0, "2", 6.0, false);
}

pub(super) fn superscript(p: &Pen) {
    p.text(6.0, 9.5, "x", 11.0, false);
    p.text(12.5, 4.0, "2", 6.0, false);
}

pub(super) fn highlight(p: &Pen, accent: Color32) {
    // A marker over a colored bar.
    p.polygon(
        &[(5.0, 9.0), (10.0, 2.0), (13.0, 4.0), (8.0, 11.0)],
        Color32::TRANSPARENT,
    );
    p.line(&[(5.0, 9.0), (4.0, 12.0), (8.0, 11.0)]);
    p.fill(2.0, 13.0, 14.0, 15.0, accent);
}

pub(super) fn font_color(p: &Pen, accent: Color32) {
    p.text(8.0, 6.5, "A", 11.0, false);
    p.fill(2.0, 13.0, 14.0, 15.0, accent);
}

#[derive(Clone, Copy)]
pub(super) enum Align {
    Left,
    Center,
    Right,
}

pub(super) fn align(p: &Pen, align: Align) {
    for (i, y) in [3.0_f32, 6.5, 10.0, 13.5].into_iter().enumerate() {
        let short = i % 2 == 1;
        let (x0, x1) = match (align, short) {
            (_, false) => (2.0, 14.0),
            (Align::Left, true) => (2.0, 10.0),
            (Align::Center, true) => (4.0, 12.0),
            (Align::Right, true) => (6.0, 14.0),
        };
        p.line(&[(x0, y), (x1, y)]);
    }
}

pub(super) fn style(p: &Pen) {
    p.text(5.0, 6.0, "A", 9.0, true);
    p.text_lines(9.0, 14.0, &[4.0, 7.0]);
    p.text_lines(2.0, 14.0, &[10.5, 13.5]);
}

pub(super) fn todo_tag(p: &Pen) {
    p.outline(2.5, 2.5, 13.5, 13.5);
    p.line(&[(5.0, 8.0), (7.0, 10.5), (11.5, 5.0)]);
}

pub(super) fn important(p: &Pen, accent: Color32) {
    // A five-pointed star, filled with the accent color, drawn as a fan of triangles so each
    // piece is convex.
    let center = (8.0_f32, 8.5_f32);
    let point = |i: usize, r: f32| {
        let angle = std::f32::consts::TAU * (i as f32) / 10.0 - std::f32::consts::FRAC_PI_2;
        (center.0 + r * angle.cos(), center.1 + r * angle.sin())
    };
    let outer = 7.0;
    let inner = 2.9;
    let ring: Vec<(f32, f32)> = (0..10)
        .map(|i| point(i, if i % 2 == 0 { outer } else { inner }))
        .collect();
    for pair in ring.windows(2) {
        if let [a, b] = pair {
            p.solid(&[center, *a, *b], accent);
        }
    }
    if let (Some(last), Some(first)) = (ring.last(), ring.first()) {
        p.solid(&[center, *last, *first], accent);
    }
    let mut closed = ring.clone();
    closed.extend(ring.first().copied());
    p.line(&closed);
}

pub(super) fn question(p: &Pen) {
    p.circle(8.0, 8.0, 6.5);
    p.text(8.0, 8.0, "?", 10.0, true);
}

pub(super) fn tag(p: &Pen) {
    p.polygon(
        &[
            (2.0, 4.0),
            (10.0, 4.0),
            (14.0, 8.0),
            (10.0, 12.0),
            (2.0, 12.0),
        ],
        Color32::TRANSPARENT,
    );
    p.dot(5.0, 8.0, 1.0);
}

pub(super) fn find_tags(p: &Pen) {
    p.polygon(
        &[(1.5, 2.5), (7.5, 2.5), (10.5, 5.5), (7.5, 8.5), (1.5, 8.5)],
        Color32::TRANSPARENT,
    );
    p.circle(10.0, 10.5, 3.0);
    p.line(&[(12.2, 12.7), (15.0, 15.5)]);
}

pub(super) fn email(p: &Pen) {
    p.outline(1.5, 3.5, 14.5, 12.5);
    p.line(&[(1.5, 3.5), (8.0, 9.0), (14.5, 3.5)]);
}

pub(super) fn meeting(p: &Pen) {
    // A calendar page with two people's heads in the corner.
    p.outline(1.5, 3.0, 12.5, 13.5);
    p.line(&[(1.5, 6.0), (12.5, 6.0)]);
    p.line(&[(4.5, 1.5), (4.5, 4.5)]);
    p.line(&[(9.5, 1.5), (9.5, 4.5)]);
    p.dot(13.0, 10.0, 1.6);
    p.line(&[(10.5, 15.0), (11.0, 13.0), (15.0, 13.0), (15.5, 15.0)]);
}
