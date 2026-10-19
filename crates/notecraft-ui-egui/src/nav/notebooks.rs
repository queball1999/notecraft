//! The notebook pane: every open notebook with its color swatch, expandable to show its
//! sections and section groups. Rows have a context menu, rename in place (double-click or
//! the menu), and drag and drop: sections and groups reorder and move into groups, and pages
//! dropped on a section move there.

use egui::{Rect, RichText, Ui};
use notecraft_engine::cmd::ids;
use notecraft_engine::Session;
use notecraft_model::section::MAX_GROUP_DEPTH;
use notecraft_model::SectionNode;

use super::edit::{self, DragPage, DragSection};
use super::{select, swatch_color, NavUi, Row, ROW_HEIGHT};
use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// A rename field in place of a row.
fn rename_row(ui: &mut Ui, nav: &mut NavUi, level: usize, out: &mut Vec<Clicked>) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::hover(),
    );
    let indent = super::INDENT * level as f32 + 24.0;
    let field = Rect::from_min_max(egui::pos2(rect.min.x + indent, rect.min.y), rect.max);
    out.extend(edit::rename_field(ui, nav, field));
}

pub(crate) fn show(
    ui: &mut Ui,
    session: &Session,
    nav: &mut NavUi,
    t: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    ui.add_space(6.0);
    ui.label(RichText::new("Notebooks").size(12.0).color(t.text_muted));
    ui.add_space(2.0);
    let current = session.selection();
    egui::ScrollArea::vertical()
        .id_salt("notebook_pane")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for notebook in session.notebooks() {
                let is_current = current.notebook.as_deref() == Some(notebook.id.as_str());
                let expanded = nav.expanded(&notebook.id, is_current);
                if nav.is_renaming(&notebook.id) {
                    rename_row(ui, nav, 0, out);
                } else {
                    let (response, on_arrow) = Row {
                        label: &notebook.name,
                        selected: false,
                        level: 0,
                        swatch: Some(swatch_color(notebook.color, t)),
                        arrow: Some(expanded),
                        muted: false,
                        item: true,
                    }
                    .show(ui, t);
                    if response.clicked() {
                        if on_arrow {
                            nav.toggle(&notebook.id);
                        } else {
                            if !expanded {
                                nav.toggle(&notebook.id);
                            }
                            out.push(select(ids::NAV_NOTEBOOK_SELECT, &notebook.id));
                        }
                    }
                    if response.double_clicked() && !on_arrow {
                        nav.begin_rename(&notebook.id, edit::Kind::Notebook, &notebook.name);
                    }
                    response.context_menu(|ui| {
                        edit::notebook_menu(ui, &notebook.id, &notebook.name, nav, out)
                    });
                    // A section or group dropped on the notebook goes last at the top level.
                    if response.dnd_hover_payload::<DragSection>().is_some() {
                        edit::drop_outline(ui, response.rect, t);
                        if let Some(dropped) = response.dnd_release_payload::<DragSection>() {
                            out.push(edit::move_section(&dropped.0, None, None));
                        }
                    }
                }
                if expanded {
                    nodes(ui, &notebook.sections, None, 1, session, nav, t, out);
                }
            }
        });
}

/// Section and group rows under a notebook or group `parent`, indented by `level`.
#[allow(clippy::too_many_arguments)]
fn nodes(
    ui: &mut Ui,
    nodes: &[SectionNode],
    parent: Option<&str>,
    level: usize,
    session: &Session,
    nav: &mut NavUi,
    t: &ThemeTokens,
    out: &mut Vec<Clicked>,
) {
    if level > MAX_GROUP_DEPTH {
        return;
    }
    let current = session.selection().section.as_deref();
    let position = |id: &str| {
        nodes.iter().position(|n| match n {
            SectionNode::Section(s) => s.id == id,
            SectionNode::Group(g) => g.id == id,
        })
    };
    for (index, node) in nodes.iter().enumerate() {
        match node {
            SectionNode::Section(section) => {
                if nav.is_renaming(&section.id) {
                    rename_row(ui, nav, level, out);
                    continue;
                }
                let (response, _) = Row {
                    label: &section.name,
                    selected: current == Some(section.id.as_str()),
                    level,
                    swatch: Some(swatch_color(section.color, t)),
                    arrow: None,
                    muted: false,
                    item: true,
                }
                .show(ui, t);
                if response.clicked() {
                    out.push(select(ids::NAV_SECTION_SELECT, &section.id));
                }
                if response.double_clicked() {
                    nav.begin_rename(&section.id, edit::Kind::Section, &section.name);
                }
                response.context_menu(|ui| {
                    edit::section_menu(ui, &section.id, &section.name, false, nav, out)
                });
                response.dnd_set_drag_payload(DragSection(section.id.clone()));
                if let Some(page) = response.dnd_hover_payload::<DragPage>() {
                    let _ = page;
                    edit::drop_outline(ui, response.rect, t);
                    if let Some(dropped) = response.dnd_release_payload::<DragPage>() {
                        out.push(edit::move_page(&dropped.0, &section.id, None));
                    }
                }
                if response.dnd_hover_payload::<DragSection>().is_some() {
                    let below = edit::pointer_below(ui, response.rect);
                    edit::drop_line(ui, response.rect, below, t);
                    if let Some(dropped) = response.dnd_release_payload::<DragSection>() {
                        let at = edit::drop_index(position(&dropped.0), index, below);
                        out.push(edit::move_section(&dropped.0, parent, Some(at)));
                    }
                }
            }
            SectionNode::Group(group) => {
                let expanded = nav.expanded(&group.id, true);
                if nav.is_renaming(&group.id) {
                    rename_row(ui, nav, level, out);
                } else {
                    let (response, on_arrow) = Row {
                        label: &group.name,
                        selected: false,
                        level,
                        swatch: None,
                        arrow: Some(expanded),
                        muted: true,
                        item: true,
                    }
                    .show(ui, t);
                    if response.clicked() {
                        nav.toggle(&group.id);
                    }
                    if response.double_clicked() && !on_arrow {
                        nav.begin_rename(&group.id, edit::Kind::Section, &group.name);
                    }
                    response.context_menu(|ui| {
                        edit::section_menu(ui, &group.id, &group.name, true, nav, out)
                    });
                    response.dnd_set_drag_payload(DragSection(group.id.clone()));
                    // Dropping a section or group on a group puts it inside, last.
                    if response.dnd_hover_payload::<DragSection>().is_some() {
                        edit::drop_outline(ui, response.rect, t);
                        if let Some(dropped) = response.dnd_release_payload::<DragSection>() {
                            out.push(edit::move_section(&dropped.0, Some(&group.id), None));
                        }
                    }
                }
                if expanded {
                    self::nodes(
                        ui,
                        &group.children,
                        Some(&group.id),
                        level + 1,
                        session,
                        nav,
                        t,
                        out,
                    );
                }
            }
        }
    }
}
