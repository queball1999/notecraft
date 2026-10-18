//! Icons for the Help tab.

use super::Pen;

/// A circle holding a question mark.
pub(super) fn help(p: &Pen) {
    p.circle(8.0, 8.0, 6.5);
    p.arc(8.0, 6.3, 2.3, 180.0, 400.0);
    p.line(&[(8.9, 8.2), (8.0, 9.2), (8.0, 10.3)]);
    p.dot(8.0, 12.2, 0.7);
}

/// A life ring: two circles with four bands between them.
pub(super) fn support(p: &Pen) {
    p.circle(8.0, 8.0, 6.5);
    p.circle(8.0, 8.0, 2.8);
    p.line(&[(3.4, 3.4), (6.0, 6.0)]);
    p.line(&[(12.6, 3.4), (10.0, 6.0)]);
    p.line(&[(3.4, 12.6), (6.0, 10.0)]);
    p.line(&[(12.6, 12.6), (10.0, 10.0)]);
}

/// A speech bubble with text lines.
pub(super) fn feedback(p: &Pen) {
    p.line(&[
        (2.0, 2.5),
        (14.0, 2.5),
        (14.0, 10.5),
        (8.0, 10.5),
        (4.5, 14.0),
        (4.5, 10.5),
        (2.0, 10.5),
        (2.0, 2.5),
    ]);
    p.text_lines(4.5, 11.5, &[5.0, 8.0]);
}

/// A four-point sparkle with a small one beside it.
pub(super) fn whats_new(p: &Pen) {
    p.line(&[
        (7.0, 1.5),
        (8.4, 6.0),
        (12.5, 7.5),
        (8.4, 9.0),
        (7.0, 13.5),
        (5.6, 9.0),
        (1.5, 7.5),
        (5.6, 6.0),
        (7.0, 1.5),
    ]);
    p.line(&[(13.0, 11.0), (13.0, 15.0)]);
    p.line(&[(11.0, 13.0), (15.0, 13.0)]);
}

/// A screen showing a play triangle.
pub(super) fn training(p: &Pen) {
    p.outline(1.5, 2.5, 14.5, 11.5);
    p.line(&[(5.5, 14.0), (10.5, 14.0)]);
    p.line(&[(8.0, 11.5), (8.0, 14.0)]);
    p.polygon(&[(6.5, 4.5), (6.5, 9.5), (10.5, 7.0)], p.fg);
}
