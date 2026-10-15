//! An editable ribbon box with a drop-down list, as OneNote's Font and Font Size boxes: the
//! field shows the current value and takes typing; the arrow opens the list. With `search`,
//! typing opens the list filtered to the matching entries (exact, then starting with the
//! text, then containing it). Up and Down move the highlight, Enter applies it (or the typed
//! text when nothing matches), and Esc puts back the current value. Only the rows in view are
//! built, so a list of every installed font stays cheap.

use egui::{
    Align2, Area, CornerRadius, FontId, Key, Modifiers, Order, Rect, Sense, Stroke, StrokeKind, Ui,
};

use super::Look;

/// Height of the box and of a list row.
pub(super) const HEIGHT: f32 = 22.0;
const ROW: f32 = 22.0;
/// Width of the arrow part.
const ARROW: f32 = 16.0;
/// Most of the list shown at once.
const LIST_HEIGHT: f32 = 300.0;

/// Between frames: the text being typed, whether the list is open, and its highlight.
#[derive(Clone, Default)]
struct State {
    text: Option<String>,
    open: bool,
    /// The list was opened by typing, so it shows matches rather than everything.
    filtered: bool,
    highlight: usize,
    /// Set when Up or Down moved the highlight, so the list scrolls to it.
    reveal: bool,
    /// The field had keyboard focus last frame.
    focused: bool,
}

/// Indices of `items` matching `query`, ignoring case: exact matches, then the names starting
/// with it, then those containing it, each in list order. A blank query matches everything.
pub(super) fn matches(query: &str, items: &[String]) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let (mut exact, mut prefix, mut inner) = (Vec::new(), Vec::new(), Vec::new());
    for (index, item) in items.iter().enumerate() {
        let name = item.to_lowercase();
        if name == query {
            exact.push(index);
        } else if name.starts_with(&query) {
            prefix.push(index);
        } else if name.contains(&query) {
            inner.push(index);
        }
    }
    exact.extend(prefix);
    exact.extend(inner);
    exact
}

/// Draw the box `width` points wide. `label` is its accessible name. Returns the entry
/// picked or the text entered this frame.
pub(super) fn show(
    ui: &mut Ui,
    label: &str,
    width: f32,
    current: &str,
    items: &[String],
    search: bool,
    look: Look<'_>,
) -> Option<String> {
    let t = look.tokens;
    let id = ui.make_persistent_id(("ribbon_combo", label));
    let edit_id = id.with("edit");
    let mut state: State = ui.data(|data| data.get_temp(id)).unwrap_or_default();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, HEIGHT), Sense::hover());
    let field = Rect::from_min_max(rect.min, egui::pos2(rect.max.x - ARROW, rect.max.y));
    let arrow = Rect::from_min_max(egui::pos2(field.max.x, rect.min.y), rect.max);

    let focused = ui.memory(|memory| memory.has_focus(edit_id));
    // Up and Down move the highlight; take them before the field moves its cursor.
    let mut step = 0_i64;
    if state.open && focused {
        ui.input_mut(|input| {
            if input.consume_key(Modifiers::NONE, Key::ArrowDown) {
                step += 1;
            }
            if input.consume_key(Modifiers::NONE, Key::ArrowUp) {
                step -= 1;
            }
        });
    }
    // egui drops focus on Esc before this runs, so last frame's focus counts too.
    let escape = (focused || state.focused || state.open)
        && ui.input(|input| input.key_pressed(Key::Escape));

    let mut text = state.text.clone().unwrap_or_else(|| current.to_owned());
    let edit = egui::TextEdit::singleline(&mut text)
        .id(edit_id)
        .font(FontId::proportional(12.0))
        .margin(egui::vec2(4.0, 3.0))
        .frame(egui::Frame::NONE)
        .desired_width(field.width() - 8.0);
    let response = ui.put(field, edit);
    ui.ctx()
        .accesskit_node_builder(edit_id, |node| node.set_label(label));
    let enter = response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter));

    // One outline around field and arrow; the arrow has its own hover fill.
    let arrow_response = ui.interact(arrow, id.with("arrow"), Sense::click());
    arrow_response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, format!("{label} list"))
    });
    let painter = ui.painter();
    if arrow_response.hovered() {
        painter.rect_filled(arrow, CornerRadius::same(t.radius), t.hover);
    }
    let outline = if focused || state.open {
        t.accent
    } else {
        t.border
    };
    painter.rect_stroke(
        rect,
        CornerRadius::same(t.radius),
        Stroke::new(1.0, outline),
        StrokeKind::Inside,
    );
    painter.vline(
        arrow.min.x,
        arrow.y_range().shrink(3.0),
        Stroke::new(1.0, t.border),
    );
    let c = arrow.center();
    painter.line(
        vec![
            c + egui::vec2(-3.0, -1.5),
            c + egui::vec2(0.0, 1.5),
            c + egui::vec2(3.0, -1.5),
        ],
        Stroke::new(1.0, t.ribbon_text),
    );

    if response.changed() {
        state.text = Some(text.clone());
        if search {
            state.open = true;
            state.filtered = true;
            state.highlight = 0;
        }
    }
    if arrow_response.clicked() {
        state.open = !state.open;
        state.filtered = false;
        state.highlight = items.iter().position(|item| item == current).unwrap_or(0);
        state.reveal = true;
        if state.open {
            ui.memory_mut(|memory| memory.request_focus(edit_id));
        }
    }

    let shown: Vec<usize> = if state.filtered {
        matches(&text, items)
    } else {
        (0..items.len()).collect()
    };
    let last = shown.len().saturating_sub(1);
    if step != 0 {
        state.highlight = if step > 0 {
            state.highlight.saturating_add(1).min(last)
        } else {
            state.highlight.saturating_sub(1)
        };
        state.reveal = true;
    }

    let mut out = None;
    if escape {
        state = State::default();
        ui.memory_mut(|memory| memory.surrender_focus(edit_id));
    } else if enter {
        let typed = text.trim().to_owned();
        let highlighted = shown.get(state.highlight).and_then(|&i| items.get(i));
        out = match highlighted {
            Some(item) if state.open && !typed.is_empty() => Some(item.clone()),
            _ if !typed.is_empty() => Some(typed),
            _ => None,
        };
        state = State::default();
    }

    if state.open && !shown.is_empty() {
        let area = Area::new(id.with("list"))
            .order(Order::Foreground)
            .fixed_pos(rect.left_bottom())
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(width.max(120.0));
                    list(ui, &shown, items, current, &mut state, look)
                })
            });
        if let Some(picked) = area.inner.inner {
            out = Some(picked);
            state = State::default();
            ui.memory_mut(|memory| memory.surrender_focus(edit_id));
        } else {
            let list_rect = area.response.rect;
            let clicked_outside = ui.input(|input| {
                input.pointer.any_pressed()
                    && input
                        .pointer
                        .interact_pos()
                        .is_some_and(|pos| !list_rect.contains(pos) && !rect.contains(pos))
            });
            if clicked_outside {
                state = State::default();
            }
        }
    }
    if !focused && !state.open && out.is_none() {
        // Not being edited: show the current value next frame.
        state.text = None;
    }
    state.focused = ui.memory(|memory| memory.has_focus(edit_id));
    ui.data_mut(|data| data.insert_temp(id, state));
    out
}

/// The list's rows, only those in view built. Returns a clicked entry.
fn list(
    ui: &mut Ui,
    shown: &[usize],
    items: &[String],
    current: &str,
    state: &mut State,
    look: Look<'_>,
) -> Option<String> {
    let t = look.tokens;
    let mut picked = None;
    // Rows touch, as in OneNote; show_rows counts the spacing too.
    ui.spacing_mut().item_spacing.y = 0.0;
    // Sized from the row count, not from last frame's content, so a list that grows (typing,
    // then the arrow) shows every row that fits.
    let height = (shown.len() as f32 * ROW).min(LIST_HEIGHT);
    let mut scroll = egui::ScrollArea::vertical()
        .min_scrolled_height(height)
        .max_height(height)
        .auto_shrink([false, false]);
    if std::mem::take(&mut state.reveal) {
        let top = state.highlight as f32 * ROW;
        scroll = scroll.vertical_scroll_offset((top - LIST_HEIGHT / 2.0).max(0.0));
    }
    scroll.show_rows(ui, ROW, shown.len(), |ui, range| {
        for position in range {
            let Some(item) = shown.get(position).and_then(|&i| items.get(i)) else {
                continue;
            };
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    item == current,
                    item,
                )
            });
            let fill = if position == state.highlight || response.hovered() {
                t.hover
            } else if item == current {
                crate::themes::mix(t.accent, t.ribbon_bg, 0.8)
            } else {
                egui::Color32::TRANSPARENT
            };
            ui.painter()
                .rect_filled(rect, CornerRadius::same(t.radius), fill);
            ui.painter().text(
                egui::pos2(rect.min.x + 6.0, rect.center().y),
                Align2::LEFT_CENTER,
                item,
                FontId::proportional(13.0),
                ui.visuals().text_color(),
            );
            if response.clicked() {
                picked = Some(item.clone());
            }
        }
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn matches_rank_exact_then_prefix_then_contains() {
        let items = names(&["Segoe UI", "Arial", "Arial Black", "Calibri", "Sans Arial"]);
        let picked: Vec<&str> = matches("arial", &items)
            .into_iter()
            .map(|i| items[i].as_str())
            .collect();
        assert_eq!(picked, ["Arial", "Arial Black", "Sans Arial"]);
        assert_eq!(matches("  ", &items).len(), items.len());
        assert!(matches("zzz", &items).is_empty());
    }
}
