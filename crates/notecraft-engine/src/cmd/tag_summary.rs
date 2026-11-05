//! Tags Summary commands (build_plan/04 PR 2.6): `tag.find`, `tag.summary.set_scope`,
//! `tag.summary.set_group`, `tag.summary.set_unchecked`, `tag.summary.refresh`,
//! `tag.summary.open_item`, `tag.summary.create_page`, `tag.summary.close`. The session keeps
//! the scope, grouping, and last results; the front end shows them.

use serde_json::json;

use super::theme::{invalid, string_arg};
use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::search::SearchScope;
use crate::session::{Session, TextCursor};
use crate::tag_summary::SummaryGroup;

/// The title of a summary page, as in OneNote.
const PAGE_TITLE: &str = "Tags Summary";

fn bad(id: CommandId, reason: &str) -> CommandError {
    CommandError::InvalidArgs {
        id,
        reason: reason.to_owned(),
    }
}

/// Run the query the state describes and keep the result.
fn rerun(session: &mut Session) {
    let (scope, group, only_unchecked) = {
        let state = session.tag_summary();
        (state.scope, state.group, state.only_unchecked)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let summary = {
        let roots: Vec<crate::search::SearchRoot> = session
            .notebooks()
            .iter()
            .filter_map(|nb| {
                let path = session
                    .notebook_roots()
                    .find_map(|(id, path)| (id == nb.id).then(|| path.to_path_buf()))?;
                Some(crate::search::SearchRoot {
                    id: nb.id.clone(),
                    name: nb.name.clone(),
                    path,
                })
            })
            .collect();
        let tags = session.tags();
        crate::tag_summary::tag_summary(
            &roots,
            session.selection(),
            scope,
            group,
            only_unchecked,
            &|slug| {
                tags.get(slug)
                    .map_or_else(|| slug.to_owned(), |def| def.name.clone())
            },
        )
    };
    #[cfg(target_arch = "wasm32")]
    let summary = crate::tag_summary::TagSummary {
        scope,
        group,
        only_unchecked,
        ..crate::tag_summary::TagSummary::default()
    };
    session.tag_summary_mut().summary = Some(summary);
    session.mark_changed();
}

fn summary_value(session: &Session) -> Value {
    let Some(summary) = &session.tag_summary().summary else {
        return Value::Null;
    };
    let groups: Vec<Value> = summary
        .groups
        .iter()
        .map(|group| {
            let items: Vec<Value> = group
                .items
                .iter()
                .map(|item| {
                    json!({
                        "tag": item.tag,
                        "tag_name": item.tag_name,
                        "checked": item.checked,
                        "text": item.text,
                        "notebook": item.notebook_id,
                        "section": item.section_id,
                        "page": item.page_id,
                        "title": item.page_title,
                        "modified": item.modified,
                        "at": item.at,
                    })
                })
                .collect();
            json!({ "label": group.label, "items": items })
        })
        .collect();
    json!({
        "scope": summary.scope.id(),
        "group": summary.group.id(),
        "only_unchecked": summary.only_unchecked,
        "count": summary.count(),
        "truncated": summary.truncated,
        "groups": groups,
    })
}

fn scope_arg(id: CommandId, args: &Map<String, Value>) -> Result<SearchScope, CommandError> {
    let text = string_arg(args, "scope");
    SearchScope::from_id(text)
        .ok_or_else(|| invalid(id, "scope", text, &SearchScope::LIST.map(SearchScope::id)))
}

fn group_arg(id: CommandId, args: &Map<String, Value>) -> Result<SummaryGroup, CommandError> {
    let text = string_arg(args, "group");
    SummaryGroup::from_id(text)
        .ok_or_else(|| invalid(id, "group", text, &SummaryGroup::LIST.map(SummaryGroup::id)))
}

/// `tag.find { scope?, group?, only_unchecked? }`: show the Tags Summary pane and fill it.
/// Returns the summary as JSON.
pub(super) fn find(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TAG_FIND;
    let scope = args
        .contains_key("scope")
        .then(|| scope_arg(id, args))
        .transpose()?;
    let group = args
        .contains_key("group")
        .then(|| group_arg(id, args))
        .transpose()?;
    let state = session.tag_summary_mut();
    if let Some(scope) = scope {
        state.scope = scope;
    }
    if let Some(group) = group {
        state.group = group;
    }
    if let Some(only) = args.get("only_unchecked").and_then(Value::as_bool) {
        state.only_unchecked = only;
    }
    state.open = true;
    // One side pane at a time: the search results make room.
    session.search_mut().open = false;
    rerun(session);
    Ok(CommandOutput {
        value: summary_value(session),
    })
}

/// `tag.summary.set_scope { scope }`
pub(super) fn set_scope(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let scope = scope_arg(ids::TAG_SUMMARY_SET_SCOPE, args)?;
    session.tag_summary_mut().scope = scope;
    rerun(session);
    Ok(CommandOutput {
        value: json!(scope.id()),
    })
}

/// `tag.summary.set_group { group }`
pub(super) fn set_group(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let group = group_arg(ids::TAG_SUMMARY_SET_GROUP, args)?;
    session.tag_summary_mut().group = group;
    rerun(session);
    Ok(CommandOutput {
        value: json!(group.id()),
    })
}

/// `tag.summary.set_unchecked { value }`: show only the unchecked To Do items (items of other
/// tags stay).
pub(super) fn set_unchecked(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let value = args.get("value").and_then(Value::as_bool).ok_or_else(|| {
        bad(
            ids::TAG_SUMMARY_SET_UNCHECKED,
            "value must be true or false",
        )
    })?;
    session.tag_summary_mut().only_unchecked = value;
    rerun(session);
    Ok(CommandOutput {
        value: json!(value),
    })
}

/// Show the new tag names in a Tags Summary that is already on screen (after a tag was renamed
/// or reset). The index keeps tag ids, so only the names change; the query runs again.
pub(super) fn names_changed(session: &mut Session) {
    if session.tag_summary().summary.is_some() {
        rerun(session);
    }
}

/// `tag.summary.refresh`: read the index again.
pub(super) fn refresh(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    rerun(session);
    Ok(CommandOutput {
        value: summary_value(session),
    })
}

/// `tag.summary.close`: hide the pane. The choices are kept for next time.
pub(super) fn close(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    if session.tag_summary().open {
        session.tag_summary_mut().open = false;
        session.mark_changed();
    }
    Ok(CommandOutput::default())
}

/// Where the item now is in the page text, when the page is loaded: the same tag and text
/// closest to where the index saw it. The index can be a little behind the text being edited.
#[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
fn live_offset(session: &Session, page: &str, at: usize, tag: &str, text: &str) -> usize {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(live) = session.page_text(page) {
        let best = crate::tags_index::rows_for_text(live)
            .into_iter()
            .filter(|row| row.tag == tag && row.text == text)
            .min_by_key(|row| row.holder_start.abs_diff(at));
        if let Some(row) = best {
            return row.holder_start;
        }
    }
    at
}

/// `tag.summary.open_item { page, at, tag?, text? }`: show the page and put the caret at the
/// paragraph that starts at byte `at`. With `tag` and `text` the paragraph is looked up again
/// in the page text, in case the page changed since the index was read.
pub(super) fn open_item(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TAG_SUMMARY_OPEN_ITEM;
    let page = string_arg(args, "page");
    let at = args
        .get("at")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| bad(id, "at must be a non-negative integer"))?;
    let found = session.notebooks().iter().find_map(|nb| {
        nb.section_of_page(page)
            .map(|s| (nb.id.clone(), s.id.clone()))
    });
    let Some((notebook, section)) = found else {
        return Err(bad(id, "no page with that id"));
    };
    let selection = crate::session::Selection {
        notebook: Some(notebook),
        section: Some(section),
        page: Some(page.to_owned()),
    };
    if *session.selection() != selection {
        *session.selection_mut() = selection;
        session.mark_changed();
    }
    let at = live_offset(
        session,
        page,
        at,
        string_arg(args, "tag"),
        string_arg(args, "text"),
    );
    // The caret is read back snapped to the text, also when the page is read a moment later.
    session.set_text_cursor(page, TextCursor::caret(at));
    Ok(CommandOutput {
        value: json!({ "page": page, "at": at }),
    })
}

/// `tag.summary.create_page`: write a page named "Tags Summary" (numbered when taken) in the
/// section shown, listing the items the pane shows, grouped the same way. Each item keeps its
/// tag and links to its page. One undo step. Returns `{ created, page }`.
pub(super) fn create_page(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TAG_SUMMARY_CREATE_PAGE;
    let section = session
        .selection()
        .section
        .clone()
        .ok_or_else(|| bad(id, "no section is shown"))?;
    let Some(summary) = session
        .tag_summary()
        .summary
        .clone()
        .filter(|s| s.count() > 0)
    else {
        return Ok(CommandOutput {
            value: json!({ "created": false, "page": null }),
        });
    };
    let current = session.selection().notebook.clone();
    let pages = session.link_pages();
    let index = session.link_index();
    let page = session
        .create_page_with(&section, Some(PAGE_TITLE), &|path| {
            let link = |item: &crate::tag_summary::SummaryItem| -> Option<String> {
                if current.as_deref() != Some(item.notebook_id.as_str()) {
                    return None;
                }
                let page = pages.iter().find(|p| p.id == item.page_id)?;
                Some(index.shortest_form(page, path))
            };
            crate::tag_summary::summary_page_text(&summary, &link)
        })
        .map_err(|err| bad(id, &err.0))?;
    Ok(CommandOutput {
        value: json!({ "created": true, "page": page }),
    })
}
