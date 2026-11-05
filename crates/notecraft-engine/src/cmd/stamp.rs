//! Insert > Date, Time, and Date & Time (TX-13): `insert.date`, `insert.time`, and
//! `insert.date_time`. Each types the current stamp at the caret (replacing the selection) as
//! one undo step. The time comes from the session's clock ([`crate::stamp::Clock`]).

use serde_json::json;

use super::text::{for_page, Ctx};
use super::{CommandError, CommandOutput, Map, Value};
use crate::session::Session;
use crate::stamp::Stamp;

fn insert_stamp(
    session: &mut Session,
    format: fn(&Stamp) -> String,
) -> Result<CommandOutput, CommandError> {
    let applied = |done: bool| {
        Ok(CommandOutput {
            value: json!({ "applied": done }),
        })
    };
    // No clock, no page to edit, or a frozen page: nothing happens, and nothing is an error.
    let Some(stamp) = session.now() else {
        return applied(false);
    };
    let Some(ctx) = Ctx::new(session) else {
        return applied(false);
    };
    let body = for_page(&ctx, &format(&stamp));
    let Some(edit) = notecraft_canvas::text::insert(ctx.cursor.anchor, ctx.cursor.head, &body)
    else {
        return applied(false);
    };
    applied(ctx.apply(session, edit, false))
}

/// `insert.date`: the date, such as `11/5/2026`.
pub(super) fn date(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    insert_stamp(session, Stamp::date)
}

/// `insert.time`: the time, such as `3:07 PM`.
pub(super) fn time(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    insert_stamp(session, Stamp::time)
}

/// `insert.date_time`: the date and time, such as `11/5/2026 3:07 PM`.
pub(super) fn date_time(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    insert_stamp(session, Stamp::date_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::Registry;
    use crate::stamp::Clock;
    use crate::undo::tests::session_with_pages;

    const PAGE: &str = "p1";

    fn session(text: &str) -> Session {
        let mut session = session_with_pages();
        session.set_page_source(PAGE, text);
        let stamp = Stamp::new(2026, 11, 5, 15, 7).expect("stamp");
        session.set_clock(Clock::fixed(stamp));
        session
    }

    fn run(session: &mut Session, id: &str, args: Value) -> Value {
        match Registry::builtin().run(session, id, args) {
            Ok(out) => out.value,
            Err(err) => panic!("{id} failed: {err}"),
        }
    }

    fn text(session: &Session) -> String {
        session.page_text(PAGE).unwrap_or_default().to_owned()
    }

    #[test]
    fn each_command_types_its_stamp_at_the_caret_as_one_undo_step() {
        for (id, stamp) in [
            ("insert.date", "11/5/2026"),
            ("insert.time", "3:07 PM"),
            ("insert.date_time", "11/5/2026 3:07 PM"),
        ] {
            let mut s = session("ab\n");
            run(&mut s, "text.set_caret", json!({ "at": 1 }));
            let out = run(&mut s, id, Value::Null);
            assert_eq!(out["applied"], json!(true), "{id}");
            assert_eq!(text(&s), format!("a{stamp}b\n"), "{id}");
            run(&mut s, "edit.undo", Value::Null);
            assert_eq!(text(&s), "ab\n", "{id}");
            run(&mut s, "edit.redo", Value::Null);
            assert_eq!(text(&s), format!("a{stamp}b\n"), "{id}");
        }
    }

    #[test]
    fn a_selection_is_replaced() {
        let mut s = session("one two\n");
        run(&mut s, "text.set_caret", json!({ "at": 0 }));
        run(&mut s, "text.set_caret", json!({ "at": 3, "extend": true }));
        run(&mut s, "insert.date", Value::Null);
        assert_eq!(text(&s), "11/5/2026 two\n");
    }

    #[test]
    fn nothing_happens_without_a_clock_or_a_page() {
        let mut s = session("ab\n");
        s.set_clock(Clock::new(|| None));
        let out = run(&mut s, "insert.time", Value::Null);
        assert_eq!(out["applied"], json!(false));
        assert_eq!(text(&s), "ab\n");

        let mut empty = Session::new();
        empty.set_clock(Clock::fixed(Stamp::new(2026, 1, 2, 3, 4).expect("stamp")));
        let out = run(&mut empty, "insert.date_time", Value::Null);
        assert_eq!(out["applied"], json!(false));
    }

    #[test]
    fn crlf_pages_keep_their_line_endings() {
        let mut s = session("a\r\nb\r\n");
        run(&mut s, "text.set_caret", json!({ "at": 1 }));
        run(&mut s, "insert.time", Value::Null);
        assert_eq!(text(&s), "a3:07 PM\r\nb\r\n");
    }

    #[test]
    fn the_shortcuts_are_bound_once() {
        let map = crate::cmd::shortcuts::default_map();
        for (chord, id) in [
            ("Alt+Shift+D", "insert.date"),
            ("Alt+Shift+T", "insert.time"),
            ("Alt+Shift+F", "insert.date_time"),
        ] {
            let found: Vec<_> = map.iter().filter(|b| b.text == chord).collect();
            assert_eq!(found.len(), 1, "{chord}");
            assert_eq!(found[0].command.as_str(), id);
        }
    }
}
