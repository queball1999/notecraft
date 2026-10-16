//! Reversible operations on the session. Each has an exact [`Op::inverse`], so undo applies
//! the inverses of a step in reverse order and redo applies the step again.

use std::sync::Arc;

use notecraft_core::{InkPoint, Stroke};
use notecraft_model::section::MAX_GROUP_DEPTH;
use notecraft_model::{Section, SectionNode};

use crate::session::Session;
use crate::view::PageStyle;

/// One reversible change to the document.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Put a stroke into a page's ink at `index` in paint order.
    InsertStroke {
        page: String,
        index: usize,
        stroke: Stroke,
    },
    /// Take the stroke at `index` out of a page's ink. `stroke` is kept to put it back.
    DeleteStroke {
        page: String,
        index: usize,
        stroke: Stroke,
    },
    /// Replace `removed` at byte offset `at` of a page's markdown source with `inserted`.
    /// Typing is an insertion (`removed` empty); deleting is a removal (`inserted` empty).
    /// Both texts are kept, so the inverse swaps them.
    ReplaceText {
        page: String,
        at: usize,
        removed: String,
        inserted: String,
    },
    /// Change a page's style (rule lines, background color, paper size) from `before` to
    /// `after`.
    SetPageStyle {
        page: String,
        before: PageStyle,
        after: PageStyle,
    },
    /// Move a page to another spot in a notebook: another section, or another place in the
    /// same one. A notebook-level change: it goes in a [`crate::undo::Scope::Notebook`]
    /// transaction. Structure editing (PR 1.13) builds on it.
    MovePage {
        notebook: String,
        page: String,
        /// Where the page is before the move.
        from: Place,
        /// Where it goes. `to.index` counts the section's pages with the page already taken
        /// out of `from`, and may be at most that count (the move is refused otherwise).
        to: Place,
    },
}

/// A spot in a notebook's page lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub section: String,
    pub index: usize,
}

/// The section `id` in `nodes`, looking through section groups to a bounded depth.
fn section_mut<'a>(
    nodes: &'a mut [SectionNode],
    id: &str,
    depth: usize,
) -> Option<&'a mut Section> {
    if depth > MAX_GROUP_DEPTH {
        return None;
    }
    for node in nodes {
        match node {
            SectionNode::Section(section) if section.id == id => return Some(section),
            SectionNode::Section(_) => {}
            SectionNode::Group(group) => {
                if let Some(found) = section_mut(&mut group.children, id, depth + 1) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// Take page `page` out of `from` and put it at `to`. False, changing nothing, when the
/// notebook, either section, or the page is not where the move says.
fn move_page(session: &mut Session, notebook: &str, page: &str, from: &Place, to: &Place) -> bool {
    let Some(nb) = session
        .notebooks_mut()
        .iter_mut()
        .find(|nb| nb.id == notebook)
    else {
        return false;
    };
    if nb.section(&to.section).is_none() {
        return false;
    }
    let taken = {
        let Some(source) = section_mut(&mut nb.sections, &from.section, 0) else {
            return false;
        };
        if source.pages.get(from.index).map(|p| p.id.as_str()) != Some(page) {
            return false;
        }
        source.pages.remove(from.index)
    };
    let placed = section_mut(&mut nb.sections, &to.section, 0).is_some_and(|target| {
        // An index past the end is refused, not clamped: the inverse must land exactly where
        // the page came from.
        let fits = to.index <= target.pages.len();
        if fits {
            target.pages.insert(to.index, taken.clone());
        }
        fits
    });
    if placed {
        return true;
    }
    // Put the page back where it was rather than lose it.
    if let Some(source) = section_mut(&mut nb.sections, &from.section, 0) {
        let at = from.index.min(source.pages.len());
        source.pages.insert(at, taken);
    }
    false
}

impl Op {
    /// The operation that reverses this one.
    pub fn inverse(&self) -> Op {
        match self {
            Op::InsertStroke {
                page,
                index,
                stroke,
            } => Op::DeleteStroke {
                page: page.clone(),
                index: *index,
                stroke: stroke.clone(),
            },
            Op::DeleteStroke {
                page,
                index,
                stroke,
            } => Op::InsertStroke {
                page: page.clone(),
                index: *index,
                stroke: stroke.clone(),
            },
            Op::ReplaceText {
                page,
                at,
                removed,
                inserted,
            } => Op::ReplaceText {
                page: page.clone(),
                at: *at,
                removed: inserted.clone(),
                inserted: removed.clone(),
            },
            Op::SetPageStyle {
                page,
                before,
                after,
            } => Op::SetPageStyle {
                page: page.clone(),
                before: *after,
                after: *before,
            },
            Op::MovePage {
                notebook,
                page,
                from,
                to,
            } => Op::MovePage {
                notebook: notebook.clone(),
                page: page.clone(),
                from: to.clone(),
                to: from.clone(),
            },
        }
    }

    /// A rough size in memory, for the history cap.
    pub fn bytes(&self) -> usize {
        const BASE: usize = 64;
        match self {
            Op::InsertStroke { page, stroke, .. } | Op::DeleteStroke { page, stroke, .. } => {
                BASE + page.len() + stroke.id.len() + stroke.points.len() * size_of::<InkPoint>()
            }
            Op::ReplaceText {
                page,
                removed,
                inserted,
                ..
            } => BASE + page.len() + removed.len() + inserted.len(),
            Op::SetPageStyle { page, .. } => BASE + page.len() + size_of::<PageStyle>() * 2,
            Op::MovePage {
                notebook,
                page,
                from,
                to,
            } => BASE + notebook.len() + page.len() + from.section.len() + to.section.len(),
        }
    }

    /// Apply the operation. Returns false, changing nothing, when it does not fit the
    /// document as it is now.
    pub(crate) fn apply(&self, session: &mut Session) -> bool {
        match self {
            Op::InsertStroke {
                page,
                index,
                stroke,
            } => {
                let layers = &mut session.ink_mut().layers;
                let mut layer = layers.remove(page).unwrap_or_default();
                let added = Arc::make_mut(&mut layer).insert(*index, stroke.clone());
                if !layer.is_empty() {
                    layers.insert(page.clone(), layer);
                }
                added
            }
            Op::DeleteStroke {
                page,
                index,
                stroke,
            } => {
                let layers = &mut session.ink_mut().layers;
                let Some(layer) = layers.get_mut(page) else {
                    return false;
                };
                let fits = layer
                    .strokes()
                    .get(*index)
                    .is_some_and(|s| s.id == stroke.id);
                if !fits {
                    return false;
                }
                Arc::make_mut(layer).remove(&stroke.id);
                if layer.is_empty() {
                    layers.remove(page);
                }
                true
            }
            Op::ReplaceText {
                page,
                at,
                removed,
                inserted,
            } => {
                let text = session.page_text(page).unwrap_or_default();
                let end = at.saturating_add(removed.len());
                // `get` is None past the end or off a character boundary, so a stale offset
                // is refused instead of panicking.
                if text.get(*at..end) != Some(removed.as_str()) {
                    return false;
                }
                let (Some(head), Some(tail)) = (text.get(..*at), text.get(end..)) else {
                    return false;
                };
                let mut next = String::with_capacity(head.len() + inserted.len() + tail.len());
                next.push_str(head);
                next.push_str(inserted);
                next.push_str(tail);
                session.set_page_source(page, next);
                true
            }
            Op::SetPageStyle {
                page,
                before,
                after,
            } => {
                if session.style_of(page) != *before {
                    return false;
                }
                session.set_style_of(page, *after);
                true
            }
            Op::MovePage {
                notebook,
                page,
                from,
                to,
            } => move_page(session, notebook, page, from, to),
        }
    }
}

/// The longest run of typing one step may grow to, in bytes.
const MAX_TYPING_RUN: usize = 512;

/// Extend the typing step `prev` with `next` when they run together: the same page, and
/// either the next characters typed right after the last ones, or deletes that continue the
/// last delete (Backspace or Delete). A word boundary starts a new step: text typed after
/// whitespace that was itself typed, so one step is a word and the space after it. Returns
/// false, changing nothing, when they do not run together; the caller then starts a new step.
pub(crate) fn coalesce_typing(prev: &mut [Op], next: &[Op]) -> bool {
    let (
        [Op::ReplaceText {
            page,
            at,
            removed,
            inserted,
        }],
        [Op::ReplaceText {
            page: next_page,
            at: next_at,
            removed: next_removed,
            inserted: next_inserted,
        }],
    ) = (&mut *prev, next)
    else {
        return false;
    };
    if page != next_page {
        return false;
    }
    let typed_on = removed.is_empty() && next_removed.is_empty();
    let deleted_on = inserted.is_empty() && next_inserted.is_empty();
    if typed_on {
        let continues = *next_at == at.saturating_add(inserted.len());
        let ends_word = inserted
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace);
        let starts_word = next_inserted
            .chars()
            .next()
            .is_some_and(|c| !c.is_whitespace());
        let fits = inserted.len() + next_inserted.len() <= MAX_TYPING_RUN;
        if continues && !(ends_word && starts_word) && fits {
            inserted.push_str(next_inserted);
            return true;
        }
    } else if deleted_on && removed.len() + next_removed.len() <= MAX_TYPING_RUN {
        if next_at.saturating_add(next_removed.len()) == *at {
            // Backspace: the new deletion sits just before the last one.
            *at = *next_at;
            removed.insert_str(0, next_removed);
            return true;
        }
        if next_at == at {
            // Delete: the next characters after the same spot.
            removed.push_str(next_removed);
            return true;
        }
    }
    false
}

/// Apply `ops` in order. If one does not fit, reverse the ones already applied and return
/// false.
pub(crate) fn apply_all<'a>(session: &mut Session, ops: impl IntoIterator<Item = &'a Op>) -> bool {
    let mut applied: Vec<&Op> = Vec::new();
    for op in ops {
        if op.apply(session) {
            applied.push(op);
        } else {
            for done in applied.iter().rev() {
                // Reversing an operation that just applied cannot fail; if it somehow does,
                // there is nothing better to do.
                let _ = done.inverse().apply(session);
            }
            return false;
        }
    }
    true
}
