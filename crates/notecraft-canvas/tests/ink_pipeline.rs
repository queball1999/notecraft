//! The ink engine end to end, through its public API only: pointer samples in, a smoothed
//! stroke, a triangle mesh, and a hit test out. Also hostile input, which must never panic
//! or produce a mesh with bad indices.

use notecraft_canvas::ink::{tessellate, InkLayer, StrokeBuilder, TessParams};
use notecraft_core::ink::MAX_STROKE_POINTS;
use notecraft_core::{InkColor, InkPoint, InkTool, PointerKind, PointerSample, Stroke, Vec2};

const DT: f64 = 1.0 / 120.0;

fn sample(x: f32, y: f32, t: f64, pressure: Option<f32>) -> Option<PointerSample> {
    PointerSample::new(Vec2::new(x, y), t, pressure, PointerKind::Pen)
}

/// A deterministic pseudo-random value in -1 to 1.
fn noise(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
}

fn mesh_is_valid(positions: &[Vec2], indices: &[u32]) -> bool {
    indices.len().is_multiple_of(3)
        && indices.iter().all(|&i| (i as usize) < positions.len())
        && positions.iter().all(|p| p.is_finite())
}

#[test]
fn a_noisy_circle_comes_out_smooth_and_hit_testable() {
    let mut builder = StrokeBuilder::default();
    let mut seed = 5;
    let centre = Vec2::new(300.0, 300.0);
    let radius = 100.0;
    // One lap in about 1.6 seconds: 400 units per second, a normal writing speed.
    let samples = 200;
    for i in 0..=samples {
        let angle = i as f32 / samples as f32 * std::f32::consts::TAU;
        let jitter = Vec2::new(0.5 * noise(&mut seed), 0.5 * noise(&mut seed));
        let x = centre.x + radius * angle.cos() + jitter.x;
        let y = centre.y + radius * angle.sin() + jitter.y;
        let pressure = 0.4 + 0.3 * (angle * 2.0).sin().abs();
        let s = sample(x, y, i as f64 * DT, Some(pressure));
        assert!(s.is_some_and(|s| builder.push(s)));
    }
    let points = builder.points();
    assert!(points.len() >= samples);

    // The smoothed path hugs the circle, a little inside it where the filter cuts the curve.
    for p in &points {
        let off = (p.pos().distance(centre) - radius).abs();
        assert!(off < 3.0, "point {off} units off the circle");
    }

    let stroke = Stroke {
        id: "s1".into(),
        tool: InkTool::Pen,
        color: InkColor::BLACK,
        width: 4.0,
        points: points.clone(),
    };
    let mesh = tessellate(&stroke.points, stroke.width, TessParams::default());
    assert!(mesh_is_valid(&mesh.positions, &mesh.indices));
    for p in &mesh.positions {
        let off = (p.distance(centre) - radius).abs();
        assert!(off < 3.0 + 2.5, "vertex {off} units off the circle");
    }

    let layer = InkLayer::from_strokes([stroke]);
    assert_eq!(layer.topmost_at(Vec2::new(400.0, 300.0), 1.0), Some("s1"));
    assert_eq!(layer.topmost_at(centre, 1.0), None);
}

#[test]
fn pressure_and_speed_change_the_mesh_width() {
    // The same line drawn with high pressure and with low pressure.
    let build = |pressure: f32| {
        let mut b = StrokeBuilder::default();
        for i in 0..60 {
            let s = sample(i as f32 * 3.0, 0.0, i as f64 * DT, Some(pressure));
            assert!(s.is_some_and(|s| b.push(s)));
        }
        tessellate(&b.points(), 8.0, TessParams::default())
    };
    let height = |positions: &[Vec2]| {
        let lo = positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let hi = positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        hi - lo
    };
    let firm = build(1.0);
    let light = build(0.0);
    assert!((height(&firm.positions) - 8.0).abs() < 0.2);
    assert!((height(&light.positions) - 4.0).abs() < 0.2);

    // A mouse (no pressure): the slow line is wider than the fast one.
    let mouse = |step: f32| {
        let mut b = StrokeBuilder::default();
        for i in 0..80 {
            let s = PointerSample::new(
                Vec2::new(i as f32 * step, 0.0),
                i as f64 * DT,
                None,
                PointerKind::Mouse,
            );
            assert!(s.is_some_and(|s| b.push(s)));
        }
        assert!(!b.has_pressure());
        tessellate(&b.points(), 8.0, TessParams::default())
    };
    assert!(height(&mouse(0.2).positions) > height(&mouse(20.0).positions) + 1.0);
}

#[test]
fn hostile_input_never_panics() {
    let mut builder = StrokeBuilder::default();
    let mut seed = 11;
    let extremes = [0.0, 1.0e30, -1.0e30, f32::MAX, f32::MIN, 1.0e-30, 123.456];
    for i in 0..60_000_u32 {
        let pick = |seed: &mut u32| extremes[(noise(seed).abs() * 6.99) as usize % extremes.len()];
        let (x, y) = (pick(&mut seed), pick(&mut seed));
        // Time that runs backwards, stalls, and jumps.
        let t = match i % 4 {
            0 => i as f64 * DT,
            1 => 0.0,
            2 => -1.0e12,
            _ => f64::MAX,
        };
        let pressure = [Some(f32::NAN), Some(7.0), None, Some(-3.0)][(i % 4) as usize];
        // Samples built by hand skip the checks in `PointerSample::new`.
        let raw = PointerSample {
            pos: Vec2::new(x, y),
            time: t,
            pressure,
            kind: PointerKind::Pen,
        };
        builder.push(raw);
    }
    let points = builder.points();
    assert!(!points.is_empty() && points.len() <= MAX_STROKE_POINTS);
    assert!(points
        .iter()
        .all(|p| p.pressure.is_finite() && (0.0..=1.0).contains(&p.pressure)));

    let mesh = tessellate(&points, f32::NAN, TessParams { cap_segments: 0 });
    assert!(mesh.indices.len().is_multiple_of(3));
    assert!(mesh
        .indices
        .iter()
        .all(|&i| (i as usize) < mesh.positions.len()));

    let mut layer = InkLayer::new();
    let stroke = Stroke {
        id: "s1".into(),
        tool: InkTool::Pen,
        color: InkColor::BLACK,
        width: f32::INFINITY,
        points,
    };
    let _ = layer.add(stroke);
    let _ = layer.hit_test(Vec2::new(f32::NAN, 0.0), f32::NAN);
    let _ = layer.hit_test(Vec2::new(1.0e30, -1.0e30), f32::MAX);
}

#[test]
fn bad_points_in_a_stored_stroke_do_not_break_the_mesh() {
    let points = [
        InkPoint::new(f32::NAN, 0.0, 0.5),
        InkPoint::new(0.0, 0.0, f32::NAN),
        InkPoint::new(10.0, 0.0, 9.0),
        InkPoint::new(f32::INFINITY, 5.0, 0.5),
        InkPoint::new(20.0, 0.0, -9.0),
    ];
    let mesh = tessellate(&points, 4.0, TessParams::default());
    assert!(mesh_is_valid(&mesh.positions, &mesh.indices));
    assert!(!mesh.is_empty());
}

#[test]
fn the_same_input_gives_the_same_mesh() {
    let build = || {
        let mut b = StrokeBuilder::default();
        for i in 0..100 {
            let s = sample(
                i as f32 * 2.0,
                (i as f32 * 0.3).sin() * 20.0,
                i as f64 * DT,
                None,
            );
            if let Some(s) = s {
                b.push(s);
            }
        }
        tessellate(&b.points(), 3.0, TessParams::default())
    };
    assert_eq!(build(), build());
}
