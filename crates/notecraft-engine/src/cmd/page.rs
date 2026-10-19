//! Page file commands: `page.convert_to_markdown` and `page.repair_canvas`.

use serde_json::json;

use super::{CommandError, CommandOutput, Map, Value};
use crate::session::{ConvertOutcome, Session};

/// `page.convert_to_markdown { confirm? }`: turn the `.notecraft` page shown into a plain
/// `.md` page. Without `confirm: true` nothing changes and the result carries the warning to
/// show first (`needs_confirm`). The conversion drops ink and note positions, so it is not
/// undoable; the page's undo steps are cleared. Returns `{ converted, ... }`.
pub(super) fn convert_to_markdown(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let confirm = args
        .get("confirm")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let value = match session.convert_current_page_to_markdown(confirm) {
        ConvertOutcome::NotApplicable => json!({ "converted": false }),
        ConvertOutcome::NeedsConfirm { warning } => {
            json!({ "converted": false, "needs_confirm": true, "warning": warning })
        }
        ConvertOutcome::Converted { new_id } => json!({ "converted": true, "page": new_id }),
    };
    Ok(CommandOutput { value })
}

/// `page.repair_canvas`: replace the unreadable canvas block of the `.notecraft` page shown
/// with a fresh one that keeps what could be read from it. Returns `{ repaired }`. The old
/// file is already in the notebook history folder (copied when the page opened); the page
/// becomes editable and unsaved, and its undo steps are cleared.
pub(super) fn repair_canvas(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let repaired = session.repair_current_canvas();
    Ok(CommandOutput {
        value: json!({ "repaired": repaired }),
    })
}
