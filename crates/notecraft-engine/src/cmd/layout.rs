//! Window layout commands: `view.ribbon.toggle`.

use serde_json::json;

use super::{CommandError, CommandOutput, Map, Value};
use crate::session::Session;

/// `view.ribbon.toggle`: collapse the ribbon to its tab names, or expand it again.
pub(super) fn ribbon_toggle(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let view = session.view_mut();
    view.ribbon_collapsed = !view.ribbon_collapsed;
    let collapsed = view.ribbon_collapsed;
    session.mark_changed();
    Ok(CommandOutput {
        value: json!({ "collapsed": collapsed }),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::cmd::{ids, Registry};
    use crate::session::Session;

    #[test]
    fn toggle_collapses_and_expands() {
        let registry = Registry::builtin();
        let mut session = Session::new();
        assert!(!session.view().ribbon_collapsed);
        let id = ids::VIEW_RIBBON_TOGGLE.as_str();
        let out = registry.run(&mut session, id, json!(null)).unwrap();
        assert_eq!(out.value, json!({ "collapsed": true }));
        assert!(session.view().ribbon_collapsed);
        registry.run(&mut session, id, json!(null)).unwrap();
        assert!(!session.view().ribbon_collapsed);
        assert_eq!(session.revision(), 2);
    }
}
