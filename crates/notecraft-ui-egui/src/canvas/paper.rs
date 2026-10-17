//! Paper: the page color, the paper edge for a fixed paper size, and the rule lines or grid.
//! Only the visible part is drawn, so the cost does not grow with the page.

use egui::{Color32, Painter, Pos2, Rect, Stroke};
use notecraft_canvas::paper::{self, RuleKind, MAX_DOTS};
use notecraft_canvas::{PaperSize, RuleLines};
use notecraft_core::Vec2;
use notecraft_engine::PageStyle;

use super::PageToScreen;
use crate::themes::{mix, ThemeTokens};

/// Lines closer than this on screen, in points, are left out: at low zoom a fine grid would
/// only gray the page.
pub const MIN_SCREEN_SPACING: f32 = 4.0;

/// The color the paper shows: the theme's page color with the page's own color over it.
/// The highlighter blends against this.
pub(super) fn paper_color(style: PageStyle, tokens: &ThemeTokens) -> Color32 {
    let base = tokens.canvas_bg;
    let Some([r, g, b, a]) = style.color else {
        return base;
    };
    let alpha = f32::from(a) / 255.0;
    let mix = |over: u8, under: u8| {
        (f32::from(over) * alpha + f32::from(under) * (1.0 - alpha)).round() as u8
    };
    Color32::from_rgb(mix(r, base.r()), mix(g, base.g()), mix(b, base.b()))
}

/// Relative luminance of a color, 0 (black) to 1 (white).
fn luminance(c: Color32) -> f32 {
    let channel = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

/// WCAG contrast ratio of two colors (1 to 21).
fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The theme with Automatic text colors for this page: text with no color of its own is dark
/// on a light page and light on a dark one, as OneNote's Automatic font color is. The theme's
/// own text colors stay when they already read well on the page color.
pub(super) fn page_tokens(style: PageStyle, tokens: &ThemeTokens) -> ThemeTokens {
    let paper = paper_color(style, tokens);
    if contrast(tokens.text, paper) >= 4.5 {
        return *tokens;
    }
    let (text, muted) = if luminance(paper) > 0.18 {
        (
            Color32::from_rgb(0x1F, 0x1F, 0x1F),
            Color32::from_rgb(0x5F, 0x5F, 0x5F),
        )
    } else {
        (
            Color32::from_rgb(0xF2, 0xF2, 0xF2),
            Color32::from_rgb(0xB8, 0xB8, 0xB8),
        )
    };
    ThemeTokens {
        text,
        text_muted: muted,
        ..*tokens
    }
}

/// Draw the paper inside `screen`, the visible part of the canvas.
pub(super) fn draw(
    painter: &Painter,
    screen: Rect,
    map: PageToScreen,
    style: PageStyle,
    tokens: &ThemeTokens,
) {
    let page_color = style
        .color
        .map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a));
    let edge = page_rect(map, style.paper_size);
    let page = edge.map_or(screen, |page| page.intersect(screen));
    if edge.is_some() {
        // Around a fixed-size page, the canvas shows a slightly darker desk.
        painter.rect_filled(screen, 0.0, mix(tokens.canvas_bg, tokens.text, 0.06));
    }
    if page.is_positive() {
        painter.rect_filled(page, 0.0, tokens.canvas_bg);
        if let Some(color) = page_color {
            painter.rect_filled(page, 0.0, color);
        }
        let margin = margin_x(page, map, style.rule_lines);
        let color = style.rule_color.map_or(tokens.canvas_rule, |[r, g, b, a]| {
            Color32::from_rgba_unmultiplied(r, g, b, a)
        });
        paint_rules(painter, page, map, style.rule_lines, margin, color, tokens);
    }
    if let Some(edge) = edge {
        let stroke = Stroke::new(1.0, tokens.border);
        painter.rect_stroke(edge, 0.0, stroke, egui::StrokeKind::Outside);
    }
}

/// Draw the rule lines, grid, margin line, or dots of `rules` inside `page`, a part of the
/// paper on screen. Shared by the canvas and the Rule Lines menu thumbnails. `margin` is the
/// screen x of the red margin line, if it shows (the canvas computes it from the page; a
/// thumbnail places it by [`thumbnail_margin_x`]). `color` is the color of the lines and dots.
fn paint_rules(
    painter: &Painter,
    page: Rect,
    map: PageToScreen,
    rules: RuleLines,
    margin: Option<f32>,
    color: Color32,
    tokens: &ThemeTokens,
) {
    let stroke = Stroke::new(1.0, color);
    let (rows, columns) = rule_lines(page, map, rules);
    for y in rows {
        painter.hline(page.x_range(), painter.round_to_pixel_center(y), stroke);
    }
    for x in columns {
        painter.vline(painter.round_to_pixel_center(x), page.y_range(), stroke);
    }
    // The margin goes after the horizontal rules so it sits on top of them.
    if let Some(x) = margin {
        let stroke = Stroke::new(1.0, tokens.canvas_margin_rule);
        painter.vline(painter.round_to_pixel_center(x), page.y_range(), stroke);
    }
    let radius = dot_radius(map.zoom);
    for dot in dot_positions(page, map, rules) {
        painter.circle_filled(dot, radius, color);
    }
}

/// Zoom of a Rule Lines menu thumbnail: a thumbnail shows the top left corner of the page at
/// this scale, so ruled and grid spacings keep their real ratios.
pub(crate) const THUMBNAIL_ZOOM: f32 = 0.45;

/// Where a thumbnail's margin line sits, as a fraction of the card width from its left edge.
/// A real page is 816 units wide and the margin is 96 from the left (about 12%), so the line
/// stays near the left edge whatever the card size.
pub(crate) const THUMBNAIL_MARGIN_FRACTION: f32 = 0.18;

/// Screen x of the margin line in a thumbnail card `rect`: near the left edge, only for ruled
/// styles. The thumbnail shows the top left corner of a page at [`THUMBNAIL_ZOOM`], which is
/// too small to hold the real 1 inch margin, so the line is placed by fraction instead.
pub(crate) fn thumbnail_margin_x(rect: Rect, rules: RuleLines) -> Option<f32> {
    rules.margin()?;
    Some(rect.min.x + rect.width() * THUMBNAIL_MARGIN_FRACTION)
}

/// Paint a miniature page in `rect` with `rules`, using the same drawing as the canvas.
pub(crate) fn paint_thumbnail(
    painter: &Painter,
    rect: Rect,
    rules: RuleLines,
    tokens: &ThemeTokens,
) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    painter.rect_filled(rect, 0.0, tokens.canvas_bg);
    let map = PageToScreen {
        origin: rect.min,
        zoom: THUMBNAIL_ZOOM,
    };
    paint_rules(
        &painter,
        rect,
        map,
        rules,
        thumbnail_margin_x(rect, rules),
        tokens.canvas_rule,
        tokens,
    );
}

/// The paper on screen, or `None` for an auto-sized page, which fills the canvas.
fn page_rect(map: PageToScreen, size: PaperSize) -> Option<Rect> {
    let size = size.size()?;
    Some(Rect::from_min_max(map.pos(Vec2::ZERO), map.pos(size)))
}

/// Screen x of the red margin line, only for ruled styles and only when it is inside `area`.
fn margin_x(area: Rect, map: PageToScreen, rules: RuleLines) -> Option<f32> {
    let x = map.pos(Vec2::new(rules.margin()?, 0.0)).x;
    area.x_range().contains(x).then_some(x)
}

/// Radius of a paper dot in points: 1.5 at 100%, growing and shrinking with the zoom but kept
/// between 1.0 and 3.0 so dots stay visible when zoomed out and never become blobs.
fn dot_radius(zoom: f32) -> f32 {
    let zoom = if zoom.is_finite() { zoom } else { 1.0 };
    (1.5 * zoom).clamp(1.0, 3.0)
}

/// Largest dot stride tried: past it a view draws no dots.
const MAX_DOT_STRIDE: u16 = 4096;

/// Screen positions of the dots inside `area`, one at every crossing of the dotted paper
/// grid. Empty for other styles or when the dots would be closer than
/// [`MIN_SCREEN_SPACING`]. When more than [`MAX_DOTS`] would be needed, only every k-th row
/// and column is kept (counted from the page origin, so the dots stay put while panning).
fn dot_positions(area: Rect, map: PageToScreen, rules: RuleLines) -> Vec<Pos2> {
    let spacing = rules.spacing();
    if rules.kind() != RuleKind::Dots || spacing * map.zoom < MIN_SCREEN_SPACING {
        return Vec::new();
    }
    let (min, max) = map.page_rect(area);
    let Some(spacing) = dot_spacing(spacing, min, max) else {
        return Vec::new();
    };
    let rows: Vec<f32> = paper::line_positions(spacing, min.y, max.y)
        .map(|y| map.pos(Vec2::new(0.0, y)).y)
        .collect();
    let columns: Vec<f32> = paper::line_positions(spacing, min.x, max.x)
        .map(|x| map.pos(Vec2::new(x, 0.0)).x)
        .collect();
    if rows.len().saturating_mul(columns.len()) > MAX_DOTS {
        return Vec::new();
    }
    let mut dots = Vec::with_capacity(rows.len() * columns.len());
    for &y in &rows {
        for &x in &columns {
            dots.push(Pos2::new(x, y));
        }
    }
    dots
}

/// The page spacing to draw dots at: `spacing` times the smallest whole stride that keeps the
/// dots between `min` and `max` (page units) within [`MAX_DOTS`]. `None` if no stride up to
/// [`MAX_DOT_STRIDE`] does.
fn dot_spacing(spacing: f32, min: Vec2, max: Vec2) -> Option<f32> {
    (1..=MAX_DOT_STRIDE)
        .map(|k| spacing * f32::from(k))
        .find(|&s| {
            span_count(s, min.y, max.y).saturating_mul(span_count(s, min.x, max.x)) <= MAX_DOTS
        })
}

/// How many multiples of `spacing` fall within `min..=max`; `usize::MAX` when that cannot be
/// counted (bad numbers or absurdly many).
fn span_count(spacing: f32, min: f32, max: f32) -> usize {
    if !(spacing > 0.0 && spacing.is_finite() && min.is_finite() && max.is_finite()) {
        return usize::MAX;
    }
    let count = (max / spacing).floor() - (min / spacing).ceil() + 1.0;
    if count <= 0.0 {
        0
    } else if count < 1.0e9 {
        // count is a whole number below 1e9, so the cast is exact.
        count as usize
    } else {
        usize::MAX
    }
}

/// Screen positions of the horizontal lines (y) and vertical lines (x) inside `area`.
fn rule_lines(area: Rect, map: PageToScreen, rules: RuleLines) -> (Vec<f32>, Vec<f32>) {
    let spacing = rules.spacing();
    if matches!(rules.kind(), RuleKind::None | RuleKind::Dots)
        || spacing * map.zoom < MIN_SCREEN_SPACING
    {
        return (Vec::new(), Vec::new());
    }
    let (min, max) = map.page_rect(area);
    let rows = paper::line_positions(spacing, min.y, max.y)
        .map(|y| map.pos(Vec2::new(0.0, y)).y)
        .collect();
    let columns = if rules.kind() == RuleKind::Grid {
        paper::line_positions(spacing, min.x, max.x)
            .map(|x| map.pos(Vec2::new(x, 0.0)).x)
            .collect()
    } else {
        Vec::new()
    };
    (rows, columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(zoom: f32) -> PageToScreen {
        PageToScreen {
            origin: egui::pos2(100.0, 50.0),
            zoom,
        }
    }

    #[test]
    fn automatic_text_reads_on_any_page_color() {
        use crate::themes::Preset;
        for preset in Preset::ALL {
            for dark in [false, true] {
                let theme = ThemeTokens::for_variant(preset, dark);
                for color in [None, Some([255, 255, 255, 255]), Some([20, 20, 30, 255])] {
                    let style = PageStyle {
                        color,
                        ..PageStyle::default()
                    };
                    let page = page_tokens(style, &theme);
                    let paper = paper_color(style, &theme);
                    assert!(
                        contrast(page.text, paper) >= 4.5,
                        "{preset:?} dark={dark} page {color:?}"
                    );
                }
                // A page with the theme's own color keeps the theme's text.
                assert_eq!(page_tokens(PageStyle::default(), &theme).text, theme.text);
            }
        }
    }

    #[test]
    fn ruled_paper_has_rows_only_and_grids_have_both() {
        let area = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(200.0, 100.0));
        let (rows, columns) = rule_lines(area, map(1.0), RuleLines::Narrow);
        // Narrow lines are 24 units apart: page y 0, 24, 48, 72, 96.
        assert_eq!(rows, [50.0, 74.0, 98.0, 122.0, 146.0]);
        assert!(columns.is_empty());
        let (rows, columns) = rule_lines(area, map(1.0), RuleLines::GridLarge);
        assert_eq!(rows, [50.0, 98.0, 146.0]);
        assert_eq!(columns, [100.0, 148.0, 196.0, 244.0, 292.0]);
        let (rows, columns) = rule_lines(area, map(1.0), RuleLines::None);
        assert!(rows.is_empty() && columns.is_empty());
    }

    #[test]
    fn lines_follow_the_zoom_and_vanish_when_too_dense() {
        let area = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(200.0, 100.0));
        let (rows, _) = rule_lines(area, map(2.0), RuleLines::Narrow);
        assert_eq!(rows, [50.0, 98.0, 146.0]);
        // A 1/8 inch grid at 10% is 1.2 points apart: not drawn.
        let (rows, columns) = rule_lines(area, map(0.1), RuleLines::GridSmall);
        assert!(rows.is_empty() && columns.is_empty());
    }

    #[test]
    fn dots_sit_at_quarter_inch_crossings() {
        let area = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(50.0, 50.0));
        let dots = dot_positions(area, map(1.0), RuleLines::Dotted);
        // Page x and y 0, 24, 48 give 3 columns and 3 rows.
        assert_eq!(dots.len(), 9);
        assert_eq!(dots[0], egui::pos2(100.0, 50.0));
        assert_eq!(dots[1], egui::pos2(124.0, 50.0));
        assert_eq!(dots[3], egui::pos2(100.0, 74.0));
        assert_eq!(dots[8], egui::pos2(148.0, 98.0));
        // Dotted paper draws no lines, and lined paper draws no dots.
        let (rows, columns) = rule_lines(area, map(1.0), RuleLines::Dotted);
        assert!(rows.is_empty() && columns.is_empty());
        assert!(dot_positions(area, map(1.0), RuleLines::GridMedium).is_empty());
        assert!(dot_positions(area, map(1.0), RuleLines::College).is_empty());
        assert!(dot_positions(area, map(1.0), RuleLines::None).is_empty());
    }

    #[test]
    fn dots_are_capped_and_vanish_when_too_dense() {
        // 4000 x 4000 points at 100% is 167 x 167 = 27889 dots: over the cap, so every
        // second row and column is kept, on multiples of 48 page units.
        let big = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(4000.0, 4000.0));
        let dots = dot_positions(big, map(1.0), RuleLines::Dotted);
        assert_eq!(dots.len(), 84 * 84);
        for dot in &dots {
            assert_eq!((dot.x - 100.0) % 48.0, 0.0);
            assert_eq!((dot.y - 50.0) % 48.0, 0.0);
        }
        // Panning by one dot spacing keeps the same page positions, only shifted on screen.
        let panned = PageToScreen {
            origin: egui::pos2(76.0, 50.0),
            zoom: 1.0,
        };
        for dot in dot_positions(big, panned, RuleLines::Dotted) {
            assert_eq!((dot.x - 76.0) % 48.0, 0.0);
        }
        // 3000 x 3000 is 126 x 126 = 15876 dots: drawn.
        let ok = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(3000.0, 3000.0));
        let dots = dot_positions(ok, map(1.0), RuleLines::Dotted);
        assert!(!dots.is_empty() && dots.len() <= MAX_DOTS);
        // At 10% the dots are 2.4 points apart: not drawn.
        let area = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(200.0, 100.0));
        assert!(dot_positions(area, map(0.1), RuleLines::Dotted).is_empty());
        assert!(dot_positions(area, map(f32::NAN), RuleLines::Dotted).is_empty());
    }

    #[test]
    fn margin_appears_only_on_ruled_paper_inside_the_view() {
        let area = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(200.0, 100.0));
        for style in [
            RuleLines::Narrow,
            RuleLines::College,
            RuleLines::Standard,
            RuleLines::Wide,
        ] {
            // 1 inch from the page edge at 100 on screen.
            assert_eq!(margin_x(area, map(1.0), style), Some(196.0));
            assert_eq!(margin_x(area, map(2.0), style), Some(292.0));
        }
        // At 3x the margin is at 388, right of the visible area.
        assert_eq!(margin_x(area, map(3.0), RuleLines::College), None);
        // Scrolled so the page edge is far left, the margin is off screen too.
        let scrolled = PageToScreen {
            origin: egui::pos2(-500.0, 50.0),
            zoom: 1.0,
        };
        assert_eq!(margin_x(area, scrolled, RuleLines::College), None);
        for style in [
            RuleLines::None,
            RuleLines::GridSmall,
            RuleLines::GridXLarge,
            RuleLines::Dotted,
        ] {
            assert_eq!(margin_x(area, map(1.0), style), None);
        }
    }

    #[test]
    fn thumbnails_show_several_lines_and_keep_ruled_and_grid_sizes_apart() {
        let thumb = PageToScreen {
            origin: egui::pos2(0.0, 0.0),
            zoom: THUMBNAIL_ZOOM,
        };
        let area = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(66.0, 46.0));
        let counts: Vec<usize> = RuleLines::ALL
            .into_iter()
            .map(|style| {
                let (rows, _) = rule_lines(area, thumb, style);
                let dots = dot_positions(area, thumb, style);
                match style.kind() {
                    RuleKind::None => assert!(rows.is_empty() && dots.is_empty()),
                    RuleKind::Dots => assert!(dots.len() >= 4, "{style:?} dots"),
                    RuleKind::Lines => {
                        assert!(rows.len() >= 4, "{style:?} rows");
                        assert!(
                            thumbnail_margin_x(area, style).is_some(),
                            "{style:?} margin"
                        );
                    }
                    RuleKind::Grid => {
                        assert!(!rows.is_empty(), "{style:?} rows");
                        assert!(thumbnail_margin_x(area, style).is_none());
                    }
                }
                rows.len()
            })
            .collect();
        // Narrow ruled paper is denser than wide, and small grids denser than extra large.
        assert!(counts[1] >= counts[4]);
        assert!(counts[5] > counts[8]);
    }

    #[test]
    fn thumbnail_margin_is_in_the_left_quarter_of_the_card() {
        // The Rule Lines menu cards are 68 x 48 points with a 1 point border.
        let card = Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(66.0, 46.0));
        for style in RuleLines::ALL {
            match thumbnail_margin_x(card, style) {
                Some(x) => {
                    assert!(style.margin().is_some(), "{style:?}");
                    assert!(
                        x > card.min.x && x <= card.min.x + card.width() / 4.0,
                        "{style:?} margin at {x}"
                    );
                }
                None => assert!(style.margin().is_none(), "{style:?}"),
            }
        }
    }

    #[test]
    fn the_canvas_margin_is_one_inch_from_the_left_page_edge() {
        // A fixed size page and an auto-sized one both start at the map origin.
        let area = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(600.0, 100.0));
        for size in [PaperSize::Letter, PaperSize::Auto] {
            let left = page_rect(map(1.0), size).map_or(area.min.x, |edge| edge.min.x);
            assert_eq!(left, 100.0);
            let x = margin_x(area, map(1.0), RuleLines::College);
            assert_eq!(x, Some(left + 96.0));
        }
    }

    #[test]
    fn dot_radius_is_clamped() {
        assert_eq!(dot_radius(1.0), 1.5);
        assert_eq!(dot_radius(0.1), 1.0);
        assert_eq!(dot_radius(10.0), 3.0);
        assert_eq!(dot_radius(f32::NAN), 1.5);
    }

    #[test]
    fn only_fixed_sizes_have_a_paper_edge() {
        assert_eq!(page_rect(map(1.0), PaperSize::Auto), None);
        let letter = page_rect(map(0.5), PaperSize::Letter);
        let expected = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(408.0, 528.0));
        assert_eq!(letter, Some(expected));
    }
}
