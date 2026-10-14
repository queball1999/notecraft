//! The notebook pane: every open notebook with its color swatch, expandable to show its
//! sections and section groups.

use egui::{RichText, Ui};
use notecraft_engine::cmd::ids;
use notecraft_engine::Session;
use notecraft_model::section::MAX_GROUP_DEPTH;
use notecraft_model::SectionNode;

use super::{select, swatch_color, NavUi, Row};
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
                let (response, on_arrow) = Row {
                    label: &notebook.name,
                    selected: false,
                    level: 0,
                    swatch: Some(swatch_color(notebook.color, t)),
                    arrow: Some(expanded),
                    muted: false,
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
                if expanded {
                    nodes(ui, &notebook.sections, 1, session, nav, t, out);
                }
            }
        });
}

/// Section and group rows under a notebook, indented by `level`.
fn nodes(
    ui: &mut Ui,
    nodes: &[SectionNode],
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
    for node in nodes {
        match node {
            SectionNode::Section(section) => {
                let (response, _) = Row {
                    label: &section.name,
                    selected: current == Some(section.id.as_str()),
                    level,
                    swatch: Some(swatch_color(section.color, t)),
                    arrow: None,
                    muted: false,
                }
                .show(ui, t);
                if response.clicked() {
                    out.push(select(ids::NAV_SECTION_SELECT, &section.id));
                }
            }
            SectionNode::Group(group) => {
                let expanded = nav.expanded(&group.id, true);
                let (response, _) = Row {
                    label: &group.name,
                    selected: false,
                    level,
                    swatch: None,
                    arrow: Some(expanded),
                    muted: true,
                }
                .show(ui, t);
                if response.clicked() {
                    nav.toggle(&group.id);
                }
                if expanded {
                    self::nodes(ui, &group.children, level + 1, session, nav, t, out);
                }
            }
        }
    }
}
