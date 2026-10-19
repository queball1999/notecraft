//! Pointer, keyboard, and IME handling for text editing on the canvas (PR 1.5). egui only
//! reports events here; every effect is a command pushed onto `clicked` (`text.set_caret`,
//! `text.move`, `container.new`, `container.move`, and so on), which the shell dispatches by
//! id. The caret and selection live in the session, so this module keeps only what lasts a
//! gesture: a drag in progress, the remembered column for Up and Down, and the IME composition.

use egui::{CursorIcon, EventFilter, PointerButton, Pos2, Rect, Sense, Stroke};
use notecraft_core::Vec2;
use notecraft_engine::cmd::{ids, CommandId, Value};
use notecraft_engine::{DrawTool, Session};
use notecraft_markdown::ContainerRun;
use notecraft_model::Container;

use super::flow::{self, Flow};
use super::keymap::{self, Action};
use super::rawtext::Raw;
use super::{PageToScreen, Parsed};
use crate::ribbon::Clicked;
use crate::themes::{mix, ThemeTokens};

/// Height of the bar above a container that moves it, in page units.
pub(super) const BAR_H: f32 = 10.0;
/// Space between the bar and the container, in page units.
const BAR_GAP: f32 = 3.0;
/// Width of the strip at the right edge that resizes a container, in page units.
const EDGE_W: f32 = 8.0;
/// Most containers that get move and resize handles in one frame.
const MAX_HANDLES: usize = 256;
/// How far outside a container a click still counts as inside it, in page units.
const HIT_SLOP: f32 = 6.0;

/// A gesture that lasts several frames.
#[derive(Clone, Debug, PartialEq)]
enum Drag {
    /// Dragging out a text selection.
    Select,
    /// Moving container `id`, which was at `start`.
    Move {
        id: String,
        start: Vec2,
        delta: Vec2,
    },
    /// Resizing container `id`, which was `start` wide.
    Resize { id: String, start: f32, delta: f32 },
}

/// Text editing state kept between frames.
#[derive(Debug, Default)]
pub(super) struct EditUi {
    /// The IME composition being typed; shown at the caret, not in the page text.
    pub preedit: String,
    /// The screen x kept while moving up and down through rows.
    h_pos: Option<f32>,
    drag: Option<Drag>,
    /// The offset last sent while dragging a selection.
    last_extend: Option<usize>,
}

impl EditUi {
    /// The geometry of each run, with a drag in progress applied for the preview.
    pub fn geometry(&self, session: &Session, page: &str, runs: &[ContainerRun]) -> Vec<Container> {
        runs.iter()
            .map(|run| {
                let mut container = session.container(page, &run.id);
                match &self.drag {
                    Some(Drag::Move { id, start, delta }) if *id == run.id => {
                        container.pos =
                            Some(notecraft_model::container::clamp_pos(*start + *delta));
                    }
                    Some(Drag::Resize { id, start, delta }) if *id == run.id => {
                        container.width = notecraft_model::container::clamp_width(start + delta);
                    }
                    _ => {}
                }
                container
            })
            .collect()
    }

    /// True while the user is dragging a container or its edge.
    pub fn dragging_container(&self) -> bool {
        matches!(self.drag, Some(Drag::Move { .. } | Drag::Resize { .. }))
    }
}

/// What the pointer is over, as a place in the page text.
enum Hit {
    Offset(usize),
    /// Empty canvas, outside every container.
    Empty,
}

/// What the canvas knows this frame.
pub(super) struct Frame<'a> {
    pub session: &'a Session,
    pub parsed: &'a Parsed,
    pub text: &'a str,
    pub page: &'a str,
    pub flow: &'a Flow,
    pub map: PageToScreen,
    pub tokens: &'a ThemeTokens,
    /// Space is held: the user is panning, not editing.
    pub panning: bool,
}

fn command(id: CommandId, args: &[(&str, Value)]) -> Clicked {
    let map = args
        .iter()
        .map(|(name, value)| ((*name).to_owned(), value.clone()))
        .collect();
    Clicked {
        command: id,
        args: Value::Object(map),
    }
}

fn set_caret(at: usize, extend: bool) -> Clicked {
    command(
        ids::TEXT_SET_CARET,
        &[("at", Value::from(at)), ("extend", Value::from(extend))],
    )
}

fn select(at: usize, kind: &str) -> Clicked {
    command(
        ids::TEXT_SET_CARET,
        &[("at", Value::from(at)), ("select", Value::from(kind))],
    )
}

fn screen_rect(map: PageToScreen, rect: Rect) -> Rect {
    Rect::from_min_max(
        map.pos(Vec2::new(rect.min.x, rect.min.y)),
        map.pos(Vec2::new(rect.max.x, rect.max.y)),
    )
}

fn bar_rect(container: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(container.min.x, container.min.y - BAR_GAP - BAR_H),
        Pos2::new(container.max.x, container.min.y - BAR_GAP),
    )
}

fn edge_rect(container: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(container.max.x, container.min.y),
        Pos2::new(container.max.x + EDGE_W, container.max.y),
    )
}

/// The place in the text under `point` (page units).
fn hit(frame: &Frame<'_>, painter: &egui::Painter, point: Pos2) -> Hit {
    let flow = frame.flow;
    let mut inside_any = false;
    let mut best: Option<(f32, usize)> = None;
    let mut consider = |distance: f32, offset: usize| {
        if best.is_none_or(|(d, _)| distance < d) {
            best = Some((distance, offset));
        }
    };
    for container in &flow.containers {
        if !container.rect.expand(HIT_SLOP).contains(point) {
            continue;
        }
        inside_any = true;
        let Some(run) = frame.parsed.runs.get(container.run) else {
            continue;
        };
        // Past the last line of a container: its end.
        consider(f32::MAX / 2.0, run.content.end);
        for raw in flow.raws.iter().filter(|r| r.run == container.run) {
            consider(raw.rect().distance_to_pos(point), raw.offset_at(point));
        }
        for block in flow.blocks.iter().filter(|b| b.run == container.run) {
            let found = frame
                .parsed
                .doc
                .blocks
                .iter()
                .find(|b| b.span == block.span);
            let Some(found) = found else { continue };
            let distance = block.rect.distance_to_pos(point);
            if distance < f32::MAX / 4.0 {
                let probe = flow::probe(
                    painter,
                    frame.tokens,
                    frame.map.zoom,
                    frame.text,
                    found,
                    block.rect.min,
                    block.rect.width(),
                );
                consider(distance, probe.offset_at(point));
            }
        }
    }
    match best {
        Some((_, offset)) => Hit::Offset(offset),
        None if inside_any => Hit::Empty,
        None => Hit::Empty,
    }
}

/// Handle the pointer, the keyboard, and the IME for one frame, and paint the container
/// handles. Does nothing unless the page can be edited and the Type tool is active.
pub(super) fn interact(
    ui: &egui::Ui,
    response: &egui::Response,
    frame: &Frame<'_>,
    edit: &mut EditUi,
    clicked: &mut Vec<Clicked>,
) {
    let session = frame.session;
    let editable = session.page_is_editable(frame.page) && session.ink().tool() == DrawTool::Type;
    if !editable {
        edit.drag = None;
        ui.ctx()
            .data_mut(|d| d.insert_temp(super::typing_flag(), false));
        return;
    }
    if response.clicked() || response.drag_started() {
        // Keys go to the canvas once the user has clicked in it.
        response.request_focus();
    }
    let over_handle = handles(ui, response, frame, edit, clicked);
    if !over_handle && !frame.panning {
        pointer(ui, response, frame, edit, clicked);
    }
    keyboard(ui, response, frame, edit, clicked);
    if response.hovered() && !over_handle && !frame.panning {
        ui.ctx().set_cursor_icon(CursorIcon::Text);
    }
}

/// The move bar and resize edge of the containers near the pointer or holding the caret.
/// Returns whether the pointer is on one.
fn handles(
    ui: &egui::Ui,
    response: &egui::Response,
    frame: &Frame<'_>,
    edit: &mut EditUi,
    clicked: &mut Vec<Clicked>,
) -> bool {
    let map = frame.map;
    let pointer = ui.input(|input| input.pointer.hover_pos());
    let caret_run = frame.session.text_cursor().and_then(|cursor| {
        frame
            .parsed
            .runs
            .iter()
            .position(|run| run.content.start <= cursor.head && cursor.head <= run.content.end)
    });
    let mut over = false;
    for container in frame.flow.containers.iter().take(MAX_HANDLES) {
        let Some(run) = frame.parsed.runs.get(container.run) else {
            continue;
        };
        let area = screen_rect(
            map,
            container.rect.expand2(egui::vec2(EDGE_W, BAR_H + BAR_GAP)),
        );
        let dragging_this = matches!(
            &edit.drag,
            Some(Drag::Move { id, .. } | Drag::Resize { id, .. }) if *id == run.id
        );
        let near = pointer.is_some_and(|p| area.contains(p)) && !frame.panning;
        let active = caret_run == Some(container.run);
        if !(near || active || dragging_this) {
            continue;
        }
        paint_chrome(
            ui.painter(),
            map,
            container.rect,
            frame.tokens,
            active || dragging_this,
        );
        if frame.panning {
            continue;
        }
        let bar = ui.interact(
            screen_rect(map, bar_rect(container.rect)),
            response.id.with(("move", &run.id)),
            Sense::drag(),
        );
        bar.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Move container")
        });
        let edge = ui.interact(
            screen_rect(map, edge_rect(container.rect)),
            response.id.with(("resize", &run.id)),
            Sense::drag(),
        );
        edge.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Resize container")
        });
        let zoom = map.zoom.max(0.01);
        if bar.drag_started_by(PointerButton::Primary) {
            let start = Vec2::new(container.rect.min.x, container.rect.min.y);
            edit.drag = Some(Drag::Move {
                id: run.id.clone(),
                start,
                delta: Vec2::ZERO,
            });
        }
        if edge.drag_started_by(PointerButton::Primary) {
            edit.drag = Some(Drag::Resize {
                id: run.id.clone(),
                start: container.rect.width(),
                delta: 0.0,
            });
        }
        if bar.dragged() || edge.dragged() {
            let moved = bar.drag_delta() + edge.drag_delta();
            match &mut edit.drag {
                Some(Drag::Move { delta, .. }) if bar.dragged() => {
                    *delta = *delta + Vec2::new(moved.x, moved.y) / zoom;
                }
                Some(Drag::Resize { delta, .. }) if edge.dragged() => *delta += moved.x / zoom,
                _ => {}
            }
        }
        if bar.hovered() || bar.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::Move);
        }
        if edge.hovered() || edge.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        if bar.drag_stopped() || edge.drag_stopped() {
            if let Some(done) = edit.drag.take() {
                clicked.extend(finish_drag(&done));
            }
        }
        over |= bar.hovered() || bar.dragged() || edge.hovered() || edge.dragged();
    }
    over || edit.dragging_container()
}

/// The command that commits a finished container drag.
fn finish_drag(drag: &Drag) -> Option<Clicked> {
    match drag {
        Drag::Move { id, start, delta } => {
            let to = *start + *delta;
            Some(command(
                ids::CONTAINER_MOVE,
                &[
                    ("id", Value::from(id.as_str())),
                    ("x", Value::from(f64::from(to.x))),
                    ("y", Value::from(f64::from(to.y))),
                ],
            ))
        }
        Drag::Resize { id, start, delta } => Some(command(
            ids::CONTAINER_RESIZE,
            &[
                ("id", Value::from(id.as_str())),
                ("width", Value::from(f64::from(start + delta))),
            ],
        )),
        Drag::Select => None,
    }
}

/// The frame, bar, and edge shown around a container that is hovered or being edited.
fn paint_chrome(
    painter: &egui::Painter,
    map: PageToScreen,
    container: Rect,
    tokens: &ThemeTokens,
    active: bool,
) {
    let line = if active {
        tokens.accent
    } else {
        mix(tokens.border, tokens.text, 0.2)
    };
    let body = screen_rect(map, container.expand(2.0));
    painter.rect_stroke(body, 1.0, Stroke::new(1.0, line), egui::StrokeKind::Outside);
    let bar = screen_rect(map, bar_rect(container));
    painter.rect_filled(bar, 1.0, mix(tokens.canvas_bg, tokens.text, 0.1));
    for i in -1..=1 {
        let dot = bar.center() + egui::vec2(i as f32 * 6.0, 0.0);
        painter.circle_filled(dot, 1.2, tokens.text_muted);
    }
    let edge = screen_rect(map, edge_rect(container));
    let x = edge.center().x;
    painter.line_segment(
        [Pos2::new(x, edge.min.y), Pos2::new(x, edge.max.y)],
        Stroke::new(2.0, line),
    );
}

/// Clicks, double and triple clicks, and dragging out a selection.
fn pointer(
    ui: &egui::Ui,
    response: &egui::Response,
    frame: &Frame<'_>,
    edit: &mut EditUi,
    clicked: &mut Vec<Clicked>,
) {
    let primary = PointerButton::Primary;
    let shift = ui.input(|input| input.modifiers.shift);
    let to_page = |p: egui::Pos2| {
        let page = frame.map.page(p);
        Pos2::new(page.x, page.y)
    };
    let hit_at = |p: egui::Pos2| hit(frame, ui.painter(), to_page(p));
    if response.drag_started_by(primary) {
        let origin = ui.input(|input| input.pointer.press_origin());
        if let Some(Hit::Offset(at)) = origin.map(hit_at) {
            clicked.push(set_caret(at, shift));
            edit.drag = Some(Drag::Select);
            edit.last_extend = Some(at);
        }
    } else if response.dragged_by(primary) && edit.drag == Some(Drag::Select) {
        let now = ui.input(|input| input.pointer.hover_pos());
        if let Some(Hit::Offset(at)) = now.map(hit_at) {
            if edit.last_extend != Some(at) {
                clicked.push(set_caret(at, true));
                edit.last_extend = Some(at);
            }
        }
    }
    if response.drag_stopped() && edit.drag == Some(Drag::Select) {
        edit.drag = None;
    }
    let click_at = response.interact_pointer_pos();
    let Some(at) = click_at.map(hit_at) else {
        return;
    };
    if response.triple_clicked() {
        if let Hit::Offset(at) = at {
            clicked.push(select(at, "line"));
        }
    } else if response.double_clicked() {
        match at {
            Hit::Offset(at) => clicked.push(select(at, "word")),
            // A double click on empty space starts a new container there.
            Hit::Empty => {
                if let Some(p) = click_at {
                    let page = frame.map.page(p);
                    clicked.push(command(
                        ids::CONTAINER_NEW,
                        &[
                            ("x", Value::from(f64::from(page.x))),
                            ("y", Value::from(f64::from(page.y))),
                        ],
                    ));
                }
            }
        }
    } else if response.clicked() {
        match at {
            Hit::Offset(at) => {
                clicked.push(set_caret(at, shift));
                edit.h_pos = None;
            }
            // A single click on empty space only ends editing; it does not make a container.
            Hit::Empty => clicked.push(command(ids::TEXT_CLEAR_CARET, &[])),
        }
    }
}

/// The raw region that holds the caret.
fn caret_raw(flow: &Flow, head: usize) -> Option<&Raw> {
    flow.raws
        .iter()
        .find(|raw| raw.span.start <= head && head <= raw.span.end)
}

/// Where Up or Down goes from a raw region's first or last row: the end of the block above or
/// the start of the block below, in the same container.
fn neighbor_block(frame: &Frame<'_>, raw: &Raw, down: bool) -> Option<usize> {
    let run = frame.parsed.runs.get(raw.run)?;
    let blocks = frame.parsed.doc.blocks.get(run.blocks.clone())?;
    if down {
        blocks
            .iter()
            .find(|b| b.span.start >= raw.span.end)
            .map(|b| b.span.start)
    } else {
        let block = blocks.iter().rev().find(|b| b.span.end <= raw.span.start)?;
        // The end of the block's text, not the line break after it.
        let text = frame.text.get(..block.span.end)?;
        Some(text.trim_end_matches(['\n', '\r']).len())
    }
}

/// Keys, text, clipboard events, and IME while the canvas has focus and a caret is placed.
fn keyboard(
    ui: &egui::Ui,
    response: &egui::Response,
    frame: &Frame<'_>,
    edit: &mut EditUi,
    clicked: &mut Vec<Clicked>,
) {
    let typing = response.has_focus() && frame.session.text_cursor().is_some();
    ui.ctx()
        .data_mut(|d| d.insert_temp(super::typing_flag(), typing));
    let Some(cursor) = frame.session.text_cursor() else {
        edit.preedit.clear();
        return;
    };
    if !typing {
        return;
    }
    // Tab and the arrows belong to the text, not to moving focus between widgets.
    ui.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            response.id,
            EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: false,
            },
        );
    });
    let events = ui.input(|input| input.events.clone());
    for action in keymap::actions(&events) {
        match action {
            Action::Command(command) => {
                edit.h_pos = None;
                clicked.push(command);
            }
            Action::Preedit(text) => edit.preedit = text,
            Action::Edge { end, extend } => {
                edit.h_pos = None;
                if let Some(raw) = caret_raw(frame.flow, cursor.head) {
                    clicked.push(set_caret(raw.row_edge(cursor.head, end), extend));
                }
            }
            Action::Vertical { down, extend } => {
                let Some(raw) = caret_raw(frame.flow, cursor.head) else {
                    continue;
                };
                let x = edit.h_pos.unwrap_or_else(|| raw.caret_x(cursor.head));
                match raw.vertical(cursor.head, down, Some(x)) {
                    Some((at, h)) => {
                        edit.h_pos = Some(h);
                        clicked.push(set_caret(at, extend));
                    }
                    None => {
                        edit.h_pos = None;
                        if let Some(at) = neighbor_block(frame, raw, down) {
                            clicked.push(set_caret(at, extend));
                        }
                    }
                }
            }
        }
    }
    // Tell the OS where the text is, so the IME places its candidate window there.
    if let Some(caret) = frame.flow.caret {
        let rect = screen_rect(frame.map, caret);
        ui.ctx().output_mut(|output| {
            output.ime = Some(egui::output::IMEOutput {
                purpose: egui::IMEPurpose::Normal,
                rect: response.rect,
                cursor_rect: rect,
                should_interrupt_composition: false,
            });
        });
    }
}

/// Paint the caret.
pub(super) fn paint_caret(
    painter: &egui::Painter,
    map: PageToScreen,
    flow: &Flow,
    tokens: &ThemeTokens,
) {
    if let Some(caret) = flow.caret {
        let rect = screen_rect(map, caret);
        painter.line_segment(
            [rect.center_top(), rect.center_bottom()],
            Stroke::new(1.5, tokens.text),
        );
    }
}
