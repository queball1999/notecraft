//! The ink on one page: strokes in paint order with a spatial index for hit testing and for
//! drawing only what is on screen. The eraser tools (PR 1.11) and the canvas block (PR 1.9)
//! build on this.

use notecraft_core::geometry::distance_to_segment;
use notecraft_core::{Aabb, Stroke, Vec2};

use super::index::GridIndex;

/// Most strokes a page keeps (docs/markdown.md section 9).
pub const MAX_STROKES: usize = 100_000;

/// Strokes in paint order (first is at the back) and an index over their bounds. The index
/// keys are positions in the stroke list, so removing a stroke rebuilds it.
#[derive(Clone, Debug, Default)]
pub struct InkLayer {
    strokes: Vec<Stroke>,
    index: GridIndex,
}

impl PartialEq for InkLayer {
    /// Layers are equal when their strokes are; the index follows from the strokes.
    fn eq(&self, other: &Self) -> bool {
        self.strokes == other.strokes
    }
}

impl InkLayer {
    pub fn new() -> Self {
        Self::default()
    }

    /// A layer holding `strokes`, each forced into range. Strokes past the page limit are
    /// dropped, and a stroke with no points is skipped.
    pub fn from_strokes(strokes: impl IntoIterator<Item = Stroke>) -> Self {
        let mut layer = Self::new();
        for stroke in strokes {
            layer.add(stroke);
        }
        layer
    }

    pub fn strokes(&self) -> &[Stroke] {
        &self.strokes
    }

    pub fn len(&self) -> usize {
        self.strokes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strokes.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Stroke> {
        self.strokes.iter().find(|s| s.id == id)
    }

    /// An id not used by any stroke here, such as `s12`.
    pub fn next_id(&self) -> String {
        let highest = self
            .strokes
            .iter()
            .filter_map(|s| s.id.strip_prefix('s')?.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        format!("s{}", highest.saturating_add(1))
    }

    /// Add a stroke on top. Returns false, changing nothing, when the stroke has no points,
    /// the page is full, or the id is already used.
    pub fn add(&mut self, stroke: Stroke) -> bool {
        let stroke = stroke.bounded();
        let Some(bounds) = stroke.bounds() else {
            return false;
        };
        if self.strokes.len() >= MAX_STROKES || self.get(&stroke.id).is_some() {
            return false;
        }
        self.index.insert(self.strokes.len(), bounds);
        self.strokes.push(stroke);
        true
    }

    /// Put a stroke back at `index` in paint order (past the end means on top). Undo uses it
    /// to restore an erased stroke where it was. Returns false, changing nothing, under the
    /// same rules as [`InkLayer::add`].
    pub fn insert(&mut self, index: usize, stroke: Stroke) -> bool {
        let stroke = stroke.bounded();
        if stroke.bounds().is_none()
            || self.strokes.len() >= MAX_STROKES
            || self.get(&stroke.id).is_some()
        {
            return false;
        }
        let at = index.min(self.strokes.len());
        self.strokes.insert(at, stroke);
        self.rebuild_index();
        true
    }

    /// Remove the stroke with `id` and return it.
    pub fn remove(&mut self, id: &str) -> Option<Stroke> {
        let at = self.strokes.iter().position(|s| s.id == id)?;
        let stroke = self.strokes.remove(at);
        self.rebuild_index();
        Some(stroke)
    }

    fn rebuild_index(&mut self) {
        self.index.clear();
        for (i, stroke) in self.strokes.iter().enumerate() {
            if let Some(bounds) = stroke.bounds() {
                self.index.insert(i, bounds);
            }
        }
    }

    /// Strokes whose bounds overlap `area`, back to front. Use it to draw only what shows.
    pub fn strokes_in(&self, area: Aabb) -> impl Iterator<Item = &Stroke> {
        self.index
            .query_rect(area)
            .into_iter()
            .filter_map(|i| self.strokes.get(i))
    }

    /// Ids of the strokes within `radius` of `point` (a stroke counts from its edge, not its
    /// center line), topmost first.
    pub fn hit_test(&self, point: Vec2, radius: f32) -> Vec<&str> {
        let radius = if radius.is_finite() {
            radius.max(0.0)
        } else {
            0.0
        };
        self.index
            .query_point(point, radius)
            .into_iter()
            .rev()
            .filter_map(|i| self.strokes.get(i))
            .filter(|stroke| stroke_distance(stroke, point) <= radius)
            .map(|stroke| stroke.id.as_str())
            .collect()
    }

    /// The topmost stroke within `radius` of `point`.
    pub fn topmost_at(&self, point: Vec2, radius: f32) -> Option<&str> {
        self.hit_test(point, radius).into_iter().next()
    }
}

/// The distance from `point` to the edge of `stroke`: the distance to its center line minus
/// half its width, at least zero.
pub fn stroke_distance(stroke: &Stroke, point: Vec2) -> f32 {
    let half = stroke.width / 2.0;
    let mut points = stroke.points.iter().map(|p| p.pos());
    let Some(first) = points.next() else {
        return f32::INFINITY;
    };
    let mut best = point.distance(first);
    let mut prev = first;
    for next in points {
        best = best.min(distance_to_segment(point, prev, next));
        prev = next;
    }
    (best - half).max(0.0)
}

#[cfg(test)]
mod tests {
    use notecraft_core::{InkColor, InkPoint, InkTool};

    use super::*;

    fn stroke(id: &str, from: (f32, f32), to: (f32, f32), width: f32) -> Stroke {
        Stroke {
            id: id.into(),
            tool: InkTool::Pen,
            color: InkColor::BLACK,
            width,
            points: vec![
                InkPoint::new(from.0, from.1, 1.0),
                InkPoint::new(to.0, to.1, 1.0),
            ],
        }
    }

    #[test]
    fn insert_restores_paint_order() {
        let mut layer = InkLayer::new();
        for id in ["s1", "s2", "s3"] {
            assert!(layer.add(stroke(id, (0.0, 0.0), (10.0, 0.0), 2.0)));
        }
        let removed = layer.remove("s2").unwrap();
        assert!(layer.insert(1, removed.clone()));
        let ids: Vec<&str> = layer.strokes().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["s1", "s2", "s3"]);
        // The index follows: the restored stroke is found by area again.
        assert_eq!(layer.hit_test(Vec2::new(5.0, 0.0), 0.0).len(), 3);
        // A duplicate id is refused, and an index past the end puts it on top.
        assert!(!layer.insert(0, removed.clone()));
        layer.remove("s1");
        assert!(layer.insert(99, stroke("s1", (0.0, 0.0), (10.0, 0.0), 2.0)));
        assert_eq!(layer.strokes().last().map(|s| s.id.as_str()), Some("s1"));
    }

    #[test]
    fn hit_testing_uses_the_stroke_edge() {
        let mut layer = InkLayer::new();
        assert!(layer.add(stroke("s1", (0.0, 0.0), (100.0, 0.0), 10.0)));
        // 5 units from the center line is on the edge of a 10-unit stroke.
        assert_eq!(layer.hit_test(Vec2::new(50.0, 5.0), 0.0), vec!["s1"]);
        assert!(layer.hit_test(Vec2::new(50.0, 6.0), 0.0).is_empty());
        assert_eq!(layer.hit_test(Vec2::new(50.0, 8.0), 3.0), vec!["s1"]);
        // Past the end of the line, measured to the end point.
        assert_eq!(layer.hit_test(Vec2::new(104.0, 0.0), 0.0), vec!["s1"]);
        assert!(layer.hit_test(Vec2::new(120.0, 0.0), 2.0).is_empty());
    }

    #[test]
    fn the_topmost_stroke_comes_first() {
        let mut layer = InkLayer::new();
        layer.add(stroke("s1", (0.0, 0.0), (100.0, 0.0), 4.0));
        layer.add(stroke("s2", (50.0, -50.0), (50.0, 50.0), 4.0));
        layer.add(stroke("s3", (0.0, 300.0), (100.0, 300.0), 4.0));
        assert_eq!(layer.hit_test(Vec2::new(50.0, 0.0), 1.0), vec!["s2", "s1"]);
        assert_eq!(layer.topmost_at(Vec2::new(50.0, 0.0), 1.0), Some("s2"));
        assert_eq!(layer.topmost_at(Vec2::new(500.0, 500.0), 1.0), None);
    }

    #[test]
    fn a_dot_can_be_hit() {
        let mut layer = InkLayer::new();
        let mut dot = stroke("s1", (10.0, 10.0), (10.0, 10.0), 6.0);
        dot.points.truncate(1);
        assert!(layer.add(dot));
        assert_eq!(layer.hit_test(Vec2::new(12.0, 10.0), 0.0), vec!["s1"]);
        assert!(layer.hit_test(Vec2::new(14.0, 10.0), 0.0).is_empty());
    }

    #[test]
    fn removing_keeps_the_index_in_step() {
        let mut layer = InkLayer::new();
        layer.add(stroke("s1", (0.0, 0.0), (10.0, 0.0), 4.0));
        layer.add(stroke("s2", (200.0, 0.0), (210.0, 0.0), 4.0));
        layer.add(stroke("s3", (400.0, 0.0), (410.0, 0.0), 4.0));
        let removed = layer.remove("s2");
        assert_eq!(removed.map(|s| s.id), Some("s2".to_owned()));
        assert!(layer.remove("s2").is_none());
        assert_eq!(layer.len(), 2);
        assert!(layer.hit_test(Vec2::new(205.0, 0.0), 1.0).is_empty());
        assert_eq!(layer.hit_test(Vec2::new(405.0, 0.0), 1.0), vec!["s3"]);
        assert_eq!(layer.hit_test(Vec2::new(5.0, 0.0), 1.0), vec!["s1"]);
    }

    #[test]
    fn add_refuses_bad_strokes() {
        let mut layer = InkLayer::new();
        let mut empty = stroke("s1", (0.0, 0.0), (1.0, 1.0), 2.0);
        empty.points.clear();
        assert!(!layer.add(empty));
        assert!(layer.add(stroke("s1", (0.0, 0.0), (1.0, 1.0), 2.0)));
        assert!(!layer.add(stroke("s1", (5.0, 5.0), (6.0, 6.0), 2.0)));
        assert_eq!(layer.len(), 1);
        // A width that is not finite is repaired, not stored.
        assert!(layer.add(stroke("s2", (0.0, 0.0), (1.0, 1.0), f32::NAN)));
        assert_eq!(layer.get("s2").map(|s| s.width), Some(1.0));
    }

    #[test]
    fn ids_count_up_past_the_highest() {
        let mut layer = InkLayer::new();
        assert_eq!(layer.next_id(), "s1");
        layer.add(stroke("s1", (0.0, 0.0), (1.0, 1.0), 2.0));
        layer.add(stroke("s9", (0.0, 0.0), (1.0, 1.0), 2.0));
        layer.add(stroke("odd", (0.0, 0.0), (1.0, 1.0), 2.0));
        assert_eq!(layer.next_id(), "s10");
    }

    #[test]
    fn only_strokes_in_view_are_listed() {
        let mut layer = InkLayer::new();
        layer.add(stroke("s1", (0.0, 0.0), (10.0, 10.0), 2.0));
        layer.add(stroke("s2", (2000.0, 2000.0), (2010.0, 2010.0), 2.0));
        layer.add(stroke("s3", (5.0, 5.0), (15.0, 15.0), 2.0));
        let view = Aabb::new(Vec2::new(-10.0, -10.0), Vec2::new(100.0, 100.0));
        let ids: Vec<&str> = layer.strokes_in(view).map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["s1", "s3"]);
    }

    #[test]
    fn a_page_of_strokes_is_searched_quickly() {
        let strokes = (0..5000).map(|i| {
            let (x, y) = ((i % 100) as f32 * 30.0, (i / 100) as f32 * 30.0);
            stroke(&format!("s{}", i + 1), (x, y), (x + 20.0, y + 20.0), 2.0)
        });
        let layer = InkLayer::from_strokes(strokes);
        assert_eq!(layer.len(), 5000);
        assert_eq!(layer.hit_test(Vec2::new(310.0, 310.0), 3.0), vec!["s1011"]);
    }

    #[test]
    fn equality_ignores_the_index() {
        let a = InkLayer::from_strokes([stroke("s1", (0.0, 0.0), (1.0, 1.0), 2.0)]);
        let b = InkLayer::from_strokes([stroke("s1", (0.0, 0.0), (1.0, 1.0), 2.0)]);
        assert_eq!(a, b);
        assert_ne!(a, InkLayer::new());
    }
}
