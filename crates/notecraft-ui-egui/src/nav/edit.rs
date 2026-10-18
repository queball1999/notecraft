//! Editing the structure from the navigation: inline rename, context menus, and drag and
//! drop. Nothing here changes the session. Every gesture ends as a command with a stable id
//! (`page.rename`, `page.move`, `section.move`, `section.set_color`, ...) that the shell
//! dispatches, so the same edits are reachable from the control channel and from tests.

use egui::{Key, Rect, TextEdit, Ui};
use notecraft_engine::cmd::{ids, CommandId, Map, Value};
use notecraft_model::{PageRef, Section};

use super::NavUi;
use crate::ribbon::Clicked;
use crate::themes::ThemeTokens;

/// What a rename applies to; it picks the command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Page,
    /// A section or a section group.
    Section,
    Notebook,
}

impl Kind {
    fn command(self) -> CommandId {
        match self {
            Kind::Page => ids::PAGE_RENAME,
            Kind::Section => ids::SECTION_RENAME,
            Kind::Notebook => ids::NOTEBOOK_RENAME,
        }
    }
}

/// A name being edited in place.
#[derive(Clone, Debug)]
pub(crate) struct Renaming {
    id: String,
    kind: Kind,
    original: String,
    text: String,
    /// The field has been given focus.
    focused: bool,
}

/// A page being dragged (the payload of a drag from the page list).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DragPage(pub String);

/// A section or section group being dragged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DragSection(pub String);

impl NavUi {
    /// Start renaming `id` in place, with `current` as the starting text.
    pub(crate) fn begin_rename(&mut self, id: &str, kind: Kind, current: &str) {
        self.renaming = Some(Renaming {
            id: id.to_owned(),
            kind,
            original: current.to_owned(),
            text: current.to_owned(),
            focused: false,
        });
    }

    /// Whether `id` is the thing being renamed.
    pub(crate) fn is_renaming(&self, id: &str) -> bool {
        self.renaming.as_ref().is_some_and(|r| r.id == id)
    }
}

/// A command with `id` and any further string arguments.
pub(crate) fn command(command: CommandId, args: &[(&str, Value)]) -> Clicked {
    let mut map = Map::new();
    for (key, value) in args {
        map.insert((*key).to_owned(), value.clone());
    }
    Clicked {
        command,
        args: Value::Object(map),
    }
}

/// The rename text field, drawn at `rect`. Enter or leaving the field commits (an unchanged or
/// empty name just ends the edit); Esc cancels. Returns the rename command when committed.
pub(crate) fn rename_field(ui: &mut Ui, nav: &mut NavUi, rect: Rect) -> Option<Clicked> {
    let renaming = nav.renaming.as_mut()?;
    let response = ui.put(
        rect,
        TextEdit::singleline(&mut renaming.text).desired_width(rect.width()),
    );
    if !renaming.focused {
        response.request_focus();
        renaming.focused = true;
        return None;
    }
    if ui.input(|i| i.key_pressed(Key::Escape)) {
        nav.renaming = None;
        return None;
    }
    let enter = response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    if !(enter || response.lost_focus()) {
        return None;
    }
    let done = nav.renaming.take()?;
    let name = done.text.trim();
    if name.is_empty() || name == done.original {
        return None;
    }
    Some(command(
        done.kind.command(),
        &[("id", done.id.as_str().into()), ("name", name.into())],
    ))
}

/// Section and notebook colors offered in the Color menus.
pub(crate) const COLOR_CHOICES: [(&str, u32); 8] = [
    ("Orange", 0xF6C28B),
    ("Blue", 0x9FD3E8),
    ("Purple", 0xC9B6E4),
    ("Green", 0xB5DCA6),
    ("Pink", 0xF3A9B8),
    ("Yellow", 0xE8D87A),
    ("Teal", 0x7CC4B8),
    ("Gray", 0xB8BEC6),
];

fn color_menu(ui: &mut Ui, label: &str, set: CommandId, id: &str, out: &mut Vec<Clicked>) {
    ui.menu_button(label, |ui| {
        for (name, rgb) in COLOR_CHOICES {
            if ui.button(name).clicked() {
                let color = format!("#{rgb:06X}");
                out.push(command(
                    set,
                    &[("id", id.into()), ("color", color.as_str().into())],
                ));
                ui.close();
            }
        }
    });
}

/// The context menu of a notebook row.
pub(crate) fn notebook_menu(
    ui: &mut Ui,
    id: &str,
    name: &str,
    nav: &mut NavUi,
    out: &mut Vec<Clicked>,
) {
    if ui.button("New Section").clicked() {
        out.push(command(ids::SECTION_NEW, &[("notebook", id.into())]));
        ui.close();
    }
    if ui.button("New Section Group").clicked() {
        out.push(command(ids::SECTION_GROUP_NEW, &[("notebook", id.into())]));
        ui.close();
    }
    if ui.button("Rename").clicked() {
        nav.begin_rename(id, Kind::Notebook, name);
        ui.close();
    }
    color_menu(ui, "Notebook Color", ids::NOTEBOOK_SET_COLOR, id, out);
    if ui.button("Close Notebook").clicked() {
        out.push(command(ids::FILE_CLOSE_NOTEBOOK, &[("id", id.into())]));
        ui.close();
    }
}

/// The context menu of a section or a section group.
pub(crate) fn section_menu(
    ui: &mut Ui,
    id: &str,
    name: &str,
    group: bool,
    nav: &mut NavUi,
    out: &mut Vec<Clicked>,
) {
    if group {
        if ui.button("New Section").clicked() {
            out.push(command(ids::SECTION_NEW, &[("parent", id.into())]));
            ui.close();
        }
        if ui.button("New Section Group").clicked() {
            out.push(command(ids::SECTION_GROUP_NEW, &[("parent", id.into())]));
            ui.close();
        }
    } else if ui.button("New Page").clicked() {
        out.push(command(ids::PAGE_NEW, &[("section", id.into())]));
        ui.close();
    }
    if ui.button("Rename").clicked() {
        nav.begin_rename(id, Kind::Section, name);
        ui.close();
    }
    color_menu(ui, "Section Color", ids::SECTION_SET_COLOR, id, out);
    if ui.button("Move to Top Level").clicked() {
        out.push(command(ids::SECTION_MOVE, &[("id", id.into())]));
        ui.close();
    }
    let delete = if group {
        "Delete Section Group"
    } else {
        "Delete Section"
    };
    if ui.button(delete).clicked() {
        out.push(command(ids::SECTION_DELETE, &[("id", id.into())]));
        ui.close();
    }
}

/// The context menu of a page in `section` at `index`. Move Up and Move Down do what a drag
/// does, for the keyboard.
pub(crate) fn page_menu(
    ui: &mut Ui,
    page: &PageRef,
    section: &Section,
    index: usize,
    nav: &mut NavUi,
    out: &mut Vec<Clicked>,
) {
    if ui.button("Rename").clicked() {
        nav.begin_rename(&page.id, Kind::Page, &page.title);
        ui.close();
    }
    let id: Value = page.id.as_str().into();
    if ui
        .add_enabled(index > 0, egui::Button::new("Move Up"))
        .clicked()
    {
        out.push(command(
            ids::PAGE_MOVE,
            &[("id", id.clone()), ("index", (index - 1).into())],
        ));
        ui.close();
    }
    let last = index + 1 >= section.pages.len();
    if ui
        .add_enabled(!last, egui::Button::new("Move Down"))
        .clicked()
    {
        out.push(command(
            ids::PAGE_MOVE,
            &[("id", id.clone()), ("index", (index + 1).into())],
        ));
        ui.close();
    }
    if ui
        .add_enabled(
            index > 0 && page.level < notecraft_model::page::MAX_PAGE_LEVEL,
            egui::Button::new("Make Subpage"),
        )
        .clicked()
    {
        out.push(command(ids::PAGE_MAKE_SUBPAGE, &[("id", id.clone())]));
        ui.close();
    }
    if ui
        .add_enabled(page.level > 0, egui::Button::new("Promote Subpage"))
        .clicked()
    {
        out.push(command(ids::PAGE_PROMOTE_SUBPAGE, &[("id", id.clone())]));
        ui.close();
    }
    let has_subpages = section
        .pages
        .get(index + 1)
        .is_some_and(|next| next.level > page.level);
    if has_subpages {
        let (label, command_id) = if page.collapsed {
            ("Expand Subpages", ids::PAGE_EXPAND_SUBPAGES)
        } else {
            ("Collapse Subpages", ids::PAGE_COLLAPSE_SUBPAGES)
        };
        if ui.button(label).clicked() {
            out.push(command(command_id, &[("id", id.clone())]));
            ui.close();
        }
    }
    if ui.button("Delete Page").clicked() {
        out.push(command(ids::PAGE_DELETE, &[("id", id)]));
        ui.close();
    }
}

/// The index to pass to a move command for dropping an item above or below row `row`.
/// `from` is the item's current index among the same siblings, if it is one of them; the
/// commands count positions with the item already taken out.
pub(crate) fn drop_index(from: Option<usize>, row: usize, below: bool) -> usize {
    let at = row + usize::from(below);
    match from {
        Some(from) if from < at => at - 1,
        _ => at,
    }
}

/// Whether the pointer is in the lower half of `rect`.
pub(crate) fn pointer_below(ui: &Ui, rect: Rect) -> bool {
    ui.input(|i| i.pointer.hover_pos())
        .is_some_and(|pos| pos.y > rect.center().y)
}

/// Draw the insertion line for a drop above or below `rect`.
pub(crate) fn drop_line(ui: &Ui, rect: Rect, below: bool, t: &ThemeTokens) {
    let y = if below { rect.max.y } else { rect.min.y };
    ui.painter()
        .hline(rect.x_range(), y, egui::Stroke::new(2.0, t.accent));
}

/// Outline `rect` as a drop target (dropping onto the item, not between items).
pub(crate) fn drop_outline(ui: &Ui, rect: Rect, t: &ThemeTokens) {
    ui.painter().rect_stroke(
        rect.shrink(1.0),
        egui::CornerRadius::same(t.radius),
        egui::Stroke::new(1.5, t.accent),
        egui::StrokeKind::Inside,
    );
}

/// `page.move` for dropping `page` into `section` at `index`.
pub(crate) fn move_page(page: &str, section: &str, index: Option<usize>) -> Clicked {
    let mut args: Vec<(&str, Value)> = vec![("id", page.into()), ("section", section.into())];
    if let Some(index) = index {
        args.push(("index", index.into()));
    }
    command(ids::PAGE_MOVE, &args)
}

/// `section.move` for dropping `id` into group `parent` (the top level when `None`).
pub(crate) fn move_section(id: &str, parent: Option<&str>, index: Option<usize>) -> Clicked {
    let mut args: Vec<(&str, Value)> = vec![("id", id.into())];
    if let Some(parent) = parent {
        args.push(("parent", parent.into()));
    }
    if let Some(index) = index {
        args.push(("index", index.into()));
    }
    command(ids::SECTION_MOVE, &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drop_positions_count_with_the_item_taken_out() {
        // Items 0..4. Dragging item 1 below row 3 (the last): lands at index 3.
        assert_eq!(drop_index(Some(1), 3, true), 3);
        // Dragging item 3 above row 0: lands first.
        assert_eq!(drop_index(Some(3), 0, false), 0);
        // Dropping an item just below itself or just above itself changes nothing.
        assert_eq!(drop_index(Some(2), 2, true), 2);
        assert_eq!(drop_index(Some(2), 2, false), 2);
        // From another section the position is the row position.
        assert_eq!(drop_index(None, 2, true), 3);
        assert_eq!(drop_index(None, 0, false), 0);
    }

    #[test]
    fn drop_commands_carry_ids_and_positions() {
        let c = move_page("p", "s", Some(2));
        assert_eq!(c.command, ids::PAGE_MOVE);
        assert_eq!(c.args["id"], "p");
        assert_eq!(c.args["section"], "s");
        assert_eq!(c.args["index"], 2);
        let c = move_section("g/s", None, None);
        assert_eq!(c.command, ids::SECTION_MOVE);
        assert!(c.args.get("parent").is_none() && c.args.get("index").is_none());
    }
}
