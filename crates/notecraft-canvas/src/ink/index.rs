//! A uniform grid over page space for finding strokes quickly: which strokes are near a
//! point or inside a rectangle. A grid beats a tree here because strokes are small next to
//! the page, are added one at a time, and are found most often by a pointer-sized query.

use std::collections::{BTreeSet, HashMap};

use notecraft_core::{Aabb, Vec2};

/// Side of a grid cell, in page units. About a hand-writing letter, so a typical stroke
/// touches a handful of cells.
pub const CELL_SIZE: f32 = 128.0;
/// A box covering more cells than this goes in the oversized list instead of the grid, so one
/// huge stroke cannot fill thousands of cells.
const MAX_CELLS_PER_ENTRY: i64 = 256;
/// Coordinates are clamped to this before they become cell numbers.
const COORD_LIMIT: f32 = 1.0e6;

type Cell = (i32, i32);

/// A grid of boxes by key. Query results are sorted by key and have no duplicates.
#[derive(Clone, Debug, Default)]
pub struct GridIndex {
    cells: HashMap<Cell, Vec<usize>>,
    /// Entries too large for the grid.
    oversized: BTreeSet<usize>,
    bounds: HashMap<usize, Aabb>,
}

impl GridIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.bounds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
    }

    /// Add `key` with its `bounds`, replacing any earlier entry for the key. A box with a
    /// value that is not finite is ignored.
    pub fn insert(&mut self, key: usize, bounds: Aabb) {
        if !(bounds.min.is_finite() && bounds.max.is_finite()) {
            return;
        }
        self.remove(key);
        self.bounds.insert(key, bounds);
        match cell_range(bounds) {
            Some(range) if range.count() <= MAX_CELLS_PER_ENTRY => {
                for cell in range.cells() {
                    self.cells.entry(cell).or_default().push(key);
                }
            }
            _ => {
                self.oversized.insert(key);
            }
        }
    }

    /// Remove `key`. Returns whether it was there.
    pub fn remove(&mut self, key: usize) -> bool {
        let Some(bounds) = self.bounds.remove(&key) else {
            return false;
        };
        if self.oversized.remove(&key) {
            return true;
        }
        if let Some(range) = cell_range(bounds) {
            for cell in range.cells() {
                if let Some(keys) = self.cells.get_mut(&cell) {
                    keys.retain(|&k| k != key);
                    if keys.is_empty() {
                        self.cells.remove(&cell);
                    }
                }
            }
        }
        true
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Keys whose boxes overlap `area`.
    pub fn query_rect(&self, area: Aabb) -> Vec<usize> {
        let mut found = BTreeSet::new();
        if let Some(range) = cell_range(area) {
            if range.count() <= MAX_CELLS_PER_ENTRY.saturating_mul(16) {
                for cell in range.cells() {
                    if let Some(keys) = self.cells.get(&cell) {
                        found.extend(keys.iter().copied());
                    }
                }
            } else {
                // A query over a huge area: scanning the entries beats walking the cells.
                found.extend(self.bounds.keys().copied());
            }
        }
        found.extend(self.oversized.iter().copied());
        found
            .into_iter()
            .filter(|key| self.bounds.get(key).is_some_and(|b| b.intersects(area)))
            .collect()
    }

    /// Keys whose boxes are within `radius` of `point`.
    pub fn query_point(&self, point: Vec2, radius: f32) -> Vec<usize> {
        let r = if radius.is_finite() {
            radius.max(0.0)
        } else {
            0.0
        };
        self.query_rect(Aabb::from_point(point).inflate(r))
    }
}

/// A block of cells.
#[derive(Clone, Copy)]
struct CellRange {
    x0: i32,
    x1: i32,
    y0: i32,
    y1: i32,
}

impl CellRange {
    fn count(self) -> i64 {
        (i64::from(self.x1) - i64::from(self.x0) + 1)
            * (i64::from(self.y1) - i64::from(self.y0) + 1)
    }

    fn cells(self) -> impl Iterator<Item = Cell> {
        (self.x0..=self.x1).flat_map(move |x| (self.y0..=self.y1).map(move |y| (x, y)))
    }
}

fn cell_of(v: f32) -> i32 {
    (v.clamp(-COORD_LIMIT, COORD_LIMIT) / CELL_SIZE).floor() as i32
}

/// The cells a box touches, or `None` for a box that is not finite.
fn cell_range(b: Aabb) -> Option<CellRange> {
    if !(b.min.is_finite() && b.max.is_finite()) {
        return None;
    }
    Some(CellRange {
        x0: cell_of(b.min.x),
        x1: cell_of(b.max.x),
        y0: cell_of(b.min.y),
        y1: cell_of(b.max.y),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f32, y: f32, size: f32) -> Aabb {
        Aabb::new(Vec2::new(x, y), Vec2::new(x + size, y + size))
    }

    #[test]
    fn finds_what_overlaps_and_nothing_else() {
        let mut grid = GridIndex::new();
        grid.insert(0, square(10.0, 10.0, 20.0));
        grid.insert(1, square(500.0, 500.0, 20.0));
        grid.insert(2, square(20.0, 20.0, 400.0));
        assert_eq!(grid.len(), 3);
        assert_eq!(grid.query_point(Vec2::new(25.0, 25.0), 0.0), vec![0, 2]);
        assert_eq!(grid.query_point(Vec2::new(510.0, 510.0), 0.0), vec![1]);
        assert_eq!(
            grid.query_point(Vec2::new(900.0, 900.0), 5.0),
            Vec::<usize>::new()
        );
        assert_eq!(grid.query_rect(square(0.0, 0.0, 1000.0)), vec![0, 1, 2]);
    }

    #[test]
    fn neighbors_in_the_same_cell_are_still_filtered_by_box() {
        let mut grid = GridIndex::new();
        grid.insert(0, square(0.0, 0.0, 10.0));
        // Same 128-unit cell, but 50 units away.
        assert!(grid.query_point(Vec2::new(60.0, 60.0), 5.0).is_empty());
        assert_eq!(grid.query_point(Vec2::new(60.0, 60.0), 60.0), vec![0]);
    }

    #[test]
    fn boxes_straddling_cells_and_negative_coordinates_work() {
        let mut grid = GridIndex::new();
        grid.insert(7, square(-5.0, -5.0, 10.0));
        grid.insert(8, square(120.0, 120.0, 20.0));
        assert_eq!(grid.query_point(Vec2::new(-1.0, -1.0), 0.0), vec![7]);
        assert_eq!(grid.query_point(Vec2::new(1.0, 1.0), 0.0), vec![7]);
        assert_eq!(grid.query_point(Vec2::new(130.0, 130.0), 0.0), vec![8]);
        assert_eq!(grid.query_point(Vec2::new(125.0, 125.0), 0.0), vec![8]);
    }

    #[test]
    fn remove_and_reinsert() {
        let mut grid = GridIndex::new();
        grid.insert(1, square(0.0, 0.0, 300.0));
        grid.insert(2, square(10.0, 10.0, 10.0));
        assert!(grid.remove(1));
        assert!(!grid.remove(1));
        assert_eq!(grid.query_rect(square(0.0, 0.0, 300.0)), vec![2]);
        // Re-inserting a key moves it.
        grid.insert(2, square(900.0, 900.0, 10.0));
        assert!(grid.query_point(Vec2::new(15.0, 15.0), 0.0).is_empty());
        assert_eq!(grid.query_point(Vec2::new(905.0, 905.0), 0.0), vec![2]);
        assert_eq!(grid.len(), 1);
        grid.clear();
        assert!(grid.is_empty());
    }

    #[test]
    fn huge_boxes_do_not_fill_the_grid() {
        let mut grid = GridIndex::new();
        grid.insert(0, square(-1.0e5, -1.0e5, 2.0e5));
        assert!(grid.cells.is_empty());
        assert_eq!(grid.query_point(Vec2::new(40.0, 40.0), 0.0), vec![0]);
        assert!(grid.remove(0));
        assert!(grid.is_empty());
    }

    #[test]
    fn bad_numbers_are_tolerated() {
        let mut grid = GridIndex::new();
        grid.insert(0, Aabb::from_point(Vec2::new(f32::NAN, 0.0)));
        assert!(grid.is_empty());
        grid.insert(1, square(f32::MAX / 2.0, 0.0, 1.0));
        assert!(grid.query_point(Vec2::ZERO, f32::NAN).is_empty());
        assert!(grid
            .query_rect(Aabb::from_point(Vec2::new(f32::INFINITY, 0.0)))
            .is_empty());
    }

    #[test]
    fn many_strokes_query_fast_enough() {
        let mut grid = GridIndex::new();
        for i in 0..20_000_usize {
            let (x, y) = ((i % 200) as f32 * 40.0, (i / 200) as f32 * 40.0);
            grid.insert(i, square(x, y, 30.0));
        }
        let hits = grid.query_point(Vec2::new(405.0, 405.0), 2.0);
        assert_eq!(hits, vec![10 * 200 + 10]);
    }
}
