//! Page links on the canvas: the pointer cursor and tooltip over a link, and a click that
//! follows it (build_plan/04 PR 2.2). A click sends `link.follow`, which shows the page the
//! link points to, or adds the page to the current section when there is none, as OneNote
//! does. Following works on every page, read-only ones too; only the Type tool follows (the
//! ink tools draw instead). Shift+click is left to the text, so a selection can still be
//! extended over a link, and a click in the block that holds the caret edits its raw text.

use egui::{CursorIcon, Response};
use notecraft_engine::cmd::{ids, Map, Value};
use notecraft_engine::{DrawTool, Session};

use super::flow::{Flow, LinkStatus};
use super::PageToScreen;
use crate::ribbon::Clicked;

/// Handle the pointer over page links for one frame. Returns whether a click followed a link,
/// so the text editing leaves that click alone.
pub(super) fn interact(
    ui: &egui::Ui,
    response: &Response,
    session: &Session,
    flow: &Flow,
    map: PageToScreen,
    clicked: &mut Vec<Clicked>,
) -> bool {
    let panning = !ui.ctx().text_edit_focused() && ui.input(|i| i.key_down(egui::Key::Space));
    if session.ink().tool() != DrawTool::Type || panning {
        return false;
    }
    let to_page = |at: egui::Pos2| {
        let page = map.page(at);
        egui::pos2(page.x, page.y)
    };
    let hover = ui.input(|input| input.pointer.hover_pos());
    if let Some(link) = hover
        .filter(|_| response.hovered())
        .and_then(|at| flow.link_at(to_page(at)))
    {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        tip(response, &link.target, &link.status);
    }
    let shift = ui.input(|input| input.modifiers.shift);
    if !response.clicked() || shift {
        return false;
    }
    let Some(link) = response
        .interact_pointer_pos()
        .and_then(|at| flow.link_at(to_page(at)))
    else {
        return false;
    };
    clicked.push(follow(&link.target));
    true
}

/// The `link.follow` command for a link target.
pub(super) fn follow(target: &str) -> Clicked {
    let mut args = Map::new();
    args.insert("target".to_owned(), Value::from(target));
    Clicked {
        command: ids::LINK_FOLLOW,
        args: Value::Object(args),
    }
}

/// The hover text of a link that needs one: which pages an ambiguous link could mean, or that
/// a missing page is created by clicking.
fn tip(response: &Response, target: &str, status: &LinkStatus) {
    match status {
        LinkStatus::Found => {}
        LinkStatus::Ambiguous(candidates) => {
            response.clone().on_hover_ui_at_pointer(|ui| {
                ui.label("More than one page matches. Opens the closest:");
                for (index, label) in candidates.iter().enumerate() {
                    let text = if index == 0 {
                        egui::RichText::new(label).strong()
                    } else {
                        egui::RichText::new(label)
                    };
                    ui.label(text);
                }
            });
        }
        LinkStatus::Missing => {
            response.clone().on_hover_ui_at_pointer(|ui| {
                ui.label(format!("No page named \"{target}\". Click to create it."));
            });
        }
    }
}

/// Where the first link of `target` on the page is, in page units (tests).
#[cfg(test)]
pub(super) fn first_link_point(flow: &Flow, target: &str) -> Option<notecraft_core::Vec2> {
    flow.units
        .iter()
        .flat_map(|unit| unit.links.iter())
        .find(|link| link.target == target)
        .and_then(|link| link.rects.first())
        .map(|rect| notecraft_core::Vec2::new(rect.center().x, rect.center().y))
}
