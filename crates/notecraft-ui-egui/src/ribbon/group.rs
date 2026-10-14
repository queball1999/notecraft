//! One ribbon group: its columns of buttons, the group name under them, and a separator.

use egui::{Align2, FontId, Sense, Stroke, Ui};

use super::button::{self, SMALL_HEIGHT};
use super::spec::{Column, Group};
use super::{Clicked, KeyFrame, Look, BODY_HEIGHT};

/// Height of the group name row.
const NAME_HEIGHT: f32 = 16.0;
/// Space between buttons.
const GAP: f32 = 2.0;

pub(super) fn show(
    ui: &mut Ui,
    group: &'static Group,
    look: Look<'_>,
    keys: &KeyFrame,
    out: &mut Vec<Clicked>,
) {
    let content_height = BODY_HEIGHT - NAME_HEIGHT - 8.0;
    let response = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(GAP, GAP);
        let content = ui.horizontal_top(|ui| {
            ui.set_min_height(content_height);
            for column in group.columns {
                show_column(ui, column, look, keys, content_height, out);
            }
        });
        let width = content.response.rect.width().max(40.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, NAME_HEIGHT), Sense::hover());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            group.name,
            FontId::proportional(11.0),
            look.tokens.text_muted,
        );
    });
    let rect = response.response.rect;
    ui.add_space(4.0);
    let x = ui.cursor().min.x;
    ui.painter().vline(
        x,
        (rect.min.y + 4.0)..=(rect.max.y - 2.0),
        Stroke::new(1.0, look.tokens.border),
    );
    ui.add_space(5.0);
}

fn show_column(
    ui: &mut Ui,
    column: &'static Column,
    look: Look<'_>,
    keys: &KeyFrame,
    content_height: f32,
    out: &mut Vec<Clicked>,
) {
    match column {
        Column::Large(large) => button::show(ui, large, look, keys, content_height, out),
        Column::Stack(buttons) => {
            ui.vertical(|ui| {
                for small in buttons.iter().take(3) {
                    button::show(ui, small, look, keys, SMALL_HEIGHT, out);
                }
            });
        }
        Column::Rows(rows) => {
            ui.vertical(|ui| {
                for row in rows.iter().take(3) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 1.0;
                        for small in row.iter() {
                            button::show(ui, small, look, keys, SMALL_HEIGHT, out);
                        }
                    });
                }
            });
        }
    }
}
