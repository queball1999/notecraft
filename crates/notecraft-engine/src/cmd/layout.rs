//! Window layout commands: `view.ribbon.toggle`, `view.full_page`, `view.normal`.

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

fn set_full_page(session: &mut Session, full_page: bool) -> CommandOutput {
    if session.view().full_page != full_page {
        session.view_mut().full_page = full_page;
        session.mark_changed();
    }
    CommandOutput {
        value: json!({ "full_page": full_page }),
    }
}

/// `view.full_page`: show only the page, or go back to the normal view (F11 both ways, as in
/// OneNote).
pub(super) fn full_page(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let full_page = !session.view().full_page;
    Ok(set_full_page(session, full_page))
}

/// `view.normal`: show the ribbon and the navigation panes again.
pub(super) fn normal(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    Ok(set_full_page(session, false))
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

    #[test]
    fn full_page_toggles_and_normal_leaves() {
        let registry = Registry::builtin();
        let mut session = Session::new();
        let full = ids::VIEW_FULL_PAGE.as_str();
        let normal = ids::VIEW_NORMAL.as_str();
        let out = registry.run(&mut session, full, json!(null)).unwrap();
        assert_eq!(out.value, json!({ "full_page": true }));
        registry.run(&mut session, full, json!(null)).unwrap();
        assert!(!session.view().full_page);
        registry.run(&mut session, full, json!(null)).unwrap();
        registry.run(&mut session, normal, json!(null)).unwrap();
        assert!(!session.view().full_page);
        registry.run(&mut session, normal, json!(null)).unwrap();
        assert_eq!(
            session.revision(),
            4,
            "normal view when already normal is not a change"
        );
    }
}
