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

#[test]
fn add_offers_the_pencil_and_the_textured_pens() {
    let mut harness = harness();
    let before = ink(&harness).tools().len();
    for (i, (label, texture, tool)) in [
        ("Pencil", "pencil", "Black Pencil"),
        ("Rainbow Pen", "rainbow", "Rainbow Pen"),
        ("Galaxy Pen", "galaxy", "Galaxy Pen"),
        ("Lava Pen", "lava", "Lava Pen"),
        ("Ocean Pen", "ocean", "Ocean Pen"),
        ("Gold Pen", "gold", "Gold Pen"),
        ("Silver Pen", "silver", "Silver Pen"),
        ("Bronze Pen", "bronze", "Bronze Pen"),
        ("Rose Gold Pen", "rose_gold", "Rose Gold Pen"),
    ]
    .into_iter()
    .enumerate()
    {
        click(&mut harness, "Add");
        click(&mut harness, label);
        assert_eq!(ink(&harness).tools().len(), before + i + 1, "{label}");
        assert_eq!(ink(&harness).selected_tool(), Some(before + i), "{label}");
        assert_eq!(ink(&harness).pen().texture.id(), texture, "{label}");
        assert_eq!(ink(&harness).tool().id(), "pen");
        // The new pen is in the gallery under its own name.
        assert!(harness.query_by_label(tool).is_some(), "{tool}");
    }
}

#[test]
fn a_textured_pens_menu_has_a_preview_and_thickness_but_no_colors() {
    let mut harness = harness();
    click(&mut harness, "Add");
    click(&mut harness, "Gold Pen");
    click(&mut harness, "Gold Pen");
    for label in ["Stroke preview", "Thickness", "Delete Tool"] {
        assert!(harness.query_by_label(label).is_some(), "{label}");
    }
    for label in ["Colors", "More Colors...", "Eyedropper"] {
        assert!(harness.query_by_label(label).is_none(), "{label}");
    }
    // The pencil takes colors, as it draws in them.
    click(&mut harness, "Delete Tool");
    click(&mut harness, "Add");
    click(&mut harness, "Pencil");
    click(&mut harness, "Black Pencil");
    assert!(harness.query_by_label("Colors").is_some());
    click(&mut harness, "Burnt Orange");
    assert_eq!(ink(&harness).pen().color.to_hex(), "#E36C09");
    assert_eq!(ink(&harness).pen().texture.id(), "pencil");
    assert!(harness.query_by_label("Burnt Orange Pencil").is_some());
}

/// The meshes of the last frame that use an image of their own (not egui's font atlas).
fn textured_meshes(harness: &Harness) -> Vec<egui::epaint::Mesh> {
    harness
        .output()
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                Some((**mesh).clone())
            }
            _ => None,
        })
        .collect()
}

fn draw_stroke(harness: &mut Harness, args: serde_json::Value) {
    assert!(harness.state_mut().dispatch("draw.pen", args));
    let points: Vec<_> = (0..=60).map(|i| json!([120 + i * 6, 160, 1.0])).collect();
    assert!(harness
        .state_mut()
        .dispatch("ink.add_stroke", json!({ "points": points })));
    harness.run();
}

#[test]
fn a_textured_stroke_is_painted_with_its_image() {
    let mut harness = harness();
    // A plain stroke uses no image of its own.
    draw_stroke(&mut harness, json!({ "color": "#0A7EA4", "width": 4 }));
    assert!(textured_meshes(&harness).is_empty());
    // A rainbow stroke is a mesh pointing at an uploaded image, with coordinates that run
    // along the stroke and stay inside the image across it, in white so the image shows true.
    draw_stroke(&mut harness, json!({ "texture": "rainbow", "width": 6 }));
    let meshes = textured_meshes(&harness);
    assert_eq!(meshes.len(), 1);
    let mesh = &meshes[0];
    assert!(mesh.vertices.len() > 40);
    assert!(mesh
        .indices
        .iter()
        .all(|i| (*i as usize) < mesh.vertices.len()));
    let (lo, hi) = mesh
        .vertices
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| {
            (lo.min(v.uv.x), hi.max(v.uv.x))
        });
    // 360 page units of stroke over a repeat of at least 260: more than one cycle.
    assert!(hi - lo > 1.0, "u runs {lo} to {hi}");
    assert!(mesh.vertices.iter().all(|v| (0.0..=1.0).contains(&v.uv.y)));
    assert!(mesh
        .vertices
        .iter()
        .all(|v| v.color == egui::Color32::WHITE || v.color == egui::Color32::TRANSPARENT));
    // The same image serves the next rainbow stroke; a gold one gets another.
    draw_stroke(&mut harness, json!({ "texture": "gold", "width": 6 }));
    let ids: std::collections::BTreeSet<_> = textured_meshes(&harness)
        .iter()
        .map(|m| format!("{:?}", m.texture_id))
        .collect();
    assert_eq!(ids.len(), 2);
}

#[test]
fn a_pencil_stroke_is_grain_tinted_by_its_color() {
    let mut harness = harness();
    draw_stroke(
        &mut harness,
        json!({ "texture": "pencil", "color": "#1565C0", "width": 3 }),
    );
    let meshes = textured_meshes(&harness);
    assert_eq!(meshes.len(), 1);
    let blue = egui::Color32::from_rgb(0x15, 0x65, 0xC0);
    assert!(meshes[0]
        .vertices
        .iter()
        .all(|v| v.color == blue || v.color == egui::Color32::TRANSPARENT));
    assert!(meshes[0].vertices.iter().any(|v| v.color == blue));
}
