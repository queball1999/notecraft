//! The simplified ribbon: the selected tab's commands as one row of icon buttons, the groups
//! split by a divider. Buttons keep their order; from the first one that does not fit, the
//! rest go in the More Options menu at the end of the row.

use craft_ui::buttons::IconButton;
use egui::{Align, Layout, Popup, PopupCloseBehavior, Rect, SetOpenCommand, Stroke, Ui, UiBuilder};

use super::button::{self, SMALL_HEIGHT};
use super::spec::{Button, Column, Group, Size};
use super::{menus, tools, Clicked, KeyFrame, Look};

/// Height of the simplified ribbon's row, under the tab strip.
pub const BODY_HEIGHT: f32 = 36.0;
/// Width of the More Options button.
const MORE_WIDTH: f32 = 28.0;
/// Space a group divider takes.
const DIVIDER: f32 = 9.0;
/// Space between buttons.
const GAP: f32 = 2.0;

/// Which of `groups`' buttons fit in `width`: the shown buttons per group (groups with none
/// left out) and, in order, the ones that go in More Options.
pub(super) fn fit(
    groups: &'static [Group],
    width: f32,
    tools_width: f32,
) -> (
    Vec<(&'static Group, Vec<&'static Button>)>,
    Vec<&'static Button>,
) {
    let mut shown: Vec<(&'static Group, Vec<&'static Button>)> = Vec::new();
    let mut more = Vec::new();
    let mut x = 0.0;
    for group in groups {
        let mut row = Vec::new();
        // The tools of a Draw tab group take room with its first button.
        let mut extra = if has_tools(group) { tools_width } else { 0.0 };
        for button in group.buttons() {
            let divider = if row.is_empty() && !shown.is_empty() {
                DIVIDER
            } else {
                0.0
            };
            let needed = divider + button::icon_only_width(button) + GAP + extra;
            if more.is_empty() && x + needed <= width {
                x += needed;
                extra = 0.0;
                row.push(button);
            } else {
                more.push(button);
            }
        }
        if !row.is_empty() {
            shown.push((group, row));
        }
    }
    (shown, more)
}

/// Whether `group` holds the Draw tab's tool list.
fn has_tools(group: &Group) -> bool {
    group
        .columns
        .iter()
        .any(|column| matches!(column, Column::Tools))
}

/// Draw the row for `groups` into `area`.
pub(super) fn show(
    ui: &mut Ui,
    area: Rect,
    groups: &'static [Group],
    look: Look<'_>,
    keys: &KeyFrame,
    out: &mut Vec<Clicked>,
) {
    let (shown, more) = fit(
        groups,
        area.width() - MORE_WIDTH,
        tools::compact_width(look),
    );
    let tokens = look.tokens;
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(area)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(GAP, 0.0);
            for (index, (group, row)) in shown.iter().enumerate() {
                if index > 0 {
                    ui.add_space(3.0);
                    let x = ui.cursor().min.x;
                    ui.painter().vline(
                        x,
                        (area.center().y - 10.0)..=(area.center().y + 10.0),
                        Stroke::new(1.0, tokens.border),
                    );
                    ui.add_space(DIVIDER - 3.0 - GAP);
                }
                for &button in row {
                    button::show_sized(ui, button, Size::IconOnly, look, keys, SMALL_HEIGHT, out);
                }
                if has_tools(group) {
                    tools::show_compact(ui, look, out);
                }
            }
        },
    );
    let tools_missing = !look.tools.is_empty() && !shown.iter().any(|(group, _)| has_tools(group));
    if more.is_empty() && !tools_missing {
        return;
    }
    let rect = Rect::from_min_size(
        egui::pos2(
            area.max.x - MORE_WIDTH + 2.0,
            area.center().y - SMALL_HEIGHT / 2.0,
        ),
        egui::vec2(MORE_WIDTH - 4.0, SMALL_HEIGHT),
    );
    let style = button::style(tokens);
    let response = ui
        .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            IconButton::new("More Options", rect.size(), &style).show(ui, |ui, rect, color| {
                for dx in [-5.0, 0.0, 5.0] {
                    ui.painter()
                        .circle_filled(rect.center() + egui::vec2(dx, 0.0), 1.3, color);
                }
            })
        })
        .inner;
    let response = crate::a11y::plain_button(response).on_hover_text("More Options");
    let toggle = response.clicked().then_some(SetOpenCommand::Toggle);
    Popup::menu(&response)
        .open_memory(toggle)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            for button in &more {
                menus::button_item(ui, button, look, out);
            }
            if tools_missing {
                tools::menu_rows(ui, look, out);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribbon::spec::{TabKind, TABS};

    fn home() -> &'static [Group] {
        TABS.iter()
            .find_map(|tab| match tab.kind {
                TabKind::Groups(groups) if tab.name == "Home" => Some(groups),
                _ => None,
            })
            .unwrap_or(&[])
    }

    #[test]
    fn everything_fits_in_a_wide_window_and_order_is_kept() {
        let groups = home();
        let (shown, more) = fit(groups, 10_000.0, 0.0);
        assert!(more.is_empty());
        let all: Vec<&str> = groups
            .iter()
            .flat_map(|g| g.buttons())
            .map(|b| b.label)
            .collect();
        let drawn: Vec<&str> = shown
            .iter()
            .flat_map(|(_, row)| row)
            .map(|b| b.label)
            .collect();
        assert_eq!(drawn, all);
    }

    #[test]
    fn a_narrow_window_moves_the_rest_to_more_options_in_order() {
        let groups = home();
        let all: Vec<&str> = groups
            .iter()
            .flat_map(|g| g.buttons())
            .map(|b| b.label)
            .collect();
        let (shown, more) = fit(groups, 200.0, 0.0);
        let drawn: Vec<&str> = shown
            .iter()
            .flat_map(|(_, row)| row)
            .map(|b| b.label)
            .collect();
        let rest: Vec<&str> = more.iter().map(|b| b.label).collect();
        assert!(!drawn.is_empty() && !rest.is_empty());
        assert_eq!([drawn, rest].concat(), all);
        let (shown, more) = fit(groups, 0.0, 0.0);
        assert!(shown.is_empty());
        assert_eq!(more.len(), all.len());
    }
}
