//! Note tag icons (PR 2.5): original drawings for the shapes in `notecraft_markdown::TagIcon`.
//! Each is designed on the 16 by 16 grid like the ribbon icons. The icon's own color fills the
//! shapes (`Pen::fg`) and white picks out details, so a tag reads the same on any page color.

use egui::Color32;
use notecraft_markdown::TagIcon;

use super::Pen;

const DETAIL: Color32 = Color32::WHITE;

/// `color` moved toward white by `t` (0 to 1).
fn lighten(color: Color32, t: f32) -> Color32 {
    let mix = |c: u8| {
        let c = f32::from(c);
        (c + (255.0 - c) * t).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(mix(color.r()), mix(color.g()), mix(color.b()))
}

/// Draw `icon` with the pen's color as its main color.
pub(super) fn draw(p: &Pen, icon: TagIcon) {
    match icon {
        TagIcon::Checkbox => checkbox(p),
        TagIcon::Star => star(p),
        TagIcon::Question => question(p),
        TagIcon::Marker => marker(p),
        TagIcon::Person => person(p),
        TagIcon::Pin => pin(p),
        TagIcon::Phone => handset(p, 0.0),
        TagIcon::Globe => globe(p),
        TagIcon::Bulb => bulb(p),
        TagIcon::Key => key(p),
        TagIcon::Alert => alert(p),
        TagIcon::Flag => flag(p),
        TagIcon::Film => film(p),
        TagIcon::Book => book(p),
        TagIcon::Note => note(p),
        TagIcon::Quote => quote(p),
        TagIcon::Pencil => pencil(p),
        TagIcon::Chat => chat(p),
        TagIcon::Mail => mail(p),
        TagIcon::Calendar => calendar(p),
        TagIcon::Callback => callback(p),
        TagIcon::Briefcase => briefcase(p),
        TagIcon::Heart => heart(p),
        TagIcon::Dot => p.dot(8.0, 8.0, 4.0),
    }
}

fn checkbox(p: &Pen) {
    p.line_colored(
        &[
            (2.5, 2.5),
            (13.5, 2.5),
            (13.5, 13.5),
            (2.5, 13.5),
            (2.5, 2.5),
        ],
        1.4,
        p.fg,
    );
    p.line_colored(&[(5.0, 8.3), (7.2, 10.8), (11.2, 5.0)], 1.6, p.fg);
}

fn star(p: &Pen) {
    // A pentagon in the middle with a triangle on each side.
    let (cx, cy, outer, inner) = (8.0_f32, 8.6_f32, 7.4_f32, 3.0_f32);
    let at = |r: f32, i: usize, half: bool| {
        let step = 72.0_f32.to_radians();
        let angle =
            -90.0_f32.to_radians() + step * (i as f32) + if half { step / 2.0 } else { 0.0 };
        (cx + r * angle.cos(), cy + r * angle.sin())
    };
    let inner_ring: Vec<(f32, f32)> = (0..5).map(|i| at(inner, i, true)).collect();
    p.solid(&inner_ring, p.fg);
    for i in 0..5 {
        let tip = at(outer, i, false);
        let a = at(inner, i, true);
        let b = at(inner, (i + 4) % 5, true);
        p.solid(&[tip, a, b], p.fg);
    }
}

fn question(p: &Pen) {
    p.dot(8.0, 8.0, 7.2);
    p.text_colored(8.0, 8.3, "?", 11.0, DETAIL);
}

fn marker(p: &Pen) {
    // A broad marker lying at an angle, with its chisel tip and a bar of ink under it.
    p.solid(&[(9.5, 1.5), (14.0, 6.0), (8.0, 12.0), (3.5, 7.5)], p.fg);
    p.solid(
        &[(3.5, 7.5), (8.0, 12.0), (4.5, 12.5), (2.5, 9.0)],
        lighten(p.fg, 0.55),
    );
    p.line_colored(&[(1.5, 14.5), (9.5, 14.5)], 1.6, p.fg);
}

fn person(p: &Pen) {
    p.dot(8.0, 4.8, 3.0);
    p.solid(
        &[
            (2.5, 14.5),
            (3.5, 10.5),
            (6.0, 8.8),
            (10.0, 8.8),
            (12.5, 10.5),
            (13.5, 14.5),
        ],
        p.fg,
    );
}

fn pin(p: &Pen) {
    p.dot(8.0, 6.0, 4.6);
    p.solid(&[(3.9, 7.9), (12.1, 7.9), (8.0, 15.0)], p.fg);
    p.dot_colored(8.0, 6.0, 1.7, DETAIL);
}

/// A telephone handset: a thick curve with a round end for the ear and the mouth.
fn handset(p: &Pen, shift: f32) {
    let pts: Vec<(f32, f32)> = [(4.5, 4.0), (6.0, 7.6), (8.6, 10.2), (12.0, 11.6)]
        .iter()
        .map(|&(x, y)| (x + shift, y + shift))
        .collect();
    p.line_colored(&pts, 3.0, p.fg);
    if let (Some(first), Some(last)) = (pts.first(), pts.last()) {
        p.dot(first.0, first.1, 2.1);
        p.dot(last.0, last.1, 2.1);
    }
}

fn globe(p: &Pen) {
    p.circle(8.0, 8.0, 6.6);
    p.line_colored(&[(1.4, 8.0), (14.6, 8.0)], 1.0, p.fg);
    let ellipse = |rx: f32| -> Vec<(f32, f32)> {
        (0..=16)
            .map(|i| {
                let t = (i as f32) / 16.0 * std::f32::consts::TAU;
                (8.0 + rx * t.cos(), 8.0 + 6.6 * t.sin())
            })
            .collect()
    };
    p.line_colored(&ellipse(3.0), 1.0, p.fg);
    p.line_colored(&[(8.0, 1.4), (8.0, 14.6)], 1.0, p.fg);
}

fn bulb(p: &Pen) {
    p.dot(8.0, 6.2, 4.8);
    p.solid(&[(5.6, 9.0), (10.4, 9.0), (9.6, 12.2), (6.4, 12.2)], p.fg);
    p.line_colored(&[(6.4, 13.4), (9.6, 13.4)], 1.4, p.fg);
    p.line_colored(&[(6.6, 5.2), (7.4, 4.4)], 1.0, DETAIL);
}

fn key(p: &Pen) {
    p.circle(4.8, 11.2, 3.1);
    p.line_colored(&[(7.0, 9.0), (14.0, 2.0)], 1.8, p.fg);
    p.line_colored(&[(11.8, 4.2), (13.4, 5.8)], 1.6, p.fg);
    p.line_colored(&[(9.6, 6.4), (11.0, 7.8)], 1.6, p.fg);
}

fn alert(p: &Pen) {
    p.solid(&[(8.0, 1.5), (15.0, 14.0), (1.0, 14.0)], p.fg);
    p.line_colored(&[(8.0, 6.0), (8.0, 10.0)], 1.6, DETAIL);
    p.dot_colored(8.0, 12.0, 0.9, DETAIL);
}

fn flag(p: &Pen) {
    p.line_colored(&[(3.5, 1.5), (3.5, 15.0)], 1.5, p.fg);
    p.solid(&[(3.5, 2.0), (13.5, 4.8), (3.5, 8.5)], p.fg);
}

fn film(p: &Pen) {
    p.fill(1.5, 3.0, 14.5, 13.0, p.fg);
    for y in [4.0, 7.0, 10.0] {
        p.fill(2.2, y, 3.6, y + 1.6, DETAIL);
        p.fill(12.4, y, 13.8, y + 1.6, DETAIL);
    }
    p.fill(5.2, 4.6, 10.8, 11.4, DETAIL);
}

fn book(p: &Pen) {
    p.solid(&[(1.5, 3.5), (7.5, 4.8), (7.5, 13.8), (1.5, 12.5)], p.fg);
    p.solid(&[(8.5, 4.8), (14.5, 3.5), (14.5, 12.5), (8.5, 13.8)], p.fg);
    p.line_colored(&[(3.2, 6.6), (6.2, 7.2)], 0.9, DETAIL);
    p.line_colored(&[(3.2, 9.0), (6.2, 9.6)], 0.9, DETAIL);
    p.line_colored(&[(9.8, 7.2), (12.8, 6.6)], 0.9, DETAIL);
    p.line_colored(&[(9.8, 9.6), (12.8, 9.0)], 0.9, DETAIL);
}

fn note(p: &Pen) {
    p.dot(4.6, 12.4, 2.3);
    p.dot(11.2, 11.0, 2.3);
    p.line_colored(&[(6.7, 12.2), (6.7, 3.6)], 1.4, p.fg);
    p.line_colored(&[(13.3, 10.8), (13.3, 2.4)], 1.4, p.fg);
    p.solid(&[(6.7, 3.2), (13.3, 1.9), (13.3, 4.7), (6.7, 6.0)], p.fg);
}

fn quote(p: &Pen) {
    for x in [2.0_f32, 8.8] {
        p.dot(x + 2.4, 6.0, 2.4);
        p.solid(&[(x, 6.2), (x + 4.8, 6.2), (x + 0.6, 12.2)], p.fg);
    }
}

fn pencil(p: &Pen) {
    p.solid(&[(10.5, 1.5), (14.5, 5.5), (6.0, 14.0), (2.0, 10.0)], p.fg);
    p.solid(
        &[(2.0, 10.0), (6.0, 14.0), (1.5, 14.8)],
        lighten(p.fg, 0.55),
    );
    p.line_colored(&[(9.0, 3.0), (13.0, 7.0)], 1.0, DETAIL);
}

fn chat(p: &Pen) {
    p.solid(&[(1.5, 2.5), (14.5, 2.5), (14.5, 10.5), (1.5, 10.5)], p.fg);
    p.solid(&[(4.0, 10.0), (8.5, 10.0), (4.0, 14.2)], p.fg);
    for x in [5.0, 8.0, 11.0] {
        p.dot_colored(x, 6.5, 0.95, DETAIL);
    }
}

fn mail(p: &Pen) {
    p.fill(1.5, 3.5, 14.5, 12.5, p.fg);
    p.line_colored(&[(1.8, 3.8), (8.0, 9.0), (14.2, 3.8)], 1.2, DETAIL);
}

fn calendar(p: &Pen) {
    p.line_colored(
        &[
            (2.0, 3.0),
            (14.0, 3.0),
            (14.0, 14.5),
            (2.0, 14.5),
            (2.0, 3.0),
        ],
        1.2,
        p.fg,
    );
    p.fill(2.0, 3.0, 14.0, 6.6, p.fg);
    p.line_colored(&[(5.0, 1.2), (5.0, 4.4)], 1.4, p.fg);
    p.line_colored(&[(11.0, 1.2), (11.0, 4.4)], 1.4, p.fg);
    for (x, y) in [
        (5.0, 9.0),
        (8.0, 9.0),
        (11.0, 9.0),
        (5.0, 12.0),
        (8.0, 12.0),
    ] {
        p.dot(x, y, 0.9);
    }
}

fn callback(p: &Pen) {
    handset(p, 1.0);
    p.line_colored(&[(6.0, 6.0), (2.2, 2.2)], 1.2, p.fg);
    p.line_colored(&[(2.2, 6.0), (2.2, 2.2), (6.0, 2.2)], 1.2, p.fg);
}

fn briefcase(p: &Pen) {
    p.line_colored(
        &[(5.5, 5.4), (5.5, 2.8), (10.5, 2.8), (10.5, 5.4)],
        1.4,
        p.fg,
    );
    p.fill(1.5, 5.0, 14.5, 13.8, p.fg);
    p.line_colored(&[(1.8, 9.2), (14.2, 9.2)], 1.0, DETAIL);
}

fn heart(p: &Pen) {
    p.dot(5.3, 5.8, 3.4);
    p.dot(10.7, 5.8, 3.4);
    p.solid(&[(2.0, 7.4), (14.0, 7.4), (8.0, 14.4)], p.fg);
}
