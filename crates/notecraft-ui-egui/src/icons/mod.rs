//! Ribbon icons: original, simple vector drawings painted with `egui::Painter`, one function
//! per icon (AGENTS.md section 1). Every icon is designed on a 16 x 16 grid and scales to any
//! square, so the same code draws the 16 px and 32 px sizes. Nothing here is traced from or
//! modeled on Microsoft artwork.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

/// Every icon the ribbon uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    // Backstage
    Back,
    // Clipboard
    Paste,
    Cut,
    Copy,
    FormatPainter,
    // Basic Text
    Font,
    FontSize,
    Bullets,
    Numbering,
    Outdent,
    Indent,
    ClearFormatting,
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Subscript,
    Superscript,
    Highlight,
    FontColor,
    AlignLeft,
    AlignCenter,
    AlignRight,
    // Styles
    Style,
    // Tags
    TodoTag,
    Important,
    Question,
    Tag,
    FindTags,
    // Email and Meetings
    Email,
    Meeting,
}

impl Icon {
    /// Every icon, for tests.
    pub const ALL: [Self; 31] = [
        Self::Back,
        Self::Paste,
        Self::Cut,
        Self::Copy,
        Self::FormatPainter,
        Self::Font,
        Self::FontSize,
        Self::Bullets,
        Self::Numbering,
        Self::Outdent,
        Self::Indent,
        Self::ClearFormatting,
        Self::Bold,
        Self::Italic,
        Self::Underline,
        Self::Strikethrough,
        Self::Subscript,
        Self::Superscript,
        Self::Highlight,
        Self::FontColor,
        Self::AlignLeft,
        Self::AlignCenter,
        Self::AlignRight,
        Self::Style,
        Self::TodoTag,
        Self::Important,
        Self::Question,
        Self::Tag,
        Self::FindTags,
        Self::Email,
        Self::Meeting,
    ];
}

/// Colors an icon may use besides its main color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconColors {
    /// Lines and shapes.
    pub fg: Color32,
    /// Accent details: the highlight bar, the font color bar, the star.
    pub accent: Color32,
}

/// Paint `icon` into `rect`. The icon fills the largest square centered in `rect`. Empty or
/// non-finite rectangles paint nothing.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, colors: IconColors) {
    let side = rect.width().min(rect.height());
    if !side.is_finite() || side <= 0.0 {
        return;
    }
    let square = Rect::from_center_size(rect.center(), egui::vec2(side, side));
    let pen = Pen {
        painter,
        rect: square,
        unit: side / 16.0,
        fg: colors.fg,
    };
    match icon {
        Icon::Back => back(&pen),
        Icon::Paste => paste(&pen),
        Icon::Cut => cut(&pen),
        Icon::Copy => copy(&pen),
        Icon::FormatPainter => format_painter(&pen),
        Icon::Font => font(&pen),
        Icon::FontSize => font_size(&pen),
        Icon::Bullets => bullets(&pen),
        Icon::Numbering => numbering(&pen),
        Icon::Outdent => outdent(&pen),
        Icon::Indent => indent(&pen),
        Icon::ClearFormatting => clear_formatting(&pen, colors.accent),
        Icon::Bold => bold(&pen),
        Icon::Italic => italic(&pen),
        Icon::Underline => underline(&pen),
        Icon::Strikethrough => strikethrough(&pen),
        Icon::Subscript => subscript(&pen),
        Icon::Superscript => superscript(&pen),
        Icon::Highlight => highlight(&pen, colors.accent),
        Icon::FontColor => font_color(&pen, colors.accent),
        Icon::AlignLeft => align(&pen, Align::Left),
        Icon::AlignCenter => align(&pen, Align::Center),
        Icon::AlignRight => align(&pen, Align::Right),
        Icon::Style => style(&pen),
        Icon::TodoTag => todo_tag(&pen),
        Icon::Important => important(&pen, colors.accent),
        Icon::Question => question(&pen),
        Icon::Tag => tag(&pen),
        Icon::FindTags => find_tags(&pen),
        Icon::Email => email(&pen),
        Icon::Meeting => meeting(&pen),
    }
}

/// Drawing helpers in 16-unit grid coordinates.
struct Pen<'a> {
    painter: &'a Painter,
    rect: Rect,
    unit: f32,
    fg: Color32,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + egui::vec2(x, y) * self.unit
    }

    fn stroke(&self, width: f32, color: Color32) -> Stroke {
        Stroke::new((width * self.unit).max(1.0), color)
    }

    fn line(&self, points: &[(f32, f32)]) {
        self.line_colored(points, 1.0, self.fg);
    }

    fn line_colored(&self, points: &[(f32, f32)], width: f32, color: Color32) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter
            .add(Shape::line(points, self.stroke(width, color)));
    }

    fn outline(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let rect = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        self.painter.rect_stroke(
            rect,
            self.unit,
            self.stroke(1.0, self.fg),
            egui::StrokeKind::Middle,
        );
    }

    fn fill(&self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color32) {
        let rect = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        self.painter.rect_filled(rect, 0.0, color);
    }

    fn circle(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_stroke(self.at(x, y), r * self.unit, self.stroke(1.0, self.fg));
    }

    fn dot(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_filled(self.at(x, y), r * self.unit, self.fg);
    }

    /// Text centered on (x, y), `size` grid units tall. `heavy` draws it twice, offset, for a
    /// bold look without needing a bold font.
    fn text(&self, x: f32, y: f32, text: &str, size: f32, heavy: bool) {
        let font = FontId::proportional(size * self.unit);
        let pos = self.at(x, y);
        self.painter
            .text(pos, Align2::CENTER_CENTER, text, font.clone(), self.fg);
        if heavy {
            let nudge = egui::vec2(0.6 * self.unit.max(1.0), 0.0);
            self.painter
                .text(pos + nudge, Align2::CENTER_CENTER, text, font, self.fg);
        }
    }

    fn polygon(&self, points: &[(f32, f32)], fill: Color32) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(Shape::convex_polygon(
            points,
            fill,
            self.stroke(1.0, self.fg),
        ));
    }

    /// A filled convex shape with no outline.
    fn solid(&self, points: &[(f32, f32)], fill: Color32) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter
            .add(Shape::convex_polygon(points, fill, Stroke::NONE));
    }

    /// Short text lines, as on a page.
    fn text_lines(&self, x0: f32, x1: f32, ys: &[f32]) {
        for &y in ys {
            self.line(&[(x0, y), (x1, y)]);
        }
    }
}

fn back(p: &Pen) {
    p.circle(8.0, 8.0, 6.5);
    p.line(&[(11.0, 8.0), (5.0, 8.0)]);
    p.line(&[(7.5, 5.5), (5.0, 8.0), (7.5, 10.5)]);
}

fn paste(p: &Pen) {
    // A clipboard with a sheet on it.
    p.outline(2.5, 3.0, 11.5, 14.5);
    p.fill(5.0, 1.5, 9.0, 4.0, p.fg);
    p.polygon(
        &[(7.5, 7.0), (14.0, 7.0), (14.0, 15.0), (7.5, 15.0)],
        Color32::TRANSPARENT,
    );
    p.text_lines(9.0, 12.5, &[9.5, 11.5, 13.5]);
}

fn cut(p: &Pen) {
    // Two blades crossing above two finger rings.
    p.line(&[(4.5, 2.0), (10.5, 10.0)]);
    p.line(&[(11.5, 2.0), (5.5, 10.0)]);
    p.circle(4.5, 12.5, 2.2);
    p.circle(11.5, 12.5, 2.2);
}

fn copy(p: &Pen) {
    p.outline(2.0, 2.0, 10.0, 11.0);
    p.polygon(
        &[(6.0, 5.0), (14.0, 5.0), (14.0, 14.5), (6.0, 14.5)],
        Color32::TRANSPARENT,
    );
    p.text_lines(8.0, 12.0, &[8.0, 10.0, 12.0]);
}

fn format_painter(p: &Pen) {
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

fn font(p: &Pen) {
    p.text(5.5, 9.0, "A", 12.0, false);
    p.text(12.0, 10.5, "a", 9.0, false);
}

fn font_size(p: &Pen) {
    p.text(5.0, 9.5, "A", 11.0, false);
    p.line(&[(11.0, 3.0), (11.0, 13.0)]);
    p.line(&[(9.5, 4.5), (11.0, 3.0), (12.5, 4.5)]);
    p.line(&[(9.5, 11.5), (11.0, 13.0), (12.5, 11.5)]);
}

fn bullets(p: &Pen) {
    for y in [4.0, 8.0, 12.0] {
        p.dot(3.0, y, 1.2);
    }
    p.text_lines(6.0, 14.0, &[4.0, 8.0, 12.0]);
}

fn numbering(p: &Pen) {
    p.text(3.0, 4.5, "1", 5.0, false);
    p.text(3.0, 8.5, "2", 5.0, false);
    p.text(3.0, 12.5, "3", 5.0, false);
    p.text_lines(6.0, 14.0, &[4.5, 8.5, 12.5]);
}

fn outdent(p: &Pen) {
    p.text_lines(2.0, 14.0, &[3.0, 13.0]);
    p.text_lines(8.0, 14.0, &[6.5, 9.5]);
    p.line(&[(6.0, 8.0), (2.0, 8.0)]);
    p.line(&[(4.0, 6.0), (2.0, 8.0), (4.0, 10.0)]);
}

fn indent(p: &Pen) {
    p.text_lines(2.0, 14.0, &[3.0, 13.0]);
    p.text_lines(8.0, 14.0, &[6.5, 9.5]);
    p.line(&[(2.0, 8.0), (6.0, 8.0)]);
    p.line(&[(4.0, 6.0), (6.0, 8.0), (4.0, 10.0)]);
}

fn clear_formatting(p: &Pen, accent: Color32) {
    p.text(6.0, 7.0, "A", 11.0, false);
    // An eraser tip across the corner.
    p.line_colored(&[(9.0, 14.0), (14.5, 8.5)], 3.0, accent);
}

fn bold(p: &Pen) {
    p.text(8.0, 8.5, "B", 13.0, true);
}

fn italic(p: &Pen) {
    // A slanted stem with serifs.
    p.line(&[(9.5, 2.5), (6.5, 13.5)]);
    p.line(&[(7.5, 2.5), (12.0, 2.5)]);
    p.line(&[(4.0, 13.5), (8.5, 13.5)]);
}

fn underline(p: &Pen) {
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

fn strikethrough(p: &Pen) {
    p.text(8.0, 8.0, "ab", 11.0, false);
    p.line(&[(2.0, 8.5), (14.0, 8.5)]);
}

fn subscript(p: &Pen) {
    p.text(6.0, 7.0, "x", 11.0, false);
    p.text(12.5, 12.0, "2", 6.0, false);
}

fn superscript(p: &Pen) {
    p.text(6.0, 9.5, "x", 11.0, false);
    p.text(12.5, 4.0, "2", 6.0, false);
}

fn highlight(p: &Pen, accent: Color32) {
    // A marker over a colored bar.
    p.polygon(
        &[(5.0, 9.0), (10.0, 2.0), (13.0, 4.0), (8.0, 11.0)],
        Color32::TRANSPARENT,
    );
    p.line(&[(5.0, 9.0), (4.0, 12.0), (8.0, 11.0)]);
    p.fill(2.0, 13.0, 14.0, 15.0, accent);
}

fn font_color(p: &Pen, accent: Color32) {
    p.text(8.0, 6.5, "A", 11.0, false);
    p.fill(2.0, 13.0, 14.0, 15.0, accent);
}

#[derive(Clone, Copy)]
enum Align {
    Left,
    Center,
    Right,
}

fn align(p: &Pen, align: Align) {
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

fn style(p: &Pen) {
    p.text(5.0, 6.0, "A", 9.0, true);
    p.text_lines(9.0, 14.0, &[4.0, 7.0]);
    p.text_lines(2.0, 14.0, &[10.5, 13.5]);
}

fn todo_tag(p: &Pen) {
    p.outline(2.5, 2.5, 13.5, 13.5);
    p.line(&[(5.0, 8.0), (7.0, 10.5), (11.5, 5.0)]);
}

fn important(p: &Pen, accent: Color32) {
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

fn question(p: &Pen) {
    p.circle(8.0, 8.0, 6.5);
    p.text(8.0, 8.0, "?", 10.0, true);
}

fn tag(p: &Pen) {
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

fn find_tags(p: &Pen) {
    p.polygon(
        &[(1.5, 2.5), (7.5, 2.5), (10.5, 5.5), (7.5, 8.5), (1.5, 8.5)],
        Color32::TRANSPARENT,
    );
    p.circle(10.0, 10.5, 3.0);
    p.line(&[(12.2, 12.7), (15.0, 15.5)]);
}

fn email(p: &Pen) {
    p.outline(1.5, 3.5, 14.5, 12.5);
    p.line(&[(1.5, 3.5), (8.0, 9.0), (14.5, 3.5)]);
}

fn meeting(p: &Pen) {
    // A calendar page with two people's heads in the corner.
    p.outline(1.5, 3.0, 12.5, 13.5);
    p.line(&[(1.5, 6.0), (12.5, 6.0)]);
    p.line(&[(4.5, 1.5), (4.5, 4.5)]);
    p.line(&[(9.5, 1.5), (9.5, 4.5)]);
    p.dot(13.0, 10.0, 1.6);
    p.line(&[(10.5, 15.0), (11.0, 13.0), (15.0, 13.0), (15.5, 15.0)]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_paints_at_both_sizes_and_tolerates_bad_rects() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let colors = IconColors {
                fg: Color32::BLACK,
                accent: Color32::RED,
            };
            for icon in Icon::ALL {
                for side in [16.0, 32.0] {
                    let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(side, side));
                    paint(ui.painter(), rect, icon, colors);
                }
                paint(ui.painter(), Rect::NOTHING, icon, colors);
                paint(ui.painter(), Rect::ZERO, icon, colors);
                let nan = Rect::from_min_size(Pos2::ZERO, egui::vec2(f32::NAN, 4.0));
                paint(ui.painter(), nan, icon, colors);
            }
        });
        output.textures_delta.clear();
    }
}
