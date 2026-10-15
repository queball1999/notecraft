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
        let stroke = Stroke::new(1.0, tokens.canvas_rule);
        let (rows, columns) = rule_lines(page, map, style.rule_lines);
        for y in rows {
            painter.hline(page.x_range(), painter.round_to_pixel_center(y), stroke);
        }
        for x in columns {
            painter.vline(painter.round_to_pixel_center(x), page.y_range(), stroke);
        }
        // The margin goes after the horizontal rules so it sits on top of them.
        if let Some(x) = margin_x(page, map, style.rule_lines) {
            let stroke = Stroke::new(1.0, tokens.canvas_margin_rule);
            painter.vline(painter.round_to_pixel_center(x), page.y_range(), stroke);
        }
        let radius = dot_radius(map.zoom);
        for dot in dot_positions(page, map, style.rule_lines) {
            painter.circle_filled(dot, radius, tokens.canvas_rule);
        }
    }
    if let Some(edge) = edge {
        let stroke = Stroke::new(1.0, tokens.border);
        painter.rect_stroke(edge, 0.0, stroke, egui::StrokeKind::Outside);
    }
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
