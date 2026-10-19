//! The page list: the current section's pages, subpages indented, with Add Page on top. Rows
//! have a context menu, rename in place (double-click or the menu), and drag to reorder.

use egui::{RichText, Ui};
use notecraft_engine::cmd::ids;
use notecraft_engine::Session;

use super::edit::{self, DragPage};
use super::{plain, select, NavUi, Row, ROW_HEIGHT};
use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

pub(crate) fn show(
    ui: &mut Ui,
    session: &Session,
    nav: &mut NavUi,
    t: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
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
            // Rows below a collapsed parent are skipped until the level comes back up.
            let mut hide_below: Option<u8> = None;
            for (index, page) in section.pages.iter().enumerate() {
                if let Some(level) = hide_below {
                    if page.level > level {
                        continue;
                    }
                    hide_below = None;
                }
                let has_subpages = section
                    .pages
                    .get(index + 1)
                    .is_some_and(|next| next.level > page.level);
                if has_subpages && page.collapsed {
                    hide_below = Some(page.level);
                }
                if nav.is_renaming(&page.id) {
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), ROW_HEIGHT),
                        egui::Sense::hover(),
                    );
                    let indent = super::INDENT * f32::from(page.level) + 6.0;
                    let field = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x + indent, rect.min.y),
                        rect.max,
                    );
                    out.extend(edit::rename_field(ui, nav, field));
                    continue;
                }
                let (response, on_arrow) = Row {
                    label: page.display_title(),
                    selected: current == Some(page.id.as_str()),
                    level: usize::from(page.level),
                    item: true,
                    swatch: None,
                    arrow: has_subpages.then_some(!page.collapsed),
                    muted: false,
                }
                .show(ui, t);
                if response.clicked() && on_arrow {
                    out.push(edit::command(
                        if page.collapsed {
                            ids::PAGE_EXPAND_SUBPAGES
                        } else {
                            ids::PAGE_COLLAPSE_SUBPAGES
                        },
                        &[("id", page.id.as_str().into())],
                    ));
                } else if response.clicked() {
                    out.push(select(ids::NAV_PAGE_SELECT, &page.id));
                }
                if response.double_clicked() && !on_arrow {
                    nav.begin_rename(&page.id, edit::Kind::Page, &page.title);
                }
                response.context_menu(|ui| edit::page_menu(ui, page, section, index, nav, out));
                response.dnd_set_drag_payload(DragPage(page.id.clone()));
                if response.dnd_hover_payload::<DragPage>().is_some() {
                    let below = edit::pointer_below(ui, response.rect);
                    edit::drop_line(ui, response.rect, below, t);
                    if let Some(dropped) = response.dnd_release_payload::<DragPage>() {
                        let from = section.pages.iter().position(|p| p.id == dropped.0);
                        let at = edit::drop_index(from, index, below);
                        out.push(edit::move_page(&dropped.0, &section.id, Some(at)));
                    }
                }
            }
        });
}
