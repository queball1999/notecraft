//! Section tabs: the current notebook's sections as colored tabs across the top of the page,
//! with a "+" tab for a new section and an overflow menu for tabs that do not fit.

use craft_ui::buttons::IconButton;
use craft_ui::tabs::Tab;
use egui::{Color32, CornerRadius, FontId, Popup, Rect, SetOpenCommand, Stroke, Ui};
use notecraft_engine::cmd::ids;
use notecraft_engine::Session;

use super::edit::{self, DragPage, DragSection};
use super::{plain, select, swatch_color, NavUi};
use crate::ribbon::Clicked;
use crate::themes::contrast::contrast_ratio;
use crate::themes::ThemeTokens;

/// Height of the section tab strip.
pub(crate) const STRIP_HEIGHT: f32 = 30.0;
const TAB_PADDING: f32 = 14.0;
const MAX_TAB_WIDTH: f32 = 180.0;
const BUTTON_WIDTH: f32 = 28.0;

/// Near-black or near-white, whichever reads better on `fill`.
fn text_on(fill: Color32) -> Color32 {
    let dark = Color32::from_gray(0x1C);
    let light = Color32::from_gray(0xF2);
    if contrast_ratio(dark, fill) >= contrast_ratio(light, fill) {
        dark
    } else {
        light
    }
}

/// Draw the strip into `strip`.
pub(crate) fn show(
    ui: &mut Ui,
    strip: Rect,
    session: &Session,
    nav: &mut NavUi,
    t: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    ui.painter().rect_filled(strip, 0.0, t.sidebar_bg);
    ui.painter().hline(
        strip.x_range(),
        strip.max.y - 0.5,
        Stroke::new(1.0, t.border),
    );
    let sections = session
        .current_notebook()
        .map(|nb| nb.all_sections())
        .unwrap_or_default();
    let current = session.selection().section.as_deref();

    // Lay out tabs left to right, keeping room for the overflow and "+" buttons.
    let right_limit = strip.max.x - 2.0 * BUTTON_WIDTH - 8.0;
    let mut x = strip.min.x + 6.0;
    let mut hidden = Vec::new();
    for section in &sections {
        let galley = ui.painter().layout_no_wrap(
            section.name.clone(),
            FontId::proportional(13.0),
            Color32::PLACEHOLDER,
        );
        let width = (galley.size().x + 2.0 * TAB_PADDING).min(MAX_TAB_WIDTH);
        if x + width > right_limit {
            hidden.push(*section);
            continue;
        }
        let selected = current == Some(section.id.as_str());
        let rect = Rect::from_min_size(
            egui::pos2(x, strip.min.y + if selected { 3.0 } else { 6.0 }),
            egui::vec2(width, strip.height() - if selected { 3.0 } else { 6.0 }),
        );
        x += width + 2.0;
        if nav.is_renaming(&section.id) {
            out.extend(edit::rename_field(ui, nav, rect));
            continue;
        }
        let fill = swatch_color(section.color, t);
        let text = text_on(fill);
        let id = ui.id().with(("section_tab", section.id.as_str()));
        let response = Tab::new(id, &section.name, selected)
            .focus_stroke(Stroke::new(1.5, t.accent))
            .show_at(ui, rect, |ui, response| {
                let fill = if response.hovered() && !selected {
                    crate::themes::mix(fill, text, 0.08)
                } else {
                    fill
                };
                let radius = t.radius.max(2);
                let corners = CornerRadius {
                    nw: radius,
                    ne: radius,
                    sw: 0,
                    se: 0,
                };
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, corners, fill);
                let size = galley.size();
                let pos = egui::pos2(rect.min.x + TAB_PADDING, rect.center().y - size.y / 2.0);
                painter.galley(pos, galley, text);
            });
        if response.clicked() {
            out.push(select(ids::NAV_SECTION_SELECT, &section.id));
        }
        if response.double_clicked() {
            nav.begin_rename(&section.id, edit::Kind::Section, &section.name);
        }
        response
            .context_menu(|ui| edit::section_menu(ui, &section.id, &section.name, false, nav, out));
        response.dnd_set_drag_payload(DragSection(section.id.clone()));
        // A page dropped on a tab moves to that section, last.
        if response.dnd_hover_payload::<DragPage>().is_some() {
            edit::drop_outline(ui, rect, t);
            if let Some(dropped) = response.dnd_release_payload::<DragPage>() {
                out.push(edit::move_page(&dropped.0, &section.id, None));
            }
        }
        // A section dropped on a tab goes just before it (or after it, on its right half).
        if response.dnd_hover_payload::<DragSection>().is_some() {
            let right = ui
                .input(|i| i.pointer.hover_pos())
                .is_some_and(|pos| pos.x > rect.center().x);
            let x = if right { rect.max.x } else { rect.min.x };
            ui.painter()
                .vline(x, rect.y_range(), Stroke::new(2.0, t.accent));
            if let Some(dropped) = response.dnd_release_payload::<DragSection>() {
                let from = sections.iter().position(|s| s.id == dropped.0);
                let row = sections
                    .iter()
                    .position(|s| s.id == section.id)
                    .unwrap_or(0);
                let at = edit::drop_index(from, row, right);
                out.push(edit::move_section(&dropped.0, None, Some(at)));
            }
        }
    }

    let style = crate::ribbon::button_style(t);
    let button = |ui: &mut Ui, index: f32, label: &str, glyph: &str| {
        let rect = Rect::from_min_size(
            egui::pos2(
                strip.max.x - (index + 1.0) * BUTTON_WIDTH - 4.0,
                strip.min.y + 4.0,
            ),
            egui::vec2(BUTTON_WIDTH - 2.0, strip.height() - 8.0),
        );
        let glyph = glyph.to_owned();
        let response = ui
            .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                IconButton::new(label, rect.size(), &style).show(ui, |ui, rect, color| {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        glyph,
                        FontId::proportional(15.0),
                        color,
                    );
                })
            })
            .inner;
        crate::a11y::plain_button(response)
    };

    let new_section = button(ui, 0.0, "New Section", "+");
    if new_section.on_hover_text("New Section (Ctrl+T)").clicked() {
        out.push(plain(ids::SECTION_NEW));
    }
    if !hidden.is_empty() {
        let more = button(ui, 1.0, "More Sections", "v");
        let toggle = more.clicked().then_some(SetOpenCommand::Toggle);
        Popup::menu(&more).open_memory(toggle).show(|ui| {
            for section in &hidden {
                if ui.button(&section.name).clicked() {
                    out.push(select(ids::NAV_SECTION_SELECT, &section.id));
                }
            }
        });
    }
}
