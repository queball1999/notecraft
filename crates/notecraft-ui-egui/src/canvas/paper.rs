//! Paper: the page color, the paper edge for a fixed paper size, and the rule lines or grid.
//! Only the visible part is drawn, so the cost does not grow with the page.

use egui::{Color32, Painter, Rect, Stroke};
use notecraft_canvas::paper::{self, RuleKind};
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

/// Screen positions of the horizontal lines (y) and vertical lines (x) inside `area`.
fn rule_lines(area: Rect, map: PageToScreen, rules: RuleLines) -> (Vec<f32>, Vec<f32>) {
    let spacing = rules.spacing();
    if rules.kind() == RuleKind::None || spacing * map.zoom < MIN_SCREEN_SPACING {
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
    fn only_fixed_sizes_have_a_paper_edge() {
        assert_eq!(page_rect(map(1.0), PaperSize::Auto), None);
        let letter = page_rect(map(0.5), PaperSize::Letter);
        let expected = Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(408.0, 528.0));
        assert_eq!(letter, Some(expected));
    }
}
