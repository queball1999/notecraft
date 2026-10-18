//! The canvas widget: scroll bars and the mouse wheel scroll the page; middle-drag and
//! Space+drag pan it; Ctrl+wheel and pinch zoom about the pointer.

use egui::containers::scroll_area::{DragScroll, ScrollSource};
use egui::{CursorIcon, PointerButton, Rect, ScrollArea, Sense, UiBuilder};
use notecraft_core::Vec2;
use notecraft_engine::cmd::{ids, Map, Value};
use notecraft_engine::{mock, DrawTool, Session};

use super::{editing, flow, ink, paper, picture_ui, pictures, title, CanvasUi, PageToScreen};
use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// Top of the page notice, in page units: under the title area, above the text.
const NOTICE_TOP: f32 = 96.0;

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
    // Text with no color of its own is Automatic: it reads on whatever color the page is.
    let page_tokens = paper::page_tokens(style, tokens);
    let tokens = &page_tokens;
    // The page text, laid out in flow layout (read-only). One seam: the page source text.
    let mut cache = std::mem::take(&mut state.pictures);
    cache.begin_frame(session.selection().page.as_deref());
    let page = session.current_page_source().map(|source| {
        let parsed = state.document(source);
        let geometry = state.edit.geometry(session, source.page_id, &parsed.runs);
        let cursor = session.text_cursor();
        let preedit = if cursor.is_some() {
            state.edit.preedit.clone()
        } else {
            String::new()
        };
        let input = flow::Input {
            text: source.visible_text(),
            doc: &parsed.doc,
            runs: &parsed.runs,
            geometry: &geometry,
            cursor,
            preedit: &preedit,
            objects: session.page_objects(source.page_id),
        };
        let mut loader = pictures::PictureLoader {
            cache: &mut cache,
            ctx: ui.ctx(),
            session,
        };
        let laid = flow::layout(ui.painter(), &input, zoom, tokens, &mut loader);
        (source.page_id, source.visible_text(), parsed, laid)
    });
    state.pictures = cache;
    let flow = page.as_ref().map(|(_, _, _, laid)| laid);
    let extent = page_extent(
        content_extent(origin, view_size, style.paper_size.size()),
        flow.map(|flow| Vec2::new(flow.width, flow.height)),
    );
    let extent = extent * zoom;
    let notice = session.current_page_notice().map(|n| n.text.clone());
    let reader_text = match (&notice, &flow) {
        (Some(notice), Some(_)) => Some(format!("{notice}\n{}", state.reader_text())),
        (Some(notice), None) => Some(notice.clone()),
        (None, Some(_)) => Some(state.reader_text().to_owned()),
        (None, None) => None,
    };

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
                state.last_map = Some((map, rect));
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
                    if let Some(text) = &reader_text {
                        node.set_value(text.clone());
                    }
                });
                let paper_color = paper::paper_color(style, tokens);
                let frame = capture_ink(
                    ui,
                    &response,
                    session,
                    state,
                    map,
                    shown,
                    clicked,
                    PageColors {
                        flow,
                        text: tokens.text,
                        paper: paper_color,
                    },
                );
                paper::draw(ui.painter(), screen, map, style, tokens);
                // Highlighter ink multiplies over the page content when the app installed the
                // paint callback (drawn after the text, below). Without it, the ink is an
                // opaque pre-blend above the paper and under the text.
                let multiply = ink::InkUi::can_multiply(
                    ui.ctx(),
                    session.current_ink(),
                    ink::visible_box(map, shown),
                );
                if !multiply {
                    paint_highlights(ui, session, state, map, shown, &frame, Some(paper_color));
                }
                if let Some(page) = session.current_page() {
                    let editable = session.page_is_editable(&page.id)
                        && session.ink().tool() == DrawTool::Type;
                    title::show(
                        ui,
                        map,
                        (&page.id, &page.title),
                        editable,
                        &mut state.title,
                        mock::CREATED,
                        tokens,
                        clicked,
                    );
                }
                if let Some((page_id, text, parsed, flow)) = &page {
                    flow::paint(ui.painter(), map, flow, tokens);
                    let type_tool = session.ink().tool() == DrawTool::Type;
                    picture_ui::files(
                        ui,
                        map,
                        flow,
                        &mut state.pictures,
                        type_tool && session.page_is_editable(page_id),
                        clicked,
                    );
                    let on_picture = type_tool
                        && picture_ui::show(
                            ui,
                            map,
                            flow,
                            &mut state.pictures,
                            session.page_is_editable(page_id),
                            tokens,
                            clicked,
                        );
                    if response.clicked() && !on_picture {
                        state.pictures.selected = None;
                    }
                    if type_tool && !on_picture && (response.clicked() || response.double_clicked())
                    {
                        if let Some(pos) = response.interact_pointer_pos() {
                            state.remember_click(page_id, map.page(pos), flow);
                        }
                    }
                    let frame = editing::Frame {
                        session,
                        parsed,
                        text,
                        page: page_id,
                        flow,
                        map,
                        tokens,
                        panning: space_held(ui),
                    };
                    editing::interact(ui, &response, &frame, &mut state.edit, clicked);
                    if response.has_focus() {
                        editing::paint_caret(ui.painter(), map, flow, tokens);
                    }
                }
                if let Some(text) = &notice {
                    // A read-only or error line under the title (the page could not be read
                    // cleanly).
                    ui.painter().text(
                        map.pos(Vec2::new(flow::LEFT, NOTICE_TOP)),
                        egui::Align2::LEFT_TOP,
                        text,
                        egui::FontId::proportional(flow::BODY_SIZE * zoom),
                        tokens.accent,
                    );
                }
                if multiply {
                    paint_highlights(ui, session, state, map, shown, &frame, None);
                }
                paint_pens(ui, &response, session, state, map, shown, &frame, tokens);
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

/// What one frame of ink capture found, for the two paint passes.
struct InkFrame {
    tool: DrawTool,
    /// The pen or highlighter style while one is drawing.
    style: Option<notecraft_engine::PenStyle>,
    /// The stroke being drawn, if any.
    preview: Option<Vec<notecraft_core::InkPoint>>,
}

/// Capture strokes and eraser drags. The tools work only while they are the active tool; Space
/// held means the user is panning, so none of them starts.
fn capture_ink(
    ui: &egui::Ui,
    response: &egui::Response,
    session: &Session,
    state: &mut CanvasUi,
    map: PageToScreen,
    shown: Rect,
    clicked: &mut Vec<Clicked>,
    colors: PageColors<'_>,
) -> InkFrame {
    let tool = session.ink().tool();
    let style = match tool {
        DrawTool::Pen => Some(session.ink().pen()),
        DrawTool::Highlighter => Some(session.ink().highlighter()),
        DrawTool::Type | DrawTool::Eraser => None,
    };
    let capture_tool = match (tool, style) {
        _ if space_held(ui) => ink::Tool::Off,
        (DrawTool::Eraser, _) => ink::Tool::Erase(
            session.ink().eraser().id(),
            session.ink().eraser_size().id(),
        ),
        (_, Some(style)) => ink::Tool::Draw(style),
        _ => ink::Tool::Off,
    };
    let layer_id = ui.layer_id();
    let ctx = ui.ctx().clone();
    if crate::ribbon::eyedropper_active(&ctx) {
        // The Eyedropper takes the next click on the page: ink under it gives its color.
        // Escape or a click elsewhere ends it.
        ctx.set_cursor_icon(CursorIcon::Crosshair);
        let escape = ctx.input(|input| input.key_pressed(egui::Key::Escape));
        if escape {
            crate::ribbon::set_eyedropper(&ctx, false);
        } else if response.clicked() {
            if let Some(at) = response.interact_pointer_pos() {
                let page = map.page(at);
                clicked.push(eyedropper_click(page, map.zoom, &colors));
                crate::ribbon::set_eyedropper(&ctx, false);
            }
        }
        return InkFrame {
            tool,
            style: None,
            preview: None,
        };
    }
    let can_start = |at: egui::Pos2| shown.contains(at) && ctx.layer_id_at(at) == Some(layer_id);
    let capture = state.ink.capture(ui, map, capture_tool, can_start, clicked);
    if tool != DrawTool::Type && response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
    }
    InkFrame {
        tool,
        style,
        preview: capture.preview,
    }
}

/// What the Eyedropper can read besides ink: the laid out page text and the paper color.
struct PageColors<'a> {
    flow: Option<&'a flow::Flow>,
    /// The color of text with no color of its own.
    text: egui::Color32,
    paper: egui::Color32,
}

/// The `draw.eyedropper` command for a click at page position `page`. The reach is a few
/// screen pixels, so thin ink is easy to hit at any zoom.
fn eyedropper_click(page: Vec2, zoom: f32, colors: &PageColors<'_>) -> Clicked {
    let round = |v: f32| (f64::from(v) * 100.0).round() / 100.0;
    let mut args = notecraft_engine::cmd::Map::new();
    args.insert("x".to_owned(), round(page.x).into());
    args.insert("y".to_owned(), round(page.y).into());
    args.insert("radius".to_owned(), round(4.0 / zoom.max(0.01)).into());
    // With no ink there, the page gives its color: the text under the point, else the paper.
    let under = colors
        .flow
        .and_then(|flow| flow.text_color_at(page, colors.text))
        .unwrap_or(colors.paper);
    let text = format!("#{:02X}{:02X}{:02X}", under.r(), under.g(), under.b());
    args.insert("page_color".to_owned(), text.into());
    Clicked {
        command: notecraft_engine::cmd::ids::DRAW_EYEDROPPER,
        args: notecraft_engine::cmd::Value::Object(args),
    }
}

/// The highlighter pass: under the page text.
fn paint_highlights(
    ui: &egui::Ui,
    session: &Session,
    state: &mut CanvasUi,
    map: PageToScreen,
    shown: Rect,
    frame: &InkFrame,
    paper: Option<egui::Color32>,
) {
    let preview = frame
        .preview
        .as_deref()
        .zip(frame.style)
        .filter(|_| frame.tool == DrawTool::Highlighter);
    let visible = ink::visible_box(map, shown);
    let page = session.selection().page.as_deref();
    match paper {
        Some(paper) => state.ink.paint_highlights(
            ui.painter(),
            map,
            visible,
            page,
            session.current_ink(),
            preview,
            paper,
        ),
        None => state.ink.paint_highlights_multiply(
            ui.painter(),
            map,
            visible,
            page,
            session.current_ink(),
            preview,
        ),
    }
}

/// The pen pass: over the page text, and the eraser's circle at the pointer.
fn paint_pens(
    ui: &egui::Ui,
    response: &egui::Response,
    session: &Session,
    state: &mut CanvasUi,
    map: PageToScreen,
    shown: Rect,
    frame: &InkFrame,
    tokens: &ThemeTokens,
) {
    let preview = frame
        .preview
        .as_deref()
        .zip(frame.style)
        .filter(|_| frame.tool == DrawTool::Pen);
    state.ink.paint_pens(
        ui.painter(),
        map,
        ink::visible_box(map, shown),
        session.selection().page.as_deref(),
        session.current_ink(),
        preview,
    );
    if frame.tool == DrawTool::Eraser && response.hovered() && !space_held(ui) {
        let hover = ui.input(|input| input.pointer.hover_pos());
        if let Some(at) = hover.filter(|at| shown.contains(*at)) {
            let radius = ink::eraser_reach_px(
                session.ink().eraser().id(),
                session.ink().eraser_size().id(),
            );
            ui.painter()
                .circle_stroke(at, radius, egui::Stroke::new(1.0, tokens.text));
        }
    }
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

/// Space is down and no text field has focus (Space+drag pans). The canvas itself may have
/// focus: a Space typed in the text still types, and a drag with Space down pans.
fn space_held(ui: &egui::Ui) -> bool {
    !ui.ctx().text_edit_focused() && ui.input(|input| input.key_down(egui::Key::Space))
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

/// The scroll extent once the content is known: it grows to the right and bottom edge of
/// the content, even past the paper (CV-06).
fn page_extent(base: Vec2, content: Option<Vec2>) -> Vec2 {
    match content {
        Some(c) => Vec2::new(base.x.max(c.x), base.y.max(c.y)),
        None => base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_grows_past_the_paper_with_a_container_placed_beyond_it() {
        let view = Vec2::new(800.0, 600.0);
        let paper = Some(Vec2::new(816.0, 1344.0));
        let base = content_extent(Vec2::ZERO, view, paper);
        assert_eq!(base, Vec2::new(1600.0, 1344.0));
        let grown = page_extent(base, Some(Vec2::new(2100.0, 3000.0)));
        assert_eq!(grown, Vec2::new(2100.0, 3000.0));
        // Content inside the paper changes nothing.
        assert_eq!(page_extent(base, Some(Vec2::new(300.0, 400.0))), base);
        assert_eq!(page_extent(base, None), base);
    }

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
