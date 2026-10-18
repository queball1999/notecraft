//! Pictures and file attachments that sit at a place of their own on the page
//! (docs/markdown.md section 3.4).
//!
//! The picture or file stays in the page text as markdown. A [`PlacedObject`] is the position
//! of one occurrence of that link, so placing, moving, and saving never touch the text. The
//! list for a page is changed by [`Op::SetObjects`], which keeps every change undoable and
//! marks the page unsaved (a `.md` page with a placed object is promoted to `.notecraft` on
//! save, like one with ink).

use notecraft_core::Vec2;
use notecraft_markdown::objects::{count_of, object_slots};
use notecraft_model::object::{MAX_OBJECTS, MAX_COORD};
use notecraft_model::{ObjectKind, PlacedObject};
use notecraft_storage::{find_canvas_block, CanvasObject};

use super::{InsertError, Session};
use crate::undo::{Op, Scope, Transaction};

fn fail(text: &str) -> InsertError {
    InsertError(text.to_owned())
}

/// The id a new object of `kind` gets: the highest `i` or `f` number on the page plus one.
pub(super) fn next_object_id(objects: &[PlacedObject], kind: ObjectKind) -> String {
    let prefix = match kind {
        ObjectKind::Picture => 'i',
        ObjectKind::File => 'f',
    };
    let highest = objects
        .iter()
        .filter_map(|o| {
            let rest = o.id.strip_prefix(prefix)?;
            rest.parse::<u64>().ok()
        })
        .max()
        .unwrap_or(0);
    format!("{prefix}{}", highest.saturating_add(1))
}

impl Session {
    /// The pictures and files of `page` that have a place of their own, in the order the
    /// canvas block lists them.
    pub fn page_objects(&self, page: &str) -> &[PlacedObject] {
        self.objects.get(page).map_or(&[], Vec::as_slice)
    }

    /// Replace the placed objects of `page` (an empty list removes the entry).
    pub(crate) fn put_objects(&mut self, page: &str, objects: Vec<PlacedObject>) {
        if objects.is_empty() {
            self.objects.remove(page);
        } else {
            self.objects.insert(page.to_owned(), objects);
        }
    }

    /// The placed objects of `page_id` whose markdown is still in the page text, as the canvas
    /// block stores them. Deleting the text of a picture or file deletes its place with it.
    pub(super) fn live_objects(&self, page_id: &str) -> Vec<CanvasObject> {
        let placed = self.page_objects(page_id);
        if placed.is_empty() {
            return Vec::new();
        }
        let Some(text) = self.page_text(page_id) else {
            return Vec::new();
        };
        let shown_end = find_canvas_block(text).map_or(text.len(), |b| b.block.start);
        let slots = object_slots(&notecraft_markdown::parse(
            text.get(..shown_end).unwrap_or(text),
        ));
        placed
            .iter()
            .filter(|o| o.nth < count_of(&slots, o.kind, &o.src))
            .map(|o| CanvasObject {
                id: o.id.clone(),
                file: o.kind == ObjectKind::File,
                src: o.src.clone(),
                nth: o.nth,
                x: o.pos.x,
                y: o.pos.y,
            })
            .collect()
    }

    /// Put the `nth` picture or file with link `src` of the page shown now at `pos`, as one
    /// undo step. An object that flowed inside its container gets a place of its own; one that
    /// already has a place moves. Nothing happens, and no undo step is made, if it is already
    /// there.
    ///
    /// The engine call `picture.move` and `attachment.move` make; it is not a command itself.
    pub fn move_object(
        &mut self,
        kind: ObjectKind,
        src: &str,
        nth: usize,
        pos: Vec2,
    ) -> Result<(), InsertError> {
        let page = self
            .selection
            .page
            .clone()
            .ok_or_else(|| fail("open a page first."))?;
        if self.page_notices.get(&page).is_some_and(|n| n.read_only) {
            return Err(fail("this page is read-only."));
        }
        let Some(text) = self.page_text(&page) else {
            return Err(fail("this page cannot be changed yet."));
        };
        let shown_end = find_canvas_block(text).map_or(text.len(), |b| b.block.start);
        let slots = object_slots(&notecraft_markdown::parse(
            text.get(..shown_end).unwrap_or(text),
        ));
        if nth >= count_of(&slots, kind, src) {
            return Err(fail(match kind {
                ObjectKind::Picture => "that picture is not on the page.",
                ObjectKind::File => "that file is not on the page.",
            }));
        }
        let before = self.page_objects(&page).to_vec();
        let mut after = before.clone();
        let pos = clamp(pos);
        match after.iter_mut().find(|o| o.is(kind, src, nth)) {
            Some(existing) => existing.pos = pos,
            None => {
                if after.iter().filter(|o| o.kind == kind).count() >= MAX_OBJECTS {
                    return Err(fail("this page has too many pictures or files placed."));
                }
                after.push(PlacedObject {
                    id: next_object_id(&before, kind),
                    kind,
                    src: src.to_owned(),
                    nth,
                    pos,
                });
            }
        }
        if after == before {
            return Ok(());
        }
        let label = match kind {
            ObjectKind::Picture => "Move picture",
            ObjectKind::File => "Move file",
        };
        let op = Op::SetObjects {
            page: page.clone(),
            before,
            after,
        };
        if !self.commit(Transaction::new(Scope::Page(page), label).with(op)) {
            return Err(fail("the page could not be changed."));
        }
        Ok(())
    }
}

/// `pos` with each coordinate limited to the page and not-a-number made 0.
pub(super) fn clamp(pos: Vec2) -> Vec2 {
    let pos = pos.finite_or_zero();
    Vec2::new(pos.x.clamp(0.0, MAX_COORD), pos.y.clamp(0.0, MAX_COORD))
}
