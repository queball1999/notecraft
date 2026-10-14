//! Ribbon icons: original, simple vector drawings painted with `egui::Painter`, one function
//! per icon (AGENTS.md section 1). Every icon is designed on a 16 x 16 grid and scales to any
//! square, so the same code draws the 16 px and 32 px sizes. Nothing here is traced from or
//! modeled on Microsoft artwork.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

mod home;

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
        Icon::Back => home::back(&pen),
        Icon::Paste => home::paste(&pen),
        Icon::Cut => home::cut(&pen),
        Icon::Copy => home::copy(&pen),
        Icon::FormatPainter => home::format_painter(&pen),
        Icon::Font => home::font(&pen),
        Icon::FontSize => home::font_size(&pen),
        Icon::Bullets => home::bullets(&pen),
        Icon::Numbering => home::numbering(&pen),
        Icon::Outdent => home::outdent(&pen),
        Icon::Indent => home::indent(&pen),
        Icon::ClearFormatting => home::clear_formatting(&pen, colors.accent),
        Icon::Bold => home::bold(&pen),
        Icon::Italic => home::italic(&pen),
        Icon::Underline => home::underline(&pen),
        Icon::Strikethrough => home::strikethrough(&pen),
        Icon::Subscript => home::subscript(&pen),
        Icon::Superscript => home::superscript(&pen),
        Icon::Highlight => home::highlight(&pen, colors.accent),
        Icon::FontColor => home::font_color(&pen, colors.accent),
        Icon::AlignLeft => home::align(&pen, home::Align::Left),
        Icon::AlignCenter => home::align(&pen, home::Align::Center),
        Icon::AlignRight => home::align(&pen, home::Align::Right),
        Icon::Style => home::style(&pen),
        Icon::TodoTag => home::todo_tag(&pen),
        Icon::Important => home::important(&pen, colors.accent),
        Icon::Question => home::question(&pen),
        Icon::Tag => home::tag(&pen),
        Icon::FindTags => home::find_tags(&pen),
        Icon::Email => home::email(&pen),
        Icon::Meeting => home::meeting(&pen),
    }
}

/// Drawing helpers in 16-unit grid coordinates.
pub(super) struct Pen<'a> {
    painter: &'a Painter,
    rect: Rect,
    unit: f32,
    pub(super) fg: Color32,
}

impl Pen<'_> {
    pub(super) fn at(&self, x: f32, y: f32) -> Pos2 {
        self.rect.min + egui::vec2(x, y) * self.unit
    }

    pub(super) fn stroke(&self, width: f32, color: Color32) -> Stroke {
        Stroke::new((width * self.unit).max(1.0), color)
    }

    pub(super) fn line(&self, points: &[(f32, f32)]) {
        self.line_colored(points, 1.0, self.fg);
    }

    pub(super) fn line_colored(&self, points: &[(f32, f32)], width: f32, color: Color32) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter
            .add(Shape::line(points, self.stroke(width, color)));
    }

    pub(super) fn outline(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let rect = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        self.painter.rect_stroke(
            rect,
            self.unit,
            self.stroke(1.0, self.fg),
            egui::StrokeKind::Middle,
        );
    }

    pub(super) fn fill(&self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color32) {
        let rect = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        self.painter.rect_filled(rect, 0.0, color);
    }

    pub(super) fn circle(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_stroke(self.at(x, y), r * self.unit, self.stroke(1.0, self.fg));
    }

    pub(super) fn dot(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_filled(self.at(x, y), r * self.unit, self.fg);
    }

    /// Text centered on (x, y), `size` grid units tall. `heavy` draws it twice, offset, for a
    /// bold look without needing a bold font.
    pub(super) fn text(&self, x: f32, y: f32, text: &str, size: f32, heavy: bool) {
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

    pub(super) fn polygon(&self, points: &[(f32, f32)], fill: Color32) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter.add(Shape::convex_polygon(
            points,
            fill,
            self.stroke(1.0, self.fg),
        ));
    }

    /// A filled convex shape with no outline.
    pub(super) fn solid(&self, points: &[(f32, f32)], fill: Color32) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter
            .add(Shape::convex_polygon(points, fill, Stroke::NONE));
    }

    /// Short text lines, as on a page.
    pub(super) fn text_lines(&self, x0: f32, x1: f32, ys: &[f32]) {
        for &y in ys {
            self.line(&[(x0, y), (x1, y)]);
        }
    }
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
