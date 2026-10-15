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
