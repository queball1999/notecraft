//! Ribbon icons: original, simple vector drawings painted with `egui::Painter`, one function
//! per icon (AGENTS.md section 1). Every icon is designed on a 16 x 16 grid and scales to any
//! square, so the same code draws the 16 px and 32 px sizes. Nothing here is traced from or
//! modeled on Microsoft artwork.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

mod draw;
mod home;
mod insert;

/// Declares [`Icon`], [`Icon::ALL`], and the dispatch from each icon to its drawing.
macro_rules! icons {
    ($($name:ident => $draw:expr;)*) => {
        /// Every icon the ribbon uses.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Icon {
            $($name,)*
        }

        impl Icon {
            /// Every icon, for tests.
            pub const ALL: &'static [Self] = &[$(Self::$name),*];
        }

        fn draw(icon: Icon, p: &Pen, c: IconColors) {
            match icon {
                $(Icon::$name => {
                    let draw: fn(&Pen, IconColors) = $draw;
                    draw(p, c);
                })*
            }
        }
    };
}

icons! {
    // Backstage
    Back => |p, _| home::back(p);
    // Home: Clipboard
    Paste => |p, _| home::paste(p);
    Cut => |p, _| home::cut(p);
    Copy => |p, _| home::copy(p);
    FormatPainter => |p, _| home::format_painter(p);
    // Home: Basic Text
    Font => |p, _| home::font(p);
    FontSize => |p, _| home::font_size(p);
    Bullets => |p, _| home::bullets(p);
    Numbering => |p, _| home::numbering(p);
    Outdent => |p, _| home::outdent(p);
    Indent => |p, _| home::indent(p);
    ClearFormatting => |p, c| home::clear_formatting(p, c.accent);
    Bold => |p, _| home::bold(p);
    Italic => |p, _| home::italic(p);
    Underline => |p, _| home::underline(p);
    Strikethrough => |p, _| home::strikethrough(p);
    Subscript => |p, _| home::subscript(p);
    Superscript => |p, _| home::superscript(p);
    Highlight => |p, c| home::highlight(p, c.accent);
    FontColor => |p, c| home::font_color(p, c.accent);
    AlignLeft => |p, _| home::align(p, home::Align::Left);
    AlignCenter => |p, _| home::align(p, home::Align::Center);
    AlignRight => |p, _| home::align(p, home::Align::Right);
    // Home: Styles
    Style => |p, _| home::style(p);
    // Home: Tags
    TodoTag => |p, _| home::todo_tag(p);
    Important => |p, c| home::important(p, c.accent);
    Question => |p, _| home::question(p);
    Tag => |p, _| home::tag(p);
    FindTags => |p, _| home::find_tags(p);
    // Home: Email and Meetings
    Email => |p, _| home::email(p);
    Meeting => |p, _| home::meeting(p);
    // Insert
    InsertSpace => |p, _| insert::insert_space(p);
    Table => |p, _| insert::table(p);
    FileAttachment => |p, _| insert::file_attachment(p);
    FilePrintout => |p, _| insert::file_printout(p);
    Picture => |p, _| insert::picture(p);
    ScreenClipping => |p, _| insert::screen_clipping(p);
    Link => |p, _| insert::link(p);
    Audio => |p, _| insert::audio(p);
    Video => |p, _| insert::video(p);
    Date => |p, _| insert::date(p);
    Time => |p, _| insert::time(p);
    DateTime => |p, _| insert::date_time(p);
    PageTemplate => |p, _| insert::page_template(p);
    Equation => |p, _| insert::equation(p);
    Symbol => |p, _| insert::symbol(p);
    // Draw
    TypeTool => |p, _| draw::type_tool(p);
    Lasso => |p, _| draw::lasso(p);
    PanHand => |p, _| draw::pan_hand(p);
    Pen => |p, c| draw::pen(p, c.accent);
    Highlighter => |p, c| draw::highlighter(p, c.accent);
    Eraser => |p, _| draw::eraser(p);
    Color => |p, c| draw::color(p, c.accent);
    Thickness => |p, _| draw::thickness(p);
    Ruler => |p, _| draw::ruler(p);
    Shapes => |p, _| draw::shapes(p);
    Delete => |p, _| draw::delete(p);
    Arrange => |p, _| draw::arrange(p);
    Rotate => |p, _| draw::rotate(p);
    InkToShape => |p, _| draw::ink_to_shape(p);
    InkToText => |p, _| draw::ink_to_text(p);
    InkToMath => |p, _| draw::ink_to_math(p);
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
    draw(icon, &pen, colors);
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

    /// An arc around (x, y) from `from` to `to` degrees; 0 points right and angles grow
    /// clockwise (screen coordinates).
    pub(super) fn arc(&self, x: f32, y: f32, r: f32, from: f32, to: f32) {
        let steps = 16;
        let points: Vec<(f32, f32)> = (0..=steps)
            .map(|i| {
                let t = from + (to - from) * (i as f32) / (steps as f32);
                let (sin, cos) = t.to_radians().sin_cos();
                (x + r * cos, y + r * sin)
            })
            .collect();
        self.line(&points);
    }

    /// A dashed polyline.
    pub(super) fn dashed(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|&(x, y)| self.at(x, y)).collect();
        let dash = 2.0 * self.unit;
        self.painter.extend(Shape::dashed_line(
            &points,
            self.stroke(1.0, self.fg),
            dash,
            dash,
        ));
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
            for &icon in Icon::ALL {
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
