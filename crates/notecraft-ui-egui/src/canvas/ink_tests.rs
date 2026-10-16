//! Capture tests with the egui_kittest harness: synthetic pointer and touch events drive the
//! real window, and the strokes they make are read back from the session. No live window or
//! OS input is involved.

use egui_kittest::kittest::Queryable;
use notecraft_engine::cmd::Value;
use serde_json::json;

use crate::app_shell::AppShell;

type Harness = egui_kittest::Harness<'static, AppShell>;

fn harness() -> Harness {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1280.0, 800.0))
        .build_ui_state(
            |ui, shell: &mut AppShell| {
                shell.logic(ui.ctx());
                shell.ui(ui);
            },
            AppShell::new(),
        );
    harness.run();
    harness
}

fn pen(harness: &mut Harness, args: Value) {
    assert!(harness.state_mut().dispatch("draw.pen", args));
    harness.step();
}

fn canvas_center(harness: &Harness) -> egui::Pos2 {
    harness.get_by_label("Canvas").rect().center()
}

/// Press at `from`, move through `steps` points toward `to` (with a wiggle), release at
/// `to`. Every event gets its own frame, as a device reporting at full rate would.
fn drag(harness: &mut Harness, from: egui::Pos2, to: egui::Pos2, steps: usize) {
    harness.hover_at(from);
    harness.step();
    harness.drag_at(from);
    harness.step();
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let wiggle = (t * 9.0).sin() * 10.0;
        harness.hover_at(from + (to - from) * t + egui::vec2(0.0, wiggle));
        harness.step();
    }
    harness.drop_at(to);
    harness.step();
    harness.step();
}

fn stroke_count(harness: &Harness) -> usize {
    harness
        .state()
        .session()
        .current_ink()
        .map_or(0, |layer| layer.len())
}

#[test]
fn dragging_with_the_pen_makes_a_stroke() {
    let mut harness = harness();
    // Pick up the pen from the Draw tab, as a user does.
    harness.get_by_label("Draw").click();
    harness.run();
    harness.get_by_label("Pen").click();
    harness.run();
    assert_eq!(harness.state().session().ink().tool().id(), "pen");
    pen(&mut harness, json!({ "color": "#1565C0", "width": 3.0 }));
    assert_eq!(stroke_count(&harness), 0);

    let start = canvas_center(&harness);
    drag(&mut harness, start, start + egui::vec2(240.0, 60.0), 40);

    assert_eq!(stroke_count(&harness), 1);
    let session = harness.state().session();
    let stroke = session
        .current_ink()
        .and_then(|layer| layer.strokes().first())
        .expect("one stroke");
    assert_eq!(stroke.color.to_hex(), "#1565C0");
    assert_eq!(stroke.width, 4.0);
    // Every move event was captured, not one per click or one per frame.
    assert!(stroke.points.len() >= 40, "{} points", stroke.points.len());
    // Zoom is 100%, so page units equal screen points: the drag spans 240 units.
    let first = stroke.points.first().map(|p| p.x).unwrap_or_default();
    let last = stroke.points.last().map(|p| p.x).unwrap_or_default();
    assert!((last - first - 240.0).abs() < 2.0, "span {}", last - first);
    // A mouse reports no pressure, so width came from speed: 0.4 to 1.
    assert!(stroke
        .points
        .iter()
        .all(|p| (0.39..=1.001).contains(&p.pressure)));
}

#[test]
fn the_stroke_is_painted_as_a_mesh() {
    let mut harness = harness();
    pen(&mut harness, json!({ "width": 5.0 }));
    let start = canvas_center(&harness);
    drag(&mut harness, start, start + egui::vec2(200.0, 0.0), 20);
    harness.step();
    let meshes: Vec<usize> = harness
        .output()
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Mesh(mesh) => Some(mesh.vertices.len()),
            _ => None,
        })
        .collect();
    assert!(
        meshes.iter().any(|&n| n > 40),
        "no stroke mesh among {meshes:?}"
    );
}

#[test]
fn a_touch_force_becomes_pressure() {
    let mut harness = harness();
    pen(&mut harness, Value::Null);
    let start = canvas_center(&harness);
    let touch = |phase, pos, force| egui::Event::Touch {
        device_id: egui::TouchDeviceId(0),
        id: egui::TouchId(7),
        phase,
        pos,
        force: Some(force),
    };
    harness.event(touch(egui::TouchPhase::Start, start, 0.1));
    harness.step();
    for i in 1..=30 {
        let pos = start + egui::vec2(i as f32 * 6.0, 0.0);
        let force = 0.1 + 0.8 * i as f32 / 30.0;
        harness.event(touch(egui::TouchPhase::Move, pos, force));
        harness.step();
    }
    let end = start + egui::vec2(186.0, 0.0);
    harness.event(touch(egui::TouchPhase::End, end, 0.0));
    harness.step();
    harness.step();

    assert_eq!(stroke_count(&harness), 1);
    let session = harness.state().session();
    let stroke = session
        .current_ink()
        .and_then(|layer| layer.strokes().first())
        .expect("one stroke");
    let first = stroke.points.first().map(|p| p.pressure).unwrap_or(1.0);
    let mid = stroke
        .points
        .get(stroke.points.len() / 2)
        .map(|p| p.pressure);
    assert!(first < 0.3, "first {first}");
    assert!(mid.is_some_and(|p| p > 0.35 && p < 0.9), "mid {mid:?}");
}

#[test]
fn a_tap_makes_a_dot() {
    let mut harness = harness();
    pen(&mut harness, Value::Null);
    let at = canvas_center(&harness);
    harness.hover_at(at);
    harness.step();
    harness.drag_at(at);
    harness.step();
    harness.drop_at(at);
    harness.step();
    harness.step();
    assert_eq!(stroke_count(&harness), 1);
    let points = harness
        .state()
        .session()
        .current_ink()
        .and_then(|layer| layer.strokes().first().map(|s| s.points.len()));
    assert_eq!(points, Some(1));
}

#[test]
fn nothing_is_drawn_unless_the_pen_is_active() {
    let mut harness = harness();
    let start = canvas_center(&harness);
    drag(&mut harness, start, start + egui::vec2(100.0, 20.0), 10);
    assert_eq!(stroke_count(&harness), 0);
    pen(&mut harness, Value::Null);
    assert!(harness.state_mut().dispatch("draw.type", Value::Null));
    harness.step();
    drag(&mut harness, start, start + egui::vec2(100.0, 20.0), 10);
    assert_eq!(stroke_count(&harness), 0);
}

#[test]
fn a_press_outside_the_page_starts_no_stroke() {
    let mut harness = harness();
    pen(&mut harness, Value::Null);
    // The ribbon is above the canvas.
    let on_ribbon = egui::pos2(640.0, 20.0);
    let into_canvas = canvas_center(&harness);
    drag(&mut harness, on_ribbon, into_canvas, 10);
    assert_eq!(stroke_count(&harness), 0);
}

#[test]
fn a_stroke_belongs_to_its_page() {
    let mut harness = harness();
    pen(&mut harness, Value::Null);
    let start = canvas_center(&harness);
    drag(&mut harness, start, start + egui::vec2(80.0, 40.0), 8);
    assert_eq!(stroke_count(&harness), 1);
    let other = harness
        .state()
        .session()
        .current_section()
        .and_then(|s| s.pages.get(1).map(|p| p.id.clone()))
        .expect("the mock section has a second page");
    assert!(harness
        .state_mut()
        .dispatch("nav.page.select", json!({ "id": other })));
    harness.step();
    assert_eq!(stroke_count(&harness), 0);
}
