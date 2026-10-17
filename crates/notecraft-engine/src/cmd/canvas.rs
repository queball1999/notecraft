//! Canvas view commands: zoom (`view.zoom_in`, `view.zoom_out`, `view.zoom`,
//! `view.zoom_actual`, `view.zoom_page_width`) and page setup (`view.rule_lines`,
//! `view.page_color`, `view.paper_size`). The front end keeps the scroll position and
//! follows the zoom the session holds.

use notecraft_canvas::viewport::{self, MAX_PERCENT, MIN_PERCENT};
use notecraft_canvas::{PaperSize, RuleLines};
use serde_json::json;

use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::session::Session;
use crate::undo::{Op, Scope, Transaction};
use crate::view::PageStyle;

fn set_zoom(session: &mut Session, percent: u16) -> CommandOutput {
    if session.view().zoom_percent != percent {
        session.view_mut().zoom_percent = percent;
        session.mark_changed();
    }
    CommandOutput {
        value: json!({ "percent": percent }),
    }
}

/// `view.zoom_in`: the next zoom step up.
pub(super) fn zoom_in(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let percent = viewport::zoom_in_step(session.view().zoom_percent);
    Ok(set_zoom(session, percent))
}

/// `view.zoom_out`: the next zoom step down.
pub(super) fn zoom_out(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let percent = viewport::zoom_out_step(session.view().zoom_percent);
    Ok(set_zoom(session, percent))
}

/// `view.zoom { percent }`: 10 to 400.
pub(super) fn zoom(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let value = args.get("percent").and_then(Value::as_i64);
    let Some(percent) = value
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| (MIN_PERCENT..=MAX_PERCENT).contains(p))
    else {
        return Err(CommandError::InvalidArgs {
            id: ids::VIEW_ZOOM,
            reason: format!("percent must be {MIN_PERCENT} to {MAX_PERCENT}"),
        });
    };
    Ok(set_zoom(session, percent))
}

/// `view.zoom_actual`: 100%.
pub(super) fn zoom_actual(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    Ok(set_zoom(session, 100))
}

/// `view.zoom_page_width`: the zoom at which the page's width fills the canvas.
pub(super) fn zoom_page_width(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let width = session.page_style().paper_size.fit_width();
    // Canvas widths are far below 2^24, so the conversion is exact.
    let canvas = session.view().canvas_width as f32;
    let percent = viewport::fit_width_percent(width, canvas);
    Ok(set_zoom(session, percent))
}

/// The string argument `name`, already known to be present by the schema check.
fn string_arg<'a>(args: &'a Map<String, Value>, name: &str) -> &'a str {
    args.get(name).and_then(Value::as_str).unwrap_or_default()
}

fn invalid(id: CommandId, what: &str, value: &str, expected: &str) -> CommandError {
    let value: String = value.chars().take(64).collect();
    CommandError::InvalidArgs {
        id,
        reason: format!("unknown {what} {value:?}; expected {expected}"),
    }
}

/// Apply `change` to the shown page's style as one undo step called `label`. With no page
/// shown, or a style that stays the same, nothing changes.
fn restyle(session: &mut Session, label: &str, change: impl FnOnce(&mut PageStyle)) {
    let Some(page) = session.selection().page.clone() else {
        return;
    };
    let before = session.page_style();
    let mut after = before;
    change(&mut after);
    if after == before {
        return;
    }
    let op = Op::SetPageStyle {
        page: page.clone(),
        before,
        after,
    };
    session.commit(Transaction::new(Scope::Page(page), label).with(op));
}

/// `view.rule_lines { style }`
pub(super) fn rule_lines(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = string_arg(args, "style");
    let Some(style) = RuleLines::from_id(id) else {
        let expected = RuleLines::ALL.map(RuleLines::id).join(", ");
        return Err(invalid(ids::VIEW_RULE_LINES, "style", id, &expected));
    };
    restyle(session, "Rule lines", |page| page.rule_lines = style);
    Ok(CommandOutput {
        value: json!(style.id()),
    })
}

/// `view.rule_line_color { color }`: the color of the rule lines and grid, `#RRGGBB`,
/// `#RRGGBBAA`, or `none` for the theme's own.
pub(super) fn rule_line_color(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let text = string_arg(args, "color");
    let color = if text.eq_ignore_ascii_case("none") {
        None
    } else {
        let Some(rgba) = parse_hex(text) else {
            return Err(invalid(
                ids::VIEW_RULE_LINE_COLOR,
                "color",
                text,
                "#RRGGBB, #RRGGBBAA, or none",
            ));
        };
        Some(rgba)
    };
    restyle(session, "Rule line color", |page| page.rule_color = color);
    Ok(CommandOutput {
        value: json!(color.map_or_else(|| "none".to_owned(), format_hex)),
    })
}

/// `view.paper_size { size }`
pub(super) fn paper_size(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = string_arg(args, "size");
    let Some(size) = PaperSize::from_id(id) else {
        let expected = PaperSize::ALL.map(PaperSize::id).join(", ");
        return Err(invalid(ids::VIEW_PAPER_SIZE, "size", id, &expected));
    };
    restyle(session, "Paper size", |page| page.paper_size = size);
    Ok(CommandOutput {
        value: json!(size.id()),
    })
}

/// `view.page_color { color }`: `#RRGGBB`, `#RRGGBBAA`, or `none`.
pub(super) fn page_color(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let text = string_arg(args, "color");
    let color = if text.eq_ignore_ascii_case("none") {
        None
    } else {
        let Some(rgba) = parse_hex(text) else {
            return Err(invalid(
                ids::VIEW_PAGE_COLOR,
                "color",
                text,
                "#RRGGBB, #RRGGBBAA, or none",
            ));
        };
        Some(rgba)
    };
    restyle(session, "Page color", |page| page.color = color);
    Ok(CommandOutput {
        value: json!(color.map_or_else(|| "none".to_owned(), format_hex)),
    })
}

/// `#RRGGBB` (opaque) or `#RRGGBBAA`, case-insensitive. Anything else is `None`.
fn parse_hex(text: &str) -> Option<[u8; 4]> {
    let digits = text.strip_prefix('#')?;
    if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| {
        digits
            .get(i..i + 2)
            .and_then(|d| u8::from_str_radix(d, 16).ok())
    };
    let alpha = if digits.len() == 8 { byte(6)? } else { u8::MAX };
    Some([byte(0)?, byte(2)?, byte(4)?, alpha])
}

/// `#RRGGBB` when opaque, `#RRGGBBAA` otherwise; uppercase.
fn format_hex([r, g, b, a]: [u8; 4]) -> String {
    if a == u8::MAX {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

#[cfg(test)]
mod tests {
    use notecraft_canvas::{PaperSize, RuleLines};
    use notecraft_model::{Notebook, PageRef, Section, SectionNode};
    use serde_json::{json, Value};

    use super::parse_hex;
    use crate::cmd::Registry;
    use crate::session::Session;

    fn run(session: &mut Session, id: &str, args: Value) -> Result<Value, &'static str> {
        Registry::builtin()
            .run(session, id, args)
            .map(|out| out.value)
            .map_err(|err| err.code())
    }

    fn percent(session: &mut Session, id: &str) -> u64 {
        let out = run(session, id, json!(null)).unwrap();
        out["percent"].as_u64().unwrap()
    }

    /// A session showing one page, `p1`, with a second page `p2` beside it.
    fn with_pages() -> Session {
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
    fn zoom_in_and_out_step_through_the_levels() {
        let mut session = Session::new();
        assert_eq!(percent(&mut session, "view.zoom_in"), 125);
        assert_eq!(percent(&mut session, "view.zoom_in"), 150);
        assert_eq!(percent(&mut session, "view.zoom_out"), 125);
        for _ in 0..20 {
            percent(&mut session, "view.zoom_out");
        }
        assert_eq!(session.view().zoom_percent, 10);
        for _ in 0..20 {
            percent(&mut session, "view.zoom_in");
        }
        assert_eq!(session.view().zoom_percent, 400);
        assert_eq!(percent(&mut session, "view.zoom_actual"), 100);
    }

    #[test]
    fn zoom_takes_10_to_400_only() {
        let mut session = Session::new();
        let out = run(&mut session, "view.zoom", json!({ "percent": 250 }));
        assert_eq!(out, Ok(json!({ "percent": 250 })));
        let before = session.clone();
        for bad in [
            json!({ "percent": 9 }),
            json!({ "percent": 401 }),
            json!({ "percent": -100 }),
            json!({ "percent": u64::MAX }),
            json!({ "percent": 1.5 }),
            json!({ "percent": "100" }),
            json!({}),
        ] {
            assert_eq!(run(&mut session, "view.zoom", bad), Err("invalid_args"));
        }
        assert_eq!(session, before);
    }

    #[test]
    fn page_width_fits_the_paper_to_the_canvas() {
        let mut session = with_pages();
        // Before the canvas reports its width, Page Width falls back to 100%.
        assert_eq!(percent(&mut session, "view.zoom_page_width"), 100);
        session.view_mut().canvas_width = 1020;
        // An auto-sized page fits Letter width: 1020 / 816 = 125%.
        assert_eq!(percent(&mut session, "view.zoom_page_width"), 125);
        run(&mut session, "view.paper_size", json!({ "size": "a5" })).unwrap();
        // A5 is 559 units wide: 1020 / 559.4 = 182%.
        assert_eq!(percent(&mut session, "view.zoom_page_width"), 182);
    }

    #[test]
    fn rule_lines_and_paper_size_set_the_page_style() {
        let mut session = with_pages();
        for style in RuleLines::ALL {
            let out = run(
                &mut session,
                "view.rule_lines",
                json!({ "style": style.id() }),
            );
            assert_eq!(out, Ok(json!(style.id())));
            assert_eq!(session.page_style().rule_lines, style);
        }
        for size in PaperSize::ALL {
            let out = run(
                &mut session,
                "view.paper_size",
                json!({ "size": size.id() }),
            );
            assert_eq!(out, Ok(json!(size.id())));
            assert_eq!(session.page_style().paper_size, size);
        }
        let before = session.clone();
        assert_eq!(
            run(&mut session, "view.rule_lines", json!({ "style": "polka" })),
            Err("invalid_args")
        );
        assert_eq!(
            run(
                &mut session,
                "view.paper_size",
                json!({ "size": "tabloid" })
            ),
            Err("invalid_args")
        );
        assert_eq!(session, before);
    }

    #[test]
    fn page_style_belongs_to_the_page() {
        let mut session = with_pages();
        run(
            &mut session,
            "view.rule_lines",
            json!({ "style": "college" }),
        )
        .unwrap();
        run(&mut session, "nav.page.select", json!({ "id": "p2" })).unwrap();
        assert_eq!(session.page_style().rule_lines, RuleLines::None);
        run(&mut session, "nav.page.select", json!({ "id": "p1" })).unwrap();
        assert_eq!(session.page_style().rule_lines, RuleLines::College);
    }

    #[test]
    fn page_color_takes_hex_or_none() {
        let mut session = with_pages();
        let out = run(
            &mut session,
            "view.page_color",
            json!({ "color": "#dcebf7" }),
        );
        assert_eq!(out, Ok(json!("#DCEBF7")));
        assert_eq!(session.page_style().color, Some([0xDC, 0xEB, 0xF7, 0xFF]));
        let out = run(
            &mut session,
            "view.page_color",
            json!({ "color": "#11223380" }),
        );
        assert_eq!(out, Ok(json!("#11223380")));
        let out = run(&mut session, "view.page_color", json!({ "color": "none" }));
        assert_eq!(out, Ok(json!("none")));
        assert_eq!(session.page_style().color, None);
        for bad in [
            "red",
            "#12345",
            "#1234567",
            "123456",
            "#GG0000",
            "",
            "#\u{e9}e9e9e",
        ] {
            assert_eq!(
                run(&mut session, "view.page_color", json!({ "color": bad })),
                Err("invalid_args"),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn rule_line_color_takes_hex_or_none_and_undoes() {
        let mut session = with_pages();
        let out = run(
            &mut session,
            "view.rule_line_color",
            json!({ "color": "#9fb6cd" }),
        );
        assert_eq!(out, Ok(json!("#9FB6CD")));
        assert_eq!(
            session.page_style().rule_color,
            Some([0x9F, 0xB6, 0xCD, 0xFF])
        );
        run(&mut session, "edit.undo", json!(null)).unwrap();
        assert_eq!(session.page_style().rule_color, None);
        run(&mut session, "edit.redo", json!(null)).unwrap();
        run(
            &mut session,
            "view.rule_line_color",
            json!({ "color": "none" }),
        )
        .unwrap();
        assert_eq!(session.page_style().rule_color, None);
        assert_eq!(
            run(
                &mut session,
                "view.rule_line_color",
                json!({ "color": "blue" })
            ),
            Err("invalid_args")
        );
    }

    #[test]
    fn setting_the_same_style_or_zoom_changes_nothing() {
        let mut session = with_pages();
        run(&mut session, "view.zoom_actual", json!(null)).unwrap();
        run(&mut session, "view.rule_lines", json!({ "style": "none" })).unwrap();
        run(&mut session, "view.paper_size", json!({ "size": "auto" })).unwrap();
        run(&mut session, "view.page_color", json!({ "color": "none" })).unwrap();
        assert_eq!(session.revision(), 0);
        assert_eq!(session, with_pages());
    }

    #[test]
    fn page_setup_with_no_page_shown_does_nothing() {
        let mut session = Session::new();
        run(&mut session, "view.rule_lines", json!({ "style": "wide" })).unwrap();
        assert_eq!(session.page_style().rule_lines, RuleLines::None);
        assert_eq!(session.revision(), 0);
    }

    #[test]
    fn hex_parsing_is_strict() {
        assert_eq!(parse_hex("#000000"), Some([0, 0, 0, 255]));
        assert_eq!(parse_hex("#FFffFF00"), Some([255, 255, 255, 0]));
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#+12345"), None);
        assert_eq!(parse_hex("##123456"), None);
    }
}
