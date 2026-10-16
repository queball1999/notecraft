//! Stroke capture: turns the pointer samples of one gesture into the points of a stroke.
//! Every sample is taken at the rate the device reports it; position and pressure are
//! smoothed with a one-euro filter (see `filter`), and when the device reports no pressure
//! the width comes from the speed of the pen instead.

use notecraft_core::ink::MAX_STROKE_POINTS;
use notecraft_core::{InkPoint, PointerSample, Vec2};

use super::filter::{OneEuro, OneEuroParams};

/// Samples closer than this to the previous one (in page units) with no pressure change are
/// the same point reported twice; they are dropped.
const SAME_POINT: f32 = 0.01;
/// Speed (page units per second) at which velocity-derived pressure bottoms out.
pub const FAST_SPEED: f32 = 1500.0;
/// Lowest velocity-derived pressure: a fast flick draws this fraction of the way down.
pub const FAST_PRESSURE: f32 = 0.4;
/// Pressure for a stroke with a single point and no pressure data.
const DOT_PRESSURE: f32 = 0.7;
/// Pressure used for a sample with none, in a stroke whose device otherwise reports it.
const MISSING_PRESSURE: f32 = 0.5;

/// How a stroke is smoothed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureParams {
    pub position: OneEuroParams,
    pub pressure: OneEuroParams,
}

impl Default for CaptureParams {
    fn default() -> Self {
        Self {
            position: OneEuroParams::POSITION,
            pressure: OneEuroParams::PRESSURE,
        }
    }
}

/// One accepted sample after smoothing.
#[derive(Clone, Copy, Debug)]
struct Captured {
    pos: Vec2,
    time: f64,
    /// Smoothed pressure, or `None` before the device has reported any.
    pressure: Option<f32>,
}

/// Builds one stroke from pointer samples. Call [`push`](Self::push) for each sample of the
/// gesture, [`points`](Self::points) any time for a preview, and use the final `points` when
/// the pen lifts.
#[derive(Clone, Debug)]
pub struct StrokeBuilder {
    position: OneEuro<2>,
    pressure: OneEuro<1>,
    points: Vec<Captured>,
    /// The latest raw sample, so the stroke ends exactly where the pen lifted.
    tip: Option<Captured>,
    /// The last pressure the device reported, to fill gaps.
    last_pressure: Option<f32>,
    /// Whether any sample had pressure.
    any_pressure: bool,
    last_raw: Option<Vec2>,
}

impl Default for StrokeBuilder {
    fn default() -> Self {
        Self::new(CaptureParams::default())
    }
}

impl StrokeBuilder {
    pub fn new(params: CaptureParams) -> Self {
        Self {
            position: OneEuro::new(params.position),
            pressure: OneEuro::new(params.pressure),
            points: Vec::new(),
            tip: None,
            last_pressure: None,
            any_pressure: false,
            last_raw: None,
        }
    }

    /// Add a sample. Returns false when it was dropped: not finite, the same point as the
    /// last one, or past the point limit.
    pub fn push(&mut self, sample: PointerSample) -> bool {
        if !sample.pos.is_finite() || !sample.time.is_finite() {
            return false;
        }
        let pressure = sample
            .pressure
            .filter(|p| p.is_finite())
            .map(|p| p.clamp(0.0, 1.0));
        if let Some(last) = self.last_raw {
            let same_pressure = pressure.is_none() || self.last_pressure == pressure;
            if last.distance(sample.pos) < SAME_POINT && same_pressure {
                return false;
            }
        }
        self.last_raw = Some(sample.pos);
        if let Some(p) = pressure {
            self.any_pressure = true;
            self.last_pressure = Some(p);
        }
        let reported = pressure.or(self.last_pressure);
        self.tip = Some(Captured {
            pos: sample.pos,
            time: sample.time,
            pressure: reported,
        });
        if self.points.len() >= MAX_STROKE_POINTS {
            return false;
        }
        let dt = self
            .points
            .last()
            .map_or(0.0, |last| (sample.time - last.time) as f32);
        let [x, y] = self.position.filter([sample.pos.x, sample.pos.y], dt);
        let smoothed_pressure = reported.map(|p| self.pressure.filter([p], dt)[0]);
        self.points.push(Captured {
            pos: Vec2::new(x, y),
            time: sample.time,
            pressure: smoothed_pressure,
        });
        true
    }

    /// Whether no sample has been accepted.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Whether the device reported pressure for this stroke (otherwise width follows speed).
    pub fn has_pressure(&self) -> bool {
        self.any_pressure
    }

    /// The stroke so far: smoothed points, ending at the latest raw position. A stroke of one
    /// sample is a one-point stroke (a dot).
    pub fn points(&self) -> Vec<InkPoint> {
        let mut captured = self.points.clone();
        if let Some(tip) = self.tip {
            // The latest sample is the pen's real position: the smoothed point for it is
            // swapped for the raw one, so the stroke ends where the pen lifted. After the
            // point limit the tip replaces the last point the same way.
            match captured.last_mut() {
                Some(last) => *last = tip,
                None => captured.push(tip),
            }
        }
        let pressures = if self.any_pressure {
            captured
                .iter()
                .map(|c| c.pressure.unwrap_or(MISSING_PRESSURE))
                .collect()
        } else {
            let samples: Vec<(Vec2, f64)> = captured.iter().map(|c| (c.pos, c.time)).collect();
            velocity_pressures(&samples)
        };
        captured
            .iter()
            .zip(pressures)
            .map(|(c, p)| InkPoint::new(c.pos.x, c.pos.y, p.clamp(0.0, 1.0)))
            .collect()
    }
}

/// Pressure for each point derived from the speed of the pen: slow is 1, at [`FAST_SPEED`] or
/// faster is [`FAST_PRESSURE`]. This is the stand-in for devices that report no pressure (a
/// mouse), so a stroke gets thinner where it is quick, as a real pen's does. Input is
/// position and time in seconds; the result has one value per point.
pub fn velocity_pressures(samples: &[(Vec2, f64)]) -> Vec<f32> {
    let n = samples.len();
    if n < 2 {
        return vec![DOT_PRESSURE; n];
    }
    // Speed of the segment ending at each point (index 0 repeats the first segment).
    let mut segment = Vec::with_capacity(n);
    for (i, &(pos, time)) in samples.iter().enumerate() {
        let before = i.checked_sub(1).and_then(|j| samples.get(j));
        let speed = match before {
            Some(&(prev, prev_time)) => {
                let dt = (time - prev_time) as f32;
                let dt = if dt.is_finite() {
                    dt.max(1.0e-3)
                } else {
                    1.0e-3
                };
                let speed = prev.distance(pos) / dt;
                // Absurd input (a jump of 1e30 units) counts as fast, never as NaN.
                if speed.is_finite() {
                    speed
                } else {
                    FAST_SPEED
                }
            }
            None => 0.0,
        };
        segment.push(speed);
    }
    if let Some(second) = segment.get(1).copied() {
        if let Some(first) = segment.first_mut() {
            *first = second;
        }
    }
    // A point's speed is the mean of its two segments, then a 3-tap blur evens out the
    // sample-to-sample noise of a mouse.
    let at = |values: &[f32], i: usize| values.get(i).copied().unwrap_or_default();
    let point_speed: Vec<f32> = (0..n)
        .map(|i| {
            let next = at(&segment, (i + 1).min(n - 1));
            0.5 * (at(&segment, i) + next)
        })
        .collect();
    (0..n)
        .map(|i| {
            let prev = at(&point_speed, i.saturating_sub(1));
            let next = at(&point_speed, (i + 1).min(n - 1));
            let speed = 0.25 * prev + 0.5 * at(&point_speed, i) + 0.25 * next;
            let fast = (speed / FAST_SPEED).clamp(0.0, 1.0);
            1.0 - (1.0 - FAST_PRESSURE) * fast
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use notecraft_core::PointerKind;

    use super::*;

    const DT: f64 = 1.0 / 120.0;

    fn sample(x: f32, y: f32, t: f64, pressure: Option<f32>) -> PointerSample {
        let kind = if pressure.is_some() {
            PointerKind::Pen
        } else {
            PointerKind::Mouse
        };
        PointerSample::new(Vec2::new(x, y), t, pressure, kind).unwrap()
    }

    fn noise(seed: &mut u32) -> f32 {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }

    #[test]
    fn a_tap_is_a_one_point_stroke() {
        let mut b = StrokeBuilder::default();
        assert!(b.points().is_empty());
        assert!(b.push(sample(5.0, 6.0, 0.0, None)));
        let points = b.points();
        assert_eq!(points.len(), 1);
        assert_eq!((points[0].x, points[0].y), (5.0, 6.0));
    }

    #[test]
    fn the_stroke_ends_where_the_pen_lifted() {
        let mut b = StrokeBuilder::default();
        for i in 0..40 {
            b.push(sample(i as f32 * 3.0, 0.0, i as f64 * DT, None));
        }
        let points = b.points();
        let last = points.last().unwrap();
        assert_eq!((last.x, last.y), (117.0, 0.0));
        let first = points.first().unwrap();
        assert_eq!((first.x, first.y), (0.0, 0.0));
    }

    #[test]
    fn every_distinct_sample_is_kept_at_full_rate() {
        let mut b = StrokeBuilder::default();
        for i in 0..500 {
            assert!(b.push(sample(i as f32, (i % 7) as f32, i as f64 * 0.002, None)));
        }
        assert_eq!(b.points().len(), 500);
    }

    #[test]
    fn repeated_and_bad_samples_are_dropped() {
        let mut b = StrokeBuilder::default();
        assert!(b.push(sample(1.0, 1.0, 0.0, None)));
        assert!(!b.push(sample(1.0, 1.0, 0.01, None)));
        let bad = PointerSample {
            pos: Vec2::new(f32::NAN, 0.0),
            time: 0.02,
            pressure: None,
            kind: PointerKind::Mouse,
        };
        assert!(!b.push(bad));
        assert!(b.push(sample(2.0, 1.0, 0.03, None)));
        assert_eq!(b.points().len(), 2);
    }

    #[test]
    fn a_pressure_change_in_place_is_kept() {
        let mut b = StrokeBuilder::default();
        assert!(b.push(sample(1.0, 1.0, 0.0, Some(0.2))));
        assert!(!b.push(sample(1.0, 1.0, 0.01, Some(0.2))));
        assert!(b.push(sample(1.0, 1.0, 0.02, Some(0.9))));
    }

    #[test]
    fn point_count_is_capped() {
        let mut b = StrokeBuilder::default();
        for i in 0..MAX_STROKE_POINTS + 50 {
            b.push(sample(i as f32, 0.0, i as f64 * DT, None));
        }
        let points = b.points();
        assert_eq!(points.len(), MAX_STROKE_POINTS);
        // The last point is still where the pen was.
        assert_eq!(
            points.last().map(|p| p.x),
            Some((MAX_STROKE_POINTS + 49) as f32)
        );
    }

    #[test]
    fn smoothing_cuts_jitter_on_a_slow_line() {
        let mut b = StrokeBuilder::default();
        let mut seed = 99;
        let mut raw_err = 0.0;
        for i in 0..400 {
            let x = i as f32 * 0.5; // 60 units per second
            let y = 50.0 + 0.6 * noise(&mut seed);
            raw_err += (y - 50.0).powi(2);
            b.push(sample(x, y, i as f64 * DT, None));
        }
        let points = b.points();
        let out_err: f32 = points.iter().map(|p| (p.y - 50.0).powi(2)).sum();
        assert!(out_err < raw_err * 0.4, "raw {raw_err}, smoothed {out_err}");
    }

    #[test]
    fn a_quick_flick_is_not_flattened() {
        // A sharp turn at speed must keep its corner within a few units.
        let mut b = StrokeBuilder::default();
        let mut t = 0.0;
        for i in 0..30 {
            b.push(sample(i as f32 * 8.0, 0.0, t, None));
            t += DT;
        }
        for i in 1..30 {
            b.push(sample(232.0, i as f32 * 8.0, t, None));
            t += DT;
        }
        let points = b.points();
        let corner_gap = points
            .iter()
            .map(|p| p.pos().distance(Vec2::new(232.0, 0.0)))
            .fold(f32::MAX, f32::min);
        assert!(corner_gap < 8.0, "corner gap {corner_gap}");
    }

    #[test]
    fn device_pressure_is_kept_and_marks_the_stroke() {
        let mut b = StrokeBuilder::default();
        for i in 0..60 {
            let p = i as f32 / 59.0;
            b.push(sample(i as f32 * 4.0, 0.0, i as f64 * DT, Some(p)));
        }
        assert!(b.has_pressure());
        let points = b.points();
        let first = points.first().unwrap().pressure;
        let last = points.last().unwrap().pressure;
        assert!(first < 0.1 && last > 0.95, "{first} {last}");
        assert!(points
            .windows(2)
            .all(|w| w[1].pressure >= w[0].pressure - 1.0e-4));
    }

    #[test]
    fn missing_pressure_in_a_pen_stroke_uses_the_last_value() {
        let mut b = StrokeBuilder::default();
        b.push(sample(0.0, 0.0, 0.0, Some(0.8)));
        b.push(sample(5.0, 0.0, DT, None));
        let points = b.points();
        assert!(points.iter().all(|p| (p.pressure - 0.8).abs() < 0.05));
    }

    #[test]
    fn speed_sets_width_when_there_is_no_pressure() {
        // Slow half (30 units/s) then fast half (1,800 units/s).
        let mut samples = Vec::new();
        let mut x = 0.0;
        let mut t = 0.0;
        for _ in 0..60 {
            samples.push((Vec2::new(x, 0.0), t));
            x += 0.25;
            t += DT;
        }
        for _ in 0..60 {
            samples.push((Vec2::new(x, 0.0), t));
            x += 15.0;
            t += DT;
        }
        let p = velocity_pressures(&samples);
        assert_eq!(p.len(), samples.len());
        assert!(p[20] > 0.95, "slow {}", p[20]);
        assert!(p[100] < FAST_PRESSURE + 0.02, "fast {}", p[100]);
        assert!(p
            .iter()
            .all(|v| (FAST_PRESSURE - 1.0e-4..=1.0 + 1.0e-4).contains(v)));
    }

    #[test]
    fn velocity_pressure_handles_tiny_inputs() {
        assert!(velocity_pressures(&[]).is_empty());
        assert_eq!(velocity_pressures(&[(Vec2::ZERO, 0.0)]), vec![DOT_PRESSURE]);
        // Two samples at the same time must not divide by zero.
        let p = velocity_pressures(&[(Vec2::ZERO, 1.0), (Vec2::new(10.0, 0.0), 1.0)]);
        assert!(p.iter().all(|v| v.is_finite()));
    }
}
