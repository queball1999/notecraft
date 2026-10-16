//! The canvas widget: scroll bars and the mouse wheel scroll the page; middle-drag and
//! Space+drag pan it; Ctrl+wheel and pinch zoom about the pointer.

use egui::containers::scroll_area::{DragScroll, ScrollSource};
use egui::{CursorIcon, PointerButton, Rect, ScrollArea, Sense, UiBuilder};
use notecraft_core::Vec2;
use notecraft_engine::cmd::{ids, Map, Value};
use notecraft_engine::{mock, Session};

use super::{paper, title, CanvasUi, PageToScreen};
use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// The accessible name of the canvas, which UI tests look up.
pub const LABEL: &str = "Canvas";

/// Draw the canvas in `rect`. Zoom gestures are pushed onto `clicked` as `view.zoom`
/// commands. Returns the canvas width in points, for `view.zoom_page_width`.
pub fn show(
    ui: &mut egui::Ui,
    rect: Rect,
    session: &Session,
    state: &mut CanvasUi,
    tokens: &ThemeTokens,
    clicked: &mut Vec<Clicked>,
) -> f32 {
    state.follow_session(session.view().zoom_percent);
    zoom_gestures(ui, rect, state, clicked);

    let zoom = state.viewport.zoom();
    let origin = state.viewport.origin();
    let view_size = Vec2::new(rect.width(), rect.height()) / zoom;
    let style = session.page_style();
    let extent = content_extent(origin, view_size, style.paper_size.size()) * zoom;

    let mut area = ScrollArea::both()
        .id_salt("canvas")
        .auto_shrink(false)
        .content_margin(0.0)
        .scroll_source(ScrollSource {
            scroll_bar: true,
            drag: DragScroll::Never,
            mouse_wheel: true,
        });
    if std::mem::take(&mut state.force_offset) {
        let offset = origin * zoom;
        area = area.scroll_offset(egui::vec2(offset.x, offset.y));
    }
    let output = ui
        .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            area.show_viewport(ui, |ui, visible| {
                let content =
                    Rect::from_min_size(ui.max_rect().min, egui::vec2(extent.x, extent.y));
                let response = ui.allocate_rect(content, Sense::click_and_drag());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Other, true, LABEL)
                });
                let map = PageToScreen {
                    origin: content.min,
                    zoom,
                };
                let screen = visible.translate(content.min.to_vec2());
                // Screen readers get the visible part of the page, not the whole scrollable
                // content, so their highlight and their click land on screen.
                let shown = screen.intersect(ui.clip_rect());
                ui.ctx().accesskit_node_builder(response.id, |node| {
                    node.set_bounds(egui::accesskit::Rect {
                        x0: shown.min.x.into(),
                        y0: shown.min.y.into(),
                        x1: shown.max.x.into(),
                        y1: shown.max.y.into(),
                    });
                });
                paper::draw(ui.painter(), screen, map, style, tokens);
                if let Some(page) = session.current_page() {
                    title::draw(ui.painter(), map, &page.title, mock::CREATED, tokens);
                }
                if response.has_focus() {
                    // Keyboard focus: a ring just inside the visible part of the page.
                    let stroke = egui::Stroke::new(2.0, tokens.accent);
                    ui.painter().rect_stroke(
                        screen.shrink(1.0),
                        0.0,
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                }
                response
            })
        })
        .inner;
    name_scroll_bars(ui.ctx(), output.id);

    // The scroll area owns the scroll position unless this frame pans by dragging.
    let offset = output.state.offset;
    state
        .viewport
        .set_origin(Vec2::new(offset.x, offset.y) / zoom);
    let response = output.inner;
    let space = space_held(ui);
    let panning = response.dragged_by(PointerButton::Middle)
        || (space && response.dragged_by(PointerButton::Primary));
    if panning {
        let delta = response.drag_delta();
        state.viewport.pan_by(Vec2::new(delta.x, delta.y));
        state.force_offset = true;
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    } else if space && response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Grab);
    }
    output.inner_rect.width()
}

/// Ctrl+wheel and pinch over the canvas zoom about the pointer.
fn zoom_gestures(ui: &egui::Ui, rect: Rect, state: &mut CanvasUi, clicked: &mut Vec<Clicked>) {
    let (factor, pointer) = ui.input(|input| (input.zoom_delta(), input.pointer.hover_pos()));
    let Some(pointer) = pointer.filter(|_| factor != 1.0 && ui.rect_contains_pointer(rect)) else {
        return;
    };
    let anchor = pointer - rect.min;
    if let Some(percent) = state.zoom_gesture(Vec2::new(anchor.x, anchor.y), factor) {
        let mut args = Map::new();
        args.insert("percent".to_owned(), Value::from(percent));
        clicked.push(Clicked {
            command: ids::VIEW_ZOOM,
            args: Value::Object(args),
        });
    }
}

/// Name the scroll bars, which egui leaves unlabeled, so a screen reader says which one it is.
/// egui gives the bar for axis `d` (0 horizontal, 1 vertical) the id `area.with(d)`; a bar not
/// shown this frame is skipped so no empty node is made for it.
fn name_scroll_bars(ctx: &egui::Context, area: egui::Id) {
    for (axis, name) in [(0_usize, "Horizontal"), (1, "Vertical")] {
        let id = area.with(axis);
        if ctx.read_response(id).is_some() {
            ctx.accesskit_node_builder(id, |node| node.set_label(name));
        }
    }
}

/// Space is down and no text field has focus (Space+drag pans).
fn space_held(ui: &egui::Ui) -> bool {
    let focused = ui.ctx().memory(|memory| memory.focused().is_some());
    !focused && ui.input(|input| input.key_down(egui::Key::Space))
}

/// How far the scroll bars reach, in page units: the paper, if it has a fixed size, and
/// always one screen beyond what is shown, so the page can grow right and down as in
/// OneNote.
fn content_extent(origin: Vec2, view: Vec2, paper: Option<Vec2>) -> Vec2 {
    let paper = paper.unwrap_or_default();
    Vec2::new(
        paper.x.max(origin.x + 2.0 * view.x),
        paper.y.max(origin.y + 2.0 * view.y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_reaches_one_screen_past_the_view_or_the_paper() {
        let view = Vec2::new(800.0, 600.0);
        assert_eq!(
            content_extent(Vec2::ZERO, view, None),
            Vec2::new(1600.0, 1200.0)
        );
        assert_eq!(
            content_extent(Vec2::new(100.0, 0.0), view, Some(Vec2::new(816.0, 1344.0))),
            Vec2::new(1700.0, 1344.0)
        );
    }
}
