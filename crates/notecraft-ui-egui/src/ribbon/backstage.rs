//! The File backstage: a full-window view with a list of entries on an accent-colored
//! column (Info, New, Open, Print, Export, Close, Options, About) and the chosen entry's page.
//! Esc or the back button returns to the notes.

use craft_ui::buttons::{ButtonVisuals, IconButton, IconButtonStyle};
use craft_ui::tabs::Tab;
use egui::{Color32, CornerRadius, FontId, Frame, Key, Margin, RichText, Sense, Stroke, Ui};
use notecraft_engine::cmd::Status;

use super::spec::{Item, TabKind, TABS};
use super::{Clicked, Look, Ribbon};
use crate::icons::{self, Icon, IconColors};
use crate::themes::mix;

const NAV_WIDTH: f32 = 200.0;
const ENTRY_HEIGHT: f32 = 36.0;

/// The backstage entries, from the File tab in the spec.
fn entries() -> &'static [Item] {
    TABS.iter()
        .find_map(|tab| match tab.kind {
            TabKind::Backstage(items) => Some(items),
            TabKind::Groups(_) => None,
        })
        .unwrap_or(&[])
}

pub(super) fn show(ribbon: &mut Ribbon, ui: &mut Ui, look: Look<'_>, out: &mut Vec<Clicked>) {
    let t = look.tokens;
    let items = entries();
    if ui.input(|input| input.key_pressed(Key::Escape)) {
        ribbon.close_backstage();
        return;
    }
    let hover_fill = mix(t.accent, t.accent_text, 0.15);
    let selected_fill = mix(t.accent, t.accent_text, 0.25);

    egui::Panel::left("backstage_nav")
        .exact_size(NAV_WIDTH)
        .resizable(false)
        .frame(Frame::NONE.fill(t.accent).inner_margin(Margin::same(8)))
        .show(ui, |ui| {
            let plain = ButtonVisuals {
                fill: Color32::TRANSPARENT,
                stroke: Stroke::NONE,
                icon: t.accent_text,
            };
            let style = IconButtonStyle {
                normal: plain,
                hovered: ButtonVisuals {
                    fill: hover_fill,
                    ..plain
                },
                pressed: None,
                selected: plain,
                corner_radius: CornerRadius::same(18),
                focus_stroke: Stroke::new(1.5, t.accent_text),
            };
            let back = IconButton::new("Back", egui::vec2(36.0, 36.0), &style).show(
                ui,
                |ui, rect, color| {
                    let colors = IconColors {
                        fg: color,
                        accent: color,
                    };
                    icons::paint(ui.painter(), rect.shrink(6.0), Icon::Back, colors);
                },
            );
            if back.on_hover_text("Back to your notes (Esc)").clicked() {
                ribbon.close_backstage();
            }
            ui.add_space(12.0);

            for (index, item) in items.iter().enumerate() {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ENTRY_HEIGHT),
                    Sense::hover(),
                );
                let selected = ribbon.backstage == Some(index);
                let id = ui.id().with(("backstage_entry", index));
                let response = Tab::new(id, item.label, selected)
                    .focus_stroke(Stroke::new(1.5, t.accent_text))
                    .show_at(ui, rect, |ui, response| {
                        let fill = if selected {
                            selected_fill
                        } else if response.hovered() {
                            hover_fill
                        } else {
                            Color32::TRANSPARENT
                        };
                        let painter = ui.painter();
                        painter.rect_filled(rect, CornerRadius::same(t.radius), fill);
                        painter.text(
                            rect.left_center() + egui::vec2(14.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            item.label,
                            FontId::proportional(15.0),
                            t.accent_text,
                        );
                    });
                if response.clicked() {
                    ribbon.backstage = Some(index);
                    out.push(Clicked::new(item.command, item.args));
                }
            }
        });

    let current = ribbon.backstage.and_then(|index| items.get(index));
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(t.canvas_bg).inner_margin(Margin::same(32)))
        .show(ui, |ui| {
            let Some(item) = current else {
                return;
            };
            ui.label(RichText::new(item.label).size(28.0).color(t.text));
            ui.add_space(12.0);
            let spec = look.registry.get(item.command.as_str());
            if let Some(spec) = spec {
                ui.label(RichText::new(spec.description).color(t.text));
            }
            if spec.is_none_or(|spec| spec.status == Status::ComingSoon) {
                ui.add_space(8.0);
                ui.label(RichText::new("Coming Soon").color(t.text_muted));
            }
        });
}
