//! The tab strip above the ribbon body.

use craft_ui::tabs::Tab;
use egui::{Color32, CornerRadius, FontId, Rect, Sense, Stroke, Ui};
use notecraft_engine::cmd::{ids, Value};

use super::spec::{TabKind, TABS};
use super::{keytips, Clicked, Look};
use crate::icons::{self, Icon, IconColors};
use crate::themes::contrast::{contrast_ratio, AA_NORMAL_TEXT};
use crate::themes::mix;

const TAB_PADDING: f32 = 12.0;

/// A click on a tab.
pub(super) struct TabEvent {
    pub(super) index: usize,
    pub(super) file: bool,
    pub(super) double_clicked: bool,
}

/// Width and height of a quick access button.
const QUICK_BUTTON: egui::Vec2 = egui::vec2(24.0, 20.0);

/// Draw the quick access buttons (Undo and Redo) at the right end of the tab strip. A button
/// is disabled, and does not click, while its stack is empty. Returns the commands clicked.
pub(super) fn show_quick_access(ui: &mut Ui, strip: Rect, look: Look<'_>) -> Vec<Clicked> {
    let t = look.tokens;
    let colored_strip = contrast_ratio(t.ribbon_text, t.ribbon_tab_bg) < AA_NORMAL_TEXT;
    let text = if colored_strip {
        t.accent_text
    } else {
        t.ribbon_text
    };
    let buttons = [
        (ids::EDIT_UNDO, Icon::Undo, look.can_undo),
        (ids::EDIT_REDO, Icon::Redo, look.can_redo),
    ];
    let mut out = Vec::new();
    // Keep clear of the Ribbon Display Options chevron's column and the window edge.
    let mut x = strip.max.x - 8.0 - QUICK_BUTTON.x * buttons.len() as f32;
    for (command, icon, enabled) in buttons {
        let rect = Rect::from_min_size(
            egui::pos2(x, strip.center().y - QUICK_BUTTON.y / 2.0 + 1.0),
            QUICK_BUTTON,
        );
        x += QUICK_BUTTON.x;
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let id = ui.id().with(("quick_access", command.as_str()));
        let response = ui.interact(rect, id, sense);
        let spec = look.registry.get(command.as_str());
        let label = spec.map_or(command.as_str(), |spec| spec.label);
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
        let color = if enabled {
            text
        } else {
            text.gamma_multiply(0.4)
        };
        if enabled && (response.hovered() || response.has_focus()) {
            let fill = mix(t.ribbon_tab_bg, text, 0.12);
            ui.painter().rect_filled(rect, t.radius, fill);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect,
                t.radius,
                Stroke::new(1.5, t.accent),
                egui::StrokeKind::Inside,
            );
        }
        let colors = IconColors {
            fg: color,
            accent: color,
        };
        icons::paint(
            ui.painter(),
            rect.shrink2(egui::vec2(4.0, 2.0)),
            icon,
            colors,
        );
        let mut tip = spec.map_or_else(String::new, |spec| spec.description.to_owned());
        if let Some(shortcut) = spec.and_then(|spec| spec.default_shortcut) {
            tip.push_str(&format!(" ({shortcut})"));
        }
        if !enabled {
            tip.push_str("\nNothing to do");
        }
        let response = response.on_hover_text(format!("{label}\n{tip}"));
        if enabled && response.clicked() {
            out.push(Clicked {
                command,
                args: Value::Null,
            });
        }
    }
    out
}

/// Draw the tab strip. `selected` is the tab drawn as open, if any; `tips` shows each tab's
/// key tip letter.
pub(super) fn show_strip(
    ui: &mut Ui,
    strip: Rect,
    selected: Option<usize>,
    tips: bool,
    look: Look<'_>,
) -> Option<TabEvent> {
    let t = look.tokens;
    ui.painter().rect_filled(strip, 0.0, t.ribbon_tab_bg);
    // On a colored strip (OneNote 2016 and 2019) the tab names use the accent text color.
    let colored_strip = contrast_ratio(t.ribbon_text, t.ribbon_tab_bg) < AA_NORMAL_TEXT;
    let strip_text = if colored_strip {
        t.accent_text
    } else {
        t.ribbon_text
    };
    let radius = t.radius;
    let top_corners = CornerRadius {
        nw: radius,
        ne: radius,
        sw: 0,
        se: 0,
    };

    let mut event = None;
    let mut x = strip.min.x + 4.0;
    for (index, tab) in TABS.iter().enumerate() {
        let galley = ui.painter().layout_no_wrap(
            tab.name.to_owned(),
            FontId::proportional(13.0),
            Color32::PLACEHOLDER,
        );
        let width = galley.size().x + 2.0 * TAB_PADDING;
        let rect = Rect::from_min_size(
            egui::pos2(x, strip.min.y + 3.0),
            egui::vec2(width, strip.height() - 3.0),
        );
        x += width + 2.0;

        let is_file = matches!(tab.kind, TabKind::Backstage(_));
        let selected = !is_file && selected == Some(index);
        let id = ui.id().with(("ribbon_tab", index));
        let response = Tab::new(id, tab.name, selected)
            .focus_stroke(Stroke::new(1.5, t.accent))
            .show_at(ui, rect, |ui, response| {
                let (fill, text) = if is_file {
                    (t.accent, t.accent_text)
                } else if selected {
                    (t.ribbon_bg, t.ribbon_text)
                } else if response.hovered() {
                    (mix(t.ribbon_tab_bg, strip_text, 0.12), strip_text)
                } else {
                    (Color32::TRANSPARENT, strip_text)
                };
                let painter = ui.painter();
                painter.rect_filled(rect, top_corners, fill);
                let size = galley.size();
                let pos = rect.center() - size / 2.0;
                painter.galley(pos, galley, text);
                if selected && !colored_strip {
                    let y = rect.max.y - 1.5;
                    let half = (size.x / 2.0).min(rect.width() / 2.0);
                    painter.hline(
                        (rect.center().x - half)..=(rect.center().x + half),
                        y,
                        Stroke::new(2.0, t.accent),
                    );
                }
            });
        if tips {
            keytips::paint_tip(ui, rect.center_bottom(), tab.key, look);
        }
        if response.double_clicked() && !is_file {
            event = Some(TabEvent {
                index,
                file: false,
                double_clicked: true,
            });
        } else if response.clicked() {
            event = Some(TabEvent {
                index,
                file: is_file,
                double_clicked: false,
            });
        }
    }
    event
}
