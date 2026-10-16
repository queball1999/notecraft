//! Reversible operations on the session. Each has an exact [`Op::inverse`], so undo applies
//! the inverses of a step in reverse order and redo applies the step again.

use std::sync::Arc;

use notecraft_core::{InkPoint, Stroke};

use crate::session::Session;

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
        }
    }

    /// A rough size in memory, for the history cap.
    pub fn bytes(&self) -> usize {
        const BASE: usize = 64;
        match self {
            Op::InsertStroke { page, stroke, .. } | Op::DeleteStroke { page, stroke, .. } => {
                BASE + page.len() + stroke.id.len() + stroke.points.len() * size_of::<InkPoint>()
            }
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
        }
    }
}

/// Extend the typing step `prev` with `next` when they run together. Returns false when they
/// do not (the caller then starts a new step).
pub(crate) fn coalesce_typing(_prev: &mut Vec<Op>, _next: &[Op]) -> bool {
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
