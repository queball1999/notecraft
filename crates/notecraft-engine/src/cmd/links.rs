//! Page link commands (build_plan/04 PR 2.2): `link.follow`, `link.picker.open`,
//! `link.picker.choose`, `link.picker.move`, `link.picker.close`.
//!
//! Following a link selects the page it points to, in any section of the notebook. A link that
//! points nowhere creates the page in the current section first, as OneNote does; that is the
//! ordinary `page.new` path, so it is one undo step. Typing `[[` opens the picker from
//! `text.insert`; the picker's state lives in the session and the page text it filters is read
//! from the caret, so the front end only shows what [`Session::link_picker_view`] returns.

use notecraft_canvas::text::TextEdit;
use serde_json::json;

use super::text::Ctx;
use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::session::{Selection, Session};

fn invalid(id: CommandId, reason: &str) -> CommandError {
    CommandError::InvalidArgs {
        id,
        reason: reason.to_owned(),
    }
}

fn applied(done: bool) -> CommandOutput {
    CommandOutput {
        value: json!({ "applied": done }),
    }
}

/// The last part of a link target such as `Section/Page`: the name of a page to create.
fn page_name_of(target: &str) -> &str {
    let name = target.rsplit('/').next().unwrap_or(target).trim();
    if name.is_empty() {
        target.trim()
    } else {
        name
    }
}

/// Show page `id`, selecting its section and notebook too. `false` for an unknown page.
fn show_page(session: &mut Session, id: &str) -> bool {
    let found = session.notebooks().iter().find_map(|nb| {
        nb.section_of_page(id)
            .map(|s| (nb.id.clone(), s.id.clone()))
    });
    let Some((notebook, section)) = found else {
        return false;
    };
    let selection = Selection {
        notebook: Some(notebook),
        section: Some(section),
        page: Some(id.to_owned()),
    };
    if *session.selection() != selection {
        *session.selection_mut() = selection;
        session.mark_changed();
    }
    true
}

/// `link.follow { target, create? }`: go to the page a link written on the page shown points
/// to (the closest one when it is ambiguous). When no page matches and `create` is not
/// `false`, add a page named like the target to the current section and show it. Returns
/// `{ page, created, ambiguous }`; `page` is null when nothing matched and nothing was
/// created.
pub(super) fn follow(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::LINK_FOLLOW;
    let target = args
        .get("target")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| invalid(id, "target must be a non-empty string"))?;
    let create = args.get("create").and_then(Value::as_bool).unwrap_or(true);
    let resolution = session.resolve_page_link(target);
    let (page, created) = match resolution.page.clone() {
        Some(page) => (Some(page), false),
        None if create => {
            let section = session
                .selection()
                .section
                .clone()
                .ok_or_else(|| invalid(id, "no section is shown to add the page to"))?;
            let made = session
                .create_page(&section, Some(page_name_of(target)))
                .map_err(|err| invalid(id, &err.0))?;
            (Some(made), true)
        }
        None => (None, false),
    };
    if let Some(page) = page.as_deref().filter(|_| !created) {
        if !show_page(session, page) {
            return Err(invalid(id, "the page is no longer in the notebook"));
        }
    }
    Ok(CommandOutput {
        value: json!({
            "page": page,
            "created": created,
            "ambiguous": resolution.ambiguous,
        }),
    })
}

/// After `inserted` was typed with `text.insert`: open the picker if the text just before the
/// caret holds `[[` that the typing completed or came after (not inside a code block, and not
/// when text is selected). A single keystroke is the usual case; a longer insert (an IME
/// commit) is looked through as a whole.
pub(super) fn after_insert(session: &mut Session, inserted_len: usize) {
    let Some(ctx) = Ctx::new(session) else {
        return;
    };
    let head = ctx.cursor.head;
    if !ctx.cursor.is_caret() || ctx.in_code(head) {
        return;
    }
    // One byte more than the insert, so a `[` typed after a `[` is seen.
    let mut from = head.saturating_sub(inserted_len + 1);
    while !ctx.text.is_char_boundary(from) {
        from += 1;
    }
    let Some(found) = ctx.text.get(from..head).and_then(|t| t.rfind("[[")) else {
        return;
    };
    session.open_link_picker(&ctx.page, from + found + 2);
}

/// `link.picker.open`: open the page picker at the caret. When the text before the caret is not
/// already `[[`, the brackets are typed first (replacing a selection).
pub(super) fn picker_open(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let head = ctx.cursor.head;
    let already = ctx.cursor.is_caret() && head >= 2 && ctx.text.get(head - 2..head) == Some("[[");
    if already {
        session.open_link_picker(&ctx.page, head);
        return Ok(applied(true));
    }
    let Some(edit) = notecraft_canvas::text::insert(ctx.cursor.anchor, ctx.cursor.head, "[[")
    else {
        return Ok(applied(false));
    };
    let page = ctx.page.clone();
    if !ctx.apply(session, edit, false) {
        return Ok(applied(false));
    }
    let at = session.text_cursor().map_or(0, |c| c.head);
    session.open_link_picker(&page, at);
    Ok(applied(true))
}

/// `link.picker.choose { page? }`: replace what was typed since `[[` with a link to `page`
/// (the highlighted entry when none is given), written in the shortest form that resolves to
/// that page and no other. The link and the closing brackets are one undo step.
pub(super) fn picker_choose(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::LINK_PICKER_CHOOSE;
    let Some(view) = session.link_picker_view() else {
        session.close_link_picker();
        return Ok(applied(false));
    };
    let chosen = match args.get("page").and_then(Value::as_str) {
        Some(page) => page.to_owned(),
        None => match view.entries.get(view.selected) {
            Some(entry) => entry.page.clone(),
            None => return Ok(applied(false)),
        },
    };
    let index = session.link_index();
    let Some(page) = index.page(&chosen) else {
        return Err(invalid(id, "no such page in the notebook"));
    };
    let from = session.current_page_path().unwrap_or_default();
    let form = index.shortest_form(page, &from).replace('|', "\\|");
    let (Some(picker), Some(ctx)) = (session.link_picker().cloned(), Ctx::new(session)) else {
        return Ok(applied(false));
    };
    let start = picker.at.saturating_sub(2);
    let mut end = ctx.cursor.head;
    if ctx
        .text
        .get(end..)
        .is_some_and(|rest| rest.starts_with("]]"))
    {
        end += 2;
    }
    let insert = format!("[[{form}]]");
    let caret = start + insert.len();
    let edit = TextEdit {
        at: start,
        remove: end.saturating_sub(start),
        insert,
        anchor: caret,
        head: caret,
    };
    let done = ctx.apply(session, edit, false);
    session.close_link_picker();
    Ok(applied(done))
}

/// `link.picker.move { delta }`: highlight another entry, wrapping around.
pub(super) fn picker_move(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let delta = args
        .get("delta")
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(ids::LINK_PICKER_MOVE, "delta must be an integer"))?;
    let open = session.link_picker_view().is_some();
    if open {
        session.move_link_picker(delta);
    }
    Ok(applied(open))
}

/// `link.picker.close`: close the picker and leave the typed text as it is.
pub(super) fn picker_close(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    Ok(applied(session.close_link_picker()))
}
