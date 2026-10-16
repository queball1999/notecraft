//! `edit.undo` and `edit.redo`: walk the session's history (see [`crate::undo`]). With nothing
//! to undo or redo they do nothing and report `null`, as a disabled button would.

use serde_json::json;

use super::{CommandError, CommandOutput, Map, Value};
use crate::session::Session;

fn output(label: Option<String>) -> CommandOutput {
    CommandOutput {
        value: label.map_or(Value::Null, |label| json!({ "label": label })),
    }
}

/// `edit.undo`: undo the latest step for the page shown. Returns `{ "label" }` of the step.
pub(super) fn undo(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    Ok(output(session.undo()))
}

/// `edit.redo`: redo the step undone last. Returns `{ "label" }` of the step.
pub(super) fn redo(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    Ok(output(session.redo()))
}
