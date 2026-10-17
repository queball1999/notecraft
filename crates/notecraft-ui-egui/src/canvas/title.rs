//! The page title area at the top left of the page: the title, the line under it, and the
//! date and time the page was created. Sizes are in page units and scale with the zoom.

use egui::{Align2, FontId, Painter, Stroke};
use notecraft_core::Vec2;

use super::PageToScreen;
use crate::themes::ThemeTokens;

/// Top-left corner of the title, in page units (1/2 inch in, 3/8 inch down).
pub const TITLE_POS: Vec2 = Vec2::new(48.0, 36.0);
/// Title text size in page units (20 pt).
pub const TITLE_SIZE: f32 = 20.0 * 96.0 / 72.0;
/// Date and time text size in page units (10 pt).
pub const DATE_SIZE: f32 = 10.0 * 96.0 / 72.0;
/// The line under the title is at least this long, in page units (4 inches), so an empty
/// title still shows where to type.
pub const MIN_LINE: f32 = 4.0 * 96.0;
/// Gap between the date and the time, in page units.
const TIME_GAP: f32 = 24.0;

/// Where the parts of the title area go on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    title: egui::Pos2,
    /// Height of the line under the title.
    line_y: f32,
    date: egui::Pos2,
}

fn layout(map: PageToScreen) -> Layout {
    let title = map.pos(TITLE_POS);
    let line_y = map.pos(Vec2::new(0.0, TITLE_POS.y + TITLE_SIZE * 1.3)).y;
    let date = map.pos(Vec2::new(TITLE_POS.x, TITLE_POS.y + TITLE_SIZE * 1.3 + 8.0));
    Layout {
        title,
        line_y,
        date,
    }
}

/// The page title being edited in place on the canvas.
#[derive(Clone, Debug)]
pub(super) struct TitleEdit {
    page: String,
    original: String,
    text: String,
    /// The field has been given focus.
    focused: bool,
}

/// The accessible name of the title, which UI tests look up.
pub const TITLE_LABEL: &str = "Title of this page";

/// The title row on screen, as wide as the line under it (at least [`MIN_LINE`]).
fn title_rect(painter: &Painter, map: PageToScreen, title: &str) -> egui::Rect {
    let at = layout(map);
    let font = FontId::proportional(TITLE_SIZE * map.zoom);
    let galley = painter.layout_no_wrap(title.trim().to_owned(), font, egui::Color32::WHITE);
    let width = galley.size().x.max(MIN_LINE * map.zoom);
    egui::Rect::from_min_max(at.title, egui::pos2(at.title.x + width, at.line_y))
}

/// The title of page `page`, which a click (with the Type tool) edits in place, as in
/// OneNote. Enter or clicking away renames the page (`page.rename`); Esc puts the name
/// back. A page that cannot be edited keeps its title. Draws the whole title area.
#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    ui: &mut egui::Ui,
    map: PageToScreen,
    (page, title): (&str, &str),
    editable: bool,
    edit: &mut Option<TitleEdit>,
    created: (&str, &str),
    tokens: &ThemeTokens,
    clicked: &mut Vec<crate::ribbon::Clicked>,
) {
    if edit.as_ref().is_some_and(|e| e.page != page) || !editable {
        *edit = None;
    }
    let Some(editing) = edit.as_mut() else {
        draw(ui.painter(), map, title, created, tokens);
        if !editable {
            return;
        }
        let rect = title_rect(ui.painter(), map, title);
        let response = ui.interact(rect, ui.id().with("page-title"), egui::Sense::click());
        // Not a text field until clicked: a button that starts the edit.
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, TITLE_LABEL));
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if response.clicked() {
            *edit = Some(TitleEdit {
                page: page.to_owned(),
                original: title.to_owned(),
                text: title.to_owned(),
                focused: false,
            });
            ui.ctx().request_repaint();
        }
        return;
    };
    draw(ui.painter(), map, "", created, tokens);
    let rect = title_rect(ui.painter(), map, &editing.text);
    let field = egui::TextEdit::singleline(&mut editing.text)
        .font(FontId::proportional(TITLE_SIZE * map.zoom))
        .text_color(tokens.text)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .desired_width(rect.width());
    let response = ui.put(rect, field);
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, TITLE_LABEL));
    if !editing.focused {
        response.request_focus();
        editing.focused = true;
        return;
    }
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        *edit = None;
        return;
    }
    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    if !(enter || response.lost_focus()) {
        return;
    }
    let Some(done) = edit.take() else {
        return;
    };
    let name = done.text.trim();
    if name.is_empty() || name == done.original.trim() {
        return;
    }
    let mut args = notecraft_engine::cmd::Map::new();
    args.insert("id".to_owned(), done.page.as_str().into());
    args.insert("name".to_owned(), name.into());
    clicked.push(crate::ribbon::Clicked {
        command: notecraft_engine::cmd::ids::PAGE_RENAME,
        args: notecraft_engine::cmd::Value::Object(args),
    });
}

/// Draw the title area for a page titled `title`, created at `date` and `time`.
pub(super) fn draw(
    painter: &Painter,
    map: PageToScreen,
    title: &str,
    (date, time): (&str, &str),
    tokens: &ThemeTokens,
) {
    let at = layout(map);
    let font = |size: f32| FontId::proportional(size * map.zoom);
    let galley = painter.layout_no_wrap(title.trim().to_owned(), font(TITLE_SIZE), tokens.text);
    let width = galley.size().x.max(MIN_LINE * map.zoom);
    painter.galley(at.title, galley, tokens.text);
    painter.hline(
        at.title.x..=at.title.x + width,
        painter.round_to_pixel_center(at.line_y),
        Stroke::new(1.0, tokens.border),
    );
    let shown = painter.text(
        at.date,
        Align2::LEFT_TOP,
        date,
        font(DATE_SIZE),
        tokens.text_muted,
    );
    let time_at = egui::pos2(shown.max.x + TIME_GAP * map.zoom, at.date.y);
    painter.text(
        time_at,
        Align2::LEFT_TOP,
        time,
        font(DATE_SIZE),
        tokens.text_muted,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_area_scales_with_the_zoom() {
        let at_100 = layout(PageToScreen {
            origin: egui::Pos2::ZERO,
            zoom: 1.0,
        });
        assert_eq!(at_100.title, egui::pos2(48.0, 36.0));
        assert!(at_100.line_y > at_100.title.y + TITLE_SIZE);
        assert!(at_100.date.y > at_100.line_y);

        let at_200 = layout(PageToScreen {
            origin: egui::pos2(10.0, 20.0),
            zoom: 2.0,
        });
        assert_eq!(at_200.title, egui::pos2(106.0, 92.0));
        assert_eq!(at_200.line_y, 20.0 + at_100.line_y * 2.0);
        assert_eq!(at_200.date.x, at_200.title.x);
    }
}
