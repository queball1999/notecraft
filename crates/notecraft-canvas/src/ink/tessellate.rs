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
    /// One per position: how far along the stroke's centerline the vertex lies, in page
    /// units (the arc length of the smoothed centerline; a cap runs on past the ends). A
    /// texture that follows the stroke reads its horizontal coordinate from this.
    pub along: Vec<f32>,
    /// One per position: where the vertex lies across the stroke, from -1 on the right edge
    /// of the direction of travel to 1 on the left edge, 0 on the centerline.
    pub across: Vec<f32>,
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
    /// Run the ribbon through a smooth curve between the stored points (see
    /// [`smooth_centerline`]) instead of straight segments. Only the mesh changes; the stored
    /// points do not.
    pub curve: bool,
}

impl Default for TessParams {
    fn default() -> Self {
        Self {
            cap_segments: 8,
            fringe: true,
            curve: true,
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
            curve: true,
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
    if params.curve {
        nodes = smooth_nodes(&nodes);
    }
    let mut mesh = StrokeMesh::default();
    let Some(first) = nodes.first().copied() else {
        return mesh;
    };
    // Outline chains (vertex and outward direction) that get a fringe, in order along the edge.
    let mut chains: Vec<Vec<RimVertex>> = Vec::new();
    // Arc length of the centerline at each node, for textures that follow the stroke.
    let mut lengths: Vec<f32> = Vec::with_capacity(nodes.len());
    let mut run = 0.0_f32;
    for (i, node) in nodes.iter().enumerate() {
        if let Some(before) = i.checked_sub(1).and_then(|j| nodes.get(j)) {
            run += before.pos.distance(node.pos);
        }
        lengths.push(run);
    }
    if nodes.len() == 1 {
        let from = mesh.positions.len();
        chains.push(disc(&mut mesh, first.pos, first.radius, cap * 2));
        tag(
            &mut mesh,
            from,
            (0.0, first.pos, Vec2::new(1.0, 0.0), first.radius),
        );
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
        let s = lengths.get(i).copied().unwrap_or(0.0);
        place(&mut mesh, l1, s, 1.0);
        place(&mut mesh, r1, s, -1.0);
        if let Some((l0, r0)) = previous {
            // Quad between the last node and this one: (l0, r0, l1) and (r0, r1, l1).
            mesh.indices.extend_from_slice(&[l0, r0, l1, r0, r1, l1]);
        }
        previous = Some((l1, r1));
        let outward = unit(offset);
        left_edge.push((l1, outward, node.radius));
        right_edge.push((r1, -outward, node.radius));
        if join {
            let from = mesh.positions.len();
            chains.push(join_disc(
                &mut mesh,
                node.pos,
                node.radius,
                (dir_at(i - 1), dir_at(i)),
                cap,
            ));
            tag(&mut mesh, from, (s, node.pos, dir_at(i - 1), node.radius));
        }
    }
    chains.push(left_edge);
    chains.push(right_edge);

    // Round caps: half discs facing away from the stroke.
    if let Some(last) = nodes.last() {
        let end_dir = dir_at(dirs.len().saturating_sub(1));
        let from = mesh.positions.len();
        chains.push(half_disc(&mut mesh, last.pos, last.radius, end_dir, cap));
        let end = lengths.last().copied().unwrap_or(0.0);
        tag(&mut mesh, from, (end, last.pos, end_dir, last.radius));
    }
    let from = mesh.positions.len();
    chains.push(half_disc(
        &mut mesh,
        first.pos,
        first.radius,
        -dir_at(0),
        cap,
    ));
    tag(&mut mesh, from, (0.0, first.pos, dir_at(0), first.radius));
    add_fringe(&mut mesh, &chains, params.fringe);
    mesh
}

/// Turn (radians) one extra centerline point stands for: a segment between two stored points
/// gets one more point for each this much its direction changes at its two ends.
const CURVE_STEP: f32 = 0.12;
/// Most points one segment is cut into.
const MAX_SUBDIVISIONS: usize = 16;
/// Most centerline points a smoothed stroke gets, so a very long stroke stays cheap.
const MAX_SMOOTH_NODES: usize = 60_000;

/// The angle between two direction vectors in radians, 0 when either has no direction.
fn turn_between(a: Vec2, b: Vec2) -> f32 {
    if a == Vec2::ZERO || b == Vec2::ZERO {
        return 0.0;
    }
    a.dot(b).clamp(-1.0, 1.0).acos()
}

/// A point on the cubic Hermite curve from `p1` to `p2` with end tangents `m1` and `m2`.
fn hermite(p1: Vec2, m1: Vec2, p2: Vec2, m2: Vec2, t: f32) -> Vec2 {
    let (t2, t3) = (t * t, t * t * t);
    p1 * (2.0 * t3 - 3.0 * t2 + 1.0)
        + m1 * (t3 - 2.0 * t2 + t)
        + p2 * (-2.0 * t3 + 3.0 * t2)
        + m2 * (t3 - t2)
}

/// Run the centerline through a smooth curve: each segment between stored points becomes a
/// Hermite curve whose end tangents follow the neighboring points (a Catmull-Rom spline with
/// tangents scaled to the segment, so uneven spacing does not overshoot). Straight stretches
/// stay one segment; bends get more points in proportion to how much they turn. Radii are
/// interpolated linearly. Every stored point stays on the curve.
fn smooth_nodes(nodes: &[Node]) -> Vec<Node> {
    let Some(first) = nodes.first().copied() else {
        return Vec::new();
    };
    if nodes.len() < 3 {
        return nodes.to_vec();
    }
    let at = |i: isize| -> Node {
        let last = nodes.len() as isize - 1;
        nodes
            .get(i.clamp(0, last) as usize)
            .copied()
            .unwrap_or(first)
    };
    // Fewer subdivisions for very long strokes keep the total bounded.
    let cap = (MAX_SMOOTH_NODES / nodes.len()).clamp(1, MAX_SUBDIVISIONS);
    let mut out = Vec::with_capacity(nodes.len() * 2);
    out.push(first);
    for i in 0..nodes.len() as isize - 1 {
        let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let chord = p2.pos - p1.pos;
        let length = chord.length();
        let (d0, d1, d2) = (unit(p1.pos - p0.pos), unit(chord), unit(p3.pos - p2.pos));
        let bend = turn_between(d0, d1) + turn_between(d1, d2);
        let count = ((bend / CURVE_STEP).ceil() as usize).clamp(1, cap);
        // Tangent directions come from the neighbors; at the ends, from the segment itself.
        let t1 = if i == 0 { d1 } else { unit(p2.pos - p0.pos) };
        let t2 = if i + 2 >= nodes.len() as isize {
            d1
        } else {
            unit(p3.pos - p1.pos)
        };
        let (m1, m2) = (t1 * length, t2 * length);
        for step in 1..count {
            let t = step as f32 / count as f32;
            out.push(Node {
                pos: hermite(p1.pos, m1, p2.pos, m2, t),
                radius: p1.radius + (p2.radius - p1.radius) * t,
            });
        }
        out.push(p2);
    }
    out
}

/// The centerline [`tessellate`] draws for `points`: the stored points with the extra points
/// that make the curve through them smooth. Pressure is interpolated along the curve.
pub fn smooth_centerline(points: &[InkPoint]) -> Vec<InkPoint> {
    // A unit width makes the node radius the width at that pressure, so pressure can be
    // recovered from it.
    let nodes: Vec<Node> = points
        .iter()
        .filter(|p| p.pos().is_finite())
        .map(|p| Node {
            pos: p.pos(),
            radius: p.pressure.clamp(0.0, 1.0),
        })
        .collect();
    smooth_nodes(&nodes)
        .into_iter()
        .map(|n| InkPoint::new(n.pos.x, n.pos.y, n.radius.clamp(0.0, 1.0)))
        .collect()
}

fn push_vertex(mesh: &mut StrokeMesh, pos: Vec2) -> u32 {
    let index = mesh.positions.len() as u32;
    mesh.positions.push(pos);
    mesh.edges.push(Edge::Inside);
    mesh.along.push(0.0);
    mesh.across.push(0.0);
    index
}

/// Set where one vertex lies along and across the stroke.
fn place(mesh: &mut StrokeMesh, vertex: u32, along: f32, across: f32) {
    if let Some(slot) = mesh.along.get_mut(vertex as usize) {
        *slot = along;
    }
    if let Some(slot) = mesh.across.get_mut(vertex as usize) {
        *slot = across;
    }
}

/// Set `along` and `across` for every vertex from index `from` on, as seen from a node of
/// the centerline: `(arc length at the node, node position, direction of travel, radius)`.
/// A vertex ahead of the node (a cap) runs on past its arc length.
fn tag(mesh: &mut StrokeMesh, from: usize, (s, center, dir, radius): (f32, Vec2, Vec2, f32)) {
    let side = perp(dir);
    for i in from..mesh.positions.len() {
        let Some(&p) = mesh.positions.get(i) else {
            break;
        };
        let offset = p - center;
        let across = if radius > 1.0e-6 {
            (offset.dot(side) / radius).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        place(mesh, i as u32, s + offset.dot(dir), across);
    }
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
            let (along, across) = (
                mesh.along.get(vertex as usize).copied().unwrap_or(0.0),
                mesh.across.get(vertex as usize).copied().unwrap_or(0.0),
            );
            place(mesh, outer, along, across);
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
    fn every_vertex_knows_where_it_lies_along_and_across_the_stroke() {
        let mesh = tessellate(&line(11, 10.0, 1.0), 6.0, TessParams::default());
        assert_eq!(mesh.along.len(), mesh.positions.len());
        assert_eq!(mesh.across.len(), mesh.positions.len());
        for ((p, s), v) in mesh.positions.iter().zip(&mesh.along).zip(&mesh.across) {
            // A straight line along x: arc length is x, and across follows y (down is right).
            assert!((s - p.x).abs() < 0.01, "{p:?} {s}");
            assert!((v + p.y / 3.0).abs() < 0.01, "{p:?} {v}");
            assert!((-1.0..=1.0).contains(v));
        }
        // The ribbon's own edges sit at exactly +1 and -1.
        assert!(mesh.across.iter().any(|v| *v == 1.0));
        assert!(mesh.across.iter().any(|v| *v == -1.0));
        // A fringe copies the vertex it hangs on.
        let bare = tessellate(
            &line(11, 10.0, 1.0),
            6.0,
            TessParams {
                fringe: false,
                ..TessParams::default()
            },
        );
        assert!(mesh.positions.len() > bare.positions.len());
        // A dot and a bend are tagged too.
        let dot = tessellate(&[InkPoint::new(5.0, 5.0, 1.0)], 4.0, TessParams::default());
        assert_eq!(dot.along.len(), dot.positions.len());
        let bend = [
            InkPoint::new(0.0, 0.0, 1.0),
            InkPoint::new(50.0, 0.0, 1.0),
            InkPoint::new(50.0, 50.0, 1.0),
        ];
        let mesh = tessellate(&bend, 4.0, TessParams::default());
        let far = mesh.along.iter().copied().fold(0.0_f32, f32::max);
        assert!((far - 102.0).abs() < 6.0, "{far}");
        assert!(mesh.along.iter().all(|s| s.is_finite()));
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
                ..TessParams::default()
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
                curve: false,
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
                curve: false,
            },
        );
        assert!(plain.triangle_count() <= 2000 * (2 + 64) + 200);
    }

    #[test]
    fn no_fringe_means_every_vertex_is_inside() {
        let params = TessParams {
            cap_segments: 8,
            fringe: false,
            curve: true,
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
                curve: true,
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
        let mesh = tessellate(
            &points,
            4.0,
            TessParams {
                curve: false,
                ..TessParams::default()
            },
        );
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
                curve: false,
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

    /// The largest turn between successive segments of a polyline, in degrees.
    fn max_turn_degrees(points: &[InkPoint]) -> f32 {
        let dirs: Vec<Vec2> = points
            .windows(2)
            .filter_map(|w| {
                let d = w[1].pos() - w[0].pos();
                (d.length() > 1.0e-4).then(|| unit(d))
            })
            .collect();
        dirs.windows(2)
            .map(|w| turn_between(w[0], w[1]).to_degrees())
            .fold(0.0, f32::max)
    }

    /// A circle of `radius` sampled `n` times a lap, as a fast pen on a slow frame would.
    fn circle(radius: f32, n: usize) -> Vec<InkPoint> {
        (0..=n)
            .map(|i| {
                let a = TAU * i as f32 / n as f32;
                InkPoint::new(300.0 + radius * a.cos(), 300.0 + radius * a.sin(), 0.8)
            })
            .collect()
    }

    #[test]
    fn a_fast_circle_has_no_sharp_angle_jumps_once_smoothed() {
        // 14 samples a lap is 25.7 degrees between samples: a visible polygon.
        let raw = circle(100.0, 14);
        let raw_turn = max_turn_degrees(&raw);
        assert!(raw_turn > 25.0, "{raw_turn}");
        let smooth = smooth_centerline(&raw);
        assert!(smooth.len() > raw.len() * 2);
        let turn = max_turn_degrees(&smooth);
        assert!(
            turn < 10.0 && turn < raw_turn / 2.0,
            "max turn {turn} degrees"
        );
        // The curve stays on the circle: no overshoot between the samples.
        for p in &smooth {
            let r = p.pos().distance(Vec2::new(300.0, 300.0));
            assert!((r - 100.0).abs() < 1.5, "radius {r}");
        }
        // The mesh uses the curve.
        let curved = tessellate(&raw, 4.0, TessParams::default());
        let straight = tessellate(
            &raw,
            4.0,
            TessParams {
                curve: false,
                ..TessParams::default()
            },
        );
        assert!(valid(&curved) && curved.positions.len() > straight.positions.len());
    }

    #[test]
    fn smoothing_keeps_every_stored_point_and_straight_lines_straight() {
        let raw = circle(80.0, 20);
        let smooth = smooth_centerline(&raw);
        for p in &raw {
            assert!(
                smooth.iter().any(|q| q.pos().distance(p.pos()) < 1.0e-3),
                "{p:?} was moved"
            );
        }
        let straight = smooth_centerline(&line(30, 10.0, 0.5));
        assert_eq!(straight.len(), 30);
        // Pressure is interpolated between the neighbors.
        let two = [
            InkPoint::new(0.0, 0.0, 0.0),
            InkPoint::new(50.0, 40.0, 1.0),
            InkPoint::new(100.0, 0.0, 0.0),
        ];
        for p in smooth_centerline(&two) {
            assert!((0.0..=1.0).contains(&p.pressure));
        }
    }

    #[test]
    fn smoothing_is_bounded_and_ignores_bad_points() {
        let zigzag: Vec<InkPoint> = (0..20_000)
            .map(|i| InkPoint::new(i as f32, if i % 2 == 0 { 0.0 } else { 30.0 }, 1.0))
            .collect();
        assert!(smooth_centerline(&zigzag).len() <= MAX_SMOOTH_NODES + zigzag.len());
        let bad = [
            InkPoint::new(f32::NAN, 0.0, 1.0),
            InkPoint::new(0.0, 0.0, 1.0),
            InkPoint::new(10.0, 5.0, 1.0),
        ];
        assert!(smooth_centerline(&bad).iter().all(|p| p.pos().is_finite()));
        assert!(smooth_centerline(&[]).is_empty());
    }
}
