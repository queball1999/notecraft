//! Text formatting commands (PR 1.6): the Home tab's Basic Text and Styles groups and the To
//! Do tag. Each one is a single splice of the page markdown (see `notecraft_markdown::format`
//! for the rules and docs/markdown.md section 3.2 for the file format), applied through the
//! same guarded path as typing, so it is one undo step and saving touches only what changed.
//!
//! They work on the selection or, with a caret, on the word at the caret (inline marks) or the
//! line (block formats), as OneNote does. The ribbon reads the state at the caret with
//! [`Session::format_state`].

use notecraft_canvas::text::TextEdit;
use notecraft_markdown::format::{self as fmt, FormatEdit, FormatState};
use notecraft_markdown::markup::{self, Mark};
use notecraft_markdown::Align;
use serde_json::json;

use super::text::Ctx;
use super::{ids, CommandError, CommandId, CommandOutput, Map, Value};
use crate::session::Session;

/// The color Font Color uses when a call names none (the ribbon's default swatch).
pub const DEFAULT_FONT_COLOR: &str = "#E53935";

fn applied(done: bool) -> CommandOutput {
    CommandOutput {
        value: json!({ "applied": done }),
    }
}

fn invalid(id: CommandId, reason: &str) -> CommandError {
    CommandError::InvalidArgs {
        id,
        reason: reason.to_owned(),
    }
}

/// The page text formatting may touch: everything before the `notecraft-canvas` block.
fn visible_text(text: &str) -> &str {
    let end = notecraft_storage::find_canvas_block(text).map_or(text.len(), |b| b.block.start);
    text.get(..end.min(text.len())).unwrap_or(text)
}

/// Where formatting may start: after the frontmatter and after the marker line of the
/// container that holds `at`, as `split_containers` reads them. Code fences are counted from
/// here.
fn floor_for(shown: &str, at: usize) -> usize {
    let doc = notecraft_markdown::parse(shown);
    let runs = notecraft_markdown::split_containers(shown, &doc);
    let marker = runs
        .iter()
        .filter(|r| r.marker.is_some() && r.content.start <= at)
        .map(|r| r.content.start)
        .max()
        .unwrap_or(0);
    doc.body_start.max(marker)
}

fn format_ctx<'a>(ctx: &'a Ctx) -> fmt::Ctx<'a> {
    let lo = ctx.cursor.anchor.min(ctx.cursor.head);
    let shown = ctx.text.get(..ctx.visible).unwrap_or(&ctx.text);
    fmt::Ctx {
        text: shown,
        floor: floor_for(shown, lo),
        anchor: ctx.cursor.anchor,
        head: ctx.cursor.head,
        eol: ctx.eol,
    }
}

/// Run a formatting edit on the shown page. Nothing to do (no caret, nothing to change, or an
/// edit the guards refuse) answers `{ applied: false }`.
fn run_edit(
    session: &mut Session,
    f: impl FnOnce(&fmt::Ctx<'_>) -> Option<FormatEdit>,
) -> Result<CommandOutput, CommandError> {
    if session.text_cursor().is_none() {
        return Ok(applied(false));
    }
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let Some(edit) = f(&format_ctx(&ctx)) else {
        return Ok(applied(false));
    };
    let edit = TextEdit {
        at: edit.at,
        remove: edit.remove,
        insert: edit.insert,
        anchor: edit.anchor,
        head: edit.head,
    };
    Ok(applied(ctx.apply(session, edit, false)))
}

fn toggle(session: &mut Session, mark: Mark) -> Result<CommandOutput, CommandError> {
    run_edit(session, |c| fmt::toggle_mark(c, mark))
}

macro_rules! toggle_command {
    ($(#[$doc:meta] $name:ident => $mark:expr;)*) => {$(
        #[$doc]
        pub(super) fn $name(
            session: &mut Session,
            _args: &Map<String, Value>,
        ) -> Result<CommandOutput, CommandError> {
            toggle(session, $mark)
        }
    )*};
}

toggle_command! {
    /// `format.bold`: `**` around the selection or the word at the caret.
    bold => Mark::Bold;
    /// `format.italic`: `*` around the selection or word.
    italic => Mark::Italic;
    /// `format.strikethrough`: `~~` around the selection or word.
    strikethrough => Mark::Strike;
    /// `format.underline`: `<u>` around the selection or word.
    underline => Mark::Underline;
    /// `format.subscript`: `<sub>` around the selection or word.
    subscript => Mark::Subscript;
    /// `format.superscript`: `<sup>` around the selection or word.
    superscript => Mark::Superscript;
}

/// The `color` argument: `None` when absent, `Some(None)` for "no color", else the color.
fn color_arg(
    id: CommandId,
    args: &Map<String, Value>,
) -> Result<Option<Option<String>>, CommandError> {
    let Some(value) = args.get("color") else {
        return Ok(None);
    };
    let text = value
        .as_str()
        .ok_or_else(|| invalid(id, "color must be a string"))?;
    if matches!(text, "none" | "automatic" | "") {
        return Ok(Some(None));
    }
    let color =
        markup::normalize_color(text).ok_or_else(|| invalid(id, "color must be #RRGGBB"))?;
    Ok(Some(Some(color)))
}

/// `format.highlight { color? }`: highlight with a color (`==` for the default yellow);
/// `none` removes it; no color toggles the default yellow.
pub(super) fn highlight(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    match color_arg(ids::FORMAT_HIGHLIGHT, args)? {
        None => toggle(session, Mark::Highlight),
        Some(color) => run_edit(session, |c| {
            fmt::set_mark(c, Mark::Highlight, color.as_deref())
        }),
    }
}

/// `format.font_color { color? }`: set the text color; `none` (Automatic) removes it.
pub(super) fn font_color(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let color = match color_arg(ids::FORMAT_FONT_COLOR, args)? {
        None => Some(DEFAULT_FONT_COLOR.to_owned()),
        Some(color) => color,
    };
    run_edit(session, |c| fmt::set_mark(c, Mark::Color, color.as_deref()))
}

/// `format.font_family { name }`: set the font family.
pub(super) fn font_family(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::FORMAT_FONT_FAMILY;
    let name = args
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(id, "name must be a string"))?;
    let name = markup::normalize_family(name)
        .ok_or_else(|| invalid(id, "name is not a usable font family"))?;
    run_edit(session, |c| fmt::set_mark(c, Mark::Family, Some(&name)))
}

/// `format.font_size { points }`: set the font size in points (1 to 1638).
pub(super) fn font_size(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::FORMAT_FONT_SIZE;
    let points = args
        .get("points")
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid(id, "points must be a number"))?;
    let size = markup::normalize_size(&points.to_string())
        .ok_or_else(|| invalid(id, "points must be from 1 to 1638"))?;
    run_edit(session, |c| fmt::set_mark(c, Mark::Size, Some(&size)))
}

/// `format.clear`: Clear All Formatting on the selection or the line at the caret.
pub(super) fn clear(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    run_edit(session, fmt::clear_formatting)
}

/// `format.bullets`: start or end a bulleted list on the selected lines.
pub(super) fn bullets(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    run_edit(session, fmt::toggle_bullets)
}

/// `format.numbering`: start or end a numbered list on the selected lines.
pub(super) fn numbering(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    run_edit(session, fmt::toggle_numbering)
}

/// `tag.todo`: add or remove a To Do check box (a task list item) on the selected lines.
pub(super) fn todo(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    run_edit(session, fmt::toggle_todo)
}

/// `format.indent`: move list items one level in, or nest a quote.
pub(super) fn indent(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    run_edit(session, |c| fmt::indent(c, false))
}

/// `format.outdent`: move list items one level out, or unnest a quote.
pub(super) fn outdent(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    run_edit(session, |c| fmt::indent(c, true))
}

fn align(session: &mut Session, align: Align) -> Result<CommandOutput, CommandError> {
    run_edit(session, |c| fmt::set_align(c, align))
}

/// `format.align_left`: remove the paragraph's alignment comment.
pub(super) fn align_left(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    align(session, Align::Left)
}

/// `format.align_center`: center the paragraph.
pub(super) fn align_center(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    align(session, Align::Center)
}

/// `format.align_right`: align the paragraph to the right.
pub(super) fn align_right(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    align(session, Align::Right)
}

/// `style.apply { style }`: a Styles gallery entry (heading1 to heading6, title, citation,
/// quote, code, normal).
pub(super) fn style_apply(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::STYLE_APPLY;
    let name = args
        .get("style")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(id, "style must be a string"))?;
    let style = fmt::Style::from_name(name).ok_or_else(|| {
        invalid(
            id,
            "style must be heading1 to heading6, title, citation, quote, code, or normal",
        )
    })?;
    run_edit(session, |c| fmt::apply_style(c, style))
}

impl Session {
    /// The formatting at the caret or selection of the shown page, for the ribbon buttons to
    /// show. All off when nothing is being edited.
    pub fn format_state(&self) -> FormatState {
        let Some(cursor) = self.text_cursor() else {
            return FormatState::default();
        };
        let Some(page) = self.selection().page.as_deref() else {
            return FormatState::default();
        };
        let Some(text) = self.page_text(page) else {
            return FormatState::default();
        };
        let lo = cursor.anchor.min(cursor.head);
        let text = visible_text(text);
        fmt::state_at(&fmt::Ctx {
            text,
            floor: floor_for(text, lo),
            anchor: cursor.anchor,
            head: cursor.head,
            eol: "\n",
        })
    }
}
