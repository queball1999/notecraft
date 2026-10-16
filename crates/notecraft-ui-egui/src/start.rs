//! The start page, shown in place of the navigation panes and the canvas while no notebook
//! is open: New Notebook, Open Notebook, Open File, and the recent list. The recent list is
//! also on the File backstage's Open page.

use egui::{Align, Button, Layout, RichText, Sense, Stroke, Ui};
use notecraft_engine::cmd::{ids, CommandId, Map, Value};
use notecraft_engine::recent::{RecentItem, RecentKind, Recents};

use crate::ribbon::Clicked;
use crate::themes::{mix, ThemeTokens};

const WIDTH: f32 = 560.0;
const ROW_HEIGHT: f32 = 44.0;

/// The three ways to get a notebook open, as on the start page and the backstage.
pub(crate) const OPEN_ACTIONS: [(&str, CommandId); 3] = [
    ("New Notebook", ids::FILE_NEW_NOTEBOOK),
    ("Open Notebook", ids::FILE_OPEN_NOTEBOOK),
    ("Open File", ids::FILE_OPEN),
];

fn clicked(command: CommandId) -> Clicked {
    Clicked {
        command,
        args: Value::Null,
    }
}

/// The command that reopens `item`, with its path.
pub(crate) fn reopen(item: &RecentItem) -> Clicked {
    let command = match item.kind {
        RecentKind::Notebook => ids::FILE_OPEN_NOTEBOOK,
        RecentKind::Page => ids::FILE_OPEN,
    };
    let mut args = Map::new();
    args.insert("path".to_owned(), Value::from(item.path.as_str()));
    Clicked {
        command,
        args: Value::Object(args),
    }
}

/// Draw the start page.
pub fn show(ui: &mut Ui, recents: &Recents, tokens: &ThemeTokens, out: &mut Vec<Clicked>) {
    let width = WIDTH.min(ui.available_width());
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        ui.set_max_width(width);
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.label(RichText::new("NoteCraft").size(30.0).color(tokens.text));
            ui.label(
                RichText::new("Open a notebook to get started.")
                    .size(15.0)
                    .color(tokens.text_muted),
            );
            ui.add_space(20.0);
            action_buttons(ui, tokens, out);
            ui.add_space(28.0);
            recent_list(ui, recents, tokens, out);
        });
    });
}

/// New Notebook, Open Notebook, and Open File, side by side.
/// A button with a visible frame. The theme's buttons are flat to blend into the ribbon,
/// which leaves a button on the page looking like plain text.
pub(crate) fn framed(label: &str, size: f32, tokens: &ThemeTokens) -> Button<'static> {
    Button::new(RichText::new(label).size(size).color(tokens.text))
        .fill(mix(tokens.canvas_bg, tokens.text, 0.06))
        .stroke(Stroke::new(1.0, tokens.border))
}

pub(crate) fn action_buttons(ui: &mut Ui, tokens: &ThemeTokens, out: &mut Vec<Clicked>) {
    ui.horizontal(|ui| {
        for (label, command) in OPEN_ACTIONS {
            let button = framed(label, 14.0, tokens).min_size(egui::vec2(140.0, 34.0));
            if ui.add(button).clicked() {
                out.push(clicked(command));
            }
        }
    });
}

/// The "Recent" heading, one row per recent notebook or page (name over path), and Clear
/// Recent. Clicking a row reopens it.
pub(crate) fn recent_list(
    ui: &mut Ui,
    recents: &Recents,
    tokens: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    ui.label(RichText::new("Recent").size(18.0).color(tokens.text));
    ui.add_space(6.0);
    if recents.is_empty() {
        ui.label(
            RichText::new("Notebooks and pages you open show up here.").color(tokens.text_muted),
        );
        return;
    }
    for item in recents.items() {
        if recent_row(ui, item, tokens).clicked() {
            out.push(reopen(item));
        }
    }
    ui.add_space(6.0);
    if ui.add(framed("Clear Recent", 13.0, tokens)).clicked() {
        out.push(clicked(ids::FILE_RECENT_CLEAR));
    }
}

fn recent_row(ui: &mut Ui, item: &RecentItem, tokens: &ThemeTokens) -> egui::Response {
    let size = egui::vec2(ui.available_width(), ROW_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let kind = match item.kind {
        RecentKind::Notebook => "Notebook",
        RecentKind::Page => "Page",
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, item.name()));
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, f32::from(tokens.radius), tokens.hover);
    }
    if response.has_focus() {
        let stroke = egui::Stroke::new(1.5, tokens.accent);
        painter.rect_stroke(
            rect,
            f32::from(tokens.radius),
            stroke,
            egui::StrokeKind::Inside,
        );
    }
    let left = rect.left_top() + egui::vec2(10.0, 5.0);
    painter.text(
        left,
        egui::Align2::LEFT_TOP,
        item.name(),
        egui::FontId::proportional(15.0),
        tokens.text,
    );
    painter.text(
        left + egui::vec2(0.0, 20.0),
        egui::Align2::LEFT_TOP,
        format!("{kind}  {}", item.path),
        egui::FontId::proportional(12.0),
        tokens.text_muted,
    );
    response.on_hover_text(&item.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_reopen_with_the_right_command() {
        let notebook = RecentItem::new(RecentKind::Notebook, "/n/Work").unwrap();
        let page = RecentItem::new(RecentKind::Page, "/n/a.md").unwrap();
        let open = reopen(&notebook);
        assert_eq!(open.command, ids::FILE_OPEN_NOTEBOOK);
        assert_eq!(open.args, serde_json::json!({ "path": "/n/Work" }));
        assert_eq!(reopen(&page).command, ids::FILE_OPEN);
    }
}
