//! Eraser geometry: the points an eraser drag passes over, which strokes it touches, and how
//! the point eraser cuts a stroke into the runs that remain. Pure functions over page units;
//! the engine turns the results into undoable operations.

use std::collections::BTreeSet;

use notecraft_core::geometry::distance_to_segment;
use notecraft_core::ink::MAX_STROKE_POINTS;
use notecraft_core::{InkPoint, Vec2};

use super::layer::InkLayer;
use super::tessellate::width_at;

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

/// Centers along the polyline `path`, spaced at most a quarter radius apart (so a fast drag
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
    let wanted = clean_radius(radius) / 4.0;
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

/// Half the width a stroke of nominal `width` has at `pressure`, as it is drawn.
fn half_width(width: f32, pressure: f32) -> f32 {
    width_at(width, pressure) / 2.0
}

/// Slack for parameters along a segment.
const EPS: f32 = 1.0e-5;

/// The part of segment `a` to `b` (parameters 0 to 1) that the eraser circle at `center` with
/// `radius` touches, counting the stroke's own half width, which varies along the segment
/// with pressure (`h0` at `a`, `h1` at `b`). `None` when it does not touch.
fn touched_span(a: Vec2, b: Vec2, h: (f32, f32), center: Vec2, radius: f32) -> Option<(f32, f32)> {
    let (h0, h1) = h;
    let d = b - a;
    let e = a - center;
    let r0 = radius + h0;
    let k = h1 - h0;
    // |e + t d|^2 <= (r0 + t k)^2, a quadratic in t.
    let qa = d.dot(d) - k * k;
    let qb = 2.0 * (d.dot(e) - r0 * k);
    let qc = e.dot(e) - r0 * r0;
    let (lo, hi) = if qa > EPS {
        let disc = qb * qb - 4.0 * qa * qc;
        if disc < 0.0 {
            return None;
        }
        let root = disc.sqrt();
        ((-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa))
    } else {
        // The width changes as fast as the pen moves (or the segment is a point): treat the
        // stroke as its widest here, which can only cut a little more.
        let reach = radius + h0.max(h1);
        let len2 = d.dot(d);
        if len2 <= f32::EPSILON {
            return (e.length() <= reach).then_some((0.0, 1.0));
        }
        let t = -e.dot(d) / len2;
        let closest = a + d * t.clamp(0.0, 1.0);
        let miss = closest.distance(center);
        if miss > reach {
            return None;
        }
        let half_chord = (reach * reach - (e.dot(e) - (e.dot(d)).powi(2) / len2))
            .max(0.0)
            .sqrt();
        let along = half_chord / len2.sqrt();
        (t - along, t + along)
    };
    let (lo, hi) = (lo.max(0.0), hi.min(1.0));
    (lo <= hi).then_some((lo, hi))
}

/// Cut out of a stroke every part the eraser circle touches, and return the runs that remain,
/// each with at least two points. The circle is tested against the stroke's outline: its half
/// width at each place (width and pressure interpolated along a segment) is added to the
/// radius, so a graze between two stored points cuts too. `None` means the eraser touched
/// nothing, so nothing changes; `Some` with no runs means the whole stroke is gone. Cut ends
/// sit exactly where the circle meets the stroke, with pressure interpolated.
pub fn split_stroke(
    points: &[InkPoint],
    width: f32,
    centers: &[Vec2],
    radius: f32,
) -> Option<Vec<Vec<InkPoint>>> {
    let radius = clean_radius(radius);
    if centers.is_empty() {
        return None;
    }
    let Some(first) = points.first() else {
        return None;
    };
    if points.len() == 1 {
        let reach = radius + half_width(width, first.pressure);
        let hit = centers.iter().any(|c| first.pos().distance(*c) <= reach);
        return hit.then(Vec::new);
    }
    let mut runs: Vec<Vec<InkPoint>> = Vec::new();
    let mut run: Vec<InkPoint> = Vec::new();
    let mut total = 0_usize;
    let mut touched = false;
    let flush = |run: &mut Vec<InkPoint>, runs: &mut Vec<Vec<InkPoint>>| {
        if run.len() >= 2 {
            runs.push(std::mem::take(run));
        } else {
            run.clear();
        }
    };
    for (p, q) in points.iter().zip(points.iter().skip(1)) {
        let (a, b) = (p.pos(), q.pos());
        let h = (half_width(width, p.pressure), half_width(width, q.pressure));
        let widest = radius + h.0.max(h.1);
        // The spans of this segment the eraser touches, sorted and merged.
        let mut spans: Vec<(f32, f32)> = centers
            .iter()
            .filter(|c| distance_to_segment(**c, a, b) <= widest)
            .filter_map(|c| touched_span(a, b, h, *c, radius))
            .collect();
        spans.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut merged: Vec<(f32, f32)> = Vec::new();
        for span in spans {
            match merged.last_mut() {
                Some(last) if span.0 <= last.1 + EPS => last.1 = last.1.max(span.1),
                _ => merged.push(span),
            }
        }
        touched |= !merged.is_empty();
        // The kept spans are what lies between the cuts.
        let mut kept: Vec<(f32, f32)> = Vec::new();
        let mut cursor = 0.0_f32;
        for (s, e) in &merged {
            if *s > cursor + EPS {
                kept.push((cursor, *s));
            }
            cursor = cursor.max(*e);
        }
        if cursor < 1.0 - EPS {
            kept.push((cursor, 1.0));
        }
        if kept.is_empty() {
            flush(&mut run, &mut runs);
            continue;
        }
        for (s, e) in kept {
            if total >= MAX_STROKE_POINTS {
                break;
            }
            // A kept span that starts at the segment's start continues the run in progress.
            if !(s <= EPS && !run.is_empty()) {
                flush(&mut run, &mut runs);
                run.push(lerp(*p, *q, s));
                total += 1;
            }
            run.push(lerp(*p, *q, e));
            total += 1;
            if e < 1.0 - EPS {
                flush(&mut run, &mut runs);
            }
        }
    }
    flush(&mut run, &mut runs);
    touched.then_some(runs)
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
            assert!(pair[0].distance(pair[1]) <= 2.5 + 1.0e-3);
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
        // Reach is 5 plus the drawn half width there (about 0.87), and the cut ends sit where
        // the circle meets the stroke, not at a stored point.
        let left = runs[0].last().map(|p| p.x).unwrap_or_default();
        let right = runs[1].first().map(|p| p.x).unwrap_or_default();
        assert!(left > 44.0 && left < 44.3, "{left}");
        assert!(right > 55.7 && right < 56.0, "{right}");
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
    fn a_graze_between_points_cuts() {
        // Two stored points far apart; the circle only grazes the stroke's outline in the
        // middle. Radius 5 and a drawn half width of 1 (pressure 1) reach 6 units.
        let points = [InkPoint::new(0.0, 0.0, 1.0), InkPoint::new(100.0, 0.0, 1.0)];
        let runs = split_stroke(&points, 2.0, &[Vec2::new(50.0, 5.5)], 5.0).expect("grazed");
        assert_eq!(runs.len(), 2);
        let left = runs[0].last().map(|p| p.x).unwrap_or_default();
        let right = runs[1].first().map(|p| p.x).unwrap_or_default();
        // The chord at depth 5.5 in a reach of 6 is about 4.8 wide.
        assert!(
            (left - 47.6).abs() < 0.2 && (right - 52.4).abs() < 0.2,
            "{left} {right}"
        );
        // Just out of reach: nothing is cut.
        assert!(split_stroke(&points, 2.0, &[Vec2::new(50.0, 6.5)], 5.0).is_none());
    }

    #[test]
    fn the_width_at_that_point_counts() {
        // A fat stroke is touched from farther away than a thin one.
        let fat = [InkPoint::new(0.0, 0.0, 1.0), InkPoint::new(100.0, 0.0, 1.0)];
        let center = [Vec2::new(50.0, 12.0)];
        assert!(split_stroke(&fat, 20.0, &center, 3.0).is_some());
        assert!(split_stroke(&fat, 2.0, &center, 3.0).is_none());
        // Light pressure narrows the stroke, so the same circle misses it.
        let light = [InkPoint::new(0.0, 0.0, 0.0), InkPoint::new(100.0, 0.0, 0.0)];
        assert!(split_stroke(&light, 20.0, &center, 3.0).is_none());
        // Width that changes along a segment: the circle near the wide end touches, near
        // the thin end does not.
        let taper = [InkPoint::new(0.0, 0.0, 1.0), InkPoint::new(100.0, 0.0, 0.0)];
        assert!(split_stroke(&taper, 20.0, &[Vec2::new(5.0, 11.0)], 2.0).is_some());
        assert!(split_stroke(&taper, 20.0, &[Vec2::new(95.0, 11.0)], 2.0).is_none());
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
