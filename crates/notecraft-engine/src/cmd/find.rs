//! Find on page commands (TX-11): `edit.find`, `edit.find.set_query`, `edit.find.next`,
//! `edit.find.previous`, and `edit.find.close`. They drive the session's find bar state; the
//! front end draws the bar and the highlights. None of them changes the page, so none is
//! undoable. Each returns `{ open, query, count, current }` where `current` is the 1-based
//! number of the match the user is on (0 with no matches).

use serde_json::json;

use super::{CommandError, CommandOutput, Map, Value};
use crate::session::Session;

fn status(session: &Session) -> CommandOutput {
    let count = session.find_matches().len();
    let state = session.find();
    let current = state.current_of(count).map_or(0, |i| i + 1);
    CommandOutput {
        value: json!({
            "open": state.open,
            "query": state.query,
            "count": count,
            "current": current,
        }),
    }
}

/// `edit.find { query? }`: show the find bar, with the given query if there is one (else the
/// last one), and put the keyboard in it.
pub(super) fn open(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    session.open_find();
    if let Some(query) = args.get("query").and_then(Value::as_str) {
        session.set_find_query(query);
    }
    Ok(status(session))
}

/// `edit.find.set_query { query }`: change what is found and go to the first match.
pub(super) fn set_query(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    session.set_find_query(query);
    Ok(status(session))
}

/// `edit.find.next`: go to the next match, wrapping to the first.
pub(super) fn next(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    session.step_find(true);
    Ok(status(session))
}

/// `edit.find.previous`: go to the previous match, wrapping to the last.
pub(super) fn previous(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    session.step_find(false);
    Ok(status(session))
}

/// `edit.find.close`: hide the find bar.
pub(super) fn close(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    session.close_find();
    Ok(status(session))
}
