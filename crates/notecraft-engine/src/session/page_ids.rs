//! Page ids and times in the frontmatter of `.notecraft` pages (docs/markdown.md section 3.1).
//!
//! The first time NoteCraft saves a `.notecraft` page it adds `notecraft.id` (a stable page
//! id), `created`, and `modified`. Later saves keep the id and `created` and update `modified`
//! when the content changed. The keys go in through the minimal-diff frontmatter edit of the
//! markdown layer, so unknown keys, comments, and key order keep their bytes.
//!
//! This runs on the one save path ([`Session::save_page`]) after the conflict check, never for
//! a read-only page, a page in conflict, or a plain `.md` page (a `.md` page gets frontmatter
//! only when it becomes a `.notecraft` page). The stamped text is what the file holds; the
//! session adopts it after the write, so the stamps are not part of what the page looks like
//! as unsaved text, the recovery journal, or the comparison with an outside edit. The disk
//! hash covers the stamped text, so the watcher event of our own write is recognized.

use notecraft_markdown::{parse_frontmatter, Editor, Frontmatter, Value};
use notecraft_storage::PageKind;

use super::Session;

fn has_id(fm: Option<&Frontmatter>) -> bool {
    fm.and_then(|fm| fm.notecraft("id"))
        .is_some_and(|id| !id.trim().is_empty())
}

/// `text` with the keys that are missing written: `created` and `modified` from `when` (an
/// RFC 3339 time), and `notecraft.id` from `id`. With `bump`, an existing `modified` that
/// differs from `when` is rewritten too. When the frontmatter cannot be edited (over the size
/// cap, or `notecraft:` is a flow map), `text` comes back unchanged.
fn text_with_stamps(text: &str, when: Option<&str>, id: Option<&str>, bump: bool) -> String {
    let fm = parse_frontmatter(text);
    if fm.as_ref().is_some_and(|fm| fm.oversize) {
        return text.to_owned();
    }
    let mut editor = Editor::new(text);
    let key_missing = |key: &str| fm.as_ref().is_none_or(|fm| fm.get(key).is_none());
    let result = (|| {
        if let Some(when) = when {
            let value = Value::Scalar(when.to_owned());
            if key_missing("created") {
                editor.set_frontmatter("created", &value)?;
            }
            let stale = fm
                .as_ref()
                .and_then(|fm| fm.get("modified"))
                .is_some_and(|e| bump && e.value != value);
            if key_missing("modified") || stale {
                editor.set_frontmatter("modified", &value)?;
            }
        }
        if let (Some(id), false) = (id, has_id(fm.as_ref())) {
            editor.set_frontmatter_map("notecraft", &[("id", Some(id))])?;
        }
        Ok::<(), notecraft_markdown::EditError>(())
    })();
    if result.is_err() {
        return text.to_owned();
    }
    editor.finish().unwrap_or_else(|_| text.to_owned())
}

/// How offsets into `old` move when it becomes `new`. The frontmatter and the body are handled
/// apart, because a save changes both ends of a page (lines added to the frontmatter, the
/// canvas block added at the end) and one common-start, common-end comparison of the whole
/// text would see them as one big change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct OffsetShift {
    /// Where the body starts in the old and the new text.
    old_body: usize,
    new_body: usize,
    /// End of the changed middle of the body, in body offsets.
    end: usize,
    delta: isize,
    old_len: usize,
    changed: bool,
}

fn body_start(text: &str) -> usize {
    parse_frontmatter(text).map_or(0, |fm| fm.span.end.min(text.len()))
}

impl OffsetShift {
    pub(super) fn between(old: &str, new: &str) -> Self {
        let (old_body, new_body) = (body_start(old), body_start(new));
        let old_text = old.get(old_body..).unwrap_or_default();
        let new_text = new.get(new_body..).unwrap_or_default();
        let max = old_text.len().min(new_text.len());
        let prefix = old_text
            .bytes()
            .zip(new_text.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        let suffix = old_text
            .bytes()
            .rev()
            .zip(new_text.bytes().rev())
            .take(max - prefix)
            .take_while(|(a, b)| a == b)
            .count();
        let old_mid = old_text.len() - prefix - suffix;
        let to_isize = |n: usize| isize::try_from(n).unwrap_or(isize::MAX);
        Self {
            old_body,
            new_body,
            end: prefix + old_mid,
            delta: to_isize(new_text.len()).saturating_sub(to_isize(old_text.len())),
            old_len: old_text.len(),
            changed: old != new,
        }
    }

    /// The new place of the offset `at` of the old text. An offset inside the old frontmatter
    /// stays; one in the body moves with the frontmatter and with changes before it; one at the
    /// end of a body that only grew at its end (the canvas block) stays in front of the block.
    pub(super) fn apply(&self, at: usize) -> usize {
        if !self.changed || at < self.old_body {
            return at;
        }
        let rel = at - self.old_body;
        let moves = rel > self.end || (rel == self.end && self.end < self.old_len);
        let rel = if moves {
            rel.saturating_add_signed(self.delta)
        } else {
            rel
        };
        self.new_body.saturating_add(rel)
    }
}

impl Session {
    /// The text a save of page `page_id` writes: `base` (the page text with its style and
    /// canvas block) with the id and times added, when the page is a `.notecraft` page or is
    /// becoming one. `disk` is what the file holds now: when the stamped text equals it, the
    /// content did not change and `modified` stays.
    pub(super) fn stamped_text(
        &self,
        page_id: &str,
        base: String,
        becoming_canvas: bool,
        disk: Option<&str>,
    ) -> String {
        if !becoming_canvas && self.kind_of_page(page_id) != Some(PageKind::Canvas) {
            return base;
        }
        let when = self
            .clock
            .now()
            .map(|stamp| stamp.rfc3339(self.clock.offset_minutes()));
        let id = if has_id(parse_frontmatter(&base).as_ref()) {
            None
        } else {
            self.ids.next(page_id).filter(|id| !id.trim().is_empty())
        };
        let (when, id) = (when.as_deref(), id.as_deref());
        let steady = text_with_stamps(&base, when, id, false);
        if disk == Some(steady.as_str()) {
            return steady;
        }
        text_with_stamps(&base, when, id, true)
    }

    /// The page text of `page_id` is about to change from its current text to `new` (a save
    /// added frontmatter lines or the canvas block): move the caret and the recorded text
    /// edits so they still point at the same text.
    pub(super) fn follow_text_change(&mut self, page_id: &str, new: &str) {
        let Some(old) = self.page_text(page_id) else {
            return;
        };
        let shift = OffsetShift::between(old, new);
        if !shift.changed {
            return;
        }
        let at = |offset: usize| shift.apply(offset);
        self.history.shift_text(page_id, &at);
        if let Some((owner, cursor)) = self.text_cursor.as_mut() {
            if owner == page_id {
                cursor.anchor = at(cursor.anchor);
                cursor.head = at(cursor.head);
            }
        }
    }
}

#[cfg(test)]
#[path = "page_ids_tests.rs"]
mod tests;
