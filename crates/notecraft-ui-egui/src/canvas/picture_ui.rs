//! Selecting a picture and resizing it with handles, as in OneNote: a click selects a picture
//! and shows a handle at each corner; dragging a corner changes the width and the picture keeps
//! its proportions. Releasing the handle sends one `picture.resize` command, so a whole drag is
//! one undo step. While the drag lasts the new size is only a preview in the layout.
//!
//! Dragging a picture (anywhere on it) or a file attachment moves it: the object follows the
//! pointer in the layout, and releasing sends one `picture.move` or `attachment.move` command
//! with the page point, so a whole drag is one undo step. An object that flowed inside its
//! container leaves it when dropped (docs/markdown.md section 3.4).

use egui::{CursorIcon, Rect, Sense, Stroke};
use notecraft_core::Vec2;
use notecraft_engine::cmd::{ids, CommandId, Map, Value};
use notecraft_markdown::image::MIN_WIDTH;
use notecraft_model::ObjectKind;

use super::flow::{Flow, PictureHit};
use super::pictures::{MoveDrag, PictureCache, ResizeDrag};
use super::PageToScreen;
use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// Side of a handle, in screen points.
const HANDLE: f32 = 9.0;

/// A corner of the picture. `sx` and `sy` are -1 for the left and top sides, 1 for the others.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Corner {
    sx: f32,
    sy: f32,
}

const CORNERS: [Corner; 4] = [
    Corner { sx: -1.0, sy: -1.0 },
    Corner { sx: 1.0, sy: -1.0 },
    Corner { sx: -1.0, sy: 1.0 },
    Corner { sx: 1.0, sy: 1.0 },
];

impl Corner {
    fn point(self, rect: Rect) -> egui::Pos2 {
        egui::pos2(
            if self.sx < 0.0 {
                rect.min.x
            } else {
                rect.max.x
            },
            if self.sy < 0.0 {
                rect.min.y
            } else {
                rect.max.y
            },
        )
    }

    fn cursor(self) -> CursorIcon {
        if self.sx * self.sy > 0.0 {
            CursorIcon::ResizeNwSe
        } else {
            CursorIcon::ResizeNeSw
        }
    }
}

/// The width after dragging `corner` by (`dx`, `dy`) page units from a picture `start` wide
/// whose width over height is `aspect`. The larger of the two movements decides, so the
/// corner follows the pointer along the diagonal and the proportions never change.
fn dragged_width(start: f32, (dx, dy): (f32, f32), aspect: f32, corner: Corner, max: f32) -> f32 {
    let by_x = corner.sx * dx;
    let by_y = corner.sy * dy * aspect;
    let delta = if by_x.abs() >= by_y.abs() { by_x } else { by_y };
    let width = start + delta;
    if width.is_finite() {
        width.clamp(f32::from(MIN_WIDTH), max.max(f32::from(MIN_WIDTH)))
    } else {
        start
    }
}

fn resize_command(link: &str, nth: usize, width: f32) -> Clicked {
    let mut args = Map::new();
    args.insert("src".to_owned(), Value::from(link));
    args.insert("width".to_owned(), Value::from(width.round() as i64));
    args.insert("nth".to_owned(), Value::from(nth));
    Clicked {
        command: ids::PICTURE_RESIZE,
        args: Value::Object(args),
    }
}

/// The command that puts an object at `pos` (page units), rounded to a tenth of a unit.
fn move_command(kind: ObjectKind, link: &str, nth: usize, pos: Vec2) -> Clicked {
    let tenth = |v: f32| f64::from((v * 10.0).round() / 10.0);
    let mut args = Map::new();
    args.insert("src".to_owned(), Value::from(link));
    args.insert("nth".to_owned(), Value::from(nth));
    args.insert("x".to_owned(), Value::from(tenth(pos.x)));
    args.insert("y".to_owned(), Value::from(tenth(pos.y)));
    let command: CommandId = match kind {
        ObjectKind::Picture => ids::PICTURE_MOVE,
        ObjectKind::File => ids::ATTACHMENT_MOVE,
    };
    Clicked {
        command,
        args: Value::Object(args),
    }
}

/// Follow a drag of one object: start it, move it with the pointer, and send the move command
/// when the pointer is released. Returns true if the object is being dragged.
fn drag_object(
    ui: &egui::Ui,
    response: &egui::Response,
    map: PageToScreen,
    cache: &mut PictureCache,
    object: (ObjectKind, &str, usize, Rect),
    clicked: &mut Vec<Clicked>,
) -> bool {
    let (kind, link, nth, rect) = object;
    let ours = |m: &MoveDrag| m.kind == kind && m.link == link && m.nth == nth;
    if response.drag_started() {
        cache.moving = Some(MoveDrag {
            kind,
            link: link.to_owned(),
            nth,
            start: Vec2::new(rect.min.x, rect.min.y),
            delta: Vec2::ZERO,
        });
    }
    let mut dragging = false;
    if response.dragged() {
        let delta = response.drag_delta() / map.zoom;
        if let Some(moving) = cache.moving.as_mut().filter(|m| ours(m)) {
            moving.delta = Vec2::new(moving.delta.x + delta.x, moving.delta.y + delta.y);
            dragging = true;
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            ui.ctx().request_repaint();
        }
    }
    if response.drag_stopped() {
        if let Some(moving) = cache.moving.take_if(|m| ours(m)) {
            let moved = moving.delta.x.abs().max(moving.delta.y.abs());
            if moved >= 1.0 {
                clicked.push(move_command(kind, link, nth, moving.pos()));
            }
        }
    }
    dragging
}

/// Click a picture to select it; draw the handles of the selected one and handle their drags.
/// Returns true if a picture was clicked this frame, so the canvas does not treat the click as
/// one on the page. `editable` is false for a read-only page: pictures can be selected but
/// show no handles.
pub(super) fn show(
    ui: &egui::Ui,
    map: PageToScreen,
    flow: &Flow,
    cache: &mut PictureCache,
    editable: bool,
    tokens: &ThemeTokens,
    clicked: &mut Vec<Clicked>,
) -> bool {
    let mut picture_clicked = false;
    if ui.input(|input| input.key_pressed(egui::Key::Escape)) && cache.drag.is_none() {
        cache.selected = None;
    }
    for hit in &flow.picture_hits {
        let rect = screen_rect(map, hit);
        if !rect.intersects(ui.clip_rect()) {
            continue;
        }
        let id = ui.id().with(("picture", &hit.link, hit.nth));
        let sense = if editable {
            Sense::click_and_drag()
        } else {
            Sense::click()
        };
        let response = ui.interact(rect, id, sense);
        let alt = hit.alt.clone();
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Picture {alt}"))
        });
        if editable {
            let object = (ObjectKind::Picture, hit.link.as_str(), hit.nth, hit.rect);
            if drag_object(ui, &response, map, cache, object, clicked) && hit.ready {
                cache.selected = Some((hit.link.clone(), hit.nth));
            }
            if response.drag_started() {
                picture_clicked = true;
            }
        }
        if response.clicked() && hit.ready {
            cache.selected = Some((hit.link.clone(), hit.nth));
            picture_clicked = true;
        }
    }
    let selected = cache.selected.clone();
    let hit = selected.and_then(|(link, nth)| {
        flow.picture_hits
            .iter()
            .find(|hit| hit.ready && hit.link == link && hit.nth == nth)
    });
    let Some(hit) = hit else {
        // The picture is gone (deleted, or the page changed): nothing stays selected.
        cache.selected = None;
        cache.drag = None;
        return picture_clicked;
    };
    let rect = screen_rect(map, hit);
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.5, tokens.accent),
        egui::StrokeKind::Outside,
    );
    if editable {
        handles(ui, map, hit, rect, cache, tokens, clicked);
    }
    picture_clicked
}

/// File attachments: a click opens the file (`attachment.open`); a drag moves it. Keyboard
/// users reach each attachment as a button and press Enter.
pub(super) fn files(
    ui: &egui::Ui,
    map: PageToScreen,
    flow: &Flow,
    cache: &mut PictureCache,
    movable: bool,
    clicked: &mut Vec<Clicked>,
) {
    for (index, hit) in flow.files.iter().enumerate() {
        let rect = Rect::from_min_max(
            map.pos(Vec2::new(hit.rect.min.x, hit.rect.min.y)),
            map.pos(Vec2::new(hit.rect.max.x, hit.rect.max.y)),
        );
        if !rect.intersects(ui.clip_rect()) {
            continue;
        }
        let id = ui.id().with(("attachment", index));
        let sense = if movable {
            Sense::click_and_drag()
        } else {
            Sense::click()
        };
        let response = ui.interact(rect, id, sense);
        let name = hit.name.clone();
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("Open {name}"))
        });
        if movable {
            let object = (ObjectKind::File, hit.link.as_str(), hit.nth, hit.rect);
            drag_object(ui, &response, map, cache, object, clicked);
        }
        if response.hovered() && !response.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if response.clicked() || response.double_clicked() {
            let mut args = Map::new();
            args.insert("src".to_owned(), Value::from(hit.link.clone()));
            clicked.push(Clicked {
                command: ids::ATTACHMENT_OPEN,
                args: Value::Object(args),
            });
        }
    }
}

fn screen_rect(map: PageToScreen, hit: &PictureHit) -> Rect {
    Rect::from_min_max(
        map.pos(Vec2::new(hit.rect.min.x, hit.rect.min.y)),
        map.pos(Vec2::new(hit.rect.max.x, hit.rect.max.y)),
    )
}

fn handles(
    ui: &egui::Ui,
    map: PageToScreen,
    hit: &PictureHit,
    rect: Rect,
    cache: &mut PictureCache,
    tokens: &ThemeTokens,
    clicked: &mut Vec<Clicked>,
) {
    let aspect = hit.rect.width() / hit.rect.height().max(1.0);
    for (index, corner) in CORNERS.iter().enumerate() {
        let center = corner.point(rect);
        let handle = Rect::from_center_size(center, egui::vec2(HANDLE, HANDLE));
        let id = ui.id().with(("picture_handle", &hit.link, hit.nth, index));
        let response = ui.interact(handle.expand(3.0), id, Sense::drag());
        let label = match (corner.sx < 0.0, corner.sy < 0.0) {
            (true, true) => "Resize picture from the top left",
            (false, true) => "Resize picture from the top right",
            (true, false) => "Resize picture from the bottom left",
            (false, false) => "Resize picture from the bottom right",
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label));
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(corner.cursor());
        }
        if response.drag_started() {
            cache.drag = Some(ResizeDrag {
                link: hit.link.clone(),
                nth: hit.nth,
                start: hit.rect.width(),
                dx: 0.0,
                dy: 0.0,
                width: hit.rect.width(),
            });
        }
        if response.dragged() {
            let delta = response.drag_delta() / map.zoom;
            if let Some(drag) = cache.drag.as_mut().filter(|d| d.link == hit.link) {
                drag.dx += delta.x;
                drag.dy += delta.y;
                drag.width = dragged_width(
                    drag.start,
                    (drag.dx, drag.dy),
                    aspect,
                    *corner,
                    hit.max_width,
                );
            }
            ui.ctx().request_repaint();
        }
        if response.drag_stopped() {
            if let Some(drag) = cache.drag.take() {
                if (drag.width - drag.start).abs() >= 0.5 {
                    clicked.push(resize_command(&drag.link, drag.nth, drag.width));
                }
            }
        }
        ui.painter().rect_filled(handle, 1.5, tokens.accent);
        ui.painter().rect_stroke(
            handle,
            1.5,
            Stroke::new(1.0, egui::Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOTTOM_RIGHT: Corner = Corner { sx: 1.0, sy: 1.0 };
    const TOP_LEFT: Corner = Corner { sx: -1.0, sy: -1.0 };

    #[test]
    fn a_corner_drag_keeps_the_proportions() {
        // 200 wide, twice as wide as high.
        let width = |corner, delta| dragged_width(200.0, delta, 2.0, corner, 700.0);
        assert_eq!(width(BOTTOM_RIGHT, (50.0, 0.0)), 250.0);
        // Dragging down 10 units grows the height by 10, so the width by 20.
        assert_eq!(width(BOTTOM_RIGHT, (0.0, 10.0)), 220.0);
        // The top left corner grows when dragged up and left, and shrinks the other way.
        assert_eq!(width(TOP_LEFT, (-40.0, 0.0)), 240.0);
        assert_eq!(width(TOP_LEFT, (30.0, 0.0)), 170.0);
    }

    #[test]
    fn the_width_stays_in_range() {
        assert_eq!(
            dragged_width(200.0, (-500.0, 0.0), 1.0, BOTTOM_RIGHT, 700.0),
            f32::from(MIN_WIDTH)
        );
        assert_eq!(
            dragged_width(200.0, (900.0, 0.0), 1.0, BOTTOM_RIGHT, 700.0),
            700.0
        );
        assert_eq!(
            dragged_width(200.0, (f32::NAN, 0.0), 1.0, BOTTOM_RIGHT, 700.0),
            200.0
        );
        // A column narrower than the smallest picture still gives a sane width.
        assert_eq!(
            dragged_width(20.0, (5.0, 0.0), 1.0, BOTTOM_RIGHT, 10.0),
            f32::from(MIN_WIDTH)
        );
    }

    #[test]
    fn the_command_carries_the_link_width_and_which_picture() {
        let c = resize_command("attachments/a.png", 1, 249.6);
        assert_eq!(c.command.as_str(), "picture.resize");
        assert_eq!(c.args["src"], "attachments/a.png");
        assert_eq!(c.args["width"], 250);
        assert_eq!(c.args["nth"], 1);
    }
}
