//! One ribbon button: large, small, or icon-only, with an optional split or dropdown menu
//! and a tooltip that says when its command is Coming Soon.

use craft_ui::buttons::{ButtonVisuals, IconButton, IconButtonStyle};
use egui::{
    Color32, CornerRadius, FontId, Galley, Popup, PopupCloseBehavior, Rect, SetOpenCommand, Stroke,
    Ui, Vec2,
};
use notecraft_engine::cmd::{Map, Status, Value};
use std::sync::Arc;

use super::spec::{Button, Combo, Menu, Size};
use super::{combo, keytips, menus, Clicked, KeyFrame, Look};
use crate::icons::{self, IconColors};
use crate::themes::{mix, ButtonStyle, ThemeTokens};

/// Height of a small or icon-only button.
pub(super) const SMALL_HEIGHT: f32 = 22.0;
/// Width of the menu arrow area on small buttons, and height of it on large ones.
const ARROW: f32 = 12.0;
const LABEL_SIZE: f32 = 12.0;
/// Large labels wrap onto a second line beyond this width.
const LARGE_WRAP: f32 = 72.0;

/// Button colors for the ribbon, from the theme tokens. At rest a button has no fill of its
/// own, so it sits on the ribbon background; hover and press fill it. Raised buttons keep a
/// border at rest.
pub(crate) fn style(t: &ThemeTokens) -> IconButtonStyle {
    let rest_fill = Color32::TRANSPARENT;
    let rest_stroke = match t.button_style {
        ButtonStyle::Raised => Stroke::new(1.0, t.border),
        ButtonStyle::Flat | ButtonStyle::Filled => Stroke::NONE,
    };
    let visuals = |fill, stroke| ButtonVisuals {
        fill,
        stroke,
        icon: t.ribbon_text,
    };
    IconButtonStyle {
        normal: visuals(rest_fill, rest_stroke),
        hovered: visuals(t.hover, Stroke::new(1.0, t.border)),
        pressed: Some(visuals(t.pressed, Stroke::new(1.0, t.accent))),
        selected: visuals(mix(t.accent, t.ribbon_bg, 0.75), Stroke::new(1.0, t.accent)),
        corner_radius: CornerRadius::same(t.radius),
        focus_stroke: Stroke::new(1.5, t.accent),
    }
}

/// Draw `button` and record a click. `large_height` is the height of a large button.
pub(super) fn show(
    ui: &mut Ui,
    button: &'static Button,
    look: Look<'_>,
    keys: &KeyFrame,
    large_height: f32,
    out: &mut Vec<Clicked>,
) {
    show_sized(ui, button, button.size, look, keys, large_height, out);
}

/// Width of `button` drawn icon-only, as the simplified ribbon draws every button (a box
/// keeps its own width).
pub(super) fn icon_only_width(button: &Button) -> f32 {
    if let Menu::Combo(combo) = button.menu {
        return combo.width;
    }
    let arrow = if button.menu.is_none() {
        0.0
    } else {
        ARROW - 2.0
    };
    24.0 + arrow
}

/// [`show`] at `size` instead of the button's own size.
pub(super) fn show_sized(
    ui: &mut Ui,
    button: &'static Button,
    size: Size,
    look: Look<'_>,
    keys: &KeyFrame,
    large_height: f32,
    out: &mut Vec<Clicked>,
) {
    if let Menu::Combo(combo) = button.menu {
        show_combo(ui, button, &combo, look, out);
        return;
    }
    let tokens = look.tokens;
    let spec = look.registry.get(button.command.as_str());
    let coming_soon = spec.is_none_or(|spec| spec.status == Status::ComingSoon);
    let has_menu = !button.menu.is_none();
    let font = FontId::proportional(LABEL_SIZE);
    let drawn = size;

    let (size, galley) = match drawn {
        Size::Large => {
            let galley = ui.painter().layout(
                button.label.to_owned(),
                font,
                Color32::PLACEHOLDER,
                LARGE_WRAP,
            );
            let width = (galley.size().x + 12.0).max(48.0);
            (egui::vec2(width, large_height), Some(galley))
        }
        Size::Small => {
            let galley =
                ui.painter()
                    .layout_no_wrap(button.label.to_owned(), font, Color32::PLACEHOLDER);
            let arrow = if has_menu { ARROW } else { 0.0 };
            let width = 4.0 + 16.0 + 5.0 + galley.size().x + 6.0 + arrow;
            (egui::vec2(width, SMALL_HEIGHT), Some(galley))
        }
        Size::IconOnly => (egui::vec2(icon_only_width(button), SMALL_HEIGHT), None),
    };

    let style = style(tokens);
    let accent = tokens.accent;
    let response = IconButton::new(button.label, size, &style).show(ui, |ui, rect, color| {
        let colors = IconColors { fg: color, accent };
        paint_contents(ui, rect, button, drawn, galley, colors, has_menu);
    });
    let response = crate::a11y::plain_button(response);

    let arrow = arrow_rect(response.rect, drawn);
    let on_arrow = response
        .interact_pointer_pos()
        .is_some_and(|pos| arrow.contains(pos));
    // A key tip code presses the button like a click on its main part.
    let by_key = keys.pressed(button);
    let clicked = response.clicked() || by_key;
    let opens_menu = clicked && has_menu && (!button.menu.is_split() || (on_arrow && !by_key));
    if let Some(code) = keys.tip(button) {
        keytips::paint_tip(ui, response.rect.center_bottom(), code, look);
    }
    if clicked && !opens_menu {
        out.push(Clicked::new(button.command, button.args));
    }
    let toggle = opens_menu.then_some(SetOpenCommand::Toggle);
    // Pickers stay open while the user works in them and close themselves on a choice.
    let picker = Popup::menu(&response)
        .open_memory(toggle)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside);
    match button.menu {
        Menu::None => {}
        Menu::Split(items) | Menu::Dropdown(items) => {
            Popup::menu(&response)
                .open_memory(toggle)
                .show(|ui| menus::items(ui, items, look, out));
        }
        Menu::Colors(colors) => {
            picker.show(|ui| menus::colors(ui, button.command, &colors, look, out));
        }
        Menu::TableGrid => {
            picker.show(|ui| menus::table_grid(ui, button.command, look, out));
        }
        Menu::Combo(_) => {}
    }

    let mut tip = spec.map_or_else(String::new, |spec| spec.description.to_owned());
    if let Some(shortcut) = spec.and_then(|spec| spec.default_shortcut) {
        tip.push_str(&format!(" ({shortcut})"));
    }
    if coming_soon {
        tip.push_str("\nComing Soon");
    }
    let _ = response.on_hover_text(format!("{}\n{tip}", button.label));
}

/// A box button: the font or size box. A pick or typed value runs the command with it; a
/// size that is not a number in range is ignored, as OneNote refuses it.
fn show_combo(
    ui: &mut Ui,
    button: &'static Button,
    combo: &Combo,
    look: Look<'_>,
    out: &mut Vec<Clicked>,
) {
    let fallback: Vec<String>;
    let items: &[String] = if combo.fonts && !look.fonts.is_empty() {
        look.fonts
    } else {
        fallback = combo
            .items
            .iter()
            .map(|item| item.label.to_owned())
            .collect();
        &fallback
    };
    ui.allocate_ui(egui::vec2(combo.width, combo::HEIGHT), |ui| {
        let Some(value) = combo::show(
            ui,
            button.label,
            combo.width,
            combo.current,
            items,
            combo.search,
            combo.fonts && !look.fonts.is_empty(),
            look,
        ) else {
            return;
        };
        let value = match combo.number {
            Some((min, max)) => match value.parse::<f64>() {
                Ok(n) if n.is_finite() && (min..=max).contains(&n) => Value::from(n),
                _ => return,
            },
            None => Value::from(value),
        };
        let mut args = Map::new();
        args.insert(combo.arg.to_owned(), value);
        out.push(Clicked {
            command: button.command,
            args: Value::Object(args),
        });
    });
}

/// The part of a button that opens its menu: the bottom strip of a large button, the right
/// edge of a small one.
fn arrow_rect(rect: Rect, size: Size) -> Rect {
    match size {
        Size::Large => {
            Rect::from_min_max(egui::pos2(rect.min.x, rect.max.y - ARROW - 2.0), rect.max)
        }
        Size::Small | Size::IconOnly => {
            Rect::from_min_max(egui::pos2(rect.max.x - ARROW, rect.min.y), rect.max)
        }
    }
}

fn paint_contents(
    ui: &Ui,
    rect: Rect,
    button: &Button,
    size: Size,
    galley: Option<Arc<Galley>>,
    colors: IconColors,
    has_menu: bool,
) {
    let painter = ui.painter();
    match size {
        Size::Large => {
            let icon = Rect::from_center_size(
                egui::pos2(rect.center().x, rect.min.y + 6.0 + 16.0),
                Vec2::splat(32.0),
            );
            icons::paint(painter, icon, button.icon, colors);
            let mut arrow = egui::pos2(rect.center().x, rect.max.y - 7.0);
            if let Some(galley) = galley {
                let size = galley.size();
                let pos = egui::pos2(rect.center().x - size.x / 2.0, icon.max.y + 4.0);
                // A two-line label leaves no room under it: the arrow follows the last line.
                if galley.rows.len() > 1 {
                    if let Some(last) = galley.rows.last() {
                        let line = last.rect().translate(pos.to_vec2());
                        arrow = egui::pos2(line.max.x + 7.0, line.center().y);
                    }
                }
                painter.galley(pos, galley, colors.fg);
            }
            if has_menu {
                chevron(ui, arrow, colors.fg);
            }
        }
        Size::Small => {
            let icon = Rect::from_min_size(
                egui::pos2(rect.min.x + 4.0, rect.center().y - 8.0),
                Vec2::splat(16.0),
            );
            icons::paint(painter, icon, button.icon, colors);
            if let Some(galley) = galley {
                let pos = egui::pos2(icon.max.x + 5.0, rect.center().y - galley.size().y / 2.0);
                painter.galley(pos, galley, colors.fg);
            }
            if has_menu {
                chevron(
                    ui,
                    egui::pos2(rect.max.x - ARROW / 2.0, rect.center().y),
                    colors.fg,
                );
            }
        }
        Size::IconOnly => {
            let icon = Rect::from_min_size(
                egui::pos2(rect.min.x + 4.0, rect.center().y - 8.0),
                Vec2::splat(16.0),
            );
            icons::paint(painter, icon, button.icon, colors);
            if has_menu {
                chevron(ui, egui::pos2(rect.max.x - 5.0, rect.center().y), colors.fg);
            }
        }
    }
}

/// A small down arrow marking a menu.
fn chevron(ui: &Ui, center: egui::Pos2, color: Color32) {
    let points = vec![
        center + egui::vec2(-3.0, -1.5),
        center + egui::vec2(0.0, 1.5),
        center + egui::vec2(3.0, -1.5),
    ];
    ui.painter()
        .add(egui::Shape::line(points, Stroke::new(1.0, color)));
}
