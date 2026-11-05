//! `insert.equation` (PR 2.7): put a LaTeX formula in the page text.
//!
//! An equation is ordinary page text, `$...$` inline or `$$...$$` display (docs/markdown.md
//! section 3.2), so inserting one is one text edit and one undo step. The formula's source is
//! selected afterwards, so the next keystroke replaces the placeholder; the canvas shows the
//! source while the caret is in the block and typesets it when the caret leaves.

use notecraft_canvas::text::TextEdit;
use notecraft_markdown::math::{wrap, MAX_FORMULA_BYTES};
use notecraft_markdown::Math;
use serde_json::json;

use super::text::Ctx;
use super::{ids, CommandError, CommandOutput, Map, Value};
use crate::session::Session;

/// The formula a new equation starts with, selected so typing replaces it.
pub const PLACEHOLDER: &str = "x";

fn invalid(reason: &str) -> CommandError {
    CommandError::InvalidArgs {
        id: ids::INSERT_EQUATION,
        reason: reason.to_owned(),
    }
}

fn applied(done: bool) -> CommandOutput {
    CommandOutput {
        value: json!({ "applied": done }),
    }
}

/// Whether `src` has a dollar sign that is not escaped, which would end the formula early.
fn has_bare_dollar(src: &str) -> bool {
    let mut escaped = false;
    for c in src.chars() {
        match c {
            '\\' => escaped = !escaped,
            '$' if !escaped => return true,
            _ => escaped = false,
        }
    }
    false
}

/// Whether `src` can be written between dollar signs as it is.
fn fits(src: &str, display: bool) -> bool {
    !src.trim().is_empty()
        && src.len() <= MAX_FORMULA_BYTES
        && !has_bare_dollar(src)
        && (display || !src.contains(['\n', '\r']))
}

/// `insert.equation { latex?, display? }`: insert a formula at the caret. With `latex` that
/// formula is inserted. Without it, a selection that fits on a line becomes the formula;
/// otherwise a placeholder is inserted. `display` makes it a `$$...$$` formula on its own
/// line. The formula's source is selected afterwards. Returns `{ "applied" }`.
pub(super) fn insert_equation(
    session: &mut Session,
    args: &Map<String, Value>,
) -> Result<CommandOutput, CommandError> {
    let display = args
        .get("display")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let given = match args.get("latex") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.as_str()),
        Some(_) => return Err(invalid("latex must be a string")),
    };
    if let Some(src) = given {
        if src.len() > MAX_FORMULA_BYTES {
            return Err(invalid("the formula is over 4 KiB"));
        }
        if !fits(src, display) {
            return Err(invalid(
                "the formula is empty or has an unescaped dollar sign",
            ));
        }
    }
    let Some(ctx) = Ctx::new(session) else {
        return Ok(applied(false));
    };
    let (anchor, head) = (ctx.cursor.anchor, ctx.cursor.head);
    let (lo, hi) = (anchor.min(head), anchor.max(head));
    let selected = ctx.text.get(lo..hi).unwrap_or_default();
    let math = if display { Math::Display } else { Math::Inline };
    let (at, remove, src) = match given {
        Some(src) => (lo, hi - lo, src.to_owned()),
        None if lo < hi && fits(selected, display) => (lo, hi - lo, selected.to_owned()),
        // A selection that cannot be a formula is left alone; the equation goes after it.
        None if lo < hi => (hi, 0, PLACEHOLDER.to_owned()),
        None => (lo, 0, PLACEHOLDER.to_owned()),
    };
    let insert = super::text::for_page(&ctx, &wrap(&src, math));
    let open = if display { 2 } else { 1 };
    let edit = TextEdit {
        at,
        remove,
        // Selection: the formula's source between the dollar signs.
        anchor: at + open,
        head: at + open + super::text::for_page(&ctx, &src).len(),
        insert,
    };
    Ok(applied(ctx.apply(session, edit, false)))
}
