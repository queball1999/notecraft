//! The search box and results pane (build_plan/04 PR 2.4). The pane reads the session's
//! search state and returns commands (`search.run`, `search.set_scope`, `search.set_sort`,
//! `search.close`, `nav.page.select`); it never changes the session itself. Ctrl+E runs
//! `search.open`, which shows the pane and bumps a counter so the box takes the caret.

use egui::text::{LayoutJob, TextFormat};
use egui::{FontId, RichText, Ui};
use notecraft_engine::cmd::{ids, Map, Value};
use notecraft_engine::{SearchHit, SearchScope, SearchSort, Session};

use crate::ribbon::Clicked;
use crate::themes::{mix, ThemeTokens};

/// Width the pane starts at, in points.
pub(crate) const WIDTH: f32 = 320.0;

/// Search UI state that is not session state.
#[derive(Debug, Default)]
pub struct SearchUi {
    /// The text in the box.
    text: String,
    /// The `open_serial` the box last took the caret for.
    seen_serial: u64,
    /// The query the session last heard from this box.
    sent: String,
}

fn command(id: notecraft_engine::cmd::CommandId, args: &[(&str, &str)]) -> Clicked {
    let mut map = Map::new();
    for (key, value) in args {
        map.insert((*key).to_owned(), Value::from(*value));
    }
    Clicked {
        command: id,
        args: Value::Object(map),
    }
}

/// `YYYY-MM-DD` for milliseconds since the Unix epoch (UTC).
pub(crate) fn date_text(millis: i64) -> String {
    let days = millis.div_euclid(86_400_000);
    // Civil date from a day count (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The snippet as a layout with the matched words highlighted.
fn snippet_job(hit: &SearchHit, t: &ThemeTokens, wrap: f32) -> LayoutJob {
    let font = FontId::proportional(12.5);
    let plain = TextFormat {
        font_id: font.clone(),
        color: t.text,
        ..TextFormat::default()
    };
    let marked = TextFormat {
        font_id: font,
        color: t.text,
        background: mix(t.canvas_bg, t.accent, if t.dark { 0.55 } else { 0.35 }),
        ..TextFormat::default()
    };
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap;
    let text = hit.snippet.as_str();
    let mut at = 0;
    for &(from, to) in &hit.highlights {
        let (Some(before), Some(word)) = (text.get(at..from), text.get(from..to)) else {
            continue;
        };
        job.append(before, 0.0, plain.clone());
        job.append(word, 0.0, marked.clone());
        at = to;
    }
    job.append(text.get(at..).unwrap_or_default(), 0.0, plain);
    job
}

fn hit_row(ui: &mut Ui, hit: &SearchHit, multi: bool, t: &ThemeTokens, out: &mut Vec<Clicked>) {
    let width = ui.available_width();
    let pad = 6.0;
    let inner = width - 2.0 * pad;
    let title = ui
        .painter()
        .layout(hit.title.clone(), FontId::proportional(13.5), t.text, inner);
    let place = if multi {
        format!(
            "{} > {} - {}",
            hit.notebook_name,
            hit.section_name,
            date_text(hit.modified)
        )
    } else {
        format!("{} - {}", hit.section_name, date_text(hit.modified))
    };
    let place = ui
        .painter()
        .layout(place, FontId::proportional(11.5), t.text_muted, inner);
    let snippet =
        (!hit.snippet.is_empty()).then(|| ui.painter().layout_job(snippet_job(hit, t, inner)));
    let height = pad
        + title.size().y
        + place.size().y
        + snippet.as_ref().map_or(0.0, |s| s.size().y + 2.0)
        + pad;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, hit.title.as_str())
    });
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
    let mut y = rect.min.y + pad;
    let x = rect.min.x + pad;
    let title_height = title.size().y;
    painter.galley(egui::pos2(x, y), title, t.text);
    y += title_height;
    let place_height = place.size().y;
    painter.galley(egui::pos2(x, y), place, t.text_muted);
    y += place_height + 2.0;
    if let Some(snippet) = snippet {
        painter.galley(egui::pos2(x, y), snippet, t.text);
    }
    if response.clicked() && !hit.page_id.is_empty() {
        out.push(command(
            ids::NAV_PAGE_SELECT,
            &[("id", hit.page_id.as_str())],
        ));
    }
}

fn choice<T: Copy + PartialEq>(
    ui: &mut Ui,
    id: &str,
    label: &str,
    all: &[T],
    current: T,
    text: impl Fn(T) -> &'static str,
) -> Option<T> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(id)
        .selected_text(text(current))
        .width(ui.available_width().max(120.0))
        .show_ui(ui, |ui| {
            for &item in all {
                if ui.selectable_label(item == current, text(item)).clicked() {
                    picked = Some(item);
                }
            }
        })
        .response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, true, label));
    picked
}

/// Draw the pane. Pushes the commands the user asked for into `out`.
pub(crate) fn show(
    ui: &mut Ui,
    session: &Session,
    state: &mut SearchUi,
    t: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    let search = session.search();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Search").strong().color(t.sidebar_text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let close = ui
                .button("Close")
                .on_hover_text("Close the search results (Esc)");
            if close.clicked() {
                out.push(Clicked {
                    command: ids::SEARCH_CLOSE,
                    args: Value::Null,
                });
            }
        });
    });

    // A search run some other way (a command) shows its text in the box.
    if state.sent != search.query {
        state.text.clone_from(&search.query);
        state.sent.clone_from(&search.query);
    }
    let edit = egui::TextEdit::singleline(&mut state.text)
        .id_salt("search_box")
        .hint_text("Search")
        .desired_width(f32::INFINITY);
    let response = ui.add(edit);
    if state.seen_serial != search.open_serial {
        state.seen_serial = search.open_serial;
        response.request_focus();
    }
    let escape = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
    if escape {
        out.push(Clicked {
            command: ids::SEARCH_CLOSE,
            args: Value::Null,
        });
    }
    let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if (response.changed() || enter) && (state.text != state.sent || enter) {
        state.sent.clone_from(&state.text);
        out.push(command(ids::SEARCH_RUN, &[("query", state.text.as_str())]));
    }

    ui.add_space(4.0);
    if let Some(scope) = choice(
        ui,
        "search_scope",
        "Search scope",
        &SearchScope::LIST,
        search.scope,
        SearchScope::label,
    ) {
        out.push(command(ids::SEARCH_SET_SCOPE, &[("scope", scope.id())]));
    }
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Sort by").color(t.text_muted));
        if let Some(sort) = choice(
            ui,
            "search_sort",
            "Sort results by",
            &SearchSort::LIST,
            search.sort,
            SearchSort::label,
        ) {
            out.push(command(ids::SEARCH_SET_SORT, &[("sort", sort.id())]));
        }
    });
    ui.separator();

    let Some(results) = &search.results else {
        ui.label(RichText::new("Type to search page titles and text.").color(t.text_muted));
        return;
    };
    if results.query.trim().is_empty() {
        ui.label(RichText::new("Type to search page titles and text.").color(t.text_muted));
        return;
    }
    let count = results.hits.len();
    let summary = match (count, results.truncated) {
        (0, _) => "No results.".to_owned(),
        (1, _) => "1 result".to_owned(),
        (n, true) => format!("First {n} results"),
        (n, false) => format!("{n} results"),
    };
    ui.label(RichText::new(summary).color(t.text_muted));
    let multi = session.notebooks().len() > 1 && results.scope == SearchScope::All;
    egui::ScrollArea::vertical()
        .id_salt("search_results")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for hit in &results.hits {
                hit_row(ui, hit, multi, t, out);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_utc_calendar_days() {
        assert_eq!(date_text(0), "1970-01-01");
        assert_eq!(date_text(86_400_000 * 365), "1971-01-01");
        // 2024-02-29 12:00 UTC
        assert_eq!(date_text(1_709_208_000_000), "2024-02-29");
        assert_eq!(date_text(-1), "1969-12-31");
    }
}
