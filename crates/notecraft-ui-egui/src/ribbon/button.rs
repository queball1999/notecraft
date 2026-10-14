//! One ribbon button: large, small, or icon-only, with an optional split or dropdown menu
//! and a tooltip that says when its command is Coming Soon.

use craft_ui::buttons::{ButtonVisuals, IconButton, IconButtonStyle};
use egui::{Color32, CornerRadius, FontId, Galley, Popup, Rect, SetOpenCommand, Stroke, Ui, Vec2};
use notecraft_engine::cmd::Status;
use std::sync::Arc;

use super::spec::{Button, Menu, Size};
use super::{keytips, Clicked, KeyFrame, Look};
use crate::icons::{self, IconColors};
use crate::themes::{mix, ButtonStyle, ThemeTokens};

/// Height of a small or icon-only button.
pub(super) const SMALL_HEIGHT: f32 = 22.0;
/// Width of the menu arrow area on small buttons, and height of it on large ones.
const ARROW: f32 = 12.0;
const LABEL_SIZE: f32 = 12.0;
/// Large labels wrap onto a second line beyond this width.
const LARGE_WRAP: f32 = 72.0;

/// Button colors for the ribbon, from the theme tokens.
pub(crate) fn style(t: &ThemeTokens) -> IconButtonStyle {
    let (rest_fill, rest_stroke) = match t.button_style {
        ButtonStyle::Flat => (Color32::TRANSPARENT, Stroke::NONE),
        ButtonStyle::Raised => (
            mix(t.ribbon_bg, t.ribbon_text, 0.04),
            Stroke::new(1.0, t.border),
        ),
        ButtonStyle::Filled => (mix(t.ribbon_bg, t.ribbon_text, 0.04), Stroke::NONE),
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
    let tokens = look.tokens;
    let spec = look.registry.get(button.command.as_str());
    let coming_soon = spec.is_none_or(|spec| spec.status == Status::ComingSoon);
    let has_menu = !matches!(button.menu, Menu::None);
    let font = FontId::proportional(LABEL_SIZE);

    let (size, galley) = match button.size {
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
        Size::IconOnly => {
            let arrow = if has_menu { ARROW - 2.0 } else { 0.0 };
            (egui::vec2(24.0 + arrow, SMALL_HEIGHT), None)
        }
    };

    let style = style(tokens);
    let accent = tokens.accent;
    let response = IconButton::new(button.label, size, &style).show(ui, |ui, rect, color| {
        let colors = IconColors { fg: color, accent };
        paint_contents(ui, rect, button, galley, colors, has_menu);
    });

    let arrow = arrow_rect(response.rect, button.size);
    let on_arrow = response
        .interact_pointer_pos()
        .is_some_and(|pos| arrow.contains(pos));
    // A key tip code presses the button like a click on its main part.
    let by_key = keys.pressed(button);
    let clicked = response.clicked() || by_key;
    let opens_menu = clicked
        && match button.menu {
            Menu::None => false,
            Menu::Split(_) => on_arrow && !by_key,
            Menu::Dropdown(_) => true,
        };
    if let Some(code) = keys.tip(button) {
        keytips::paint_tip(ui, response.rect.center_bottom(), code, look);
    }
    if clicked && !opens_menu {
        out.push(Clicked::new(button.command, button.args));
    }
    if has_menu {
        let toggle = opens_menu.then_some(SetOpenCommand::Toggle);
        Popup::menu(&response).open_memory(toggle).show(|ui| {
            for item in button.menu.items() {
                if ui.button(item.label).clicked() {
                    out.push(Clicked::new(item.command, item.args));
                }
            }
        });
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
    galley: Option<Arc<Galley>>,
    colors: IconColors,
    has_menu: bool,
) {
    let painter = ui.painter();
    match button.size {
        Size::Large => {
            let icon = Rect::from_center_size(
                egui::pos2(rect.center().x, rect.min.y + 6.0 + 16.0),
                Vec2::splat(32.0),
            );
            icons::paint(painter, icon, button.icon, colors);
            if let Some(galley) = galley {
                let size = galley.size();
                let pos = egui::pos2(rect.center().x - size.x / 2.0, icon.max.y + 4.0);
                painter.galley(pos, galley, colors.fg);
            }
            if has_menu {
                chevron(ui, egui::pos2(rect.center().x, rect.max.y - 7.0), colors.fg);
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
