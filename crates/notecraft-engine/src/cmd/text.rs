//! Text editing commands (PR 1.5): `text.set_caret`, `text.move`, `text.insert`, `text.delete`,
//! `text.enter`, `text.tab`, and the clipboard and selection commands that share the
//! Home tab ids (`edit.cut`, `edit.copy`, `edit.paste`, `edit.paste_text_only`,
//! `edit.select_all`, `edit.delete`).
//!
//! The caret and selection are byte offsets in the page's markdown source (see
//! [`crate::session::TextCursor`]). Every change goes through `Session::edit_page_text`, so it
//! is one undo step (typing coalesces into word steps) and the page is marked changed for
//! auto-save. The edit is a splice on the source text, so saving rewrites only the bytes that
//! were touched. A page cut short at the size cap, or that was never read, is not editable.
//!
//! Where the caret may sit: not in the frontmatter and not on a container marker line
//! (docs/markdown.md section 3.3). A change that would break a marker or reach into the
//! frontmatter is refused and nothing changes.

use notecraft_canvas::text::{self, Motion, TextEdit};
use notecraft_markdown::containers::{new_container_text, next_container_id};
use notecraft_markdown::{parse, split_containers, BlockKind, ContainerRun, Document, LineEnding};
use notecraft_model::container::{clamp_pos, clamp_width};
use notecraft_model::Container;
use serde_json::json;

use super::{ids, CommandError, CommandOutput, Map, Value};
use crate::session::{Session, TextCursor};
use crate::undo::{Op, Scope, Transaction};

/// Most bytes one paste or insert may add.
pub const MAX_INSERT_BYTES: usize = 4 * 1024 * 1024;
/// Width of a container made by a click on empty canvas, in page units.
pub const NEW_CONTAINER_WIDTH: f32 = 420.0;

/// The page being edited and everything the commands need to know about it.
pub(super) struct Ctx {
    pub(super) page: String,
    pub(super) text: String,
    pub(super) doc: Document,
    pub(super) runs: Vec<ContainerRun>,
    pub(super) cursor: TextCursor,
    pub(super) eol: &'static str,
}

fn null() -> CommandOutput {
    CommandOutput::default()
}

fn applied(done: bool) -> CommandOutput {
    CommandOutput {
        value: json!({ "applied": done }),
    }
}

impl Ctx {
    /// The shown page, if it can be edited. The caret is where the user left it, or the end of
    /// the page's text when there is none yet.
    pub(super) fn new(session: &Session) -> Option<Self> {
        let page = session.selection().page.clone()?;
        if !session.page_is_editable(&page) {
            return None;
        }
        let text = session.page_text(&page).unwrap_or_default().to_owned();
        let doc = parse(&text);
        let runs = split_containers(&text, &doc);
        let eol = LineEnding::detect(&text).as_str();
        let mut ctx = Self {
            page,
            text,
            doc,
            runs,
            cursor: TextCursor::default(),
            eol,
        };
        let cursor = session.text_cursor().unwrap_or_else(|| {
            let end = ctx.runs.last().map_or(ctx.text.len(), |r| r.content.end);
            TextCursor::caret(end)
        });
        ctx.cursor = TextCursor {
            anchor: ctx.snap_editable(cursor.anchor, true),
            head: ctx.snap_editable(cursor.head, true),
        };
        Some(ctx)
    }

    /// `at` moved to the nearest place the caret may be: on a character boundary, in the body,
    /// and not on a marker line. `forward` picks which way to go when a choice has to be made.
    fn snap_editable(&self, at: usize, forward: bool) -> usize {
        let len = self.text.len();
        let mut at = text::snap(&self.text, at).max(self.doc.body_start.min(len));
        if let Some(marker) = self.runs.first().and_then(|r| r.marker) {
            if at <= marker.start {
                return marker.end.min(len);
            }
        }
        for run in &self.runs {
            if let Some(marker) = run.marker {
                if at > marker.start && at < marker.end {
                    at = if forward { marker.end } else { marker.start };
                }
            }
        }
        at.min(len)
    }

    /// The run whose content holds `at` (the end of the content counts).
    pub(super) fn run_at(&self, at: usize) -> Option<&ContainerRun> {
        self.runs
            .iter()
            .find(|run| run.content.start <= at && at <= run.content.end)
    }

    /// Whether `at` is inside a fenced or indented code block (not on its first byte).
    fn in_code(&self, at: usize) -> bool {
        self.doc.blocks.iter().any(|block| {
            matches!(block.kind, BlockKind::Code { .. })
                && block.span.start < at
                && at < block.span.end
        })
    }

    /// Check `edit` against the markers and the frontmatter, fixing the one case that can be
    /// fixed: text typed at the very end of a container would land on the next marker's line,
    /// so a line break goes after it.
    fn guard(&self, mut edit: TextEdit) -> Option<TextEdit> {
        let end = edit.at.checked_add(edit.remove)?;
        if end > self.text.len() || edit.at < self.doc.body_start {
            return None;
        }
        let at_marker_line = self
            .runs
            .iter()
            .filter_map(|r| r.marker)
            .any(|m| m.start == end && m.start > 0);
        if !edit.insert.is_empty() && at_marker_line && !edit.insert.ends_with('\n') {
            edit.insert.push_str(self.eol);
        }
        let head = self.text.get(..edit.at)?;
        let tail = self.text.get(end..)?;
        let new_text = format!("{head}{}{tail}", edit.insert);
        for marker in self.runs.iter().filter_map(|r| r.marker) {
            let overlaps = edit.at < marker.end && end > marker.start;
            if overlaps {
                if edit.at <= marker.start && end >= marker.end {
                    // The whole marker is removed with the text around it.
                    continue;
                }
                return None;
            }
            // The marker stays; it must still start a line.
            let start = if marker.start >= end {
                marker.start - edit.remove + edit.insert.len()
            } else {
                marker.start
            };
            if start > 0 && !new_text.get(..start)?.ends_with('\n') {
                return None;
            }
        }
        Some(edit)
    }

    /// Apply `edit` as one undo step and put the selection where it says.
    pub(super) fn apply(&self, session: &mut Session, edit: TextEdit, typing: bool) -> bool {
        let Some(edit) = self.guard(edit) else {
            return false;
        };
        if !session.edit_page_text(&self.page, edit.at, edit.remove, &edit.insert, typing) {
            return false;
        }
        session.set_text_cursor(
            &self.page,
            TextCursor {
                anchor: edit.anchor,
                head: edit.head,
            },
        );
        true
    }
}

/// If the caret is leaving a container that has no text (and its marker), remove that
/// container as its own undo step, as OneNote does. Returns `cursor` shifted for the
/// removal. `ctx` is out of date afterwards.
fn prune_left_container(session: &mut Session, ctx: &Ctx, cursor: TextCursor) -> TextCursor {
    let Some(run) = ctx.run_at(ctx.cursor.head) else {
        return cursor;
    };
    let Some(marker) = run.marker else {
        return cursor;
    };
    let staying = cursor.head >= run.content.start && cursor.head <= run.content.end;
    if staying || !run.is_blank(&ctx.text) {
        return cursor;
    }
    // Take the blank lines above the marker with it.
    let mut start = marker.start;
    while start > 0 {
        let line_start = text::line_start(&ctx.text, start - 1);
        let line = ctx.text.get(line_start..start).unwrap_or("x");
        if !line.trim().is_empty() {
            break;
        }
        start = line_start;
    }
    let end = run.content.end.max(marker.end);
    let removed = ctx.text.get(start..end).unwrap_or_default().to_owned();
    let mut tx = Transaction::new(Scope::Page(ctx.page.clone()), "Remove empty container").with(
        Op::ReplaceText {
            page: ctx.page.clone(),
            at: start,
            removed,
            inserted: String::new(),
        },
    );
    if let Some(geometry) = session.stored_container(&ctx.page, &run.id) {
        tx = tx.with(Op::SetContainer {
            page: ctx.page.clone(),
            id: run.id.clone(),
            before: Some(geometry),
            after: None,
        });
    }
    if !session.commit(tx) {
        return cursor;
    }
    let shift = |at: usize| {
        if at >= end {
            at - (end - start)
        } else if at > start {
            start
        } else {
            at
        }
    };
    TextCursor {
        anchor: shift(cursor.anchor),
        head: shift(cursor.head),
    }
}

fn bool_arg(args: &Map<String, Value>, name: &str) -> bool {
    args.get(name).and_then(Value::as_bool).unwrap_or(false)
}

fn str_arg<'a>(args: &'a Map<String, Value>, name: &str) -> &'a str {
    args.get(name).and_then(Value::as_str).unwrap_or_default()
}

fn invalid(id: super::CommandId, reason: &str) -> CommandError {
    CommandError::InvalidArgs {
        id,
        reason: reason.to_owned(),
    }
}

/// `text.set_caret { at, extend?, select? }`: put the caret at byte offset `at` of the page
/// text (or extend the selection to it). `select` is `word` or `line` to select the word or line
/// around `at`, as a double or triple click does.
pub(super) fn set_caret(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TEXT_SET_CARET;
    let at = args
        .get("at")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(id, "at must be a non-negative integer"))?;
    let at = usize::try_from(at).unwrap_or(usize::MAX);
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let had_cursor = session.text_cursor().is_some();
    let at = ctx.snap_editable(at, true);
    let range = match str_arg(args, "select") {
        "" => None,
        "word" => Some(text::word_range(&ctx.text, at)),
        "line" => Some(text::line_range(&ctx.text, at)),
        _ => return Err(invalid(id, "select must be word or line")),
    };
    let cursor = match range {
        Some(range) => TextCursor {
            anchor: ctx.snap_editable(range.start, true),
            head: ctx.snap_editable(range.end, false),
        },
        None if bool_arg(args, "extend") => TextCursor {
            anchor: ctx.cursor.anchor,
            head: at,
        },
        None => TextCursor::caret(at),
    };
    let cursor = if had_cursor {
        prune_left_container(session, &ctx, cursor)
    } else {
        cursor
    };
    session.set_text_cursor(&ctx.page, cursor);
    Ok(applied(true))
}

/// `text.move { motion, extend? }`: move the caret (or extend the selection) by a character,
/// word, or line end, or to the start or end of the text.
pub(super) fn move_caret(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TEXT_MOVE;
    let name = str_arg(args, "motion");
    let motion = Motion::from_name(name).ok_or_else(|| {
        invalid(
            id,
            "motion must be left, right, word_left, word_right, line_start, line_end, start, or end",
        )
    })?;
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let extend = bool_arg(args, "extend");
    let cur = ctx.cursor;
    let forward = matches!(
        motion,
        Motion::Right | Motion::WordRight | Motion::LineEnd | Motion::End
    );
    let cursor = if !extend && !cur.is_caret() && matches!(motion, Motion::Left | Motion::Right) {
        // With a selection, Left and Right collapse to its edge.
        let range = cur.range();
        TextCursor::caret(if forward { range.end } else { range.start })
    } else {
        let to = ctx.snap_editable(text::move_caret(&ctx.text, cur.head, motion), forward);
        if extend {
            TextCursor {
                anchor: cur.anchor,
                head: to,
            }
        } else {
            TextCursor::caret(to)
        }
    };
    let cursor = prune_left_container(session, &ctx, cursor);
    session.set_text_cursor(&ctx.page, cursor);
    Ok(applied(true))
}

/// `edit.select_all`: select the text of the container the caret is in; when that is already
/// selected, select the text of the whole page.
pub(super) fn select_all(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let container = ctx.run_at(ctx.cursor.head).map(|r| r.content);
    let page = ctx
        .runs
        .first()
        .zip(ctx.runs.last())
        .map(|(first, last)| first.content.start..last.content.end);
    let (Some(container), Some(page)) = (container, page) else {
        return Ok(applied(false));
    };
    let range = ctx.cursor.range();
    let all = if range == (container.start..container.end) {
        page
    } else {
        container.start..container.end
    };
    session.set_text_cursor(
        &ctx.page,
        TextCursor {
            anchor: all.start,
            head: all.end,
        },
    );
    Ok(applied(true))
}

/// Normalize the line endings of text to insert to the page's own.
fn for_page(ctx: &Ctx, text: &str) -> String {
    let eol = if ctx.eol == "\r\n" {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    };
    eol.normalize(text)
}

/// `text.insert { text, typing? }`: type `text` at the caret, replacing the selection.
/// `typing` marks keystrokes, which coalesce into word-sized undo steps.
pub(super) fn insert(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TEXT_INSERT;
    let raw = args
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(id, "text must be a string"))?;
    if raw.len() > MAX_INSERT_BYTES {
        return Err(invalid(id, "text is too long"));
    }
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let inserted = for_page(&ctx, raw);
    let Some(edit) = text::insert(ctx.cursor.anchor, ctx.cursor.head, &inserted) else {
        return Ok(applied(false));
    };
    let typing = bool_arg(args, "typing") && ctx.cursor.is_caret();
    Ok(applied(ctx.apply(session, edit, typing)))
}

/// `text.delete { direction, unit? }`: Backspace (`backward`) or Delete (`forward`) by a
/// character or, with unit `word`, a word. A selection is deleted whole.
pub(super) fn delete(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::TEXT_DELETE;
    let backward = match str_arg(args, "direction") {
        "backward" => true,
        "forward" => false,
        _ => return Err(invalid(id, "direction must be backward or forward")),
    };
    let word = match str_arg(args, "unit") {
        "" | "char" => false,
        "word" => true,
        _ => return Err(invalid(id, "unit must be char or word")),
    };
    Ok(applied(delete_at_caret(session, backward, word)))
}

fn delete_at_caret(session: &mut Session, backward: bool, word: bool) -> bool {
    let Some(ctx) = Ctx::new(session) else {
        return false;
    };
    let (a, h) = (ctx.cursor.anchor, ctx.cursor.head);
    let edit = if backward {
        text::backspace(&ctx.text, a, h, word)
    } else {
        text::delete_forward(&ctx.text, a, h, word)
    };
    let Some(edit) = edit else {
        return false;
    };
    // A caret delete stays inside its container; only a selection can reach across.
    if ctx.cursor.is_caret() {
        let Some(run) = ctx.run_at(h) else {
            return false;
        };
        if edit.at < run.content.start || edit.at + edit.remove > run.content.end {
            return false;
        }
    }
    let typing = ctx.cursor.is_caret() && !word;
    ctx.apply(session, edit, typing)
}

/// `edit.delete`: delete the selection, or the character after the caret.
pub(super) fn edit_delete(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    Ok(applied(delete_at_caret(session, false, false)))
}

/// `text.enter { soft? }`: Enter. A new paragraph, or with `soft` a line break; list items
/// continue, and Enter on an empty item ends the list.
pub(super) fn enter(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let (a, h) = (ctx.cursor.anchor, ctx.cursor.head);
    let in_code = ctx.in_code(h);
    let edit = text::enter(&ctx.text, a, h, ctx.eol, bool_arg(args, "soft"), in_code);
    Ok(applied(ctx.apply(session, edit, false)))
}

/// `text.tab { shift? }`: Tab and Shift+Tab move list items in and out a level.
pub(super) fn tab(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let (a, h) = (ctx.cursor.anchor, ctx.cursor.head);
    let Some(edit) = text::tab(&ctx.text, a, h, bool_arg(args, "shift")) else {
        return Ok(applied(false));
    };
    Ok(applied(ctx.apply(session, edit, false)))
}

/// The selected text, as markdown source or, with `plain`, without markdown syntax.
fn selected_text(ctx: &Ctx, plain: bool) -> Option<String> {
    let range = ctx.cursor.range();
    let source = ctx.text.get(range)?;
    if source.is_empty() {
        return None;
    }
    if plain {
        let text = parse(source).plain_text();
        let text = text.trim_end_matches('\n');
        return Some(if text.is_empty() {
            source.to_owned()
        } else {
            text.to_owned()
        });
    }
    Some(source.to_owned())
}

/// `edit.copy`: the selected markdown source, returned as `{ "text" }` for the front end to put
/// on the clipboard (which also holds it as plain text). `null` with nothing selected.
pub(super) fn copy(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let Some(ctx) = Ctx::new(session) else {
        return Ok(null());
    };
    Ok(
        selected_text(&ctx, false).map_or_else(null, |text| CommandOutput {
            value: json!({ "text": text }),
        }),
    )
}

/// `edit.cut`: copy, then delete the selection.
pub(super) fn cut(
    session: &mut Session,
    _args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let Some(ctx) = Ctx::new(session) else {
        return Ok(null());
    };
    let Some(text) = selected_text(&ctx, false) else {
        return Ok(null());
    };
    let Some(edit) = text::insert(ctx.cursor.anchor, ctx.cursor.head, "") else {
        return Ok(null());
    };
    if !ctx.apply(session, edit, false) {
        return Ok(null());
    }
    Ok(CommandOutput {
        value: json!({ "text": text }),
    })
}

fn paste_with(
    session: &mut Session,
    args: &Map<String, Value>,
    plain: bool,
) -> Result<CommandOutput, CommandError> {
    let Some(raw) = args.get("text").and_then(Value::as_str) else {
        // The front end reads the clipboard and passes the text; with none there is nothing
        // to paste.
        return Ok(applied(false));
    };
    if raw.len() > MAX_INSERT_BYTES {
        return Err(invalid(ids::EDIT_PASTE, "text is too long"));
    }
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let body = if plain {
        parse(raw).plain_text().trim_end_matches('\n').to_owned()
    } else {
        raw.to_owned()
    };
    let inserted = for_page(&ctx, &body);
    let Some(edit) = text::insert(ctx.cursor.anchor, ctx.cursor.head, &inserted) else {
        return Ok(applied(false));
    };
    Ok(applied(ctx.apply(session, edit, false)))
}

/// `edit.paste { text? }`: paste markdown or plain text at the caret.
pub(super) fn paste(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    paste_with(session, args, false)
}

/// `edit.paste_text_only { text? }`: paste with the markdown syntax removed.
pub(super) fn paste_text_only(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    paste_with(session, args, true)
}

fn coordinate(
    id: super::CommandId,
    args: &Map<String, Value>,
    name: &str,
) -> Result<f32, CommandError> {
    args.get(name)
        .and_then(Value::as_f64)
        .map(|v| v as f32)
        .filter(|v| v.is_finite())
        .ok_or_else(|| invalid(id, &format!("{name} must be a number")))
}

/// `container.new { x, y }`: start a new container at page point (`x`, `y`) and put the caret
/// in it. The container is a marker line appended to the page text (docs/markdown.md section
/// 3.3) with its position kept beside it. A container the caret was leaving with no text is
/// removed first. Returns `{ "id" }`.
pub(super) fn container_new(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::CONTAINER_NEW;
    let pos = notecraft_core::Vec2::new(coordinate(id, args, "x")?, coordinate(id, args, "y")?);
    let Some(ctx) = Ctx::new(session) else {
        return Ok(null());
    };
    if session.text_cursor().is_some() {
        // Leaving an empty container removes it, as moving the caret away does.
        let away = TextCursor::caret(usize::MAX);
        prune_left_container(session, &ctx, away);
    }
    let Some(ctx) = Ctx::new(session) else {
        return Ok(null());
    };
    let new_id = next_container_id(&ctx.runs);
    let added = new_container_text(&ctx.text, &new_id);
    let at = ctx.text.len();
    let container = Container::at(&new_id, pos, NEW_CONTAINER_WIDTH);
    let tx = Transaction::new(Scope::Page(ctx.page.clone()), "New container")
        .with(Op::ReplaceText {
            page: ctx.page.clone(),
            at,
            removed: String::new(),
            inserted: added.clone(),
        })
        .with(Op::SetContainer {
            page: ctx.page.clone(),
            id: new_id.clone(),
            before: session.stored_container(&ctx.page, &new_id),
            after: Some(container),
        });
    if !session.commit(tx) {
        return Ok(null());
    }
    session.set_text_cursor(&ctx.page, TextCursor::caret(at + added.len()));
    Ok(CommandOutput {
        value: json!({ "id": new_id }),
    })
}

/// The container `id` of the shown page and its stored geometry, for move and resize.
fn container_target(
    session: &Session,
    command: super::CommandId,
    args: &Map<String, Value>,
) -> Result<(String, String, Container), CommandError> {
    let container_id = str_arg(args, "id");
    let Some(ctx) = Ctx::new(session) else {
        return Err(invalid(command, "no page that can be edited is shown"));
    };
    if !ctx.runs.iter().any(|run| run.id == container_id) {
        return Err(invalid(command, "no container has this id"));
    }
    let current = session.container(&ctx.page, container_id);
    Ok((ctx.page, container_id.to_owned(), current))
}

fn set_container(
    session: &mut Session,
    page: &str,
    id: &str,
    after: Container,
    label: &str,
) -> bool {
    let before = session.stored_container(page, id);
    if before.as_ref() == Some(&after) {
        return false;
    }
    let tx = Transaction::new(Scope::Page(page.to_owned()), label).with(Op::SetContainer {
        page: page.to_owned(),
        id: id.to_owned(),
        before,
        after: Some(after),
    });
    session.commit(tx)
}

/// `container.move { id, x, y }`: put container `id` at page point (`x`, `y`). A container in
/// the flow layout leaves it.
pub(super) fn container_move(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::CONTAINER_MOVE;
    let pos = notecraft_core::Vec2::new(coordinate(id, args, "x")?, coordinate(id, args, "y")?);
    let (page, container_id, current) = container_target(session, id, args)?;
    let after = Container {
        pos: Some(clamp_pos(pos)),
        ..current
    };
    Ok(applied(set_container(
        session,
        &page,
        &container_id,
        after,
        "Move container",
    )))
}

/// `container.resize { id, width }`: set the width of container `id`, in page units.
pub(super) fn container_resize(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let id = ids::CONTAINER_RESIZE;
    let width = coordinate(id, args, "width")?;
    let (page, container_id, current) = container_target(session, id, args)?;
    let after = Container {
        width: clamp_width(width),
        ..current
    };
    Ok(applied(set_container(
        session,
        &page,
        &container_id,
        after,
        "Resize container",
    )))
}
