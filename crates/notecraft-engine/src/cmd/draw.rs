//! Draw tab commands: `draw.type`, `draw.pen`, `draw.highlighter`, `draw.eraser`,
//! `draw.color`, `draw.thickness`, `ink.add_stroke` (the canvas sends it when a pen or
//! highlighter gesture ends), and `ink.erase` (it sends that while the eraser drags). Strokes
//! are kept in the session per page; the canvas block stores them in PR 1.9.

use notecraft_canvas::ink::erase as erase_geometry;
use notecraft_canvas::RuleLines;
use notecraft_core::ink::MAX_STROKE_POINTS;
use notecraft_core::{InkColor, InkPoint, InkTool, Stroke, Vec2};
use serde_json::json;

use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::ink::{
    DrawTool, EraserMode, EraserSize, PenTool, ToolKind, MAX_PEN_POINTS, MAX_PEN_TOOLS,
    MIN_PEN_POINTS,
};
use crate::session::Session;
use crate::undo::Op;

/// Widest eraser reach `ink.erase` takes, in page units.
const MAX_ERASE_RADIUS: f32 = 1000.0;

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

/// What the pen commands return: the tool now in effect and its color and width (the
/// highlighter's while it is the tool, the pen's otherwise), plus the eraser mode while the
/// eraser is the tool.
fn pen_output(session: &Session) -> CommandOutput {
    let ink = session.ink();
    let brush = ink.brush();
    let value = if ink.tool() == DrawTool::Eraser {
        json!({
            "tool": ink.tool().id(),
            "color": brush.color.to_hex(),
            "width": brush.width_pt,
            "mode": ink.eraser().id(),
            "size": ink.eraser_size().id(),
        })
    } else {
        json!({
            "tool": ink.tool().id(),
            "color": brush.color.to_hex(),
            "width": brush.width_pt,
        })
    };
    CommandOutput { value }
}

/// The tool list entry still in use after `draw.pen` or `draw.highlighter`: kept when the
/// command changed nothing about the pen and the entry is of `kind`.
fn keep_selection(ink: &crate::ink::InkState, kind: ToolKind, restyled: bool) -> Option<usize> {
    let slot = ink.selected.filter(|i| *i < ink.tools.len())?;
    let same = ink.tools.get(slot).is_some_and(|tool| tool.kind == kind);
    (same && !restyled).then_some(slot)
}

/// `draw.tool_select { index }`: pick up the pen or highlighter at `index` of the list, with
/// its color and width.
pub(super) fn tool_select(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::DRAW_TOOL_SELECT;
    let index = index_arg(id, args, session.ink().tools.len())?;
    let ink = session.ink_mut();
    let Some(tool) = ink.tools.get(index).copied() else {
        return Err(invalid(id, "no such tool".to_owned()));
    };
    ink.selected = Some(index);
    match tool.kind {
        ToolKind::Pen => {
            ink.tool = DrawTool::Pen;
            ink.pen = tool.style;
        }
        ToolKind::Highlighter => {
            ink.tool = DrawTool::Highlighter;
            ink.highlighter = tool.style;
        }
    }
    session.mark_changed();
    Ok(tools_output(session))
}

/// `draw.tool_add { kind?, color?, width? }`: add a pen (or highlighter) to the end of the
/// list and pick it up. A color or width not given comes from the kind's default.
pub(super) fn tool_add(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::DRAW_TOOL_ADD;
    let kind = match args.get("kind") {
        None => ToolKind::Pen,
        Some(value) => value
            .as_str()
            .and_then(ToolKind::parse)
            .ok_or_else(|| invalid(id, "kind must be \"pen\" or \"highlighter\"".to_owned()))?,
    };
    let color = color_arg(id, args)?;
    let width = width_arg(id, args)?;
    if session.ink().tools.len() >= MAX_PEN_TOOLS {
        return Err(invalid(
            id,
            format!("the list holds at most {MAX_PEN_TOOLS} tools"),
        ));
    }
    let base = kind.default_style();
    let tool = PenTool::new(
        kind,
        color.unwrap_or(base.color),
        width.unwrap_or(base.width_pt),
    );
    let ink = session.ink_mut();
    ink.tools.push(tool);
    let index = ink.tools.len() - 1;
    let mut select = Map::new();
    select.insert("index".to_owned(), Value::from(index));
    tool_select(session, &select)
}

/// `draw.tool_delete { index? }`: remove a tool from the list; the one in use when no index
/// is given (and nothing changes when none is in use). Deleting the tool in use goes back to typing and selecting.
pub(super) fn tool_delete(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::DRAW_TOOL_DELETE;
    let ink = session.ink();
    let index = if args.contains_key("index") {
        index_arg(id, args, ink.tools.len())?
    } else {
        // Nothing in use and no index: nothing to delete.
        let Some(selected) = ink.selected_tool() else {
            return Ok(tools_output(session));
        };
        selected
    };
    let ink = session.ink_mut();
    if index < ink.tools.len() {
        ink.tools.remove(index);
    }
    match ink.selected {
        Some(selected) if selected == index => {
            ink.selected = None;
            ink.tool = DrawTool::Type;
        }
        Some(selected) if selected > index => ink.selected = Some(selected - 1),
        _ => {}
    }
    session.mark_changed();
    Ok(tools_output(session))
}

/// `draw.eyedropper { x, y, radius?, page_color? }`: set the color of the current pen (or highlighter)
/// to the color of the topmost stroke at the page point. Nothing there changes nothing and
/// returns null.
pub(super) fn eyedropper(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::DRAW_EYEDROPPER;
    let number = |name: &str| {
        args.get(name)
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .filter(|v| v.is_finite())
    };
    let (Some(x), Some(y)) = (number("x"), number("y")) else {
        return Err(invalid(id, "x and y must be finite numbers".to_owned()));
    };
    let radius = number("radius").unwrap_or(2.0).clamp(0.0, 100.0);
    // Page colors: with no stroke there, the front end passes the color it draws under the
    // point (the text color, or the page background) as `page_color`.
    let page_color = match args.get("page_color") {
        None => None,
        Some(value) => match InkColor::parse(value.as_str().unwrap_or_default()) {
            Some(color) => Some(InkColor([color.0[0], color.0[1], color.0[2], u8::MAX])),
            None => {
                return Err(invalid(
                    id,
                    "page_color must be #RRGGBB or #RRGGBBAA".to_owned(),
                ))
            }
        },
    };
    let found = session.current_ink().and_then(|layer| {
        let point = Vec2::new(x, y);
        let stroke = layer
            .topmost_at(point, radius)
            .and_then(|id| layer.get(id))?;
        Some(stroke.color)
    });
    let Some(color) = found.or(page_color) else {
        return Ok(CommandOutput { value: Value::Null });
    };
    // The eyedropper works for a pen: from typing or the eraser it picks the pen up. The
    // color keeps its alpha, so a translucent stroke gives a translucent pen.
    if matches!(session.ink().tool, DrawTool::Type | DrawTool::Eraser) {
        let ink = session.ink_mut();
        ink.tool = DrawTool::Pen;
        ink.selected = None;
    }
    let ink = session.ink_mut();
    ink.brush_mut().color = color;
    ink.sync_selected();
    session.mark_changed();
    Ok(pen_output(session))
}

/// `draw.always_rule_lines { enabled?, style? }`: Always Create Pages with Rule Lines. New pages
/// start with the style. Flips the setting when `enabled` is omitted; turning it on without a
/// `style` uses the style of the page shown (narrow when that page has none). A setting, not a
/// page change, so it is not an undo step.
pub(super) fn always_rule_lines(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::DRAW_ALWAYS_RULE_LINES;
    let current = session.default_rule_lines();
    let enabled = args
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(current == RuleLines::None);
    let style = match args.get("style").and_then(Value::as_str) {
        None => None,
        Some(text) => match RuleLines::from_id(text) {
            Some(style) => Some(style),
            None => {
                let shown: String = text.chars().take(64).collect();
                return Err(invalid(id, format!("unknown style {shown:?}")));
            }
        },
    };
    let ruled = |s: &RuleLines| *s != RuleLines::None;
    let next = if enabled {
        style
            .filter(ruled)
            .or_else(|| Some(session.page_style().rule_lines).filter(ruled))
            .unwrap_or(RuleLines::Narrow)
    } else {
        RuleLines::None
    };
    if next != current {
        session.set_default_rule_lines(next);
        session.mark_changed();
    }
    Ok(CommandOutput {
        value: json!({ "enabled": next != RuleLines::None, "style": next.id() }),
    })
}

/// The `index` argument, in range for a list of `len`.
fn index_arg(id: CommandId, args: &Map<String, Value>, len: usize) -> Result<usize, CommandError> {
    let index = args
        .get("index")
        .and_then(Value::as_u64)
        .and_then(|i| usize::try_from(i).ok())
        .filter(|i| *i < len);
    index.ok_or_else(|| invalid(id, format!("index must be 0 to {}", len.saturating_sub(1))))
}

/// What the tool list commands return: the tool in use as `pen_output` reports it, plus
/// the number of tools and which one is in use.
fn tools_output(session: &Session) -> CommandOutput {
    let mut out = pen_output(session);
    if let Some(map) = out.value.as_object_mut() {
        map.insert("count".to_owned(), json!(session.ink().tools().len()));
        map.insert("selected".to_owned(), json!(session.ink().selected_tool()));
    }
    out
}

/// `draw.type`: dragging on the canvas types and selects again.
pub(super) fn draw_type(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    if session.ink().tool != DrawTool::Type {
        let ink = session.ink_mut();
        ink.tool = DrawTool::Type;
        ink.selected = None;
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
    let before = (
        session.ink().tool,
        session.ink().pen,
        session.ink().selected,
    );
    let ink = session.ink_mut();
    ink.tool = DrawTool::Pen;
    if let Some(color) = color {
        ink.pen.color = color;
    }
    if let Some(width) = width {
        ink.pen.width_pt = width;
    }
    ink.selected = keep_selection(ink, ToolKind::Pen, color.is_some() || width.is_some());
    if (ink.tool, ink.pen, ink.selected) != before {
        session.mark_changed();
    }
    Ok(pen_output(session))
}

/// `draw.highlighter { color?, width? }`: pick up the highlighter, optionally with a color and
/// a width. The color is kept as picked; the canvas blends it with the paper when drawing.
pub(super) fn draw_highlighter(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let color = color_arg(ids::DRAW_HIGHLIGHTER, args)?;
    let width = width_arg(ids::DRAW_HIGHLIGHTER, args)?;
    let before = (
        session.ink().tool,
        session.ink().highlighter,
        session.ink().selected,
    );
    let ink = session.ink_mut();
    ink.tool = DrawTool::Highlighter;
    if let Some(color) = color {
        ink.highlighter.color = color;
    }
    if let Some(width) = width {
        ink.highlighter.width_pt = width;
    }
    ink.selected = keep_selection(
        ink,
        ToolKind::Highlighter,
        color.is_some() || width.is_some(),
    );
    if (ink.tool, ink.highlighter, ink.selected) != before {
        session.mark_changed();
    }
    Ok(pen_output(session))
}

/// `draw.eraser { mode?, size? }`: pick up the eraser, `stroke` (whole strokes) or `point` (the
/// part under the eraser), and for the point eraser a `size` of `small`, `medium`, or `large`.
/// Without a mode the last one stays; a size without a mode means the point eraser.
pub(super) fn draw_eraser(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let mode = match args.get("mode") {
        None => None,
        Some(value) => {
            let parsed = value.as_str().and_then(EraserMode::parse);
            match parsed {
                Some(mode) => Some(mode),
                None => {
                    return Err(invalid(
                        ids::DRAW_ERASER,
                        "mode must be \"stroke\" or \"point\"".to_owned(),
                    ))
                }
            }
        }
    };
    let size = match args.get("size") {
        None => None,
        Some(value) => match value.as_str().and_then(EraserSize::parse) {
            Some(size) => Some(size),
            None => {
                return Err(invalid(
                    ids::DRAW_ERASER,
                    "size must be \"small\", \"medium\", or \"large\"".to_owned(),
                ))
            }
        },
    };
    let before = (
        session.ink().tool,
        session.ink().eraser,
        session.ink().eraser_size,
        session.ink().selected,
    );
    let ink = session.ink_mut();
    ink.tool = DrawTool::Eraser;
    ink.selected = None;
    if let Some(mode) = mode {
        ink.eraser = mode;
    } else if size.is_some() {
        ink.eraser = EraserMode::Point;
    }
    if let Some(size) = size {
        ink.eraser_size = size;
    }
    if (ink.tool, ink.eraser, ink.eraser_size, ink.selected) != before {
        session.mark_changed();
    }
    Ok(pen_output(session))
}

/// `draw.color { color }`: the color of the current pen (the highlighter while it is the tool).
pub(super) fn draw_color(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let color = color_arg(ids::DRAW_COLOR, args)?;
    if let Some(color) = color.filter(|c| *c != session.ink().brush().color) {
        let ink = session.ink_mut();
        ink.brush_mut().color = color;
        ink.sync_selected();
        session.mark_changed();
    }
    Ok(pen_output(session))
}

/// `draw.thickness { width }`: the width of the current pen (the highlighter while it is the
/// tool), in points.
pub(super) fn draw_thickness(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let width = width_arg(ids::DRAW_THICKNESS, args)?;
    if let Some(width) = width.filter(|w| *w != session.ink().brush().width_pt) {
        let ink = session.ink_mut();
        ink.brush_mut().width_pt = width;
        ink.sync_selected();
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
    // The highlighter's strokes are marked as such; anything else is drawn with the pen.
    let (tool, style) = if session.ink().tool() == DrawTool::Highlighter {
        (InkTool::Highlighter, session.ink().highlighter())
    } else {
        (InkTool::Pen, session.ink().pen())
    };
    let stroke = Stroke {
        id: session
            .ink()
            .layer(&page)
            .map_or_else(|| "s1".to_owned(), |layer| layer.next_id()),
        tool,
        color: style.color,
        width: style.width_units(),
        points,
    };
    let stroke_id = stroke.id.clone();
    // One undo step; commit also marks the session changed.
    if !session.commit_add_stroke(&page, stroke) {
        return Err(invalid(
            id,
            "the page cannot hold another stroke".to_owned(),
        ));
    }
    Ok(CommandOutput {
        value: json!({ "id": stroke_id }),
    })
}

/// Most points one `ink.erase` command takes (a frame of a drag).
const MAX_ERASE_POINTS: usize = 4096;

/// `ink.erase { points, radius, mode?, join? }`: erase along the path of the eraser's center
/// on the page shown now. In `stroke` mode every stroke the eraser touches is removed; in
/// `point` mode the parts under the eraser are cut out and the rest of each stroke stays as
/// strokes of its own (same tool, color, and width). All of it is one undo step; with `join`
/// it joins the erase before it, so one drag undoes at once. Returns the ids removed and the
/// ids added. Touching nothing changes nothing.
pub(super) fn erase(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::INK_ERASE;
    let items = args
        .get("points")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if items.is_empty() || items.len() > MAX_ERASE_POINTS {
        return Err(invalid(
            id,
            format!("points must hold 1 to {MAX_ERASE_POINTS} points"),
        ));
    }
    let mut path = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        match parse_point(item) {
            Some(p) => path.push(Vec2::new(p.x, p.y)),
            None => {
                return Err(invalid(
                    id,
                    format!("point {i} must be [x, y] with finite numbers"),
                ))
            }
        }
    }
    let radius = args
        .get("radius")
        .and_then(Value::as_f64)
        .map(|r| r as f32)
        .filter(|r| r.is_finite() && (0.0..=MAX_ERASE_RADIUS).contains(r));
    let Some(radius) = radius else {
        return Err(invalid(
            id,
            format!("radius must be 0 to {MAX_ERASE_RADIUS} page units"),
        ));
    };
    let mode = match args.get("mode") {
        None => session.ink().eraser(),
        Some(value) => match value.as_str().and_then(EraserMode::parse) {
            Some(mode) => mode,
            None => {
                return Err(invalid(
                    id,
                    "mode must be \"stroke\" or \"point\"".to_owned(),
                ))
            }
        },
    };
    let join = args.get("join").and_then(Value::as_bool).unwrap_or(false);
    let none = || CommandOutput {
        value: json!({ "removed": [], "added": [] }),
    };
    let Some(page) = session.selection().page.clone() else {
        return Ok(none());
    };
    if !join {
        // A new drag never joins the one before it.
        session.end_erase_chain();
    }
    let Some(layer) = session.ink().layer(&page) else {
        return Ok(none());
    };
    let centers = erase_geometry::path_samples(&path, radius);
    let touched = erase_geometry::strokes_touched(layer, &centers, radius);
    if touched.is_empty() {
        return Ok(none());
    }

    // Build the ops from the topmost stroke down, so the paint-order index of each stroke is
    // still right when its turn comes: a change at a higher index never moves a lower one.
    let mut cuts: Vec<(usize, Stroke, Vec<Vec<InkPoint>>)> = Vec::new();
    for stroke_id in touched {
        let (Some(at), Some(stroke)) = (layer.position(&stroke_id), layer.get(&stroke_id)) else {
            continue;
        };
        let runs = match mode {
            EraserMode::Stroke => Vec::new(),
            EraserMode::Point => {
                match erase_geometry::split_stroke(&stroke.points, stroke.width, &centers, radius) {
                    Some(runs) => runs,
                    // The stroke's edge was in reach but none of its points: leave it.
                    None => continue,
                }
            }
        };
        cuts.push((at, stroke.clone(), runs));
    }
    cuts.sort_by_key(|(at, _, _)| std::cmp::Reverse(*at));
    let new_runs: usize = cuts.iter().map(|(_, _, runs)| runs.len()).sum();
    let mut fresh = layer.next_ids(new_runs).into_iter();
    let mut ops = Vec::new();
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for (at, stroke, runs) in cuts {
        removed.push(stroke.id.clone());
        // Take the stroke out, then put its runs where it was, in order.
        ops.push(Op::DeleteStroke {
            page: page.clone(),
            index: at,
            stroke: stroke.clone(),
        });
        for (k, points) in runs.into_iter().enumerate() {
            let Some(new_id) = fresh.next() else {
                break;
            };
            added.push(new_id.clone());
            ops.push(Op::InsertStroke {
                page: page.clone(),
                index: at + k,
                stroke: Stroke {
                    id: new_id,
                    points,
                    ..stroke.clone()
                },
            });
        }
    }
    if ops.is_empty() {
        return Ok(none());
    }
    if !session.commit_erase(&page, ops, join) {
        return Err(invalid(
            id,
            "the page cannot hold the erased parts".to_owned(),
        ));
    }
    Ok(CommandOutput {
        value: json!({ "removed": removed, "added": added }),
    })
}

#[cfg(test)]
mod tests {
    use notecraft_model::{Notebook, PageRef, Section, SectionNode};
    use serde_json::{json, Value};

    use crate::cmd::Registry;
    use crate::ink::{DrawTool, EraserMode, EraserSize};
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

    #[test]
    fn the_highlighter_has_its_own_style_and_marks_its_strokes() {
        let mut session = with_page();
        // The defaults: yellow, 8 pt.
        let out = run(&mut session, "draw.highlighter", json!(null));
        assert_eq!(
            out,
            Ok(json!({ "tool": "highlighter", "color": "#FFF176", "width": 8.0 }))
        );
        run(
            &mut session,
            "draw.highlighter",
            json!({ "color": "#80DEEA", "width": 12 }),
        )
        .unwrap();
        // Color and thickness follow the tool in use, and leave the pen alone.
        run(&mut session, "draw.color", json!({ "color": "#F48FB1" })).unwrap();
        run(&mut session, "draw.thickness", json!({ "width": 10 })).unwrap();
        assert_eq!(session.ink().highlighter().color.to_hex(), "#F48FB1");
        assert_eq!(session.ink().highlighter().width_pt, 10.0);
        assert_eq!(session.ink().pen().color.to_hex(), "#1F1F1F");
        let out = run(
            &mut session,
            "ink.add_stroke",
            json!({ "points": [[0, 0], [50, 0]] }),
        );
        assert_eq!(out, Ok(json!({ "id": "s1" })));
        let stroke = session.current_ink().unwrap().get("s1").unwrap().clone();
        assert_eq!(stroke.tool, notecraft_core::InkTool::Highlighter);
        assert_eq!(stroke.color.to_hex(), "#F48FB1");
        assert!((stroke.width - 10.0 * 96.0 / 72.0).abs() < 1.0e-4);
        // Back to the pen: the pen's own style and tool.
        run(&mut session, "draw.pen", json!(null)).unwrap();
        run(
            &mut session,
            "ink.add_stroke",
            json!({ "points": [[0, 9]] }),
        )
        .unwrap();
        let pen_stroke = session.current_ink().unwrap().get("s2").unwrap().clone();
        assert_eq!(pen_stroke.tool, notecraft_core::InkTool::Pen);
        assert_eq!(pen_stroke.color.to_hex(), "#1F1F1F");
        // Highlighter strokes undo like pen strokes.
        run(&mut session, "edit.undo", json!(null)).unwrap();
        run(&mut session, "edit.undo", json!(null)).unwrap();
        assert!(session.current_ink().is_none());
    }

    #[test]
    fn the_eraser_takes_a_mode_and_keeps_the_last_one() {
        let mut session = with_page();
        let out = run(&mut session, "draw.eraser", json!(null));
        assert_eq!(
            out,
            Ok(json!({
                "tool": "eraser", "color": "#1F1F1F", "width": 1.0,
                "mode": "stroke", "size": "medium"
            }))
        );
        run(&mut session, "draw.eraser", json!({ "mode": "point" })).unwrap();
        assert_eq!(session.ink().eraser(), EraserMode::Point);
        run(&mut session, "draw.pen", json!(null)).unwrap();
        run(&mut session, "draw.eraser", json!(null)).unwrap();
        assert_eq!(session.ink().eraser(), EraserMode::Point);
        let before = session.clone();
        for mode in [json!("lasso"), json!(3), json!(null)] {
            let result = run(&mut session, "draw.eraser", json!({ "mode": mode }));
            assert_eq!(result, Err("invalid_args"));
        }
        assert_eq!(session, before);
    }

    /// A page with three horizontal strokes, 100 units long, at y = 0, 20, and 40.
    fn with_lines() -> Session {
        let mut session = with_page();
        for y in [0, 20, 40] {
            let points: Vec<Value> = (0..=100).map(|x| json!([x, y, 1.0])).collect();
            run(&mut session, "ink.add_stroke", json!({ "points": points })).unwrap();
        }
        session
    }

    fn ids_on_page(session: &Session) -> Vec<String> {
        let layer = session.current_ink();
        let strokes = layer.map(|l| l.strokes()).unwrap_or_default();
        strokes.iter().map(|s| s.id.clone()).collect()
    }

    #[test]
    fn the_stroke_eraser_removes_whole_strokes_in_one_step() {
        let mut session = with_lines();
        run(&mut session, "draw.eraser", json!({ "mode": "stroke" })).unwrap();
        let out = run(
            &mut session,
            "ink.erase",
            json!({ "points": [[50, 0], [50, 22]], "radius": 3 }),
        )
        .unwrap();
        assert_eq!(out, json!({ "removed": ["s2", "s1"], "added": [] }));
        assert_eq!(ids_on_page(&session), ["s3"]);
        run(&mut session, "edit.undo", json!(null)).unwrap();
        assert_eq!(ids_on_page(&session), ["s1", "s2", "s3"]);
        run(&mut session, "edit.redo", json!(null)).unwrap();
        assert_eq!(ids_on_page(&session), ["s3"]);
        // Touching nothing records nothing.
        let revision = session.revision();
        let out = run(
            &mut session,
            "ink.erase",
            json!({ "points": [[500, 500]], "radius": 3 }),
        );
        assert_eq!(out, Ok(json!({ "removed": [], "added": [] })));
        assert_eq!(session.revision(), revision);
    }

    #[test]
    fn one_drag_is_one_undo_step() {
        let mut session = with_lines();
        let tx = |join: bool, y: i32| json!({ "points": [[50, y]], "radius": 3, "join": join });
        run(&mut session, "ink.erase", tx(false, 0)).unwrap();
        run(&mut session, "ink.erase", tx(true, 20)).unwrap();
        run(&mut session, "ink.erase", tx(true, 40)).unwrap();
        assert!(ids_on_page(&session).is_empty());
        run(&mut session, "edit.undo", json!(null)).unwrap();
        assert_eq!(ids_on_page(&session), ["s1", "s2", "s3"]);
        // The three strokes were the only steps on this page besides drawing them.
        run(&mut session, "edit.redo", json!(null)).unwrap();
        assert!(ids_on_page(&session).is_empty());
    }

    #[test]
    fn a_new_drag_and_other_steps_start_a_new_step() {
        let mut session = with_lines();
        let args = |join: bool, y: i32| json!({ "points": [[50, y]], "radius": 3, "join": join });
        run(&mut session, "ink.erase", args(false, 0)).unwrap();
        // A new drag, even flagged to join by mistake at its first sample, is its own step.
        run(&mut session, "ink.erase", args(false, 20)).unwrap();
        run(&mut session, "edit.undo", json!(null)).unwrap();
        assert_eq!(ids_on_page(&session), ["s2", "s3"]);
        // A join after an undo does not merge into the step below.
        run(&mut session, "ink.erase", args(true, 40)).unwrap();
        run(&mut session, "edit.undo", json!(null)).unwrap();
        assert_eq!(ids_on_page(&session), ["s2", "s3"]);
        // A join right after drawing does not swallow the stroke that was drawn.
        run(
            &mut session,
            "ink.add_stroke",
            json!({ "points": [[0, 60], [100, 60]] }),
        )
        .unwrap();
        run(&mut session, "ink.erase", args(true, 40)).unwrap();
        run(&mut session, "edit.undo", json!(null)).unwrap();
        assert_eq!(ids_on_page(&session), ["s2", "s3", "s4"]);
    }

    #[test]
    fn the_point_eraser_splits_a_stroke_and_undo_puts_it_back() {
        let mut session = with_lines();
        run(&mut session, "draw.eraser", json!({ "mode": "point" })).unwrap();
        let original = session.current_ink().unwrap().get("s2").unwrap().clone();
        let out = run(
            &mut session,
            "ink.erase",
            json!({ "points": [[50, 20]], "radius": 5 }),
        )
        .unwrap();
        assert_eq!(out, json!({ "removed": ["s2"], "added": ["s4", "s5"] }));
        // The runs sit where the stroke was in paint order and keep its look.
        assert_eq!(ids_on_page(&session), ["s1", "s4", "s5", "s3"]);
        let layer = session.current_ink().unwrap();
        let (left, right) = (layer.get("s4").unwrap(), layer.get("s5").unwrap());
        assert_eq!(left.width, original.width);
        assert_eq!(right.color, original.color);
        assert!(left.points.last().is_some_and(|p| p.x < 50.0));
        assert!(right.points.first().is_some_and(|p| p.x > 50.0));
        run(&mut session, "edit.undo", json!(null)).unwrap();
        assert_eq!(ids_on_page(&session), ["s1", "s2", "s3"]);
        assert_eq!(session.current_ink().unwrap().get("s2"), Some(&original));
    }

    #[test]
    fn the_point_eraser_has_three_sizes() {
        let mut session = with_page();
        assert_eq!(session.ink().eraser_size(), EraserSize::Medium);
        // A size alone means the point eraser.
        let out = run(&mut session, "draw.eraser", json!({ "size": "large" })).unwrap();
        assert_eq!(out["mode"], "point");
        assert_eq!(out["size"], "large");
        run(&mut session, "draw.eraser", json!({ "mode": "stroke" })).unwrap();
        // The size is kept while the stroke eraser is picked, and with no arguments.
        assert_eq!(session.ink().eraser_size(), EraserSize::Large);
        run(&mut session, "draw.eraser", json!(null)).unwrap();
        assert_eq!(session.ink().eraser(), EraserMode::Stroke);
        run(
            &mut session,
            "draw.eraser",
            json!({ "mode": "point", "size": "small" }),
        )
        .unwrap();
        assert_eq!(session.ink().eraser_size(), EraserSize::Small);
        let before = session.clone();
        for bad in [json!("huge"), json!(2), json!(null)] {
            let result = run(&mut session, "draw.eraser", json!({ "size": bad }));
            assert_eq!(result, Err("invalid_args"));
        }
        assert_eq!(session, before);
        // Picking the same eraser again changes nothing.
        let revision = session.revision();
        run(
            &mut session,
            "draw.eraser",
            json!({ "mode": "point", "size": "small" }),
        )
        .unwrap();
        assert_eq!(session.revision(), revision);
    }

    #[test]
    fn the_point_eraser_cuts_a_graze_between_stored_points() {
        let mut session = with_page();
        // Two stored points 200 apart at full pressure; width 3 pt is 4 units.
        run(&mut session, "draw.pen", json!({ "width": 3 })).unwrap();
        run(
            &mut session,
            "ink.add_stroke",
            json!({ "points": [[0, 0, 1.0], [200, 0, 1.0]] }),
        )
        .unwrap();
        // The circle (radius 5) is 6 units from the center line: inside radius plus half width.
        let out = run(
            &mut session,
            "ink.erase",
            json!({ "points": [[100, 6]], "radius": 5, "mode": "point" }),
        )
        .unwrap();
        assert_eq!(out, json!({ "removed": ["s1"], "added": ["s2", "s3"] }));
        // 8 units away is out of reach: the stroke stays whole.
        let out = run(
            &mut session,
            "ink.erase",
            json!({ "points": [[50, 8]], "radius": 5, "mode": "point" }),
        )
        .unwrap();
        assert_eq!(out, json!({ "removed": [], "added": [] }));
    }

    #[test]
    fn the_mode_argument_overrides_the_eraser_in_use() {
        let mut session = with_lines();
        run(&mut session, "draw.eraser", json!({ "mode": "point" })).unwrap();
        let out = run(
            &mut session,
            "ink.erase",
            json!({ "points": [[50, 0]], "radius": 3, "mode": "stroke" }),
        )
        .unwrap();
        assert_eq!(out["added"], json!([]));
        assert_eq!(ids_on_page(&session), ["s2", "s3"]);
    }

    #[test]
    fn bad_erase_arguments_change_nothing() {
        let mut session = with_lines();
        let before = session.clone();
        for bad in [
            json!({ "radius": 3 }),
            json!({ "points": [], "radius": 3 }),
            json!({ "points": [[1]], "radius": 3 }),
            json!({ "points": [["a", 1]], "radius": 3 }),
            json!({ "points": [[1, 1]] }),
            json!({ "points": [[1, 1]], "radius": -1 }),
            json!({ "points": [[1, 1]], "radius": 1.0e9 }),
            json!({ "points": [[1, 1]], "radius": "big" }),
            json!({ "points": [[1, 1]], "radius": 3, "mode": "lasso" }),
        ] {
            let result = run(&mut session, "ink.erase", bad.clone());
            assert_eq!(result, Err("invalid_args"), "{bad}");
        }
        assert_eq!(session, before);
        // No page shown: nothing to erase.
        let mut empty = Session::new();
        let out = run(
            &mut empty,
            "ink.erase",
            json!({ "points": [[1, 1]], "radius": 3 }),
        );
        assert_eq!(out, Ok(json!({ "removed": [], "added": [] })));
    }

    // The Draw tab's list of pens

    fn tool_ids(session: &Session) -> Vec<(&'static str, String, f32)> {
        session
            .ink()
            .tools()
            .iter()
            .map(|t| (t.kind.id(), t.style.color.to_hex(), t.style.width_pt))
            .collect()
    }

    #[test]
    fn a_fresh_session_has_black_red_blue_green_pens_and_a_highlighter() {
        let session = with_page();
        let tools = tool_ids(&session);
        assert_eq!(tools.len(), 5);
        assert_eq!(tools[0], ("pen", "#000000".to_owned(), 1.0));
        assert_eq!(tools[4].0, "highlighter");
        assert_eq!(session.ink().selected_tool(), None);
    }

    #[test]
    fn picking_up_a_tool_sets_the_pen_and_marks_the_tool() {
        let mut session = with_page();
        let out = run(&mut session, "draw.tool_select", json!({ "index": 1 })).unwrap();
        assert_eq!(out["tool"], "pen");
        assert_eq!(out["color"], "#E81123");
        assert_eq!(out["selected"], 1);
        assert_eq!(session.ink().pen().color.to_hex(), "#E81123");
        let out = run(&mut session, "draw.tool_select", json!({ "index": 4 })).unwrap();
        assert_eq!(out["tool"], "highlighter");
        assert_eq!(session.ink().tool(), DrawTool::Highlighter);
        assert_eq!(session.ink().highlighter().width_pt, 8.0);
        // Other tools let go of the selection.
        run(&mut session, "draw.eraser", json!(null)).unwrap();
        assert_eq!(session.ink().selected_tool(), None);
        run(&mut session, "draw.tool_select", json!({ "index": 0 })).unwrap();
        run(&mut session, "draw.type", json!(null)).unwrap();
        assert_eq!(session.ink().selected_tool(), None);
        for bad in [json!({ "index": 5 }), json!({ "index": -1 }), json!({})] {
            assert_eq!(
                run(&mut session, "draw.tool_select", bad.clone()),
                Err("invalid_args"),
                "{bad}"
            );
        }
    }

    #[test]
    fn color_and_thickness_change_the_tool_in_use() {
        let mut session = with_page();
        run(&mut session, "draw.tool_select", json!({ "index": 2 })).unwrap();
        run(&mut session, "draw.color", json!({ "color": "#7F6000" })).unwrap();
        run(&mut session, "draw.thickness", json!({ "width": 3.5 })).unwrap();
        assert_eq!(tool_ids(&session)[2], ("pen", "#7F6000".to_owned(), 3.5));
        // The other tools stay as they were.
        assert_eq!(tool_ids(&session)[1].1, "#E81123");
        // A pen not from the list changes no entry.
        run(&mut session, "draw.pen", json!({ "color": "#112233" })).unwrap();
        assert_eq!(session.ink().selected_tool(), None);
        run(&mut session, "draw.color", json!({ "color": "#445566" })).unwrap();
        assert_eq!(tool_ids(&session)[2].1, "#7F6000");
        // Picking the entry up again brings its look back.
        run(&mut session, "draw.tool_select", json!({ "index": 2 })).unwrap();
        assert_eq!(session.ink().pen().color.to_hex(), "#7F6000");
        // A plain draw.pen keeps the entry in use.
        run(&mut session, "draw.type", json!(null)).unwrap();
        run(&mut session, "draw.tool_select", json!({ "index": 2 })).unwrap();
        run(&mut session, "draw.pen", json!(null)).unwrap();
        assert_eq!(session.ink().selected_tool(), Some(2));
    }

    #[test]
    fn tools_are_added_with_defaults_and_up_to_a_limit() {
        let mut session = with_page();
        let out = run(&mut session, "draw.tool_add", json!(null)).unwrap();
        assert_eq!(out["count"], 6);
        assert_eq!(out["selected"], 5);
        assert_eq!(tool_ids(&session)[5], ("pen", "#000000".to_owned(), 1.0));
        let out = run(
            &mut session,
            "draw.tool_add",
            json!({ "kind": "highlighter", "color": "#80DEEA", "width": 12 }),
        )
        .unwrap();
        assert_eq!(out["tool"], "highlighter");
        assert_eq!(
            tool_ids(&session)[6],
            ("highlighter", "#80DEEA".to_owned(), 12.0)
        );
        for bad in [
            json!({ "kind": "pencil" }),
            json!({ "color": "red" }),
            json!({ "width": 500 }),
        ] {
            assert_eq!(
                run(&mut session, "draw.tool_add", bad.clone()),
                Err("invalid_args"),
                "{bad}"
            );
        }
        while session.ink().tools().len() < crate::MAX_PEN_TOOLS {
            run(&mut session, "draw.tool_add", json!(null)).unwrap();
        }
        let before = session.clone();
        assert_eq!(
            run(&mut session, "draw.tool_add", json!(null)),
            Err("invalid_args")
        );
        assert_eq!(session, before);
    }

    #[test]
    fn deleting_a_tool_keeps_the_selection_in_step() {
        let mut session = with_page();
        run(&mut session, "draw.tool_select", json!({ "index": 3 })).unwrap();
        // Deleting an earlier tool moves the selection up one place.
        let out = run(&mut session, "draw.tool_delete", json!({ "index": 0 })).unwrap();
        assert_eq!(out["count"], 4);
        assert_eq!(session.ink().selected_tool(), Some(2));
        assert_eq!(session.ink().tool(), DrawTool::Pen);
        // Deleting a later one leaves it.
        run(&mut session, "draw.tool_delete", json!({ "index": 3 })).unwrap();
        assert_eq!(session.ink().selected_tool(), Some(2));
        // With no index the tool in use goes, and typing is back.
        run(&mut session, "draw.tool_delete", json!(null)).unwrap();
        assert_eq!(session.ink().selected_tool(), None);
        assert_eq!(session.ink().tool(), DrawTool::Type);
        assert_eq!(session.ink().tools().len(), 2);
        // No tool in use and no index: nothing to delete.
        let before = session.clone();
        assert!(run(&mut session, "draw.tool_delete", json!(null)).is_ok());
        assert_eq!(session, before);
        assert_eq!(
            run(&mut session, "draw.tool_delete", json!({ "index": 9 })),
            Err("invalid_args")
        );
        // The list can be emptied; Add still works.
        run(&mut session, "draw.tool_delete", json!({ "index": 0 })).unwrap();
        run(&mut session, "draw.tool_delete", json!({ "index": 0 })).unwrap();
        assert!(session.ink().tools().is_empty());
        run(&mut session, "draw.tool_add", json!(null)).unwrap();
        assert_eq!(session.ink().tools().len(), 1);
    }

    #[test]
    fn saved_tools_restore_bounded_and_without_a_selection() {
        use crate::ink::{PenTool, ToolKind};
        let mut session = with_page();
        run(&mut session, "draw.tool_select", json!({ "index": 1 })).unwrap();
        let wild = PenTool::new(ToolKind::Pen, notecraft_core::InkColor::BLACK, f32::NAN);
        let huge = PenTool::new(
            ToolKind::Highlighter,
            notecraft_core::InkColor::BLACK,
            1.0e9,
        );
        let mut list = vec![wild, huge];
        list.extend(std::iter::repeat_n(wild, 100));
        session.set_pen_tools(list);
        assert_eq!(session.ink().tools().len(), crate::MAX_PEN_TOOLS);
        assert_eq!(session.ink().selected_tool(), None);
        assert_eq!(session.ink().tools()[0].style.width_pt, 1.0);
        assert_eq!(session.ink().tools()[1].style.width_pt, 72.0);
    }

    #[test]
    fn the_eyedropper_takes_the_color_of_the_stroke_under_the_point() {
        let mut session = with_page();
        run(&mut session, "draw.pen", json!({ "color": "#C62828" })).unwrap();
        let points: Vec<Value> = (0..=100).map(|x| json!([x, 20, 1.0])).collect();
        run(&mut session, "ink.add_stroke", json!({ "points": points })).unwrap();
        run(&mut session, "draw.pen", json!({ "color": "#1565C0" })).unwrap();
        run(&mut session, "draw.type", json!(null)).unwrap();
        let out = run(
            &mut session,
            "draw.eyedropper",
            json!({ "x": 50, "y": 20.5 }),
        )
        .unwrap();
        assert_eq!(out["color"], "#C62828");
        // The pen was picked up with that color.
        assert_eq!(session.ink().tool(), DrawTool::Pen);
        assert_eq!(session.ink().pen().color.to_hex(), "#C62828");
        // Empty paper changes nothing.
        let before = session.clone();
        let out = run(
            &mut session,
            "draw.eyedropper",
            json!({ "x": 50, "y": 300 }),
        );
        assert_eq!(out, Ok(Value::Null));
        assert_eq!(session, before);
        for bad in [json!({ "x": 1 }), json!({ "x": "a", "y": 2 })] {
            assert_eq!(
                run(&mut session, "draw.eyedropper", bad.clone()),
                Err("invalid_args"),
                "{bad}"
            );
        }
        // It also follows a tool of the list.
        run(&mut session, "draw.tool_select", json!({ "index": 3 })).unwrap();
        run(&mut session, "draw.eyedropper", json!({ "x": 10, "y": 20 })).unwrap();
        assert_eq!(tool_ids(&session)[3].1, "#C62828");
    }

    #[test]
    fn the_eyedropper_reads_page_colors_when_no_ink_is_there() {
        let mut session = with_page();
        run(&mut session, "draw.pen", json!({ "color": "#C62828" })).unwrap();
        let points: Vec<Value> = (0..=100).map(|x| json!([x, 20, 1.0])).collect();
        run(&mut session, "ink.add_stroke", json!({ "points": points })).unwrap();
        // Ink wins over the page color under it.
        let out = run(
            &mut session,
            "draw.eyedropper",
            json!({ "x": 50, "y": 20.5, "page_color": "#336699" }),
        )
        .unwrap();
        assert_eq!(out["color"], "#C62828");
        // Bare paper gives the page color, opaque, and picks the pen up.
        run(&mut session, "draw.type", json!(null)).unwrap();
        let out = run(
            &mut session,
            "draw.eyedropper",
            json!({ "x": 50, "y": 300, "page_color": "#336699" }),
        )
        .unwrap();
        assert_eq!(out["color"], "#336699");
        assert_eq!(session.ink().tool(), DrawTool::Pen);
        // A bad color is refused.
        assert_eq!(
            run(
                &mut session,
                "draw.eyedropper",
                json!({ "x": 1, "y": 1, "page_color": "blue" })
            ),
            Err("invalid_args")
        );
    }

    #[test]
    fn always_rule_lines_flips_and_remembers_a_style() {
        use notecraft_canvas::RuleLines;
        let mut session = with_page();
        assert_eq!(session.default_rule_lines(), RuleLines::None);
        // On with no style and a plain page: narrow.
        let out = run(&mut session, "draw.always_rule_lines", json!({})).unwrap();
        assert_eq!(out["enabled"], true);
        assert_eq!(session.default_rule_lines(), RuleLines::Narrow);
        // Flip off.
        run(&mut session, "draw.always_rule_lines", json!({})).unwrap();
        assert_eq!(session.default_rule_lines(), RuleLines::None);
        // On with a style, and the page's own style when none is given.
        run(
            &mut session,
            "draw.always_rule_lines",
            json!({ "enabled": true, "style": "grid_small" }),
        )
        .unwrap();
        assert_eq!(session.default_rule_lines(), RuleLines::GridSmall);
        run(
            &mut session,
            "draw.always_rule_lines",
            json!({ "enabled": false }),
        )
        .unwrap();
        run(&mut session, "view.rule_lines", json!({ "style": "wide" })).unwrap();
        run(
            &mut session,
            "draw.always_rule_lines",
            json!({ "enabled": true }),
        )
        .unwrap();
        assert_eq!(session.default_rule_lines(), RuleLines::Wide);
        assert_eq!(
            run(
                &mut session,
                "draw.always_rule_lines",
                json!({ "style": "zigzag" })
            ),
            Err("invalid_args")
        );
    }
}
