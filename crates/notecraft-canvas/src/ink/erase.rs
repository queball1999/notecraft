//! Eraser geometry: the points an eraser drag passes over, which strokes it touches, and how
//! the point eraser cuts a stroke into the runs that remain. Pure functions over page units;
//! the engine turns the results into undoable operations.

use std::collections::BTreeSet;

use notecraft_core::geometry::distance_to_segment;
use notecraft_core::ink::MAX_STROKE_POINTS;
use notecraft_core::{InkPoint, Vec2};

use super::layer::InkLayer;

/// Most centers one erase command tests. A longer path is spread over this many.
pub const MAX_PATH_SAMPLES: usize = 512;
/// Smallest spacing between sampled centers, in page units.
const MIN_SPACING: f32 = 0.25;

/// A usable eraser radius: finite and not negative.
fn clean_radius(radius: f32) -> f32 {
    if radius.is_finite() {
        radius.max(0.0)
    } else {
        0.0
    }
}

/// Centers along the polyline `path`, spaced at most half a radius apart (so a fast drag
/// cannot step over a stroke), at most [`MAX_PATH_SAMPLES`] of them. Points that are not
/// finite are skipped.
pub fn path_samples(path: &[Vec2], radius: f32) -> Vec<Vec2> {
    let path: Vec<Vec2> = path.iter().copied().filter(|p| p.is_finite()).collect();
    let Some(first) = path.first().copied() else {
        return Vec::new();
    };
    let total: f32 = path
        .iter()
        .zip(path.iter().skip(1))
        .map(|(a, b)| a.distance(*b))
        .sum();
    let wanted = clean_radius(radius) / 2.0;
    let spacing = wanted.max(MIN_SPACING).max(total / MAX_PATH_SAMPLES as f32);
    let mut samples = vec![first];
    for (a, b) in path.iter().copied().zip(path.iter().skip(1).copied()) {
        let length = a.distance(b);
        // Whole spacings only; the segment's end is always sampled.
        let steps = (length / spacing).ceil().max(1.0) as usize;
        for i in 1..=steps {
            if samples.len() >= MAX_PATH_SAMPLES {
                return samples;
            }
            samples.push(a + (b - a) * (i as f32 / steps as f32));
        }
    }
    samples
}

/// Ids of the strokes within `radius` of any of `centers`, in paint order (back to front).
pub fn strokes_touched(layer: &InkLayer, centers: &[Vec2], radius: f32) -> Vec<String> {
    let radius = clean_radius(radius);
    let mut found = BTreeSet::new();
    for center in centers {
        for id in layer.hit_test(*center, radius) {
            found.insert(id.to_owned());
        }
    }
    let mut ids: Vec<(usize, String)> = found
        .into_iter()
        .filter_map(|id| Some((layer.position(&id)?, id)))
        .collect();
    ids.sort_by_key(|(at, _)| *at);
    ids.into_iter().map(|(_, id)| id).collect()
}

fn lerp(a: InkPoint, b: InkPoint, t: f32) -> InkPoint {
    InkPoint::new(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.pressure + (b.pressure - a.pressure) * t,
    )
}

/// Cut the parts of a stroke that lie within `radius` of any of `centers` (the stroke's own
/// half width counts, as in hit testing) and return the runs that remain, each with at least
/// two points. `None` means the eraser touched no point of the stroke, so nothing changes;
/// `Some` with no runs means the whole stroke is gone.
///
/// A long segment between two stored points is first split into steps no longer than half the
/// reach where the eraser passes near it, so a sparse stroke is cut where the eraser crosses
/// it and not only at its stored points. Pressure is interpolated.
pub fn split_stroke(
    points: &[InkPoint],
    width: f32,
    centers: &[Vec2],
    radius: f32,
) -> Option<Vec<Vec<InkPoint>>> {
    let half_width = if width.is_finite() {
        width.max(0.0) / 2.0
    } else {
        0.0
    };
    let reach = clean_radius(radius) + half_width;
    let spacing = (reach / 2.0).max(MIN_SPACING);
    let near = |a: Vec2, b: Vec2| {
        centers
            .iter()
            .any(|c| distance_to_segment(*c, a, b) <= reach)
    };
    let mut dense: Vec<InkPoint> = Vec::with_capacity(points.len());
    for (i, point) in points.iter().enumerate() {
        if let Some(prev) = i.checked_sub(1).and_then(|j| points.get(j)) {
            let (a, b) = (prev.pos(), point.pos());
            let length = a.distance(b);
            if length > spacing && near(a, b) {
                let steps = (length / spacing).ceil() as usize;
                if dense.len() + steps > MAX_STROKE_POINTS {
                    // No room to cut finer; fall back to the stored points.
                } else {
                    for k in 1..steps {
                        dense.push(lerp(*prev, *point, k as f32 / steps as f32));
                    }
                }
            }
        }
        dense.push(*point);
    }
    let inside = |p: &InkPoint| centers.iter().any(|c| p.pos().distance(*c) <= reach);
    if !dense.iter().any(inside) {
        return None;
    }
    let mut runs = Vec::new();
    let mut run: Vec<InkPoint> = Vec::new();
    for point in dense {
        if inside(&point) {
            if run.len() >= 2 {
                runs.push(std::mem::take(&mut run));
            } else {
                run.clear();
            }
        } else {
            run.push(point);
        }
    }
    if run.len() >= 2 {
        runs.push(run);
    }
    Some(runs)
}

#[cfg(test)]
mod tests {
    use notecraft_core::{InkColor, InkTool, Stroke};

    use super::*;

    fn line(id: &str, y: f32, x0: f32, x1: f32, count: usize) -> Stroke {
        let points = (0..count)
            .map(|i| {
                let t = i as f32 / (count - 1).max(1) as f32;
                InkPoint::new(x0 + (x1 - x0) * t, y, 0.5 + 0.5 * t)
            })
            .collect();
        Stroke {
            id: id.into(),
            tool: InkTool::Pen,
            color: InkColor::BLACK,
            width: 2.0,
            points,
        }
    }

    #[test]
    fn a_drag_is_sampled_without_gaps() {
        let path = [Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)];
        let samples = path_samples(&path, 10.0);
        assert_eq!(samples.first(), Some(&Vec2::new(0.0, 0.0)));
        assert_eq!(samples.last(), Some(&Vec2::new(100.0, 0.0)));
        for pair in samples.windows(2) {
            assert!(pair[0].distance(pair[1]) <= 5.0 + 1.0e-3);
        }
        // One point is one sample; nothing usable is none.
        assert_eq!(path_samples(&path[..1], 10.0).len(), 1);
        assert!(path_samples(&[Vec2::new(f32::NAN, 0.0)], 10.0).is_empty());
        // A huge drag with a tiny radius stays bounded.
        let far = [Vec2::new(0.0, 0.0), Vec2::new(1.0e6, 0.0)];
        assert!(path_samples(&far, 0.0).len() <= MAX_PATH_SAMPLES);
    }

    #[test]
    fn touched_strokes_come_back_back_to_front() {
        let mut layer = InkLayer::new();
        layer.add(line("s1", 0.0, 0.0, 100.0, 2));
        layer.add(line("s2", 50.0, 0.0, 100.0, 2));
        layer.add(line("s3", 2.0, 0.0, 100.0, 2));
        let hit = strokes_touched(&layer, &[Vec2::new(50.0, 1.0)], 3.0);
        assert_eq!(hit, ["s1", "s3"]);
        let swept = strokes_touched(
            &layer,
            &path_samples(&[Vec2::new(50.0, 0.0), Vec2::new(50.0, 60.0)], 3.0),
            3.0,
        );
        assert_eq!(swept, ["s1", "s2", "s3"]);
        assert!(strokes_touched(&layer, &[Vec2::new(500.0, 500.0)], 3.0).is_empty());
    }

    #[test]
    fn the_middle_is_cut_out_and_both_ends_stay() {
        // Dense points, 1 unit apart.
        let stroke = line("s1", 0.0, 0.0, 100.0, 101);
        let runs = split_stroke(&stroke.points, stroke.width, &[Vec2::new(50.0, 0.0)], 5.0)
            .expect("touched");
        assert_eq!(runs.len(), 2);
        // Reach is 5 + 1: points 44..=56 are cut.
        assert_eq!(runs[0].last().map(|p| p.x), Some(43.0));
        assert_eq!(runs[1].first().map(|p| p.x), Some(57.0));
        // Pressure is kept as it was.
        assert!((runs[0][0].pressure - 0.5).abs() < 1.0e-6);
        assert!(runs.iter().all(|r| r.len() >= 2));
    }

    #[test]
    fn a_sparse_stroke_is_cut_where_the_eraser_crosses_it() {
        // Two stored points 100 apart: no stored point is near the eraser at x = 50.
        let stroke = line("s1", 0.0, 0.0, 100.0, 2);
        let runs = split_stroke(&stroke.points, stroke.width, &[Vec2::new(50.0, 0.0)], 4.0)
            .expect("touched");
        assert_eq!(runs.len(), 2);
        let left_end = runs[0].last().map(|p| p.x).unwrap_or_default();
        let right_start = runs[1].first().map(|p| p.x).unwrap_or_default();
        assert!(left_end < 50.0 && left_end > 40.0, "{left_end}");
        assert!(right_start > 50.0 && right_start < 60.0, "{right_start}");
        // Interpolated pressure sits between the ends.
        assert!(runs[0]
            .last()
            .is_some_and(|p| p.pressure > 0.5 && p.pressure < 1.0));
    }

    #[test]
    fn untouched_stays_and_swallowed_goes() {
        let stroke = line("s1", 0.0, 0.0, 100.0, 11);
        assert!(
            split_stroke(&stroke.points, stroke.width, &[Vec2::new(50.0, 40.0)], 5.0).is_none()
        );
        let runs = split_stroke(&stroke.points, stroke.width, &[Vec2::new(50.0, 0.0)], 80.0)
            .expect("touched");
        assert!(runs.is_empty());
        // A cut near the end leaves one run; a run of a single point is dropped.
        let runs = split_stroke(&stroke.points, stroke.width, &[Vec2::new(0.0, 0.0)], 25.0)
            .expect("touched");
        assert_eq!(runs.len(), 1);
        let lone = [
            InkPoint::new(0.0, 0.0, 1.0),
            InkPoint::new(1.0, 0.0, 1.0),
            InkPoint::new(50.0, 0.0, 1.0),
        ];
        let runs = split_stroke(&lone, 0.0, &[Vec2::new(1.0, 0.0)], 0.5).expect("touched");
        // 0 and 1 are both within 0.5 of... only point 1 is; point 0 is a single leftover.
        assert!(runs.iter().all(|r| r.len() >= 2));
        // A dot is erased whole.
        let dot = [InkPoint::new(5.0, 5.0, 1.0)];
        let runs = split_stroke(&dot, 2.0, &[Vec2::new(5.0, 5.0)], 1.0).expect("touched");
        assert!(runs.is_empty());
    }

    #[test]
    fn hostile_input_is_bounded() {
        let stroke = line("s1", 0.0, 0.0, 1.0e7, 2);
        let runs = split_stroke(&stroke.points, f32::NAN, &[Vec2::new(5.0e6, 0.0)], 1.0)
            .unwrap_or_default();
        let total: usize = runs.iter().map(Vec::len).sum();
        assert!(total <= MAX_STROKE_POINTS);
        assert!(split_stroke(&stroke.points, 2.0, &[], 5.0).is_none());
        assert!(split_stroke(&stroke.points, 2.0, &[Vec2::new(0.0, 0.0)], f32::NAN).is_some());
    }
}
