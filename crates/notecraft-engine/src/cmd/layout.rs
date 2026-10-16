//! Window layout commands: `view.ribbon.toggle`, `view.ribbon.layout`, `view.ribbon.show`,
//! `view.full_page`, `view.normal`.

use serde_json::json;

use super::theme::{invalid, string_arg};
use super::{ids, CommandError, CommandOutput, Map, Value};
use crate::session::Session;
use crate::view::{RibbonLayout, RibbonShow};

fn set_ribbon_show(session: &mut Session, show: RibbonShow) -> CommandOutput {
    if session.view().ribbon_show != show {
        session.view_mut().ribbon_show = show;
        session.mark_changed();
    }
    CommandOutput {
        value: json!({ "show": show.id(), "collapsed": show == RibbonShow::TabsOnly }),
    }
}

/// `view.ribbon.toggle` (Ctrl+F1): show only the ribbon's tab names, or show it all again.
/// From full-screen mode it shows the whole ribbon, as OneNote does.
pub(super) fn ribbon_toggle(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let show = match session.view().ribbon_show {
        RibbonShow::Always => RibbonShow::TabsOnly,
        RibbonShow::TabsOnly | RibbonShow::FullScreen => RibbonShow::Always,
    };
    Ok(set_ribbon_show(session, show))
}

/// `view.ribbon.show { mode }`
pub(super) fn ribbon_show(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = string_arg(args, "mode");
    let Some(show) = RibbonShow::from_id(id) else {
        let expected = RibbonShow::ALL.map(RibbonShow::id);
        return Err(invalid(ids::VIEW_RIBBON_SHOW, "mode", id, &expected));
    };
    Ok(set_ribbon_show(session, show))
}

/// `view.ribbon.layout { layout }`
pub(super) fn ribbon_layout(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = string_arg(args, "layout");
    let Some(layout) = RibbonLayout::from_id(id) else {
        let expected = RibbonLayout::ALL.map(RibbonLayout::id);
        return Err(invalid(ids::VIEW_RIBBON_LAYOUT, "layout", id, &expected));
    };
    if session.view().ribbon_layout != layout {
        session.view_mut().ribbon_layout = layout;
        session.mark_changed();
    }
    Ok(CommandOutput {
        value: json!(layout.id()),
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
    use crate::view::{RibbonLayout, RibbonShow};

    #[test]
    fn toggle_collapses_and_expands() {
        let registry = Registry::builtin();
        let mut session = Session::new();
        assert_eq!(session.view().ribbon_show, RibbonShow::Always);
        let id = ids::VIEW_RIBBON_TOGGLE.as_str();
        let out = registry.run(&mut session, id, json!(null)).unwrap();
        assert_eq!(out.value, json!({ "show": "tabs", "collapsed": true }));
        assert_eq!(session.view().ribbon_show, RibbonShow::TabsOnly);
        registry.run(&mut session, id, json!(null)).unwrap();
        assert_eq!(session.view().ribbon_show, RibbonShow::Always);
        assert_eq!(session.revision(), 2);
    }

    #[test]
    fn show_sets_each_mode_and_toggle_leaves_full_screen() {
        let registry = Registry::builtin();
        let mut session = Session::new();
        let show = ids::VIEW_RIBBON_SHOW.as_str();
        for mode in RibbonShow::ALL {
            registry
                .run(&mut session, show, json!({ "mode": mode.id() }))
                .unwrap();
            assert_eq!(session.view().ribbon_show, mode);
        }
        let toggle = ids::VIEW_RIBBON_TOGGLE.as_str();
        registry.run(&mut session, toggle, json!(null)).unwrap();
        assert_eq!(session.view().ribbon_show, RibbonShow::Always);
        let before = session.clone();
        for args in [json!({ "mode": "hidden" }), json!({}), json!({ "mode": 1 })] {
            let err = registry.run(&mut session, show, args).unwrap_err();
            assert_eq!(err.code(), "invalid_args");
        }
        assert_eq!(session, before);
    }

    #[test]
    fn layout_switches_and_rejects_unknown_layouts() {
        let registry = Registry::builtin();
        let mut session = Session::new();
        let id = ids::VIEW_RIBBON_LAYOUT.as_str();
        let out = registry
            .run(&mut session, id, json!({ "layout": "simplified" }))
            .unwrap();
        assert_eq!(out.value, json!("simplified"));
        assert_eq!(session.view().ribbon_layout, RibbonLayout::Simplified);
        registry
            .run(&mut session, id, json!({ "layout": "simplified" }))
            .unwrap();
        assert_eq!(session.revision(), 1, "the same layout is not a change");
        let err = registry
            .run(&mut session, id, json!({ "layout": "compact" }))
            .unwrap_err();
        assert_eq!(err.code(), "invalid_args");
        assert_eq!(session.view().ribbon_layout, RibbonLayout::Simplified);
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
