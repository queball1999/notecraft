//! The Tags Summary pane (build_plan/04 PR 2.6). It reads the session's tag summary state and
//! returns commands (`tag.summary.set_scope`, `tag.summary.set_group`,
//! `tag.summary.set_unchecked`, `tag.summary.refresh`, `tag.summary.open_item`,
//! `tag.summary.create_page`, `tag.summary.close`); it never changes the session itself.

use egui::{Color32, FontId, RichText, Ui};
use notecraft_engine::cmd::{ids, Map, Value};
use notecraft_engine::{SearchScope, Session, SummaryGroup, SummaryItem};

use crate::ribbon::Clicked;
use crate::search_pane::{choice, command};
use crate::themes::{mix, ThemeTokens};

/// Width the pane starts at, in points.
pub(crate) const WIDTH: f32 = 340.0;

const ICON: f32 = 16.0;

fn rgb(value: u32) -> Color32 {
    Color32::from_rgb(
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    )
}

fn plain(id: notecraft_engine::cmd::CommandId) -> Clicked {
    Clicked {
        command: id,
        args: Value::Null,
    }
}

/// The tag icon of an item; a To Do shows its check box, ticked when it is checked.
fn paint_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    item: &SummaryItem,
    session: &Session,
    t: &ThemeTokens,
) {
    let def = session.tags().get(&item.tag);
    let color = def.map_or(t.accent, |d| rgb(d.icon_color));
    match item.checked {
        Some(checked) => {
            let inner = rect.shrink(2.0);
            painter.rect_stroke(
                inner,
                egui::CornerRadius::same(2),
                egui::Stroke::new(1.5, color),
                egui::StrokeKind::Inside,
            );
            if checked {
                let at = |x: f32, y: f32| inner.min + egui::vec2(x, y) * inner.width();
                painter.add(egui::Shape::line(
                    vec![at(0.2, 0.55), at(0.42, 0.78), at(0.8, 0.25)],
                    egui::Stroke::new(2.0, color),
                ));
            }
        }
        None => {
            let icon = def.map_or(notecraft_markdown::TagIcon::Dot, |d| d.icon);
            crate::icons::paint_tag(painter, rect, icon, color);
        }
    }
}

fn item_row(
    ui: &mut Ui,
    item: &SummaryItem,
    session: &Session,
    multi: bool,
    t: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    let width = ui.available_width();
    let pad = 6.0;
    let inner = width - 2.0 * pad - ICON - 6.0;
    let done = item.checked == Some(true);
    let shown = if item.text.is_empty() {
        item.tag_name.as_str()
    } else {
        item.text.as_str()
    };
    let body_color = if done { t.text_muted } else { t.text };
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = inner;
    job.append(
        shown,
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(13.0),
            color: body_color,
            strikethrough: if done {
                egui::Stroke::new(1.0, body_color)
            } else {
                egui::Stroke::NONE
            },
            ..egui::TextFormat::default()
        },
    );
    let body = ui.painter().layout_job(job);
    let place = if multi {
        format!(
            "{} > {} > {}",
            item.notebook_name, item.section_name, item.page_title
        )
    } else {
        format!("{} > {}", item.section_name, item.page_title)
    };
    let place = ui
        .painter()
        .layout(place, FontId::proportional(11.5), t.text_muted, inner);
    let height = pad + body.size().y + place.size().y + pad;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, shown));
    let painter = ui.painter_at(rect);
    if response.hovered() || response.has_focus() {
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(t.radius),
            mix(t.sidebar_bg, t.sidebar_text, 0.08),
        );
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            egui::CornerRadius::same(t.radius),
            egui::Stroke::new(1.5, t.accent),
            egui::StrokeKind::Inside,
        );
    }
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + pad, rect.min.y + pad),
        egui::vec2(ICON, ICON),
    );
    paint_icon(&painter, icon_rect, item, session, t);
    let x = rect.min.x + pad + ICON + 6.0;
    let mut y = rect.min.y + pad;
    let body_height = body.size().y;
    painter.galley(egui::pos2(x, y), body, body_color);
    y += body_height;
    painter.galley(egui::pos2(x, y), place, t.text_muted);
    if response.clicked() {
        let mut map = Map::new();
        map.insert("page".to_owned(), Value::from(item.page_id.as_str()));
        map.insert("at".to_owned(), Value::from(item.at));
        map.insert("tag".to_owned(), Value::from(item.tag.as_str()));
        map.insert("text".to_owned(), Value::from(item.text.as_str()));
        out.push(Clicked {
            command: ids::TAG_SUMMARY_OPEN_ITEM,
            args: Value::Object(map),
        });
    }
}

/// Draw the pane. Pushes the commands the user asked for into `out`.
pub(crate) fn show(ui: &mut Ui, session: &Session, t: &ThemeTokens, out: &mut Vec<Clicked>) {
    let state = session.tag_summary();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Tags Summary").strong().color(t.sidebar_text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Close")
                .on_hover_text("Close the Tags Summary")
                .clicked()
            {
                out.push(plain(ids::TAG_SUMMARY_CLOSE));
            }
        });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Group tags by").color(t.text_muted));
        if let Some(group) = choice(
            ui,
            "tag_summary_group",
            "Group tags by",
            &SummaryGroup::LIST,
            state.group,
            SummaryGroup::label,
        ) {
            out.push(command(
                ids::TAG_SUMMARY_SET_GROUP,
                &[("group", group.id())],
            ));
        }
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Search").color(t.text_muted));
        if let Some(scope) = choice(
            ui,
            "tag_summary_scope",
            "Tags search scope",
            &SearchScope::LIST,
            state.scope,
            SearchScope::label,
        ) {
            out.push(command(
                ids::TAG_SUMMARY_SET_SCOPE,
                &[("scope", scope.id())],
            ));
        }
    });
    let mut only = state.only_unchecked;
    if ui
        .checkbox(&mut only, "Show only unchecked items")
        .changed()
    {
        let mut map = Map::new();
        map.insert("value".to_owned(), Value::from(only));
        out.push(Clicked {
            command: ids::TAG_SUMMARY_SET_UNCHECKED,
            args: Value::Object(map),
        });
    }
    let count = state.summary.as_ref().map_or(0, |s| s.count());
    ui.horizontal(|ui| {
        if ui.button("Refresh Results").clicked() {
            out.push(plain(ids::TAG_SUMMARY_REFRESH));
        }
        let create = ui
            .add_enabled(count > 0, egui::Button::new("Create Summary Page"))
            .on_hover_text("Write a new page in this section listing these items");
        if create.clicked() {
            out.push(plain(ids::TAG_SUMMARY_CREATE_PAGE));
        }
    });
    ui.separator();

    let Some(summary) = &state.summary else {
        ui.label(RichText::new("Press Refresh Results to list tagged notes.").color(t.text_muted));
        return;
    };
    let text = match (count, summary.truncated) {
        (0, _) => "No tagged notes found.".to_owned(),
        (1, _) => "1 note".to_owned(),
        (n, true) => format!("First {n} notes"),
        (n, false) => format!("{n} notes"),
    };
    ui.label(RichText::new(text).color(t.text_muted));
    let multi = session.notebooks().len() > 1 && summary.scope == SearchScope::All;
    egui::ScrollArea::vertical()
        .id_salt("tag_summary_items")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for group in &summary.groups {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!("{} ({})", group.label, group.items.len()))
                        .strong()
                        .color(t.sidebar_text),
                );
                for item in &group.items {
                    item_row(ui, item, session, multi, t, out);
                }
            }
        });
}
