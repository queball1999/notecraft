//! Draw tab commands for the pen: `draw.type`, `draw.pen`, `draw.color`, `draw.thickness`,
//! and `ink.add_stroke`, which the canvas sends when a pen gesture ends. Strokes are kept in
//! the session per page; the canvas block stores them in PR 1.9.

use std::sync::Arc;

use notecraft_core::ink::MAX_STROKE_POINTS;
use notecraft_core::{InkColor, InkPoint, InkTool, Stroke};
use serde_json::json;

use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::ink::{DrawTool, MAX_PEN_POINTS, MIN_PEN_POINTS};
use crate::session::Session;

fn invalid(id: CommandId, reason: String) -> CommandError {
    CommandError::InvalidArgs { id, reason }
}

/// The `color` argument as an opaque or translucent pen color.
fn color_arg(id: CommandId, args: &Map<String, Value>) -> Result<Option<InkColor>, CommandError> {
    let Some(value) = args.get("color") else {
        return Ok(None);
    };
    let text = value.as_str().unwrap_or_default();
    match InkColor::parse(text) {
        Some(color) => Ok(Some(color)),
        None => {
            let shown: String = text.chars().take(64).collect();
            Err(invalid(
                id,
                format!("unknown color {shown:?}; expected #RRGGBB or #RRGGBBAA"),
            ))
        }
    }
}

/// The `width` argument in points, 0.25 to 72.
fn width_arg(id: CommandId, args: &Map<String, Value>) -> Result<Option<f32>, CommandError> {
    let Some(value) = args.get("width") else {
        return Ok(None);
    };
    let width = value.as_f64().map(|w| w as f32).filter(|w| w.is_finite());
    match width.filter(|w| (MIN_PEN_POINTS..=MAX_PEN_POINTS).contains(w)) {
        Some(width) => Ok(Some(width)),
        None => Err(invalid(
            id,
            format!("width must be {MIN_PEN_POINTS} to {MAX_PEN_POINTS} points"),
        )),
    }
}

/// What the pen commands return: the tool and pen now in effect.
fn pen_output(session: &Session) -> CommandOutput {
    let ink = session.ink();
    CommandOutput {
        value: json!({
            "tool": ink.tool().id(),
            "color": ink.pen().color.to_hex(),
            "width": ink.pen().width_pt,
        }),
    }
}

/// `draw.type`: dragging on the canvas types and selects again.
pub(super) fn draw_type(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    if session.ink().tool != DrawTool::Type {
        session.ink_mut().tool = DrawTool::Type;
        session.mark_changed();
    }
    Ok(pen_output(session))
}

/// `draw.pen { color?, width? }`: pick up the pen, optionally with a color and a width.
pub(super) fn draw_pen(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let color = color_arg(ids::DRAW_PEN, args)?;
    let width = width_arg(ids::DRAW_PEN, args)?;
    let before = (session.ink().tool, session.ink().pen);
    let ink = session.ink_mut();
    ink.tool = DrawTool::Pen;
    if let Some(color) = color {
        ink.pen.color = color;
    }
    if let Some(width) = width {
        ink.pen.width_pt = width;
    }
    if (ink.tool, ink.pen) != before {
        session.mark_changed();
    }
    Ok(pen_output(session))
}

/// `draw.color { color }`: the color of the current pen.
pub(super) fn draw_color(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let color = color_arg(ids::DRAW_COLOR, args)?;
    if let Some(color) = color.filter(|c| *c != session.ink().pen.color) {
        session.ink_mut().pen.color = color;
        session.mark_changed();
    }
    Ok(pen_output(session))
}

/// `draw.thickness { width }`: the width of the current pen, in points.
pub(super) fn draw_thickness(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let width = width_arg(ids::DRAW_THICKNESS, args)?;
    if let Some(width) = width.filter(|w| *w != session.ink().pen.width_pt) {
        session.ink_mut().pen.width_pt = width;
        session.mark_changed();
    }
    Ok(pen_output(session))
}

/// One point from `[x, y]` or `[x, y, pressure]`. A missing pressure is full pressure.
fn parse_point(value: &Value) -> Option<InkPoint> {
    let numbers = value.as_array()?;
    if !(2..=3).contains(&numbers.len()) {
        return None;
    }
    let mut it = numbers.iter().map(|n| n.as_f64().map(|v| v as f32));
    let x = it.next()??;
    let y = it.next()??;
    let pressure = match it.next() {
        Some(p) => p?,
        None => 1.0,
    };
    let finite = x.is_finite() && y.is_finite() && pressure.is_finite();
    finite.then(|| InkPoint::new(x, y, pressure.clamp(0.0, 1.0)))
}

/// `ink.add_stroke { points }`: add a stroke with the current pen to the page shown now. With
/// no page shown, nothing changes. Returns `{ "id": ... }` for the new stroke.
pub(super) fn add_stroke(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::INK_ADD_STROKE;
    let items = args
        .get("points")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if items.is_empty() || items.len() > MAX_STROKE_POINTS {
        return Err(invalid(
            id,
            format!("points must hold 1 to {MAX_STROKE_POINTS} points"),
        ));
    }
    let mut points = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let Some(point) = parse_point(item) else {
            return Err(invalid(
                id,
                format!("point {i} must be [x, y] or [x, y, pressure] with finite numbers"),
            ));
        };
        points.push(point);
    }
    let Some(page) = session.selection().page.clone() else {
        return Ok(CommandOutput { value: Value::Null });
    };
    let pen = session.ink().pen();
    let layer = session.ink_mut().layers.entry(page).or_default();
    let layer = Arc::make_mut(layer);
    let stroke = Stroke {
        id: layer.next_id(),
        tool: InkTool::Pen,
        color: pen.color,
        width: pen.width_units(),
        points,
    };
    let stroke_id = stroke.id.clone();
    if !layer.add(stroke) {
        return Err(invalid(
            id,
            "the page cannot hold another stroke".to_owned(),
        ));
    }
    session.mark_changed();
    Ok(CommandOutput {
        value: json!({ "id": stroke_id }),
    })
}

#[cfg(test)]
mod tests {
    use notecraft_model::{Notebook, PageRef, Section, SectionNode};
    use serde_json::{json, Value};

    use crate::cmd::Registry;
    use crate::ink::DrawTool;
    use crate::session::Session;

    fn run(session: &mut Session, id: &str, args: Value) -> Result<Value, &'static str> {
        Registry::builtin()
            .run(session, id, args)
            .map(|out| out.value)
            .map_err(|err| err.code())
    }

    fn with_page() -> Session {
        let section = Section {
            id: "s1".into(),
            name: "Notes".into(),
            color: 0,
            pages: vec![PageRef::new("p1", "One", 0), PageRef::new("p2", "Two", 0)],
        };
        let notebook = Notebook {
            id: "nb".into(),
            name: "Notebook".into(),
            color: 0,
            sections: vec![SectionNode::Section(section)],
        };
        let mut session = Session::new();
        session.set_notebooks(vec![notebook]);
        session
    }

    #[test]
    fn the_pen_takes_a_color_and_a_width() {
        let mut session = with_page();
        assert_eq!(session.ink().tool(), DrawTool::Type);
        let out = run(
            &mut session,
            "draw.pen",
            json!({ "color": "#1565c0", "width": 3.5 }),
        );
        assert_eq!(
            out,
            Ok(json!({ "tool": "pen", "color": "#1565C0", "width": 3.5 }))
        );
        assert_eq!(session.ink().tool(), DrawTool::Pen);
        // Color and thickness change the pen without dropping it.
        run(&mut session, "draw.color", json!({ "color": "#C62828" })).unwrap();
        run(&mut session, "draw.thickness", json!({ "width": 5 })).unwrap();
        assert_eq!(session.ink().pen().color.to_hex(), "#C62828");
        assert_eq!(session.ink().pen().width_pt, 5.0);
        assert_eq!(session.ink().tool(), DrawTool::Pen);
        // A pen without arguments keeps the color and width.
        run(&mut session, "draw.type", json!(null)).unwrap();
        assert_eq!(session.ink().tool(), DrawTool::Type);
        run(&mut session, "draw.pen", json!(null)).unwrap();
        assert_eq!(session.ink().pen().color.to_hex(), "#C62828");
        assert_eq!(session.ink().tool(), DrawTool::Pen);
    }

    #[test]
    fn bad_pen_arguments_change_nothing() {
        let mut session = with_page();
        let before = session.clone();
        for (id, args) in [
            ("draw.pen", json!({ "color": "red" })),
            ("draw.pen", json!({ "width": 0.1 })),
            ("draw.pen", json!({ "width": 1000 })),
            ("draw.color", json!({ "color": "none" })),
            ("draw.color", json!({ "color": "#12345" })),
            ("draw.thickness", json!({ "width": -1 })),
            ("draw.thickness", json!({ "width": "thick" })),
            ("draw.thickness", json!({})),
        ] {
            assert_eq!(
                run(&mut session, id, args.clone()),
                Err("invalid_args"),
                "{id} {args}"
            );
        }
        assert_eq!(session, before);
    }

    #[test]
    fn a_stroke_lands_on_the_page_with_the_current_pen() {
        let mut session = with_page();
        run(
            &mut session,
            "draw.pen",
            json!({ "color": "#2E7D32", "width": 3.0 }),
        )
        .unwrap();
        let out = run(
            &mut session,
            "ink.add_stroke",
            json!({ "points": [[10, 20, 0.5], [12.5, 22], [15, 25, 0.9]] }),
        );
        assert_eq!(out, Ok(json!({ "id": "s1" })));
        let layer = session.current_ink().unwrap();
        let stroke = layer.get("s1").unwrap();
        assert_eq!(stroke.color.to_hex(), "#2E7D32");
        assert_eq!(stroke.width, 4.0);
        assert_eq!(stroke.points.len(), 3);
        assert_eq!(stroke.points[1].pressure, 1.0);
        // The second stroke gets the next id; the other page has none.
        let out = run(
            &mut session,
            "ink.add_stroke",
            json!({ "points": [[0, 0]] }),
        );
        assert_eq!(out, Ok(json!({ "id": "s2" })));
        run(&mut session, "nav.page.select", json!({ "id": "p2" })).unwrap();
        assert!(session.current_ink().is_none());
        run(&mut session, "nav.page.select", json!({ "id": "p1" })).unwrap();
        assert_eq!(session.current_ink().map(|l| l.len()), Some(2));
    }

    #[test]
    fn bad_strokes_are_rejected_whole() {
        let mut session = with_page();
        let before = session.clone();
        for bad in [
            json!({ "points": [] }),
            json!({ "points": [[1]] }),
            json!({ "points": [[1, 2, 3, 4]] }),
            json!({ "points": [["a", 2]] }),
            json!({ "points": [[1, 2], [null, 3]] }),
            json!({ "points": [[1, 2], [3, 4, "x"]] }),
            json!({ "points": "nope" }),
        ] {
            let result = run(&mut session, "ink.add_stroke", bad.clone());
            assert_eq!(result, Err("invalid_args"), "{bad}");
        }
        assert_eq!(session, before);
        // Pressure outside 0 to 1 is clamped, not refused.
        run(
            &mut session,
            "ink.add_stroke",
            json!({ "points": [[1, 2, 9], [3, 4, -9]] }),
        )
        .unwrap();
        let stroke = session.current_ink().unwrap().get("s1").unwrap().clone();
        assert_eq!(stroke.points[0].pressure, 1.0);
        assert_eq!(stroke.points[1].pressure, 0.0);
    }

    #[test]
    fn with_no_page_nothing_changes() {
        let mut session = Session::new();
        let out = run(
            &mut session,
            "ink.add_stroke",
            json!({ "points": [[1, 2]] }),
        );
        assert_eq!(out, Ok(Value::Null));
        assert_eq!(session.revision(), 0);
    }

    #[test]
    fn setting_the_same_pen_changes_nothing() {
        let mut session = with_page();
        run(&mut session, "draw.pen", json!(null)).unwrap();
        let revision = session.revision();
        run(&mut session, "draw.pen", json!(null)).unwrap();
        run(&mut session, "draw.color", json!({ "color": "#1F1F1F" })).unwrap();
        run(&mut session, "draw.thickness", json!({ "width": 1 })).unwrap();
        assert_eq!(session.revision(), revision);
    }
}
