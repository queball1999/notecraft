//! Note tag commands (PR 2.5): `tag.apply`, `tag.remove`, `tag.customize`, and the Customize
//! Tags dialog's `tag.custom_add`, `tag.custom_edit`, and `tag.custom_delete`.
//!
//! Applying a tag edits the page text like any formatting command (see `format.rs`): one splice
//! of `#slug ` at the start of each touched paragraph, so it is one undo step. The gallery
//! (the default tags and the custom ones) lives in the session; the app saves the custom ones
//! in settings.

use notecraft_markdown::format as fmt;
use notecraft_markdown::markup;
use notecraft_markdown::{TagError, TagIcon};
use serde_json::json;

use super::format::run_edit;
use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::session::Session;

fn invalid(id: CommandId, reason: impl Into<String>) -> CommandError {
    CommandError::InvalidArgs {
        id,
        reason: reason.into(),
    }
}

fn tag_error(id: CommandId, err: TagError) -> CommandError {
    invalid(id, err.to_string())
}

fn text_arg<'a>(
    id: CommandId,
    args: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, CommandError> {
    args.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(id, format!("{name} must be a string")))
}

/// `tag.apply { tag }`: add the tag to the paragraphs at the caret or selection, or take it
/// off when they all have it. `todo` is the check box of a task list item.
pub(super) fn apply(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TAG_APPLY;
    let slug = text_arg(id, args, "tag")?;
    let def = session
        .tags()
        .get(slug)
        .ok_or_else(|| invalid(id, format!("no tag named {slug}")))?;
    let slug = def.slug.clone();
    if slug == "todo" {
        return run_edit(session, fmt::toggle_todo);
    }
    run_edit(session, |c| fmt::toggle_tag(c, &slug))
}

/// `tag.remove`: take every tag (and a To Do check box) off the paragraphs at the caret or
/// selection. Words that look like tags but are not in the gallery stay.
pub(super) fn remove(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let tags = session.tags().clone();
    run_edit(session, |c| {
        fmt::remove_tags(c, &|name| tags.get(name).is_some())
    })
}

/// `tag.customize`: open the Customize Tags dialog. The front end owns the dialog, so this
/// succeeds and does no more.
pub(super) fn customize(
    _session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    Ok(CommandOutput::default())
}

/// A color argument: `None` when absent, `Some(None)` for `none`, else the color.
fn color_arg(
    id: CommandId,
    args: &Map<String, Value>,
    name: &str,
) -> Result<Option<Option<u32>>, CommandError> {
    let Some(value) = args.get(name) else {
        return Ok(None);
    };
    let text = value
        .as_str()
        .ok_or_else(|| invalid(id, format!("{name} must be a string")))?;
    if matches!(text.trim(), "" | "none" | "automatic") {
        return Ok(Some(None));
    }
    markup::color_value(text)
        .map(|rgb| Some(Some(rgb)))
        .ok_or_else(|| invalid(id, format!("{name} must be #RRGGBB or none")))
}

fn icon_arg(id: CommandId, args: &Map<String, Value>) -> Result<Option<TagIcon>, CommandError> {
    let Some(value) = args.get("icon") else {
        return Ok(None);
    };
    value
        .as_str()
        .and_then(TagIcon::from_id)
        .map(Some)
        .ok_or_else(|| invalid(id, "icon must be the id of one of the tag icons"))
}

/// `tag.custom_add { name, icon?, font_color?, highlight? }`: add a custom tag to the gallery.
/// Returns its id (the slug stored after the `#`).
pub(super) fn custom_add(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TAG_CUSTOM_ADD;
    let name = text_arg(id, args, "name")?;
    let icon = icon_arg(id, args)?.unwrap_or(TagIcon::Dot);
    let font = color_arg(id, args, "font_color")?.flatten();
    let highlight = color_arg(id, args, "highlight")?.flatten();
    let slug = session
        .tags_mut()
        .add_custom(name, icon, font, highlight)
        .map_err(|err| tag_error(id, err))?;
    session.mark_changed();
    Ok(CommandOutput {
        value: json!({ "tag": slug }),
    })
}

/// `tag.custom_edit { tag, name?, icon?, font_color?, highlight? }`: change a custom tag. The
/// id (slug) stays the same, so pages keep their tags when the name changes. A color of `none`
/// clears it.
pub(super) fn custom_edit(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TAG_CUSTOM_EDIT;
    let slug = text_arg(id, args, "tag")?;
    let name = match args.get("name") {
        None => None,
        Some(_) => Some(text_arg(id, args, "name")?),
    };
    let icon = icon_arg(id, args)?;
    let font = color_arg(id, args, "font_color")?;
    let highlight = color_arg(id, args, "highlight")?;
    session
        .tags_mut()
        .edit_custom(slug, name, icon, font, highlight)
        .map_err(|err| tag_error(id, err))?;
    session.mark_changed();
    Ok(CommandOutput::default())
}

/// `tag.custom_delete { tag }`: remove a custom tag from the gallery. Pages keep the
/// `#slug` text; it just has no icon any more.
pub(super) fn custom_delete(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TAG_CUSTOM_DELETE;
    let slug = text_arg(id, args, "tag")?;
    session
        .tags_mut()
        .delete_custom(slug)
        .map_err(|err| tag_error(id, err))?;
    session.mark_changed();
    Ok(CommandOutput::default())
}
