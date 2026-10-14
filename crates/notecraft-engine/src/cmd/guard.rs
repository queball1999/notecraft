//! The command guard: the last line of the never-crash rule (AGENTS.md section 3). A command
//! that panics anyway, or fails, leaves the session exactly as it was before it ran.

use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

use super::{CommandError, CommandId, CommandOutput};
use crate::session::Session;

/// Run `command` on `session`. On a panic: log it, restore the session, and return
/// [`CommandError::Internal`]. On an error: restore the session and return the error.
///
/// The rollback is a snapshot of the session taken before the command runs. That is cheap
/// while the session holds no notes; Phase 1 replaces it with transaction rollback
/// (docs/architecture.md section 6).
pub(super) fn run_guarded(
    id: CommandId,
    session: &mut Session,
    command: impl FnOnce(&mut Session) -> Result<CommandOutput, CommandError>,
) -> Result<CommandOutput, CommandError> {
    let snapshot = session.clone();
    // AssertUnwindSafe: if the command unwinds, the session is replaced by the snapshot
    // below, so no half-changed state is ever observed.
    match catch_unwind(AssertUnwindSafe(|| command(session))) {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(err)) => {
            *session = snapshot;
            Err(err)
        }
        Err(payload) => {
            *session = snapshot;
            let message = panic_message(payload.as_ref());
            tracing::error!("command {id} panicked: {message}");
            Err(CommandError::Internal { id, message })
        }
    }
}

/// The text of a panic payload (`panic!` with a literal or a formatted message).
fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "unknown panic".to_owned()
    }
}
