//! Variable-width tessellation: a stroke becomes a triangle list in page units. The mesh has
//! positions and indices only; the front end picks the color and turns it into its own
//! vertices (an egui `Mesh`), so this module knows no UI type.
//!
//! The shape is a ribbon along the stroke whose half-width follows pressure, with a round cap
//! at each end and a round join where the stroke turns sharply. Triangles overlap at joins,
//! so a translucent stroke must be drawn as one layer (the highlighter uses a blend mode, not
//! per-triangle alpha).
//!
//! Anti-aliasing: the mesh can carry a fringe, a one-pixel strip of triangles along every
//! outer edge (a sharp turn fringes only the wedge of its join disc that shows). The fringe is built in page units, so it does not depend on zoom; each vertex
//! records in [`Edge`] which way the front end should move it, in screen pixels, to make the
//! strip one pixel wide whatever the zoom. Where the fringe overlaps the stroke's own body it
//! is drawn over the same color, so it is invisible; that holds for opaque strokes only, so a
//! translucent stroke asks for no fringe (`TessParams::fringe`).

use std::f32::consts::{PI, TAU};

use notecraft_core::ink::MAX_STROKE_WIDTH;
use notecraft_core::{InkPoint, Vec2};

/// Points closer than this to the previous one add nothing and are skipped.
const MIN_STEP: f32 = 1.0e-3;
/// A turn sharper than this (cosine of the angle between segments) gets a round join.
const JOIN_COS: f32 = 0.85;
/// The ribbon's offset at a bend is stretched by at most this factor.
const MAX_MITER: f32 = 2.0;
/// Fewest and most segments in a half circle cap.
const MIN_CAP_SEGMENTS: u32 = 3;
const MAX_CAP_SEGMENTS: u32 = 32;

/// A triangle list: every three indices are one triangle.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StrokeMesh {
    pub positions: Vec<Vec2>,
    pub indices: Vec<u32>,
    /// One per position: how the front end places and fades the vertex. All [`Edge::Inside`]
    /// when the mesh has no fringe.
    pub edges: Vec<Edge>,
}

/// What a vertex is for anti-aliasing. Directions are unit vectors pointing away from the
/// stroke's body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edge {
    /// Not on the outline: drawn where it is, at full coverage.
    Inside,
    /// On the outline: drawn up to half a pixel inward (against the direction, never past
    /// the centerline, which is `radius` away), at full coverage.
    Rim { dir: Vec2, radius: f32 },
    /// A fringe vertex over a rim vertex: drawn half a pixel outward, with no coverage.
    Outer(Vec2),
}

impl StrokeMesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Mesh quality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TessParams {
    /// Segments in a half circle cap. More is rounder and costs more triangles.
    pub cap_segments: u32,
    /// Add the anti-aliasing fringe. Leave it off for a translucent stroke.
    pub fringe: bool,
}

impl Default for TessParams {
    fn default() -> Self {
        Self {
            cap_segments: 8,
            fringe: true,
        }
    }
}

impl TessParams {
    /// Enough segments that a cap of `screen_radius` (in screen points) stays round to the eye.
    pub fn for_screen_radius(screen_radius: f32) -> Self {
        let r = if screen_radius.is_finite() {
            screen_radius.max(0.0)
        } else {
            0.0
        };
        // Keep the gap between a chord and the arc under a quarter point.
        let step = 2.0 * (1.0 - 0.25 / r.max(0.25)).clamp(-1.0, 1.0).acos();
        let segments = (PI / step.max(0.01)).ceil();
        Self {
            cap_segments: (segments as u32).clamp(MIN_CAP_SEGMENTS, MAX_CAP_SEGMENTS),
            fringe: true,
        }
    }
}

/// The width a stroke has at `pressure` (0 to 1): half the nominal width at no pressure,
/// the full width at full pressure.
pub fn width_at(width: f32, pressure: f32) -> f32 {
    let w = if width.is_finite() {
        width.clamp(0.0, MAX_STROKE_WIDTH)
    } else {
        0.0
    };
    let p = if pressure.is_finite() {
        pressure.clamp(0.0, 1.0)
    } else {
        0.5
    };
    w * (0.5 + 0.5 * p)
}

/// An outline vertex: its index, its outward direction and its distance from the centerline.
type RimVertex = (u32, Vec2, f32);

/// One vertex of the centerline after cleaning.
#[derive(Clone, Copy)]
struct Node {
    pos: Vec2,
    radius: f32,
}

/// Tessellate a stroke of nominal `width`. An empty stroke gives an empty mesh and a
/// one-point stroke a disc.
pub fn tessellate(points: &[InkPoint], width: f32, params: TessParams) -> StrokeMesh {
    let cap = params
        .cap_segments
        .clamp(MIN_CAP_SEGMENTS, MAX_CAP_SEGMENTS);
    let mut nodes: Vec<Node> = Vec::with_capacity(points.len());
    for p in points {
        let pos = p.pos();
        if !pos.is_finite() {
            continue;
        }
        let radius = width_at(width, p.pressure) / 2.0;
        match nodes.last_mut() {
            Some(last) if last.pos.distance(pos) < MIN_STEP => {
                last.radius = last.radius.max(radius);
            }
            _ => nodes.push(Node { pos, radius }),
        }
    }
    let mut mesh = StrokeMesh::default();
    let Some(first) = nodes.first().copied() else {
        return mesh;
    };
    // Outline chains (vertex and outward direction) that get a fringe, in order along the edge.
    let mut chains: Vec<Vec<RimVertex>> = Vec::new();
    if nodes.len() == 1 {
        chains.push(disc(&mut mesh, first.pos, first.radius, cap * 2));
        add_fringe(&mut mesh, &chains, params.fringe);
        return mesh;
    }

    // Unit direction of each segment.
    let dirs: Vec<Vec2> = nodes
        .windows(2)
        .map(|pair| match pair {
            [a, b] => unit(b.pos - a.pos),
            _ => Vec2::ZERO,
        })
        .collect();
    let dir_at = |i: usize| dirs.get(i).copied().unwrap_or(Vec2::ZERO);

    // Ribbon: a left and right vertex per node.
    let mut previous: Option<(u32, u32)> = None;
    let (mut left_edge, mut right_edge) = (Vec::new(), Vec::new());
    for (i, node) in nodes.iter().enumerate() {
        let (offset, join) = if i == 0 {
            (perp(dir_at(0)), false)
        } else if i + 1 == nodes.len() {
            (perp(dir_at(i - 1)), false)
        } else {
            let (a, b) = (dir_at(i - 1), dir_at(i));
            let n = unit(perp(a) + perp(b));
            // Stretch to keep the ribbon's edges parallel to both segments at the bend.
            let cos_half = n.dot(perp(a)).max(1.0 / MAX_MITER);
            (n / cos_half, a.dot(b) < JOIN_COS)
        };
        let left = node.pos + offset * node.radius;
        let right = node.pos - offset * node.radius;
        let (l1, r1) = (push_vertex(&mut mesh, left), push_vertex(&mut mesh, right));
        if let Some((l0, r0)) = previous {
            // Quad between the last node and this one: (l0, r0, l1) and (r0, r1, l1).
            mesh.indices.extend_from_slice(&[l0, r0, l1, r0, r1, l1]);
        }
        previous = Some((l1, r1));
        let outward = unit(offset);
        left_edge.push((l1, outward, node.radius));
        right_edge.push((r1, -outward, node.radius));
        if join {
            chains.push(join_disc(
                &mut mesh,
                node.pos,
                node.radius,
                (dir_at(i - 1), dir_at(i)),
                cap,
            ));
        }
    }
    chains.push(left_edge);
    chains.push(right_edge);

    // Round caps: half discs facing away from the stroke.
    if let Some(last) = nodes.last() {
        let end_dir = dir_at(dirs.len().saturating_sub(1));
        chains.push(half_disc(&mut mesh, last.pos, last.radius, end_dir, cap));
    }
    chains.push(half_disc(
        &mut mesh,
        first.pos,
        first.radius,
        -dir_at(0),
        cap,
    ));
    add_fringe(&mut mesh, &chains, params.fringe);
    mesh
}

fn push_vertex(mesh: &mut StrokeMesh, pos: Vec2) -> u32 {
    let index = mesh.positions.len() as u32;
    mesh.positions.push(pos);
    mesh.edges.push(Edge::Inside);
    index
}

/// Add the fringe along each outline chain: every chain vertex becomes a rim vertex, gets an
/// outer twin at the same place, and each pair of neighbors gets a quad between the two rows.
/// With `enabled` false the mesh is left as it is.
fn add_fringe(mesh: &mut StrokeMesh, chains: &[Vec<RimVertex>], enabled: bool) {
    if !enabled {
        return;
    }
    for chain in chains {
        let mut previous: Option<(u32, u32)> = None;
        for &(vertex, outward, radius) in chain {
            let Some(&pos) = mesh.positions.get(vertex as usize) else {
                continue;
            };
            if let Some(edge) = mesh.edges.get_mut(vertex as usize) {
                *edge = Edge::Rim {
                    dir: outward,
                    radius,
                };
            }
            let outer = push_vertex(mesh, pos);
            if let Some(edge) = mesh.edges.get_mut(outer as usize) {
                *edge = Edge::Outer(outward);
            }
            if let Some((rim0, outer0)) = previous {
                mesh.indices
                    .extend_from_slice(&[rim0, vertex, outer, rim0, outer, outer0]);
            }
            previous = Some((vertex, outer));
        }
    }
}

/// A unit vector along `v`, or zero when `v` is too short to have a direction.
fn unit(v: Vec2) -> Vec2 {
    let len = v.length();
    if len > 1.0e-6 && len.is_finite() {
        v / len
    } else {
        Vec2::ZERO
    }
}

/// `v` turned a quarter turn (to the left of travel on a y-down page).
fn perp(v: Vec2) -> Vec2 {
    Vec2::new(v.y, -v.x)
}

/// A full disc as a triangle fan. Returns its rim for the fringe.
fn disc(mesh: &mut StrokeMesh, center: Vec2, radius: f32, segments: u32) -> Vec<RimVertex> {
    arc_fan(mesh, center, radius, 0.0, TAU, segments)
}

/// The disc that rounds a sharp turn from direction `a` to direction `b`, in two fans: the
/// wedge on the outside of the turn, which is the only part that shows past the ribbon, and the
/// rest of the circle, which lies inside the ribbon. Only the wedge's arc is returned for the
/// fringe, and it starts and ends exactly where the ribbon's edges leave the node.
fn join_disc(
    mesh: &mut StrokeMesh,
    center: Vec2,
    radius: f32,
    (a, b): (Vec2, Vec2),
    cap: u32,
) -> Vec<RimVertex> {
    let turn = (a.x * b.y - a.y * b.x).atan2(a.dot(b));
    let side = if turn >= 0.0 { perp(a) } else { -perp(a) };
    let start = side.y.atan2(side.x);
    let step = PI / cap.max(1) as f32;
    let segments = |sweep: f32| ((sweep.abs() / step).ceil() as u32).clamp(1, cap.max(1) * 2);
    let wedge = arc_fan(mesh, center, radius, start, turn, segments(turn));
    let rest = (TAU - turn.abs()).copysign(turn);
    arc_fan(mesh, center, radius, start + turn, rest, segments(rest));
    wedge
}

/// The half disc on the side of `facing`.
fn half_disc(
    mesh: &mut StrokeMesh,
    center: Vec2,
    radius: f32,
    facing: Vec2,
    segments: u32,
) -> Vec<RimVertex> {
    let facing = if facing == Vec2::ZERO {
        Vec2::new(1.0, 0.0)
    } else {
        facing
    };
    let start = facing.y.atan2(facing.x) - PI / 2.0;
    arc_fan(mesh, center, radius, start, PI, segments)
}

/// A fan of triangles from `center` over an arc of `sweep` radians starting at `start`.
/// Returns the arc's vertices with their outward directions.
fn arc_fan(
    mesh: &mut StrokeMesh,
    center: Vec2,
    radius: f32,
    start: f32,
    sweep: f32,
    n: u32,
) -> Vec<RimVertex> {
    let hub = push_vertex(mesh, center);
    let first = hub + 1;
    let mut rim = Vec::with_capacity(n as usize + 1);
    for step in 0..=n {
        let angle = start + sweep * step as f32 / n as f32;
        let outward = Vec2::new(angle.cos(), angle.sin());
        rim.push((
            push_vertex(mesh, center + outward * radius),
            outward,
            radius,
        ));
    }
    for step in 0..n {
        mesh.indices
            .extend_from_slice(&[hub, first + step, first + step + 1]);
    }
    rim
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize, step: f32, pressure: f32) -> Vec<InkPoint> {
        (0..n)
            .map(|i| InkPoint::new(i as f32 * step, 0.0, pressure))
            .collect()
    }

    /// Sum of triangle areas.
    fn area(mesh: &StrokeMesh) -> f32 {
        let (triangles, _) = mesh.indices.as_chunks::<3>();
        triangles
            .iter()
            .map(|t| {
                let p = |i: usize| mesh.positions[t[i] as usize];
                let (a, b, c) = (p(0), p(1), p(2));
                ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs() / 2.0
            })
            .sum()
    }

    fn bounds(mesh: &StrokeMesh) -> (Vec2, Vec2) {
        let mut lo = Vec2::new(f32::MAX, f32::MAX);
        let mut hi = Vec2::new(f32::MIN, f32::MIN);
        for p in &mesh.positions {
            lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        (lo, hi)
    }

    fn valid(mesh: &StrokeMesh) -> bool {
        mesh.indices.len().is_multiple_of(3)
            && mesh
                .indices
                .iter()
                .all(|&i| (i as usize) < mesh.positions.len())
            && mesh.positions.iter().all(|p| p.is_finite())
    }

    #[test]
    fn empty_and_single_point_strokes() {
        assert!(tessellate(&[], 2.0, TessParams::default()).is_empty());
        let dot = tessellate(&[InkPoint::new(5.0, 5.0, 1.0)], 4.0, TessParams::default());
        assert!(valid(&dot) && !dot.is_empty());
        let (lo, hi) = bounds(&dot);
        assert!((hi.x - lo.x - 4.0).abs() < 0.05, "diameter {}", hi.x - lo.x);
    }

    #[test]
    fn a_straight_line_is_a_rectangle_with_round_ends() {
        let mesh = tessellate(
            &line(11, 10.0, 1.0),
            6.0,
            TessParams {
                cap_segments: 16,
                fringe: true,
            },
        );
        assert!(valid(&mesh));
        let (lo, hi) = bounds(&mesh);
        // 100 long plus a radius of 3 at each end, 6 wide.
        assert!((lo.x + 3.0).abs() < 0.01 && (hi.x - 103.0).abs() < 0.01);
        assert!((hi.y - 3.0).abs() < 0.01 && (lo.y + 3.0).abs() < 0.01);
        // Rectangle 100 x 6 plus one disc of radius 3, close to pi * 9.
        let expected = 600.0 + PI * 9.0;
        assert!((area(&mesh) - expected).abs() < 1.0, "area {}", area(&mesh));
    }

    #[test]
    fn pressure_changes_the_width() {
        let mut points = line(21, 5.0, 1.0);
        for (i, p) in points.iter_mut().enumerate() {
            p.pressure = i as f32 / 20.0;
        }
        let mesh = tessellate(&points, 8.0, TessParams::default());
        assert!(valid(&mesh));
        // Width at a vertex pair is twice the radius: half the nominal width at pressure 0,
        // the full width at pressure 1. Ribbon vertices come first, two per node.
        let gap = |i: usize| mesh.positions[2 * i].distance(mesh.positions[2 * i + 1]);
        assert!((gap(0) - 4.0).abs() < 1.0e-3, "{}", gap(0));
        assert!((gap(20) - 8.0).abs() < 1.0e-3, "{}", gap(20));
        assert!(gap(10) > gap(5) && gap(15) > gap(10));
    }

    #[test]
    fn width_at_maps_pressure() {
        assert_eq!(width_at(10.0, 0.0), 5.0);
        assert_eq!(width_at(10.0, 1.0), 10.0);
        assert_eq!(width_at(10.0, 5.0), 10.0);
        assert_eq!(width_at(10.0, f32::NAN), 7.5);
        assert_eq!(width_at(f32::NAN, 1.0), 0.0);
    }

    #[test]
    fn duplicate_and_bad_points_are_skipped() {
        let points = [
            InkPoint::new(0.0, 0.0, 1.0),
            InkPoint::new(0.0, 0.0, 1.0),
            InkPoint {
                x: f32::NAN,
                y: 0.0,
                pressure: 1.0,
            },
            InkPoint::new(10.0, 0.0, 1.0),
        ];
        let mesh = tessellate(&points, 2.0, TessParams::default());
        assert!(valid(&mesh));
        let two = tessellate(&[points[0], points[3]], 2.0, TessParams::default());
        assert_eq!(mesh, two);
    }

    #[test]
    fn a_sharp_turn_gets_a_round_join_and_stays_finite() {
        // An almost full reversal: the worst case for a ribbon.
        let points = [
            InkPoint::new(0.0, 0.0, 1.0),
            InkPoint::new(50.0, 0.0, 1.0),
            InkPoint::new(0.0, 1.0, 1.0),
        ];
        let mesh = tessellate(&points, 4.0, TessParams::default());
        assert!(valid(&mesh));
        let (lo, hi) = bounds(&mesh);
        // The miter is capped, so nothing spikes far past the corner.
        assert!(hi.x < 50.0 + 2.0 * 4.0 + 1.0, "{}", hi.x);
        assert!(lo.x > -10.0);
        // A gentle bend adds no join disc: compare triangle counts.
        let gentle = [
            InkPoint::new(0.0, 0.0, 1.0),
            InkPoint::new(50.0, 0.0, 1.0),
            InkPoint::new(100.0, 5.0, 1.0),
        ];
        let smooth = tessellate(&gentle, 4.0, TessParams::default());
        assert!(mesh.triangle_count() > smooth.triangle_count());
    }

    #[test]
    fn a_curve_stays_within_its_width_of_the_path() {
        // A quarter circle of radius 100.
        let points: Vec<InkPoint> = (0..=40)
            .map(|i| {
                let a = i as f32 / 40.0 * PI / 2.0;
                InkPoint::new(100.0 * a.cos(), 100.0 * a.sin(), 1.0)
            })
            .collect();
        let mesh = tessellate(&points, 6.0, TessParams::default());
        assert!(valid(&mesh));
        for p in &mesh.positions {
            let from_center = p.length();
            assert!((from_center - 100.0).abs() <= 3.0 + 0.05, "{from_center}");
        }
    }

    #[test]
    fn size_is_bounded_by_the_point_count() {
        let points: Vec<InkPoint> = (0..2000)
            .map(|i| InkPoint::new(i as f32, ((i * 7) % 13) as f32 * 5.0, 0.8))
            .collect();
        let mesh = tessellate(
            &points,
            3.0,
            TessParams {
                cap_segments: 99,
                fringe: true,
            },
        );
        assert!(valid(&mesh));
        // At most a join disc (2 * 32 segments) and its fringe (two triangles per segment)
        // per node, plus the ribbon and its fringe.
        assert!(mesh.triangle_count() <= 2000 * (64 + 128 + 8) + 400);
        let plain = tessellate(
            &points,
            3.0,
            TessParams {
                cap_segments: 99,
                fringe: false,
            },
        );
        assert!(plain.triangle_count() <= 2000 * (2 + 64) + 200);
    }

    #[test]
    fn no_fringe_means_every_vertex_is_inside() {
        let params = TessParams {
            cap_segments: 8,
            fringe: false,
        };
        let mesh = tessellate(&line(5, 10.0, 1.0), 6.0, params);
        assert_eq!(mesh.edges.len(), mesh.positions.len());
        assert!(mesh.edges.iter().all(|e| *e == Edge::Inside));
    }

    #[test]
    fn the_fringe_wraps_the_outline_and_adds_no_area() {
        let points = line(11, 10.0, 1.0);
        let plain = tessellate(
            &points,
            6.0,
            TessParams {
                cap_segments: 8,
                fringe: false,
            },
        );
        let fringed = tessellate(&points, 6.0, TessParams::default());
        assert!(valid(&fringed));
        assert_eq!(fringed.edges.len(), fringed.positions.len());
        // The fringe sits on the outline in page units, so the area is the same.
        assert!((area(&fringed) - area(&plain)).abs() < 1.0e-3);
        let rims = fringed
            .edges
            .iter()
            .filter(|e| matches!(e, Edge::Rim { .. }))
            .count();
        let outers = fringed
            .edges
            .iter()
            .filter(|e| matches!(e, Edge::Outer(_)))
            .count();
        assert!(rims > 0 && rims == outers);
        // Every direction is a unit vector, and a rim vertex faces away from the body: the
        // mesh center line is y = 0, so a rim on the long sides points along y.
        for (edge, pos) in fringed.edges.iter().zip(&fringed.positions) {
            if let Edge::Rim { dir: d, .. } | Edge::Outer(d) = edge {
                assert!((d.length() - 1.0).abs() < 1.0e-3, "{d:?}");
                if pos.x > 0.5 && pos.x < 99.5 {
                    assert!(d.x.abs() < 1.0e-3 && (d.y * pos.y) > 0.0, "{pos:?} {d:?}");
                }
            }
        }
    }

    #[test]
    fn a_dot_and_a_turn_get_a_fringe_with_valid_indices() {
        let dot = tessellate(&[InkPoint::new(5.0, 5.0, 1.0)], 4.0, TessParams::default());
        assert!(valid(&dot));
        assert!(dot.edges.iter().any(|e| matches!(e, Edge::Outer(_))));
        let turn = tessellate(
            &[
                InkPoint::new(0.0, 0.0, 1.0),
                InkPoint::new(50.0, 0.0, 1.0),
                InkPoint::new(0.0, 1.0, 1.0),
            ],
            4.0,
            TessParams::default(),
        );
        assert!(valid(&turn));
        assert_eq!(turn.edges.len(), turn.positions.len());
    }

    #[test]
    fn a_join_fringes_only_its_outer_wedge() {
        // A right-angle turn at (50, 0): the outside of the turn is the +x, -y corner.
        let points = [
            InkPoint::new(0.0, 0.0, 1.0),
            InkPoint::new(50.0, 0.0, 1.0),
            InkPoint::new(50.0, 50.0, 1.0),
        ];
        let mesh = tessellate(&points, 4.0, TessParams::default());
        assert!(valid(&mesh));
        let corner = Vec2::new(50.0, 0.0);
        let mut on_disc = 0;
        for (edge, pos) in mesh.edges.iter().zip(&mesh.positions) {
            if let Edge::Rim { dir, radius } = edge {
                assert!(*radius > 0.0);
                if (pos.distance(corner) - 2.0).abs() < 1.0e-3 {
                    on_disc += 1;
                    assert!(dir.x > -1.0e-3 && dir.y < 1.0e-3, "{pos:?} {dir:?}");
                }
            }
        }
        assert!(on_disc >= 3, "{on_disc}");
        // The same turn without a fringe has as many triangles in the fill.
        let plain = tessellate(
            &points,
            4.0,
            TessParams {
                cap_segments: 8,
                fringe: false,
            },
        );
        assert!(mesh.triangle_count() > plain.triangle_count());
        assert!((area(&mesh) - area(&plain)).abs() < 1.0e-2);
    }

    #[test]
    fn cap_quality_follows_the_screen_size() {
        let small = TessParams::for_screen_radius(0.5).cap_segments;
        let big = TessParams::for_screen_radius(40.0).cap_segments;
        assert!(small >= MIN_CAP_SEGMENTS && big > small && big <= MAX_CAP_SEGMENTS);
        assert_eq!(
            TessParams::for_screen_radius(f32::NAN).cap_segments,
            MIN_CAP_SEGMENTS
        );
    }
}
