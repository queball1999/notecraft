//! Ink on the canvas: turns egui pointer events into engine pointer samples, builds strokes
//! while the pen is down, sends each finished stroke as an `ink.add_stroke` command, and
//! paints the page's strokes as meshes.
//!
//! What pressure arrives here: egui has no pressure on mouse events. It reports a force on
//! `Event::Touch` only, which egui-winit fills from winit's touch force. winit 0.30 supplies
//! that force on Windows (pen and touch through the pointer messages) and on iOS, and
//! nothing on macOS, X11, or Wayland. So a pen on Windows draws with pressure, and
//! everywhere else the stroke width follows speed. See build_plan/03 for the full notes.

use std::collections::HashMap;
use std::sync::Arc;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Painter, Shape};
use notecraft_canvas::ink::{tessellate, Edge, InkLayer, StrokeBuilder, StrokeMesh, TessParams};
use notecraft_core::{
    Aabb, InkColor, InkPoint, PointerEvent, PointerKind, PointerPhase, PointerSample, Stroke,
};
use notecraft_engine::cmd::{ids, Value};
use notecraft_engine::PenStyle;

use super::PageToScreen;
use crate::ribbon::Clicked;

/// Pointer events read from one frame, at most.
const MAX_EVENTS_PER_FRAME: usize = 4096;
/// A frame gap longer than this is treated as one normal frame when spreading event times.
const MAX_FRAME_GAP: f64 = 0.1;
const NORMAL_FRAME: f64 = 1.0 / 120.0;

/// Convert the events of one frame into engine pointer events.
///
/// Mouse events come from `PointerButton` and `PointerMoved`. When the frame has touch
/// events (a touch screen or a pen that winit reports as touch), those are used instead,
/// because egui also emulates a mouse from them and each point would count twice. Touch with
/// a force is reported as a pen with that pressure; touch without one is touch. egui events
/// carry no time, so the frame's events are spread evenly between the previous frame time and
/// this one.
pub(crate) fn pointer_events(
    events: &[egui::Event],
    map: PageToScreen,
    previous_time: Option<f64>,
    now: f64,
    active_touch: &mut Option<egui::TouchId>,
) -> Vec<PointerEvent> {
    let touching = events
        .iter()
        .any(|event| matches!(event, egui::Event::Touch { .. }));
    // (phase, position, pressure, kind) before times are assigned.
    let mut raw = Vec::new();
    for event in events.iter().take(MAX_EVENTS_PER_FRAME) {
        match *event {
            egui::Event::Touch {
                id,
                phase,
                pos,
                force,
                ..
            } => {
                let phase = match phase {
                    egui::TouchPhase::Start if active_touch.is_none() => {
                        *active_touch = Some(id);
                        PointerPhase::Down
                    }
                    _ if *active_touch != Some(id) => continue,
                    egui::TouchPhase::Start => continue,
                    egui::TouchPhase::Move => PointerPhase::Move,
                    egui::TouchPhase::End => {
                        *active_touch = None;
                        PointerPhase::Up
                    }
                    egui::TouchPhase::Cancel => {
                        *active_touch = None;
                        PointerPhase::Cancel
                    }
                };
                let kind = if force.is_some() {
                    PointerKind::Pen
                } else {
                    PointerKind::Touch
                };
                raw.push((phase, pos, force, kind));
            }
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                ..
            } if !touching => {
                let phase = if pressed {
                    PointerPhase::Down
                } else {
                    PointerPhase::Up
                };
                raw.push((phase, pos, None, PointerKind::Mouse));
            }
            egui::Event::PointerMoved(pos) if !touching => {
                raw.push((PointerPhase::Move, pos, None, PointerKind::Mouse));
            }
            _ => {}
        }
    }
    let start = match previous_time {
        Some(prev) if now > prev && now - prev <= MAX_FRAME_GAP => prev,
        _ => now - NORMAL_FRAME,
    };
    let count = raw.len().max(1) as f64;
    raw.into_iter()
        .enumerate()
        .filter_map(|(i, (phase, pos, pressure, kind))| {
            let time = start + (now - start) * (i + 1) as f64 / count;
            let page = map.page(pos);
            let sample = PointerSample::new(page, time, pressure, kind)?;
            Some(PointerEvent { phase, sample })
        })
        .collect()
}

/// The `points` argument of `ink.add_stroke`: `[[x, y, pressure], ...]` rounded to two places.
pub(crate) fn points_arg(points: &[InkPoint]) -> Value {
    // Rounded in f64 so the JSON shows 1.23, not 1.2300000190734863.
    let round = |v: f32| (f64::from(v) * 100.0).round() / 100.0;
    Value::Array(
        points
            .iter()
            .map(|p| Value::from(vec![round(p.x), round(p.y), round(p.pressure)]))
            .collect(),
    )
}

/// Ink state kept between frames: the stroke being drawn and the tessellation cache.
#[derive(Debug, Default)]
pub(crate) struct InkUi {
    active: Option<StrokeBuilder>,
    touch: Option<egui::TouchId>,
    last_time: Option<f64>,
    cache: MeshCache,
}

/// What a frame of capture decided.
pub(crate) struct Capture {
    /// The stroke to draw as a preview this frame (in progress, or just finished while the
    /// command takes effect).
    pub preview: Option<Vec<InkPoint>>,
}

impl InkUi {
    /// Read this frame's pointer events. While `pen` is `Some` (the pen is the active tool),
    /// a press on the canvas starts a stroke, moves extend it, and a release finishes it and
    /// pushes `ink.add_stroke` onto `clicked`. `can_start` says whether a press at a screen
    /// position may start a stroke (inside the canvas, not under a menu).
    pub fn capture(
        &mut self,
        ui: &egui::Ui,
        map: PageToScreen,
        pen: Option<PenStyle>,
        can_start: impl Fn(egui::Pos2) -> bool,
        clicked: &mut Vec<Clicked>,
    ) -> Capture {
        let now = ui.input(|input| input.time);
        let previous = self.last_time.replace(now);
        let events =
            ui.input(|input| pointer_events(&input.events, map, previous, now, &mut self.touch));
        if pen.is_none() {
            self.active = None;
            return Capture { preview: None };
        }
        let mut finished = None;
        for event in events {
            match event.phase {
                PointerPhase::Down => {
                    let at = map.pos(event.sample.pos);
                    if self.active.is_none() && can_start(at) {
                        let mut builder = StrokeBuilder::default();
                        builder.push(event.sample);
                        self.active = Some(builder);
                    }
                }
                PointerPhase::Move => {
                    if let Some(builder) = self.active.as_mut() {
                        builder.push(event.sample);
                    }
                }
                PointerPhase::Up => {
                    if let Some(mut builder) = self.active.take() {
                        builder.push(event.sample);
                        finished = Some(builder.points());
                    }
                }
                PointerPhase::Cancel => self.active = None,
            }
        }
        // A release the window never saw (focus lost mid-stroke) still ends the stroke.
        let down = ui.input(|input| input.pointer.primary_down());
        if finished.is_none() && !down && self.touch.is_none() {
            if let Some(builder) = self.active.take() {
                finished = Some(builder.points());
            }
        }
        if let Some(points) = finished {
            if points.is_empty() {
                return Capture { preview: None };
            }
            let mut args = notecraft_engine::cmd::Map::new();
            args.insert("points".to_owned(), points_arg(&points));
            clicked.push(Clicked {
                command: ids::INK_ADD_STROKE,
                args: Value::Object(args),
            });
            // Draw the whole stroke once more this frame; the committed one replaces it on
            // the next, which the repaint request below brings right away.
            ui.ctx().request_repaint();
            return Capture {
                preview: Some(points),
            };
        }
        if let Some(builder) = self.active.as_ref() {
            ui.ctx().request_repaint();
            return Capture {
                preview: Some(builder.points()),
            };
        }
        Capture { preview: None }
    }

    /// Paint the page's strokes that touch `visible` (page units) and then the preview.
    pub fn paint(
        &mut self,
        painter: &Painter,
        map: PageToScreen,
        visible: Aabb,
        page: Option<&str>,
        layer: Option<&InkLayer>,
        preview: Option<(&[InkPoint], PenStyle)>,
    ) {
        self.cache.set_page(page);
        let half_pixel = 0.5 / painter.ctx().pixels_per_point();
        let mut mesh = Mesh::default();
        if let Some(layer) = layer {
            for stroke in layer.strokes_in(visible) {
                let tessellated = self.cache.mesh_for(stroke, map.zoom);
                append(&mut mesh, &tessellated, map, stroke.color, half_pixel);
            }
        }
        if let Some((points, pen)) = preview {
            let params = TessParams {
                fringe: pen.color.0[3] == u8::MAX,
                ..TessParams::for_screen_radius(pen.width_units() / 2.0 * map.zoom)
            };
            let tessellated = tessellate(points, pen.width_units(), params);
            append(&mut mesh, &tessellated, map, pen.color, half_pixel);
        }
        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }
}

/// Add a page-space triangle list to an egui mesh, in screen space and one color.
///
/// Anti-aliasing: a rim vertex moves `half_pixel` (egui points) inward, but never past the
/// centerline, and an outer vertex moves `half_pixel` outward with no coverage. The edge then
/// fades over one device pixel, like egui's own feathered shapes. The page-to-screen map only
/// scales and shifts, so the directions are the same on screen.
fn append(
    mesh: &mut Mesh,
    tessellated: &StrokeMesh,
    map: PageToScreen,
    color: InkColor,
    half_pixel: f32,
) {
    let [r, g, b, a] = color.0;
    let color = Color32::from_rgba_unmultiplied(r, g, b, a);
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(
        tessellated
            .positions
            .iter()
            .zip(&tessellated.edges)
            .map(|(p, edge)| {
                let at = map.pos(*p);
                let (shift, color) = match *edge {
                    Edge::Inside => (egui::Vec2::ZERO, color),
                    Edge::Rim { dir, radius } => {
                        let inset = half_pixel.min(radius * map.zoom);
                        (-egui::vec2(dir.x, dir.y) * inset, color)
                    }
                    Edge::Outer(dir) => {
                        (egui::vec2(dir.x, dir.y) * half_pixel, Color32::TRANSPARENT)
                    }
                };
                Vertex {
                    pos: at + shift,
                    uv: WHITE_UV,
                    color,
                }
            }),
    );
    mesh.indices
        .extend(tessellated.indices.iter().map(|i| base + i));
}

/// A tessellated stroke and what it was made from, to notice when it is stale.
#[derive(Debug)]
struct CachedMesh {
    points: usize,
    last: Option<InkPoint>,
    width: f32,
    cap_segments: u32,
    fringe: bool,
    mesh: Arc<StrokeMesh>,
}

/// Page-space meshes by stroke id, for the page being shown. Strokes are drawn every frame, so
/// tessellating each one once saves most of the work; zoom only re-tessellates when the cap
/// quality changes.
#[derive(Debug, Default)]
struct MeshCache {
    page: Option<String>,
    meshes: HashMap<String, CachedMesh>,
}

impl MeshCache {
    fn set_page(&mut self, page: Option<&str>) {
        if self.page.as_deref() != page {
            self.page = page.map(str::to_owned);
            self.meshes.clear();
        }
    }

    fn mesh_for(&mut self, stroke: &Stroke, zoom: f32) -> Arc<StrokeMesh> {
        let cap_segments = TessParams::for_screen_radius(stroke.width / 2.0 * zoom).cap_segments;
        // A translucent stroke overlaps itself, so a fringe would show through: it gets none.
        let fringe = stroke.color.0[3] == u8::MAX;
        let fresh = |cached: &CachedMesh| {
            cached.points == stroke.points.len()
                && cached.last == stroke.points.last().copied()
                && cached.width == stroke.width
                && cached.cap_segments == cap_segments
                && cached.fringe == fringe
        };
        if let Some(cached) = self.meshes.get(&stroke.id).filter(|c| fresh(c)) {
            return Arc::clone(&cached.mesh);
        }
        let mesh = Arc::new(tessellate(
            &stroke.points,
            stroke.width,
            TessParams {
                cap_segments,
                fringe,
            },
        ));
        self.meshes.insert(
            stroke.id.clone(),
            CachedMesh {
                points: stroke.points.len(),
                last: stroke.points.last().copied(),
                width: stroke.width,
                cap_segments,
                fringe,
                mesh: Arc::clone(&mesh),
            },
        );
        mesh
    }
}

/// The visible part of the page as a box, for culling.
pub(crate) fn visible_box(map: PageToScreen, screen: egui::Rect) -> Aabb {
    let (min, max) = map.page_rect(screen);
    Aabb::from_corners(min, max)
}

#[cfg(test)]
mod tests {
    use notecraft_core::Vec2;

    use super::*;

    const MAP: PageToScreen = PageToScreen {
        origin: egui::pos2(100.0, 50.0),
        zoom: 2.0,
    };

    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn touch(id: u64, phase: egui::TouchPhase, pos: egui::Pos2, force: Option<f32>) -> egui::Event {
        egui::Event::Touch {
            device_id: egui::TouchDeviceId(0),
            id: egui::TouchId(id),
            phase,
            pos,
            force,
        }
    }

    #[test]
    fn mouse_events_become_page_samples_without_pressure() {
        let events = [
            button(egui::pos2(110.0, 70.0), true),
            egui::Event::PointerMoved(egui::pos2(120.0, 70.0)),
            button(egui::pos2(130.0, 70.0), false),
        ];
        let mut touch = None;
        let out = pointer_events(&events, MAP, Some(1.0), 1.01, &mut touch);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].phase, PointerPhase::Down);
        assert_eq!(out[0].sample.pos, Vec2::new(5.0, 10.0));
        assert_eq!(out[1].sample.pos, Vec2::new(10.0, 10.0));
        assert_eq!(out[2].phase, PointerPhase::Up);
        assert!(out.iter().all(|e| e.sample.pressure.is_none()));
        assert!(out.iter().all(|e| e.sample.kind == PointerKind::Mouse));
        // Times rise through the frame and end at its time.
        assert!(out[0].sample.time < out[1].sample.time);
        assert!((out[2].sample.time - 1.01).abs() < 1.0e-9);
    }

    #[test]
    fn touch_force_becomes_pen_pressure_and_hides_the_emulated_mouse() {
        let events = [
            touch(
                1,
                egui::TouchPhase::Start,
                egui::pos2(110.0, 70.0),
                Some(0.25),
            ),
            // egui also emulates a mouse from touch; those must not count twice.
            button(egui::pos2(110.0, 70.0), true),
            egui::Event::PointerMoved(egui::pos2(120.0, 70.0)),
            touch(
                1,
                egui::TouchPhase::Move,
                egui::pos2(120.0, 70.0),
                Some(0.75),
            ),
            touch(1, egui::TouchPhase::End, egui::pos2(130.0, 70.0), Some(0.0)),
        ];
        let mut active = None;
        let out = pointer_events(&events, MAP, None, 5.0, &mut active);
        let phases: Vec<_> = out.iter().map(|e| e.phase).collect();
        assert_eq!(
            phases,
            [PointerPhase::Down, PointerPhase::Move, PointerPhase::Up]
        );
        assert_eq!(out[0].sample.pressure, Some(0.25));
        assert_eq!(out[1].sample.pressure, Some(0.75));
        assert!(out.iter().all(|e| e.sample.kind == PointerKind::Pen));
        assert_eq!(active, None);
    }

    #[test]
    fn touch_without_force_is_touch_and_only_one_finger_draws() {
        let events = [
            touch(1, egui::TouchPhase::Start, egui::pos2(110.0, 70.0), None),
            touch(2, egui::TouchPhase::Start, egui::pos2(300.0, 300.0), None),
            touch(2, egui::TouchPhase::Move, egui::pos2(310.0, 300.0), None),
            touch(1, egui::TouchPhase::Move, egui::pos2(120.0, 70.0), None),
            touch(1, egui::TouchPhase::Cancel, egui::pos2(120.0, 70.0), None),
        ];
        let mut active = None;
        let out = pointer_events(&events, MAP, None, 5.0, &mut active);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|e| e.sample.kind == PointerKind::Touch));
        assert_eq!(out[2].phase, PointerPhase::Cancel);
        assert_eq!(active, None);
    }

    #[test]
    fn bad_positions_and_other_buttons_are_ignored() {
        let events = [
            button(egui::pos2(f32::NAN, 0.0), true),
            egui::Event::PointerButton {
                pos: egui::pos2(110.0, 70.0),
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        let mut touch = None;
        assert!(pointer_events(&events, MAP, None, 1.0, &mut touch).is_empty());
        let many = vec![egui::Event::PointerMoved(egui::pos2(110.0, 70.0)); 100_000];
        let out = pointer_events(&many, MAP, None, 1.0, &mut touch);
        assert_eq!(out.len(), MAX_EVENTS_PER_FRAME);
    }

    #[test]
    fn points_become_a_command_argument() {
        let arg = points_arg(&[InkPoint::new(1.234, 5.0, 0.5), InkPoint::new(2.0, 3.0, 1.0)]);
        assert_eq!(arg[0][0], 1.23);
        assert_eq!(arg[0][2], 0.5);
        assert_eq!(arg[1][1], 3.0);
    }

    #[test]
    fn a_fringed_mesh_fades_over_one_pixel_around_the_edge() {
        let points = [InkPoint::new(0.0, 0.0, 1.0), InkPoint::new(20.0, 0.0, 1.0)];
        let page = tessellate(&points, 8.0, TessParams::default());
        let map = PageToScreen {
            origin: egui::pos2(0.0, 0.0),
            zoom: 1.0,
        };
        let mut mesh = Mesh::default();
        // A half pixel of 0.25 points, as on a 2x display.
        append(&mut mesh, &page, map, InkColor::BLACK, 0.25);
        assert_eq!(mesh.vertices.len(), page.positions.len());
        assert!(mesh
            .indices
            .iter()
            .all(|i| (*i as usize) < mesh.vertices.len()));
        let [r, g, b, a] = InkColor::BLACK.0;
        let solid = Color32::from_rgba_unmultiplied(r, g, b, a);
        let mut seen = (0, 0);
        for ((vertex, edge), at) in mesh.vertices.iter().zip(&page.edges).zip(&page.positions) {
            let moved = (vertex.pos - egui::pos2(at.x, at.y)).length();
            match edge {
                Edge::Inside => assert!(moved < 1.0e-6 && vertex.color == solid),
                Edge::Rim { .. } => {
                    seen.0 += 1;
                    assert!((moved - 0.25).abs() < 1.0e-4 && vertex.color == solid);
                }
                Edge::Outer(_) => {
                    seen.1 += 1;
                    assert!((moved - 0.25).abs() < 1.0e-4 && vertex.color == Color32::TRANSPARENT);
                }
            }
        }
        assert!(seen.0 > 0 && seen.0 == seen.1);
        // A line thinner than a pixel is not turned inside out: the inset stops at the center.
        let thin = tessellate(&points, 0.2, TessParams::default());
        let mut mesh = Mesh::default();
        append(&mut mesh, &thin, map, InkColor::BLACK, 0.5);
        assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
        for (vertex, edge) in mesh.vertices.iter().zip(&thin.edges) {
            if matches!(edge, Edge::Rim { .. }) && vertex.pos.x > 5.0 && vertex.pos.x < 15.0 {
                assert!(vertex.pos.y.abs() < 1.0e-4, "{:?}", vertex.pos);
            }
        }
    }

    #[test]
    fn the_mesh_cache_reuses_and_refreshes() {
        let mut cache = MeshCache::default();
        cache.set_page(Some("p1"));
        let mut stroke = Stroke {
            id: "s1".into(),
            tool: notecraft_core::InkTool::Pen,
            color: InkColor::BLACK,
            width: 4.0,
            points: vec![InkPoint::new(0.0, 0.0, 1.0), InkPoint::new(10.0, 0.0, 1.0)],
        };
        let a = cache.mesh_for(&stroke, 1.0);
        let b = cache.mesh_for(&stroke, 1.0);
        assert!(Arc::ptr_eq(&a, &b));
        // A stroke that reuses the id with other points is not served from the cache.
        stroke.points.push(InkPoint::new(20.0, 5.0, 1.0));
        let c = cache.mesh_for(&stroke, 1.0);
        assert!(!Arc::ptr_eq(&a, &c));
        // Changing page empties it.
        cache.set_page(Some("p2"));
        assert!(cache.meshes.is_empty());
    }
}
