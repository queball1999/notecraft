//! The tab strip above the ribbon body.

use craft_ui::tabs::Tab;
use egui::{Color32, CornerRadius, FontId, Rect, Stroke, Ui};

use super::spec::{TabKind, TABS};
use super::{Look, Ribbon};
use crate::themes::contrast::{contrast_ratio, AA_NORMAL_TEXT};
use crate::themes::mix;

const TAB_PADDING: f32 = 12.0;

pub(super) fn show_strip(ui: &mut Ui, strip: Rect, ribbon: &mut Ribbon, look: Look<'_>) {
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
        let selected = !is_file && index == ribbon.selected;
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
        if response.clicked() {
            if is_file {
                ribbon.open_backstage();
            } else {
                ribbon.selected = index;
            }
        }
    }
}
