//! Navigation: the notebook pane, the section tabs, and the page list. Each one reads the
//! session and returns commands (`nav.*.select`, `section.new`, `page.new`) for the shell to
//! dispatch; none of them changes the session directly.

mod edit;
mod notebooks;
mod pages;
mod sections;

use std::collections::BTreeSet;

use egui::{Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, Ui};
use notecraft_engine::cmd::{CommandId, Map, Value};

use crate::ribbon::Clicked;
use crate::themes::{hex, mix, Preset, ThemeTokens};

pub(crate) use notebooks::show as notebook_pane;
pub(crate) use pages::show as page_list;
pub(crate) use sections::{show as section_tabs, STRIP_HEIGHT as SECTION_STRIP_HEIGHT};

/// Height of a row in the notebook pane and the page list.
pub(crate) const ROW_HEIGHT: f32 = 26.0;
/// Indent per level, in points.
pub(crate) const INDENT: f32 = 16.0;

/// Where the page list goes. The 2016 and 2019 presets put it right of the canvas; 2021 and
/// Microsoft 365 put it on the left, next to the notebook pane (build_plan/02, Layout
/// target). This is the only layout choice a preset makes.
pub fn pages_on_left(preset: Preset) -> bool {
    matches!(preset, Preset::OneNote2021 | Preset::M365)
}

/// Navigation state that is not session state: which notebooks and groups are expanded.
#[derive(Debug, Default)]
pub struct NavUi {
    /// Ids whose expanded state the user flipped from the default (the current notebook
    /// and every section group start expanded).
    toggled: BTreeSet<String>,
    /// The name being edited in place, if any (see `edit`).
    renaming: Option<edit::Renaming>,
}

impl NavUi {
    pub(crate) fn expanded(&self, id: &str, default: bool) -> bool {
        default != self.toggled.contains(id)
    }

    pub(crate) fn toggle(&mut self, id: &str) {
        if !self.toggled.remove(id) {
            self.toggled.insert(id.to_owned());
        }
    }
}

/// A command that selects `id`.
pub(crate) fn select(command: CommandId, id: &str) -> Clicked {
    let mut args = Map::new();
    args.insert("id".to_owned(), Value::from(id));
    Clicked {
        command,
        args: Value::Object(args),
    }
}

/// A command with no arguments.
pub(crate) fn plain(command: CommandId) -> Clicked {
    Clicked {
        command,
        args: Value::Null,
    }
}

/// The color of a section or notebook swatch, softened in dark mode so it does not glare.
pub(crate) fn swatch_color(rgb: u32, t: &ThemeTokens) -> Color32 {
    let color = hex(rgb);
    if t.dark {
        mix(color, t.canvas_bg, 0.45)
    } else {
        color
    }
}

/// One clickable row: an optional disclosure arrow, an optional color swatch, and a label.
pub(crate) struct Row<'a> {
    pub label: &'a str,
    pub selected: bool,
    /// Indent level.
    pub level: usize,
    pub swatch: Option<Color32>,
    /// Some(expanded) draws a disclosure arrow.
    pub arrow: Option<bool>,
    pub muted: bool,
    /// A notebook, section, or page a screen reader selects, rather than a command button.
    pub item: bool,
}

impl Row<'_> {
    /// Draw the row across the available width. Returns its response and whether the
    /// arrow (if any) was the part clicked.
    pub(crate) fn show(self, ui: &mut Ui, t: &ThemeTokens) -> (Response, bool) {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ROW_HEIGHT),
            Sense::click_and_drag(),
        );
        let label = self.label;
        let selected = self.selected;
        if self.item {
            crate::a11y::selectable_row(&response, label, selected);
        } else {
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        }

        let painter = ui.painter_at(rect);
        let fill = if self.selected {
            t.sidebar_selected
        } else if response.hovered() {
            mix(t.sidebar_bg, t.sidebar_text, 0.06)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, CornerRadius::same(t.radius), fill);

        let mut x = rect.min.x + 6.0 + INDENT * self.level as f32;
        let mid = rect.center().y;
        let arrow_rect = Rect::from_min_size(
            egui::pos2(x - 2.0, rect.min.y),
            egui::vec2(16.0, ROW_HEIGHT),
        );
        if let Some(expanded) = self.arrow {
            let c = egui::pos2(x + 5.0, mid);
            let points = if expanded {
                vec![
                    c + egui::vec2(-3.5, -1.5),
                    c + egui::vec2(0.0, 2.0),
                    c + egui::vec2(3.5, -1.5),
                ]
            } else {
                vec![
                    c + egui::vec2(-1.5, -3.5),
                    c + egui::vec2(2.0, 0.0),
                    c + egui::vec2(-1.5, 3.5),
                ]
            };
            painter.add(egui::Shape::line(points, Stroke::new(1.2, t.sidebar_text)));
            x += 14.0;
        }
        if let Some(color) = self.swatch {
            let swatch = Rect::from_min_size(egui::pos2(x, mid - 6.0), egui::vec2(12.0, 12.0));
            painter.rect_filled(swatch, CornerRadius::same(2), color);
            x += 18.0;
        }
        let color = if self.muted {
            t.text_muted
        } else {
            t.sidebar_text
        };
        painter.text(
            egui::pos2(x, mid),
            egui::Align2::LEFT_CENTER,
            self.label,
            FontId::proportional(13.0),
            color,
        );
        if response.has_focus() {
            painter.rect_stroke(
                rect.shrink(1.0),
                CornerRadius::same(t.radius),
                Stroke::new(1.5, t.accent),
                egui::StrokeKind::Inside,
            );
        }
        let on_arrow = self.arrow.is_some()
            && response
                .interact_pointer_pos()
                .is_some_and(|pos| arrow_rect.contains(pos));
        (response, on_arrow)
    }
}
