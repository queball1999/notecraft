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
    Aabb, InkColor, InkPoint, InkTool, PointerEvent, PointerKind, PointerPhase, PointerSample,
    Stroke, Vec2,
};
use notecraft_engine::cmd::{ids, Value};
use notecraft_engine::PenStyle;

use super::{multiply, PageToScreen};
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
    /// The eraser drag in progress: the last page position sent and whether the next batch is
    /// the drag's first (which starts an undo step; later ones join it).
    erasing: Option<EraseDrag>,
    touch: Option<egui::TouchId>,
    last_time: Option<f64>,
    cache: MeshCache,
    /// The stroke being drawn, tessellated in finished chunks so a frame only works on the
    /// newest points.
    preview: PreviewCache,
}

#[derive(Clone, Copy, Debug)]
struct EraseDrag {
    last: Vec2,
    first: bool,
}

/// What the pointer does on the canvas this frame.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Tool {
    /// Nothing is captured.
    Off,
    /// Draw strokes with this style (pen or highlighter).
    Draw(PenStyle),
    /// Erase; the mode id (`stroke` or `point`) and the point eraser size id.
    Erase(&'static str, &'static str),
}

/// How far the stroke eraser reaches past a stroke's edge, in screen pixels.
pub(crate) const STROKE_ERASER_REACH_PX: f32 = 6.0;
/// Radii of the point eraser's circle by size, in screen pixels.
pub(crate) const POINT_ERASER_SMALL_PX: f32 = 5.0;
pub(crate) const POINT_ERASER_MEDIUM_PX: f32 = 10.0;
pub(crate) const POINT_ERASER_LARGE_PX: f32 = 20.0;

/// The eraser's reach in screen pixels for a mode id and a point eraser size id.
pub(crate) fn eraser_reach_px(mode: &str, size: &str) -> f32 {
    if mode != "point" {
        return STROKE_ERASER_REACH_PX;
    }
    match size {
        "small" => POINT_ERASER_SMALL_PX,
        "large" => POINT_ERASER_LARGE_PX,
        _ => POINT_ERASER_MEDIUM_PX,
    }
}

/// What a frame of capture decided.
pub(crate) struct Capture {
    /// The stroke to draw as a preview this frame (in progress, or just finished while the
    /// command takes effect).
    pub preview: Option<Vec<InkPoint>>,
}

impl InkUi {
    /// Read this frame's pointer events. While drawing (`Tool::Draw`), a press on the canvas
    /// starts a stroke, moves extend it, and a release finishes it and pushes `ink.add_stroke`
    /// onto `clicked`. While erasing (`Tool::Erase`), a press starts a drag, and each frame of
    /// it pushes one `ink.erase` with the path the pointer took; only the first of a drag
    /// starts an undo step and the rest join it. `can_start` says whether a press at a screen
    /// position may start (inside the canvas, not under a menu).
    pub fn capture(
        &mut self,
        ui: &egui::Ui,
        map: PageToScreen,
        tool: Tool,
        can_start: impl Fn(egui::Pos2) -> bool,
        clicked: &mut Vec<Clicked>,
    ) -> Capture {
        let now = ui.input(|input| input.time);
        let previous = self.last_time.replace(now);
        let events =
            ui.input(|input| pointer_events(&input.events, map, previous, now, &mut self.touch));
        let pen = match tool {
            Tool::Draw(pen) => Some(pen),
            Tool::Off | Tool::Erase(..) => None,
        };
        if let Tool::Erase(mode, size) = tool {
            self.active = None;
            self.capture_erase(ui, map, (mode, size), events, can_start, clicked);
            return Capture { preview: None };
        }
        self.erasing = None;
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

    /// Turn one frame of pointer events into an `ink.erase` command for the drag in progress.
    fn capture_erase(
        &mut self,
        ui: &egui::Ui,
        map: PageToScreen,
        eraser: (&'static str, &'static str),
        events: Vec<PointerEvent>,
        can_start: impl Fn(egui::Pos2) -> bool,
        clicked: &mut Vec<Clicked>,
    ) {
        // The path the eraser took this frame, starting where the last batch ended.
        let mut path: Vec<Vec2> = Vec::new();
        if let Some(drag) = self.erasing {
            path.push(drag.last);
        }
        let mut ended = false;
        for event in events {
            let at = event.sample.pos;
            match event.phase {
                PointerPhase::Down => {
                    if self.erasing.is_none() && can_start(map.pos(at)) {
                        self.erasing = Some(EraseDrag {
                            last: at,
                            first: true,
                        });
                        path.push(at);
                    }
                }
                PointerPhase::Move => {
                    if self.erasing.is_some() {
                        path.push(at);
                    }
                }
                PointerPhase::Up => {
                    if self.erasing.is_some() {
                        path.push(at);
                        ended = true;
                    }
                }
                PointerPhase::Cancel => {
                    self.erasing = None;
                    path.clear();
                }
            }
        }
        // A release the window never saw (focus lost mid-drag) still ends the drag.
        let down = ui.input(|input| input.pointer.primary_down());
        if !ended && !down && self.touch.is_none() && self.erasing.is_some() {
            ended = true;
        }
        if let (Some(drag), Some(last)) = (self.erasing.as_mut(), path.last().copied()) {
            // A lone start point still erases (a tap); a path that did not move since the
            // last batch has nothing new to say.
            let moved = path.len() > 1 && path.iter().any(|p| *p != drag.last);
            if drag.first || moved {
                let radius = eraser_reach_px(eraser.0, eraser.1) / map.zoom.max(f32::EPSILON);
                clicked.push(Clicked {
                    command: ids::INK_ERASE,
                    args: erase_args(&path, radius, eraser.0, !drag.first),
                });
                drag.first = false;
            }
            drag.last = last;
        }
        if ended {
            self.erasing = None;
        }
        if self.erasing.is_some() {
            ui.ctx().request_repaint();
        }
    }

    /// Paint the page's highlighter strokes that touch `visible` (page units), and then the
    /// highlighter stroke being drawn. This pass goes under the page text.
    ///
    /// Each stroke is one opaque mesh in its color blended with the paper at about half
    /// strength, so a stroke cannot darken itself where its own triangles overlap and the
    /// anti-aliased fringe stays valid. True multiply (strokes darkening each other, images
    /// tinted) would be an `egui_wgpu` callback later; nothing stored changes.
    pub fn paint_highlights(
        &mut self,
        painter: &Painter,
        map: PageToScreen,
        visible: Aabb,
        page: Option<&str>,
        layer: Option<&InkLayer>,
        preview: Option<(&[InkPoint], PenStyle)>,
        paper: Color32,
    ) {
        self.cache.set_page(page);
        let half_pixel = 0.5 / painter.ctx().pixels_per_point();
        let mut mesh = Mesh::default();
        if let Some(layer) = layer {
            for stroke in layer.strokes_in(visible) {
                if stroke.tool != InkTool::Highlighter {
                    continue;
                }
                let tessellated = self.cache.mesh_for(stroke, map.zoom);
                let color = highlight_color(stroke.color, paper);
                append(&mut mesh, &tessellated, map, color, half_pixel);
            }
        }
        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
        if let Some((points, style)) = preview {
            let color = highlight_color(style.color, paper);
            let shapes =
                self.preview
                    .shapes(points, style.width_units(), color, true, map, half_pixel);
            painter.extend(shapes);
        } else {
            self.preview.clear();
        }
    }

    /// Whether the page's highlighter strokes in `visible` can go through the multiply paint
    /// callback: the app installed it and there are not more strokes than one frame draws.
    pub fn can_multiply(ctx: &egui::Context, layer: Option<&InkLayer>, visible: Aabb) -> bool {
        multiply::ready(ctx)
            && layer.is_none_or(|layer| {
                layer
                    .strokes_in(visible)
                    .filter(|stroke| stroke.tool == InkTool::Highlighter)
                    .count()
                    < multiply::MAX_STROKES
            })
    }

    /// The multiply version of [`Self::paint_highlights`]: each highlighter stroke (and the one
    /// being drawn) is sent to a paint callback that multiplies it over everything painted so
    /// far, so this pass goes after the page text and images and before the pens. Rule lines
    /// and text show through the color, and strokes that cross darken each other. Use only
    /// when [`Self::can_multiply`] is true.
    pub fn paint_highlights_multiply(
        &mut self,
        painter: &Painter,
        map: PageToScreen,
        visible: Aabb,
        page: Option<&str>,
        layer: Option<&InkLayer>,
        preview: Option<(&[InkPoint], PenStyle)>,
    ) {
        self.cache.set_page(page);
        let scale = painter.ctx().pixels_per_point();
        let half_pixel = 0.5 / scale;
        let mut strokes = Vec::new();
        if let Some(layer) = layer {
            for stroke in layer.strokes_in(visible) {
                if stroke.tool != InkTool::Highlighter {
                    continue;
                }
                let tessellated = self.cache.mesh_for(stroke, map.zoom);
                let mut mesh = Mesh::default();
                let opaque =
                    InkColor([stroke.color.0[0], stroke.color.0[1], stroke.color.0[2], 255]);
                append(&mut mesh, &tessellated, map, opaque, half_pixel);
                strokes.push(gpu_stroke(&[&mesh], stroke.color));
            }
        }
        if let Some((points, style)) = preview {
            let color = style.color;
            let opaque = InkColor([color.0[0], color.0[1], color.0[2], 255]);
            let shapes =
                self.preview
                    .shapes(points, style.width_units(), opaque, true, map, half_pixel);
            let meshes: Vec<&Mesh> = shapes
                .iter()
                .filter_map(|shape| match shape {
                    Shape::Mesh(mesh) => Some(mesh.as_ref()),
                    _ => None,
                })
                .collect();
            strokes.push(gpu_stroke(&meshes, color));
        } else {
            self.preview.clear();
        }
        if !strokes.is_empty() {
            painter.add(egui_wgpu::Callback::new_paint_callback(
                painter.clip_rect(),
                multiply::HighlightCallback { strokes },
            ));
        }
    }

    /// Paint the page's pen strokes that touch `visible` (page units) and then the pen stroke
    /// being drawn. This pass goes over the page text.
    pub fn paint_pens(
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
                if stroke.tool == InkTool::Highlighter {
                    continue;
                }
                let tessellated = self.cache.mesh_for(stroke, map.zoom);
                append(&mut mesh, &tessellated, map, stroke.color, half_pixel);
            }
        }
        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
        if let Some((points, pen)) = preview {
            let fringe = pen.color.0[3] == u8::MAX;
            let shapes = self.preview.shapes(
                points,
                pen.width_units(),
                pen.color,
                fringe,
                map,
                half_pixel,
            );
            painter.extend(shapes);
        } else {
            self.preview.clear();
        }
    }
}

/// Strength of the highlighter over the paper when the stroke's color has no alpha of its own.
const HIGHLIGHT_ALPHA: f32 = 0.5;

/// One highlighter stroke for the multiply callback: the meshes (screen space, the stroke's
/// color opaque, so a vertex's alpha is its edge coverage) merged into one list. `color` is
/// the stroke's own color; its alpha is the strength, or [`HIGHLIGHT_ALPHA`] when stored
/// opaque.
fn gpu_stroke(meshes: &[&Mesh], color: InkColor) -> multiply::GpuStroke {
    let mut stroke = multiply::GpuStroke {
        vertices: Vec::new(),
        indices: Vec::new(),
        rgb: [color.0[0], color.0[1], color.0[2]],
        strength: if color.0[3] == u8::MAX {
            HIGHLIGHT_ALPHA
        } else {
            f32::from(color.0[3]) / 255.0
        },
    };
    for mesh in meshes {
        let base = stroke.vertices.len() as u32;
        stroke.vertices.extend(
            mesh.vertices
                .iter()
                .map(|v| ([v.pos.x, v.pos.y], f32::from(v.color.a()) / 255.0)),
        );
        stroke.indices.extend(mesh.indices.iter().map(|i| base + i));
    }
    stroke
}

/// The opaque color a highlighter stroke is drawn in: `color` over `paper` at the stroke's own
/// alpha, or at [`HIGHLIGHT_ALPHA`] when the stroke is stored opaque.
pub(crate) fn highlight_color(color: InkColor, paper: Color32) -> InkColor {
    let [r, g, b, a] = color.0;
    let alpha = if a == u8::MAX {
        HIGHLIGHT_ALPHA
    } else {
        f32::from(a) / 255.0
    };
    let mix = |over: u8, under: u8| {
        (f32::from(over) * alpha + f32::from(under) * (1.0 - alpha)).round() as u8
    };
    InkColor([
        mix(r, paper.r()),
        mix(g, paper.g()),
        mix(b, paper.b()),
        u8::MAX,
    ])
}

/// The arguments of `ink.erase` for a path of page positions.
pub(crate) fn erase_args(path: &[Vec2], radius: f32, mode: &str, join: bool) -> Value {
    let round = |v: f32| (f64::from(v) * 100.0).round() / 100.0;
    let mut args = notecraft_engine::cmd::Map::new();
    args.insert(
        "points".to_owned(),
        Value::Array(
            path.iter()
                .map(|p| Value::from(vec![round(p.x), round(p.y)]))
                .collect(),
        ),
    );
    args.insert("radius".to_owned(), Value::from(round(radius)));
    args.insert("mode".to_owned(), Value::from(mode));
    args.insert("join".to_owned(), Value::from(join));
    Value::Object(args)
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

/// Points in one finished chunk of the stroke being drawn.
const PREVIEW_CHUNK: usize = 48;
/// Points either side of a chunk that are tessellated with it, so the curve through its
/// ends matches the whole stroke's and the chunks join without a gap. The overlapping parts
/// are drawn twice in the same color.
const PREVIEW_OVERLAP: usize = 3;

/// What a finished chunk was made from; a change in any of it rebuilds the chunks.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PreviewKey {
    origin: egui::Pos2,
    zoom: f32,
    half_pixel: f32,
    color: InkColor,
    width: f32,
    fringe: bool,
    first: Option<InkPoint>,
}

/// The stroke in progress as finished chunks plus a tail. A frame adds a point or several;
/// without this every frame would tessellate the whole stroke again, which slows down as the
/// stroke grows. Chunks are screen-space meshes shared with the painter by `Arc`.
#[derive(Debug, Default)]
struct PreviewCache {
    key: Option<PreviewKey>,
    chunks: Vec<Arc<Mesh>>,
}

impl PreviewCache {
    fn clear(&mut self) {
        self.key = None;
        self.chunks.clear();
    }

    /// The shapes that draw `points`: cached chunks for the settled part and a mesh for the
    /// rest, built now. A translucent stroke (`fringe` false) is drawn whole, because
    /// overlapping chunks would darken it.
    fn shapes(
        &mut self,
        points: &[InkPoint],
        width: f32,
        color: InkColor,
        fringe: bool,
        map: PageToScreen,
        half_pixel: f32,
    ) -> Vec<Shape> {
        let key = PreviewKey {
            origin: map.origin,
            zoom: map.zoom,
            half_pixel,
            color,
            width,
            fringe,
            first: points.first().copied(),
        };
        let too_many = self.chunks.len() * PREVIEW_CHUNK + PREVIEW_OVERLAP > points.len();
        if self.key != Some(key) || too_many {
            self.chunks.clear();
            self.key = Some(key);
        }
        let params = TessParams {
            fringe,
            ..TessParams::for_screen_radius(width / 2.0 * map.zoom)
        };
        let chunked = fringe;
        let build = |range: std::ops::Range<usize>| {
            let slice = points.get(range).unwrap_or_default();
            let mut mesh = Mesh::default();
            append(
                &mut mesh,
                &tessellate(slice, width, params),
                map,
                color,
                half_pixel,
            );
            mesh
        };
        // A chunk is final once OVERLAP more points follow it.
        while chunked && (self.chunks.len() + 1) * PREVIEW_CHUNK + PREVIEW_OVERLAP <= points.len() {
            let index = self.chunks.len();
            let start = (index * PREVIEW_CHUNK).saturating_sub(PREVIEW_OVERLAP);
            let end = (index + 1) * PREVIEW_CHUNK + PREVIEW_OVERLAP;
            self.chunks.push(Arc::new(build(start..end)));
        }
        let mut shapes: Vec<Shape> = self
            .chunks
            .iter()
            .map(|chunk| Shape::mesh(Arc::clone(chunk)))
            .collect();
        let settled = self.chunks.len() * PREVIEW_CHUNK;
        let tail = build(settled.saturating_sub(PREVIEW_OVERLAP)..points.len());
        if !tail.is_empty() {
            shapes.push(Shape::mesh(tail));
        }
        shapes
    }
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
        // A translucent pen stroke overlaps itself, so a fringe would show through: it gets
        // none. A highlighter stroke is painted opaque (pre-blended), so it always has one.
        let fringe = stroke.color.0[3] == u8::MAX || stroke.tool == InkTool::Highlighter;
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
                curve: true,
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
    fn a_highlighter_color_is_blended_with_the_paper_and_opaque() {
        let paper = Color32::from_rgb(255, 255, 255);
        let yellow = InkColor::parse("#FFF176").unwrap();
        // Half way to white: 0xF1 and 0x76 move toward 0xFF.
        assert_eq!(highlight_color(yellow, paper).0, [255, 248, 187, 255]);
        // On a dark page the same highlighter is darker: it follows the paper.
        let dark = Color32::from_rgb(30, 30, 30);
        assert_eq!(highlight_color(yellow, dark).0, [143, 136, 74, 255]);
        // A color stored with its own alpha uses that alpha.
        let faint = InkColor([200, 0, 0, 51]);
        assert_eq!(highlight_color(faint, paper).0, [244, 204, 204, 255]);
    }

    #[test]
    fn erase_arguments_round_to_two_places() {
        let args = erase_args(
            &[Vec2::new(1.234, 5.0), Vec2::new(2.0, 3.0)],
            7.777,
            "point",
            true,
        );
        assert_eq!(args["points"][0][0], 1.23);
        assert_eq!(args["points"][1][1], 3.0);
        assert_eq!(args["radius"], 7.78);
        assert_eq!(args["mode"], "point");
        assert_eq!(args["join"], true);
    }

    #[test]
    fn the_eraser_reach_is_in_screen_pixels() {
        assert_eq!(eraser_reach_px("point", "small"), POINT_ERASER_SMALL_PX);
        assert_eq!(eraser_reach_px("point", "medium"), POINT_ERASER_MEDIUM_PX);
        assert_eq!(eraser_reach_px("point", "large"), POINT_ERASER_LARGE_PX);
        assert_eq!(eraser_reach_px("point", "odd"), POINT_ERASER_MEDIUM_PX);
        // The stroke eraser has one reach whatever the size is.
        assert_eq!(eraser_reach_px("stroke", "large"), STROKE_ERASER_REACH_PX);
        assert_eq!(eraser_reach_px("anything", "small"), STROKE_ERASER_REACH_PX);
    }

    #[test]
    fn the_mesh_cache_reuses_and_refreshes() {
        let mut cache = MeshCache::default();
        cache.set_page(Some("p1"));
        let mut stroke = Stroke {
            texture: Default::default(),
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

    /// Mouse moves for one frame: `n` distinct positions along a line.
    fn moves(n: usize) -> Vec<egui::Event> {
        (0..n)
            .map(|i| egui::Event::PointerMoved(egui::pos2(110.0 + 3.0 * i as f32, 70.0)))
            .collect()
    }

    #[test]
    fn every_pointer_sample_of_a_frame_reaches_the_stroke() {
        // egui delivers many moves in one frame when the app is slower than the mouse; none
        // may be dropped, only the same point twice.
        let mut events = vec![button(egui::pos2(110.0, 70.0), true)];
        events.extend(moves(200));
        events.push(button(egui::pos2(110.0 + 3.0 * 200.0, 70.0), false));
        let mut touch = None;
        let out = pointer_events(&events, MAP, Some(1.0), 1.016, &mut touch);
        assert_eq!(out.len(), 202);
        assert!(out.windows(2).all(|w| w[0].sample.time < w[1].sample.time));
        let mut builder = StrokeBuilder::default();
        for event in out {
            builder.push(event.sample);
        }
        // The first move repeats the press position, so one sample is a duplicate.
        assert_eq!(builder.points().len(), 201);
    }

    fn circle_points(n: usize) -> Vec<InkPoint> {
        (0..n)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / 120.0;
                InkPoint::new(300.0 + 100.0 * a.cos(), 300.0 + 100.0 * a.sin(), 0.8)
            })
            .collect()
    }

    #[test]
    fn the_preview_reuses_finished_chunks_and_matches_a_whole_tessellation() {
        let map = PageToScreen {
            origin: egui::pos2(0.0, 0.0),
            zoom: 1.0,
        };
        let mut cache = PreviewCache::default();
        let all = circle_points(200);
        let first = cache.shapes(&all[..120], 4.0, InkColor::BLACK, true, map, 0.25);
        let chunks = cache.chunks.len();
        assert_eq!(chunks, (120 - PREVIEW_OVERLAP) / PREVIEW_CHUNK);
        assert!(chunks >= 2);
        let kept: Vec<Arc<Mesh>> = cache.chunks.clone();
        let more = cache.shapes(&all, 4.0, InkColor::BLACK, true, map, 0.25);
        // The chunks that were finished are the very same meshes.
        for (old, new) in kept.iter().zip(&cache.chunks) {
            assert!(Arc::ptr_eq(old, new));
        }
        assert!(cache.chunks.len() > chunks);
        assert!(more.len() > first.len() - 1);
        // The drawn area covers the same box as the whole stroke tessellated at once.
        let bounds = |shapes: &[Shape]| {
            shapes
                .iter()
                .filter_map(|shape| match shape {
                    Shape::Mesh(mesh) => Some(mesh.calc_bounds()),
                    _ => None,
                })
                .fold(egui::Rect::NOTHING, |a, b| a.union(b))
        };
        let mut whole = Mesh::default();
        let full = tessellate(&all, 4.0, TessParams::default());
        append(&mut whole, &full, map, InkColor::BLACK, 0.25);
        let (a, b) = (bounds(&more), whole.calc_bounds());
        assert!((a.min - b.min).length() < 0.5 && (a.max - b.max).length() < 0.5);
        // A different stroke starts over, and a translucent one is drawn whole.
        let _ = cache.shapes(&all[10..], 4.0, InkColor::BLACK, true, map, 0.25);
        assert!(cache.chunks.len() * PREVIEW_CHUNK + PREVIEW_OVERLAP <= all.len() - 10);
        let _ = cache.shapes(&all, 4.0, InkColor([0, 0, 0, 128]), false, map, 0.25);
        assert!(cache.chunks.is_empty());
    }
}
