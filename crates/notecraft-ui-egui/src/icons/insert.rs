//! Icons for the Insert tab.

use egui::Color32;

use super::Pen;

pub(super) fn insert_space(p: &Pen) {
    // Two text bands pushed apart by a double arrow.
    p.text_lines(1.5, 14.5, &[2.0, 14.0]);
    p.line(&[(8.0, 4.5), (8.0, 11.5)]);
    p.line(&[(6.0, 6.5), (8.0, 4.5), (10.0, 6.5)]);
    p.line(&[(6.0, 9.5), (8.0, 11.5), (10.0, 9.5)]);
}

pub(super) fn table(p: &Pen) {
    p.outline(1.5, 2.5, 14.5, 13.5);
    p.fill(1.5, 2.5, 14.5, 5.0, p.fg);
    p.line(&[(1.5, 9.0), (14.5, 9.0)]);
    p.line(&[(6.0, 2.5), (6.0, 13.5)]);
    p.line(&[(10.5, 2.5), (10.5, 13.5)]);
}

pub(super) fn file_attachment(p: &Pen) {
    // A paper clip: an elongated loop inside a loop.
    p.line(&[
        (10.5, 5.0),
        (10.5, 11.5),
        (9.5, 13.5),
        (7.5, 14.0),
        (5.5, 13.5),
        (4.5, 11.5),
        (4.5, 3.5),
        (5.5, 2.0),
        (7.0, 1.5),
        (8.5, 2.0),
        (9.0, 3.5),
        (9.0, 10.5),
        (7.5, 12.0),
        (6.0, 10.5),
        (6.0, 5.0),
    ]);
}

pub(super) fn file_printout(p: &Pen) {
    // A page with a folded corner and printed lines.
    p.polygon(
        &[
            (3.0, 1.5),
            (10.0, 1.5),
            (13.0, 4.5),
            (13.0, 14.5),
            (3.0, 14.5),
        ],
        Color32::TRANSPARENT,
    );
    p.line(&[(10.0, 1.5), (10.0, 4.5), (13.0, 4.5)]);
    p.text_lines(5.0, 11.0, &[7.0, 9.0, 11.0, 13.0]);
}

pub(super) fn picture(p: &Pen) {
    p.outline(1.5, 2.5, 14.5, 13.5);
    p.circle(5.0, 6.0, 1.3);
    p.line(&[
        (1.5, 12.0),
        (6.0, 8.0),
        (8.5, 10.5),
        (11.0, 7.5),
        (14.5, 11.5),
    ]);
}

pub(super) fn screen_clipping(p: &Pen) {
    // A dashed selection with crop corners.
    p.dashed(&[
        (4.0, 4.0),
        (12.0, 4.0),
        (12.0, 12.0),
        (4.0, 12.0),
        (4.0, 4.0),
    ]);
    p.line(&[(1.5, 4.0), (4.0, 4.0), (4.0, 1.5)]);
    p.line(&[(14.5, 12.0), (12.0, 12.0), (12.0, 14.5)]);
}

pub(super) fn link(p: &Pen) {
    // Two chain links at an angle.
    p.polygon(
        &[(2.0, 9.5), (5.5, 6.0), (8.0, 8.5), (4.5, 12.0)],
        Color32::TRANSPARENT,
    );
    p.polygon(
        &[(8.0, 7.5), (11.5, 4.0), (14.0, 6.5), (10.5, 10.0)],
        Color32::TRANSPARENT,
    );
    p.line(&[(6.0, 10.0), (10.0, 6.0)]);
}

pub(super) fn audio(p: &Pen) {
    // A microphone on a stand.
    p.outline(5.5, 1.5, 10.5, 9.5);
    p.arc(8.0, 8.0, 4.5, 0.0, 180.0);
    p.line(&[(8.0, 12.5), (8.0, 14.5)]);
    p.line(&[(5.5, 14.5), (10.5, 14.5)]);
}

pub(super) fn video(p: &Pen) {
    // A camera body with a lens hood.
    p.outline(1.5, 4.0, 10.5, 12.0);
    p.polygon(
        &[(10.5, 7.0), (14.5, 4.5), (14.5, 11.5), (10.5, 9.0)],
        Color32::TRANSPARENT,
    );
}

fn calendar(p: &Pen, x0: f32, y0: f32, x1: f32, y1: f32) {
    p.outline(x0, y0, x1, y1);
    p.fill(x0, y0, x1, y0 + 2.5, p.fg);
}

fn clock(p: &Pen, x: f32, y: f32, r: f32) {
    p.circle(x, y, r);
    p.line(&[(x, y - r * 0.6), (x, y), (x + r * 0.5, y + r * 0.3)]);
}

pub(super) fn date(p: &Pen) {
    calendar(p, 1.5, 2.0, 14.5, 14.5);
    for row in [7.5_f32, 10.5, 13.0] {
        for col in [4.0_f32, 8.0, 12.0] {
            p.dot(col, row, 0.8);
        }
    }
}

pub(super) fn time(p: &Pen) {
    clock(p, 8.0, 8.0, 6.5);
}

pub(super) fn date_time(p: &Pen) {
    calendar(p, 1.0, 1.5, 11.0, 11.0);
    clock(p, 11.0, 11.0, 4.0);
}

pub(super) fn page_template(p: &Pen) {
    // A page with a title block and two columns.
    p.outline(2.0, 1.5, 14.0, 14.5);
    p.fill(4.0, 3.5, 12.0, 5.5, p.fg);
    p.outline(4.0, 7.5, 7.5, 12.5);
    p.text_lines(9.0, 12.0, &[8.0, 10.0, 12.0]);
}

pub(super) fn equation(p: &Pen) {
    // A summation sign drawn as strokes.
    p.line(&[
        (12.5, 3.5),
        (12.5, 2.0),
        (3.5, 2.0),
        (8.5, 8.0),
        (3.5, 14.0),
        (12.5, 14.0),
        (12.5, 12.5),
    ]);
}

pub(super) fn symbol(p: &Pen) {
    p.text(8.0, 8.5, "&", 13.0, false);
}
