//! Draw tab tests with the egui_kittest harness: the groups, the tool list and its menu
//! (pick up, thickness, color, add, delete), the Eyedropper, and Format Background. The
//! harness clicks the real widgets; nothing touches a live window.

use egui_kittest::kittest::Queryable;
use serde_json::json;

use crate::app_shell::AppShell;

type Harness = egui_kittest::Harness<'static, AppShell>;

fn harness() -> Harness {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1400.0, 800.0))
        .build_ui_state(
            |ui, shell: &mut AppShell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
            },
            AppShell::new(),
        );
    harness.run();
    harness.get_by_label("Draw").click();
    harness.run();
    harness
}

fn click(harness: &mut Harness, label: &str) {
    harness.get_by_label(label).click();
    harness.run();
}

fn ink(harness: &Harness) -> &notecraft_engine::InkState {
    harness.state().session().ink()
}

#[test]
fn the_draw_tab_has_onenote_groups_and_buttons() {
    let harness = harness();
    for label in [
        "Select",
        "Lasso Select",
        "Eraser",
        "Black Pen",
        "Red Pen",
        "Yellow Highlighter",
        "Add",
        "Panning Hand",
        "Ruler",
        "Insert Space",
        "Format Background",
        "Shapes",
        "Automatic Shapes",
        "Ink to Text",
        "Ink to Math",
        "Ink Replay",
        "Full Page View",
        "Ink Help",
    ] {
        assert!(
            harness.query_by_label(label).is_some(),
            "{label} is missing from the Draw tab"
        );
    }
}

#[test]
fn clicking_a_tool_picks_it_up_and_clicking_it_again_opens_its_menu() {
    let mut harness = harness();
    assert_eq!(ink(&harness).selected_tool(), None);
    click(&mut harness, "Red Pen");
    assert_eq!(ink(&harness).selected_tool(), Some(1));
    assert_eq!(ink(&harness).pen().color.to_hex(), "#E81123");
    assert!(harness.query_by_label("Thickness").is_none());
    click(&mut harness, "Red Pen");
    for label in [
        "Thickness",
        "Colors",
        "More Colors...",
        "Eyedropper",
        "Delete Tool",
        "Stroke preview",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
}

#[test]
fn picking_a_thickness_changes_the_tool() {
    let mut harness = harness();
    click(&mut harness, "Black Pen");
    click(&mut harness, "Black Pen");
    click(&mut harness, "3.5 pt");
    assert_eq!(ink(&harness).pen().width_pt, 3.5);
    assert_eq!(ink(&harness).tools()[0].style.width_pt, 3.5);
    // The menu stays open for another pick, and the plus button steps up.
    click(&mut harness, "Thicker");
    assert_eq!(ink(&harness).pen().width_pt, 4.0);
    click(&mut harness, "Thinner");
    click(&mut harness, "Thinner");
    assert_eq!(ink(&harness).pen().width_pt, 3.0);
}

#[test]
fn picking_a_color_changes_the_tool_and_is_remembered() {
    let mut harness = harness();
    click(&mut harness, "Black Pen");
    click(&mut harness, "Black Pen");
    click(&mut harness, "Burnt Orange");
    assert_eq!(ink(&harness).pen().color.to_hex(), "#E36C09");
    assert_eq!(ink(&harness).tools()[0].style.color.to_hex(), "#E36C09");
    // The pen button now carries the color in its name; the menu closed.
    assert!(harness.query_by_label("Burnt Orange Pen").is_some());
    assert!(harness.query_by_label("Thickness").is_none());
    assert_eq!(
        harness.state().recent_colors().first().map(String::as_str),
        Some("#E36C09")
    );
    // Open again: the pick shows under Recent Colors.
    click(&mut harness, "Burnt Orange Pen");
    assert!(harness.query_by_label("Recent Colors").is_some());
}

#[test]
fn more_colors_opens_the_colors_dialog() {
    let mut harness = harness();
    click(&mut harness, "Green Pen");
    click(&mut harness, "Green Pen");
    click(&mut harness, "More Colors...");
    assert!(harness.query_by_label("OK").is_some());
}

#[test]
fn add_creates_a_pen_or_a_highlighter() {
    let mut harness = harness();
    assert_eq!(ink(&harness).tools().len(), 5);
    click(&mut harness, "Add");
    click(&mut harness, "Highlighter");
    assert_eq!(ink(&harness).tools().len(), 6);
    assert_eq!(ink(&harness).selected_tool(), Some(5));
    assert_eq!(ink(&harness).tool().id(), "highlighter");
    click(&mut harness, "Add");
    click(&mut harness, "Pen");
    assert_eq!(ink(&harness).tools().len(), 7);
    assert_eq!(ink(&harness).tool().id(), "pen");
}

#[test]
fn delete_tool_removes_the_tool_in_use() {
    let mut harness = harness();
    click(&mut harness, "Light Blue Pen");
    click(&mut harness, "Light Blue Pen");
    click(&mut harness, "Delete Tool");
    assert_eq!(ink(&harness).tools().len(), 4);
    assert_eq!(ink(&harness).selected_tool(), None);
    assert!(harness.query_by_label("Light Blue Pen").is_none());
}

#[test]
fn the_eyedropper_takes_the_color_of_ink_clicked_on_the_page() {
    let mut harness = harness();
    assert!(harness
        .state_mut()
        .dispatch("draw.pen", json!({ "color": "#0A7EA4", "width": 4 })));
    let points: Vec<_> = (0..=40).map(|i| json!([100 + i * 5, 100, 1.0])).collect();
    assert!(harness
        .state_mut()
        .dispatch("ink.add_stroke", json!({ "points": points })));
    harness.run();
    click(&mut harness, "Black Pen");
    click(&mut harness, "Black Pen");
    click(&mut harness, "Eyedropper");
    assert!(crate::ribbon::eyedropper_active(&harness.ctx));
    let canvas = harness.get_by_label("Canvas").rect().min;
    let target = canvas + egui::vec2(200.0, 100.0);
    harness.hover_at(target);
    harness.run();
    harness.drag_at(target);
    harness.run();
    harness.drop_at(target);
    harness.run();
    assert!(!crate::ribbon::eyedropper_active(&harness.ctx));
    assert_eq!(ink(&harness).pen().color.to_hex(), "#0A7EA4");
}

#[test]
fn the_eyedropper_takes_the_page_color_from_bare_paper() {
    let mut harness = harness();
    assert!(harness
        .state_mut()
        .dispatch("view.page_color", json!({ "color": "#F7E7B4" })));
    harness.run();
    click(&mut harness, "Black Pen");
    click(&mut harness, "Black Pen");
    click(&mut harness, "Eyedropper");
    let canvas = harness.get_by_label("Canvas").rect().min;
    let target = canvas + egui::vec2(900.0, 500.0);
    harness.hover_at(target);
    harness.run();
    harness.drag_at(target);
    harness.run();
    harness.drop_at(target);
    harness.run();
    assert!(!crate::ribbon::eyedropper_active(&harness.ctx));
    assert_eq!(ink(&harness).pen().color.to_hex(), "#F7E7B4");
}

#[test]
fn always_create_pages_with_rule_lines_is_a_checked_setting() {
    let mut harness = harness();
    assert_eq!(harness.state().session().default_rule_lines().id(), "none");
    click(&mut harness, "Format Background");
    click(&mut harness, "Always Create Pages with Rule Lines");
    assert_eq!(
        harness.state().session().default_rule_lines().id(),
        "narrow"
    );
    // The menu marks it while on, and the same row turns it off.
    click(&mut harness, "Format Background");
    click(&mut harness, "Always Create Pages with Rule Lines");
    assert_eq!(harness.state().session().default_rule_lines().id(), "none");
}

#[test]
fn format_background_offers_rule_lines_grids_and_colors() {
    let mut harness = harness();
    click(&mut harness, "Format Background");
    for label in [
        "Rule Lines",
        "Grid Lines",
        "None",
        "Narrow Ruled",
        "College Ruled",
        "Standard Ruled",
        "Wide Ruled",
        "Small Grid",
        "Medium Grid",
        "Large Grid",
        "Very Large Grid",
        "Always Create Pages with Rule Lines",
    ] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    let _ = harness.get_by_label_contains("Rule Line Color");
    let _ = harness.get_by_label_contains("Page Color");
    click(&mut harness, "College Ruled");
    assert_eq!(
        harness.state().session().page_style().rule_lines.id(),
        "college"
    );
}

#[test]
fn the_rule_line_color_submenu_sets_the_color() {
    let mut harness = harness();
    click(&mut harness, "Format Background");
    harness.get_by_label_contains("Rule Line Color").click();
    harness.run();
    click(&mut harness, "#7BA7D6");
    assert_eq!(
        harness.state().session().page_style().rule_color,
        Some([0x7B, 0xA7, 0xD6, 0xFF])
    );
}
