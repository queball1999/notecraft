//! The find on page bar (TX-11), under the section tabs. It draws the query box, the match
//! count, and the Previous, Next, and Close buttons; everything it does is a command
//! (`edit.find`, `edit.find.set_query`, `edit.find.next`, `edit.find.previous`,
//! `edit.find.close`). The highlights and the scroll to the current match are in the canvas.
//!
//! Keys while the box has focus: Enter and F3 go to the next match, Shift+Enter and Shift+F3
//! to the previous one, Ctrl+F selects the query again, and Esc closes the bar. (With the
//! focus elsewhere, F3 and Shift+F3 are ordinary shortcuts.)

use egui::{Align, Key, Layout, Modifiers, Rect, RichText, Ui};
use notecraft_engine::cmd::{ids, CommandId, Map, Value};
use notecraft_engine::Session;

use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// Height of the bar when it shows.
pub const HEIGHT: f32 = 36.0;

/// The accessible name of the query box, which UI tests look up.
pub const LABEL: &str = "Find on page";

fn field_id() -> egui::Id {
    egui::Id::new("notecraft.find.query")
}

/// The room the bar takes: its height, or 0 while it is closed.
pub fn height(session: &Session) -> f32 {
    if session.find().open {
        HEIGHT
    } else {
        0.0
    }
}

/// How many matches the query has on the page shown. Kept for as long as the page text and
/// the query stay the same, so the bar does not parse the page every frame.
fn match_count(ui: &Ui, session: &Session) -> usize {
    let state = session.find();
    let Some(source) = session.current_page_source() else {
        return 0;
    };
    if state.query.is_empty() {
        return 0;
    }
    let key = (
        source.page_id.to_owned(),
        source.revision,
        state.query.clone(),
    );
    let id = egui::Id::new("notecraft.find.count");
    let cached = ui.data(|d| d.get_temp::<((String, u64, String), usize)>(id));
    match cached {
        Some((k, count)) if k == key => count,
        _ => {
            let count = session.find_matches().len();
            ui.data_mut(|d| d.insert_temp(id, (key, count)));
            count
        }
    }
}

/// The text beside the box: which match, or that there is none.
pub fn count_text(query_empty: bool, count: usize, current: Option<usize>) -> String {
    match (query_empty, current) {
        (true, _) => String::new(),
        (false, Some(i)) => format!("{} of {count}", i + 1),
        (false, None) => "No matches".to_owned(),
    }
}

fn push(out: &mut Vec<Clicked>, command: CommandId, args: Value) {
    out.push(Clicked { command, args });
}

/// Draw the bar in `rect` (as tall as [`height`]).
pub fn show(
    ui: &mut Ui,
    rect: Rect,
    session: &Session,
    tokens: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    let state = session.find();
    if !state.open {
        return;
    }
    let count = match_count(ui, session);
    let current = state.current_of(count);
    ui.painter().rect_filled(rect, 0.0, tokens.ribbon_bg);
    ui.painter().hline(
        rect.x_range(),
        rect.max.y - 0.5,
        egui::Stroke::new(1.0, tokens.border),
    );
    let id = field_id();
    let mut text = state.query.clone();
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add_space(10.0);
            ui.label(RichText::new("Find").size(13.0).color(tokens.text));
            // Keys are taken before the box sees them: it would give up the keyboard on Enter,
            // and egui drops the focus on Esc before this runs, so the box counts as focused
            // if it was in the last frame.
            let had_focus = ui.data(|d| d.get_temp::<bool>(id.with("had")));
            if had_focus == Some(true) || ui.memory(|m| m.has_focus(id)) {
                keys(ui, out);
            }
            let field = egui::TextEdit::singleline(&mut text)
                .id(id)
                .hint_text("Search this page")
                .desired_width(260.0);
            let response = ui.add(field);
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, LABEL));
            // The command came with a request for the keyboard: take it and select the query.
            let wanted = ui.data(|d| d.get_temp::<u64>(id.with("focus")));
            if wanted != Some(state.focus) {
                ui.data_mut(|d| d.insert_temp(id.with("focus"), state.focus));
                response.request_focus();
                select_all(ui.ctx(), id, text.chars().count());
            }
            let focused = response.has_focus();
            ui.data_mut(|d| d.insert_temp(id.with("had"), focused));
            if text != state.query {
                let mut args = Map::new();
                args.insert("query".to_owned(), Value::from(text.as_str()));
                push(out, ids::EDIT_FIND_SET_QUERY, Value::Object(args));
            }
            ui.label(
                RichText::new(count_text(state.query.is_empty(), count, current))
                    .size(13.0)
                    .color(tokens.text_muted),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(10.0);
                // Right to left, so the last entry draws leftmost.
                if ui.button("Close").clicked() {
                    push(out, ids::EDIT_FIND_CLOSE, Value::Null);
                }
                if ui
                    .add_enabled(count > 0, egui::Button::new("Next"))
                    .clicked()
                {
                    push(out, ids::EDIT_FIND_NEXT, Value::Null);
                }
                if ui
                    .add_enabled(count > 0, egui::Button::new("Previous"))
                    .clicked()
                {
                    push(out, ids::EDIT_FIND_PREVIOUS, Value::Null);
                }
            });
        });
    });
}

/// The keys the box handles itself while it has focus.
fn keys(ui: &mut Ui, out: &mut Vec<Clicked>) {
    // Chords with Shift first: egui's key matching ignores an extra Shift.
    let pressed = |ui: &mut Ui, modifiers: Modifiers, key: Key| {
        ui.input_mut(|input| input.consume_key(modifiers, key))
    };
    if pressed(ui, Modifiers::SHIFT, Key::Enter) || pressed(ui, Modifiers::SHIFT, Key::F3) {
        push(out, ids::EDIT_FIND_PREVIOUS, Value::Null);
    } else if pressed(ui, Modifiers::NONE, Key::Enter) || pressed(ui, Modifiers::NONE, Key::F3) {
        push(out, ids::EDIT_FIND_NEXT, Value::Null);
    }
    if pressed(ui, Modifiers::COMMAND, Key::F) {
        push(out, ids::EDIT_FIND, Value::Null);
    }
    if pressed(ui, Modifiers::NONE, Key::Escape) {
        push(out, ids::EDIT_FIND_CLOSE, Value::Null);
    }
}

/// Select the whole text of the box `id`, so typing replaces the last query.
fn select_all(ctx: &egui::Context, id: egui::Id, chars: usize) {
    use egui::text::{CCursor, CCursorRange};
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::two(
        CCursor::new(0),
        CCursor::new(chars),
    )));
    state.store(ctx, id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_count_text_says_which_match_or_none() {
        assert_eq!(count_text(true, 0, None), "");
        assert_eq!(count_text(false, 0, None), "No matches");
        assert_eq!(count_text(false, 5, Some(0)), "1 of 5");
        assert_eq!(count_text(false, 5, Some(4)), "5 of 5");
        assert!(count_text(false, 1, Some(0)).is_ascii());
    }

    #[test]
    fn a_closed_bar_takes_no_room() {
        assert_eq!(height(&Session::new()), 0.0);
    }
}
