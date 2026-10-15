//! Screen reader fixes for widgets whose accessibility node says the wrong thing.

use egui::Response;

/// Mark `response` as a plain push button. craft-ui's `IconButton` always reports an on or off
/// state, which screen readers announce as "toggle button, off" and which hides the button
/// from UI Automation's Invoke. None of NoteCraft's icon buttons hold a state yet.
pub(crate) fn plain_button(response: Response) -> Response {
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.clear_toggled());
    response
}

/// Name the scroll bars of the scroll area `area` (its `ScrollAreaOutput::id`), which egui
/// leaves unlabeled. Same approach as the canvas scroll bars: the bar for axis `d` (0
/// horizontal, 1 vertical) has the id `area.with(d)`; a bar not shown this frame is skipped.
pub(crate) fn name_scroll_bars(ctx: &egui::Context, area: egui::Id) {
    for (axis, name) in [(0_usize, "Horizontal"), (1, "Vertical")] {
        let id = area.with(axis);
        if ctx.read_response(id).is_some() {
            ctx.accesskit_node_builder(id, |node| node.set_label(name));
        }
    }
}

/// Report a navigation row as a selectable list item: "Welcome, list item, selected" rather
/// than "Welcome, toggle button, on". UI Automation then offers SelectionItem, whose Select
/// AccessKit sends as a click. Only item roles get that pattern; a button with a selected
/// state gets no pattern at all.
pub(crate) fn selectable_row(response: &Response, label: &str, selected: bool) {
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    response.ctx.accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::ListItem);
        node.set_selected(selected);
    });
}
