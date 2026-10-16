//! The page list: the current section's pages, subpages indented, with Add Page on top.

use egui::{RichText, Ui};
use notecraft_engine::cmd::ids;
use notecraft_engine::Session;

use super::{plain, select, Row};
use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

pub(crate) fn show(ui: &mut Ui, session: &Session, t: &ThemeTokens, out: &mut Vec<Clicked>) {
    ui.add_space(6.0);
    let (add, _) = Row {
        label: "Add Page",
        item: false,
        selected: false,
        level: 0,
        swatch: None,
        arrow: None,
        muted: false,
    }
    .show(ui, t);
    if add
        .on_hover_text("Add a page to this section (Ctrl+N)")
        .clicked()
    {
        out.push(plain(ids::PAGE_NEW));
    }
    ui.separator();

    let Some(section) = session.current_section() else {
        ui.label(RichText::new("No section is open.").color(t.text_muted));
        return;
    };
    if section.pages.is_empty() {
        ui.label(RichText::new("This section is empty.").color(t.text_muted));
        return;
    }
    let current = session.selection().page.as_deref();
    egui::ScrollArea::vertical()
        .id_salt("page_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for page in &section.pages {
                let (response, _) = Row {
                    label: page.display_title(),
                    selected: current == Some(page.id.as_str()),
                    level: usize::from(page.level),
                    item: true,
                    swatch: None,
                    arrow: None,
                    muted: false,
                }
                .show(ui, t);
                if response.clicked() {
                    out.push(select(ids::NAV_PAGE_SELECT, &page.id));
                }
            }
        });
}
