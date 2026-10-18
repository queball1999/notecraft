//! Ribbon icons: original, simple vector drawings painted with `egui::Painter`, one function
//! per icon (AGENTS.md section 1). Every icon is designed on a 16 x 16 grid and scales to any
//! square, so the same code draws the 16 px and 32 px sizes. Nothing here is traced from or
//! modeled on Microsoft artwork.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke};

mod draw;
mod help;
mod home;
mod insert;
mod review;
mod view;

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
    // Quick access
    Undo => |p, _| home::history_arrow(p, false);
    Redo => |p, _| home::history_arrow(p, true);
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
    Select => |p, _| draw::select(p);
    AddTool => |p, c| draw::add_tool(p, c.accent);
    FormatBackground => |p, c| draw::format_background(p, c.accent);
    InkReplay => |p, _| draw::ink_replay(p);
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
    // History
    NextUnread => |p, c| review::next_unread(p, c.accent);
    MarkRead => |p, _| review::mark_read(p);
    ShowUnread => |p, c| review::show_unread(p, c.accent);
    FindByAuthor => |p, _| review::find_by_author(p);
    HideAuthors => |p, _| review::hide_authors(p);
    PageVersions => |p, _| review::page_versions(p);
    RecycleBin => |p, _| review::recycle_bin(p);
    RecentEdits => |p, _| review::recent_edits(p);
    // Review
    Spelling => |p, _| review::spelling(p);
    Thesaurus => |p, _| review::thesaurus(p);
    Language => |p, _| review::language(p);
    Password => |p, _| review::password(p);
    LockAll => |p, _| review::lock_all(p);
    LinkedNotes => |p, _| review::linked_notes(p);
    // View
    NormalView => |p, _| view::normal_view(p);
    FullPage => |p, _| view::full_page(p);
    Dock => |p, _| view::dock(p);
    Navigation => |p, _| view::navigation(p);
    PageColor => |p, c| view::page_color(p, c.accent);
    RuleLines => |p, _| view::rule_lines(p);
    HideTitle => |p, _| view::hide_title(p);
    PaperSize => |p, _| view::paper_size(p);
    ZoomIn => |p, _| view::zoom_in(p);
    ZoomOut => |p, _| view::zoom_out(p);
    Zoom => |p, _| view::zoom(p);
    ZoomActual => |p, _| view::zoom_actual(p);
    PageWidth => |p, _| view::page_width(p);
    NewWindow => |p, _| view::new_window(p);
    NewDockedWindow => |p, _| view::new_docked_window(p);
    Theme => |p, c| view::theme(p, c.accent);
    DarkMode => |p, _| view::dark_mode(p);
    // Help
    Help => |p, _| help::help(p);
    Support => |p, _| help::support(p);
    Feedback => |p, _| help::feedback(p);
    Training => |p, _| help::training(p);
    WhatsNew => |p, _| help::whats_new(p);
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

/// Which pen shape a Draw tab tool is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolShape {
    /// A slim pen with a pointed tip.
    Pen,
    /// A pencil: a body with an eraser cap, a sharpened wooden cone, and a point.
    Pencil,
    /// A broad marker with a chisel tip.
    Highlighter,
}

/// Paint a Draw tab tool as a tall pen, tip down, in `rect`. The barrel shows `color` in a
/// band and at the tip, so each tool can be told apart by its color. `body` fills the barrel
/// and `outline` draws the edges. The shape is designed on a 16 by 40 grid and fills the
/// largest such box that fits in `rect`, centered. Empty or non-finite rectangles paint
/// nothing.
pub fn paint_tool(
    painter: &Painter,
    rect: Rect,
    shape: ToolShape,
    color: Color32,
    body: Color32,
    outline: Color32,
) {
    let unit = (rect.width() / 16.0).min(rect.height() / 40.0);
    if !unit.is_finite() || unit <= 0.0 {
        return;
    }
    let size = egui::vec2(16.0, 40.0) * unit;
    let pen = Pen {
        painter,
        rect: Rect::from_center_size(rect.center(), size),
        unit,
        fg: outline,
    };
    draw::tool(&pen, shape, color, body);
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
