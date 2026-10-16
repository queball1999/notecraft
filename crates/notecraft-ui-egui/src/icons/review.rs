//! Icons for the History and Review tabs.

use egui::Color32;

use super::Pen;

/// A page outline with a folded corner, used by several icons.
fn page(p: &Pen, x0: f32, y0: f32, x1: f32, y1: f32) {
    let fold = (x1 - x0) * 0.3;
    p.polygon(
        &[
            (x0, y0),
            (x1 - fold, y0),
            (x1, y0 + fold),
            (x1, y1),
            (x0, y1),
        ],
        Color32::TRANSPARENT,
    );
}

/// A head and shoulders.
fn person(p: &Pen, x: f32, y: f32, scale: f32) {
    p.circle(x, y - 2.5 * scale, 2.0 * scale);
    p.arc(x, y + 3.5 * scale, 3.5 * scale, 180.0, 360.0);
}

pub(super) fn next_unread(p: &Pen, accent: Color32) {
    page(p, 1.5, 1.5, 10.5, 14.5);
    p.solid(&[(5.0, 7.0), (7.0, 7.0), (7.0, 9.0), (5.0, 9.0)], accent);
    p.line(&[(10.5, 8.0), (15.0, 8.0)]);
    p.line(&[(13.0, 6.0), (15.0, 8.0), (13.0, 10.0)]);
}

pub(super) fn mark_read(p: &Pen) {
    page(p, 2.5, 1.5, 13.5, 14.5);
    p.line(&[(5.0, 8.5), (7.5, 11.0), (11.5, 5.5)]);
}

pub(super) fn show_unread(p: &Pen, accent: Color32) {
    page(p, 2.5, 1.5, 13.5, 14.5);
    p.fill(4.5, 5.0, 11.5, 7.0, accent);
    p.text_lines(4.5, 11.5, &[9.5, 12.0]);
}

pub(super) fn find_by_author(p: &Pen) {
    person(p, 6.0, 7.0, 1.0);
    p.circle(11.5, 11.0, 2.5);
    p.line(&[(13.3, 12.8), (15.0, 14.5)]);
}

pub(super) fn hide_authors(p: &Pen) {
    person(p, 8.0, 7.5, 1.2);
    p.line(&[(2.0, 14.0), (14.0, 2.0)]);
}

pub(super) fn page_versions(p: &Pen) {
    p.outline(5.0, 1.5, 14.0, 11.0);
    p.outline(3.5, 3.0, 12.5, 12.5);
    page(p, 2.0, 4.5, 11.0, 14.5);
}

pub(super) fn recycle_bin(p: &Pen) {
    p.line(&[(2.0, 4.0), (14.0, 4.0)]);
    p.line(&[(6.0, 4.0), (6.5, 2.0), (9.5, 2.0), (10.0, 4.0)]);
    p.polygon(
        &[(3.5, 4.0), (12.5, 4.0), (11.5, 14.5), (4.5, 14.5)],
        Color32::TRANSPARENT,
    );
    p.line(&[(6.5, 6.5), (6.8, 12.5)]);
    p.line(&[(9.5, 6.5), (9.2, 12.5)]);
}

pub(super) fn recent_edits(p: &Pen) {
    page(p, 1.5, 1.5, 10.5, 13.0);
    p.text_lines(3.5, 8.0, &[5.0, 7.5]);
    p.circle(11.5, 11.5, 3.5);
    p.line(&[(11.5, 9.5), (11.5, 11.5), (13.0, 12.5)]);
}

pub(super) fn spelling(p: &Pen) {
    p.text(6.5, 5.0, "ABC", 7.0, true);
    p.line(&[(4.0, 11.0), (7.0, 14.0), (13.5, 7.5)]);
}

pub(super) fn thesaurus(p: &Pen) {
    // An open book.
    p.line(&[(8.0, 4.0), (8.0, 14.0)]);
    p.polygon(
        &[(1.5, 3.0), (8.0, 4.0), (8.0, 14.0), (1.5, 13.0)],
        Color32::TRANSPARENT,
    );
    p.polygon(
        &[(8.0, 4.0), (14.5, 3.0), (14.5, 13.0), (8.0, 14.0)],
        Color32::TRANSPARENT,
    );
    p.text_lines(3.0, 6.5, &[6.5, 9.0]);
    p.text_lines(9.5, 13.0, &[6.5, 9.0]);
}

pub(super) fn language(p: &Pen) {
    // A globe: an outline, a meridian, and two parallels.
    p.circle(8.0, 8.0, 6.5);
    p.line(&[(1.5, 8.0), (14.5, 8.0)]);
    p.line(&[(2.5, 4.5), (13.5, 4.5)]);
    p.line(&[(2.5, 11.5), (13.5, 11.5)]);
    p.line(&[(8.0, 1.5), (5.5, 4.5), (5.0, 8.0), (5.5, 11.5), (8.0, 14.5)]);
    p.line(&[
        (8.0, 1.5),
        (10.5, 4.5),
        (11.0, 8.0),
        (10.5, 11.5),
        (8.0, 14.5),
    ]);
}

/// A padlock with its shackle.
fn padlock(p: &Pen, x: f32, y: f32, scale: f32) {
    p.outline(x - 4.0 * scale, y, x + 4.0 * scale, y + 6.0 * scale);
    p.arc(x, y, 2.6 * scale, 180.0, 360.0);
    p.dot(x, y + 3.0 * scale, 0.8 * scale);
}

pub(super) fn password(p: &Pen) {
    padlock(p, 8.0, 7.0, 1.2);
}

pub(super) fn lock_all(p: &Pen) {
    padlock(p, 5.0, 5.0, 0.85);
    padlock(p, 11.0, 8.5, 0.85);
}

pub(super) fn linked_notes(p: &Pen) {
    page(p, 1.5, 1.5, 9.5, 12.0);
    p.text_lines(3.5, 7.5, &[5.0, 7.5]);
    p.circle(10.5, 12.5, 2.0);
    p.circle(13.0, 10.0, 2.0);
}
