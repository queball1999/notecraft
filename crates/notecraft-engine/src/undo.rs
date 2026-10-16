//! Undo and redo (docs/architecture.md section 5). A change to the document is a
//! [`Transaction`]: a labeled list of reversible [`Op`]s. [`Session::commit`] applies a
//! transaction and records it; [`Session::undo`] and [`Session::redo`] walk the history.
//!
//! History is kept per page (strokes and text belong to a page) and once more for the
//! notebook (changes that cross pages). Each stack holds at most [`MAX_STEPS`] steps or
//! [`MAX_BYTES`] bytes, oldest dropped first. Undo takes whichever of the shown page's stack and
//! the notebook stack changed last, so Ctrl+Z undoes things in the order they happened.
//!
//! Entries are shared (`Arc`) so the command guard's snapshot of the session, which includes
//! the history, stays cheap. A command that panics or fails is rolled back with the rest of the
//! session, history included.

use std::collections::BTreeMap;
use std::sync::Arc;

use notecraft_core::Stroke;

use crate::session::Session;

mod ops;

pub use ops::{Op, Place};

/// Most steps one stack keeps.
pub const MAX_STEPS: usize = 500;
/// Most bytes (an estimate) one stack keeps.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;

/// Which stack a transaction belongs to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    /// Changes to one page, by page id.
    Page(String),
    /// Changes that cross pages or change the notebook tree (moves, renames with link
    /// updates). They undo as one step.
    Notebook,
}

/// How a transaction relates to the step before it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Merge {
    /// Its own step.
    #[default]
    No,
    /// Joins the previous step of the same stack, so the two undo together. The stroke eraser
    /// uses it: the first stroke a drag erases starts a step and the rest join it.
    IntoPrevious,
    /// Typing: joins the previous step if that was typing that this continues, up to a word
    /// boundary (see [`Session::edit_page_text`]).
    Typing,
}

/// A labeled list of reversible operations that undoes and redoes as one step.
#[derive(Clone, Debug, PartialEq)]
pub struct Transaction {
    pub scope: Scope,
    /// What the step is called in the history, for example "Ink stroke".
    pub label: String,
    pub merge: Merge,
    pub ops: Vec<Op>,
}

impl Transaction {
    pub fn new(scope: Scope, label: impl Into<String>) -> Self {
        Self {
            scope,
            label: label.into(),
            merge: Merge::No,
            ops: Vec::new(),
        }
    }

    /// Add an operation, applied after the ones already here.
    pub fn with(mut self, op: Op) -> Self {
        self.ops.push(op);
        self
    }

    /// Merge into the previous step of the same stack instead of starting a new one. When
    /// there is no previous step this is an ordinary step.
    pub fn merge_into_previous(mut self) -> Self {
        self.merge = Merge::IntoPrevious;
        self
    }

    /// Mark as typing, which coalesces into word-sized steps.
    pub fn typing(mut self) -> Self {
        self.merge = Merge::Typing;
        self
    }
}

/// One recorded step.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    /// When the step last moved between stacks or merged; the larger, the more recent.
    tick: u64,
    label: String,
    /// True when the step is typing that a later keystroke may extend.
    typing: bool,
    ops: Vec<Op>,
}

impl Entry {
    fn bytes(&self) -> usize {
        self.label.len() + self.ops.iter().map(Op::bytes).sum::<usize>()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Stack {
    undo: Vec<Arc<Entry>>,
    redo: Vec<Arc<Entry>>,
}

impl Stack {
    /// Drop the oldest steps of both lists until they fit, always keeping the newest.
    fn enforce(&mut self, max_steps: usize, max_bytes: usize) {
        trim_oldest(&mut self.undo, max_steps, max_bytes);
        trim_oldest(&mut self.redo, max_steps, max_bytes);
    }
}

fn trim_oldest(list: &mut Vec<Arc<Entry>>, max_steps: usize, max_bytes: usize) {
    let mut bytes: usize = list.iter().map(|entry| entry.bytes()).sum();
    let mut drop_count = 0;
    for entry in list.iter() {
        let remaining = list.len() - drop_count;
        if remaining <= 1 || (remaining <= max_steps && bytes <= max_bytes) {
            break;
        }
        bytes = bytes.saturating_sub(entry.bytes());
        drop_count += 1;
    }
    list.drain(..drop_count);
}

/// One line of the history for display, newest last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub label: String,
    pub scope: Scope,
}

/// Every undo and redo stack of a session.
#[derive(Clone, Debug, PartialEq)]
pub struct History {
    pages: BTreeMap<String, Stack>,
    notebook: Stack,
    clock: u64,
    /// True while the newest step of some stack is an erase that the rest of the same eraser
    /// drag may join.
    erase_chain: bool,
    max_steps: usize,
    max_bytes: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::with_limits(MAX_STEPS, MAX_BYTES)
    }
}

impl History {
    /// An empty history with its own caps (the defaults are [`MAX_STEPS`] and [`MAX_BYTES`]).
    pub fn with_limits(max_steps: usize, max_bytes: usize) -> Self {
        Self {
            pages: BTreeMap::new(),
            notebook: Stack::default(),
            clock: 0,
            erase_chain: false,
            max_steps: max_steps.max(1),
            max_bytes,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    fn stack(&self, scope: &Scope) -> Option<&Stack> {
        match scope {
            Scope::Page(id) => self.pages.get(id),
            Scope::Notebook => Some(&self.notebook),
        }
    }

    fn stack_mut(&mut self, scope: &Scope) -> &mut Stack {
        match scope {
            Scope::Page(id) => self.pages.entry(id.clone()).or_default(),
            Scope::Notebook => &mut self.notebook,
        }
    }

    /// Record a transaction that has been applied. Clears the redo steps of its stack, then
    /// merges it into the previous step or adds a step, then enforces the caps.
    pub fn record(&mut self, tx: Transaction) {
        if tx.ops.is_empty() {
            return;
        }
        if tx.merge != Merge::IntoPrevious {
            self.erase_chain = false;
        }
        let tick = self.tick();
        let (max_steps, max_bytes) = (self.max_steps, self.max_bytes);
        let stack = self.stack_mut(&tx.scope);
        stack.redo.clear();
        let typing = tx.merge == Merge::Typing;
        let merged = match (tx.merge, stack.undo.last_mut()) {
            (Merge::IntoPrevious, Some(top)) => {
                let top = Arc::make_mut(top);
                top.ops.extend(tx.ops.iter().cloned());
                top.typing = false;
                top.tick = tick;
                true
            }
            (Merge::Typing, Some(top)) if top.typing => {
                let top = Arc::make_mut(top);
                let ok = ops::coalesce_typing(&mut top.ops, &tx.ops);
                if ok {
                    top.tick = tick;
                }
                ok
            }
            _ => false,
        };
        if !merged {
            stack.undo.push(Arc::new(Entry {
                tick,
                label: tx.label,
                typing,
                ops: tx.ops,
            }));
        }
        stack.enforce(max_steps, max_bytes);
    }

    /// The scope whose newest undo step is the most recent among the shown page's stack and
    /// the notebook stack.
    fn pick(&self, page: Option<&str>, redo: bool) -> Option<Scope> {
        let tick_of = |scope: &Scope| -> Option<u64> {
            let stack = self.stack(scope)?;
            let list = if redo { &stack.redo } else { &stack.undo };
            list.last().map(|entry| entry.tick)
        };
        let page_scope = page.map(|id| Scope::Page(id.to_owned()));
        let page_tick = page_scope.as_ref().and_then(tick_of);
        let notebook_tick = tick_of(&Scope::Notebook);
        match (page_scope, page_tick, notebook_tick) {
            (Some(scope), Some(p), Some(n)) => Some(if p >= n { scope } else { Scope::Notebook }),
            (Some(scope), Some(_), None) => Some(scope),
            (_, None, Some(_)) | (None, _, Some(_)) => Some(Scope::Notebook),
            _ => None,
        }
    }

    fn top_label(&self, page: Option<&str>, redo: bool) -> Option<&str> {
        let scope = self.pick(page, redo)?;
        let stack = self.stack(&scope)?;
        let list = if redo { &stack.redo } else { &stack.undo };
        list.last().map(|entry| entry.label.as_str())
    }

    /// Whether there is a step to undo while `page` is shown.
    pub fn can_undo(&self, page: Option<&str>) -> bool {
        self.pick(page, false).is_some()
    }

    /// Whether there is an undone step to redo while `page` is shown.
    pub fn can_redo(&self, page: Option<&str>) -> bool {
        self.pick(page, true).is_some()
    }

    /// The label of the step Undo would undo.
    pub fn undo_label(&self, page: Option<&str>) -> Option<&str> {
        self.top_label(page, false)
    }

    /// The label of the step Redo would redo.
    pub fn redo_label(&self, page: Option<&str>) -> Option<&str> {
        self.top_label(page, true)
    }

    /// The undo steps that apply while `page` is shown (the page's and the notebook's), oldest
    /// first. A History view lists these.
    pub fn undo_steps(&self, page: Option<&str>) -> Vec<Step> {
        self.steps(page, false)
    }

    /// The redo steps, with the next one to redo last.
    pub fn redo_steps(&self, page: Option<&str>) -> Vec<Step> {
        self.steps(page, true)
    }

    fn steps(&self, page: Option<&str>, redo: bool) -> Vec<Step> {
        let mut found: Vec<(u64, Step)> = Vec::new();
        let page_scope = page.map(|id| Scope::Page(id.to_owned()));
        for scope in page_scope.into_iter().chain([Scope::Notebook]) {
            let Some(stack) = self.stack(&scope) else {
                continue;
            };
            let list = if redo { &stack.redo } else { &stack.undo };
            for entry in list {
                let step = Step {
                    label: entry.label.clone(),
                    scope: scope.clone(),
                };
                found.push((entry.tick, step));
            }
        }
        found.sort_by_key(|(tick, _)| *tick);
        found.into_iter().map(|(_, step)| step).collect()
    }

    fn pop(&mut self, scope: &Scope, redo: bool) -> Option<Arc<Entry>> {
        self.erase_chain = false;
        let stack = self.stack_mut(scope);
        if redo {
            return stack.redo.pop();
        }
        let popped = stack.undo.pop();
        // After an undo, typing starts a new step rather than extending the one below.
        if let Some(top) = stack.undo.last_mut().filter(|top| top.typing) {
            Arc::make_mut(top).typing = false;
        }
        popped
    }

    /// Put an entry on the other stack after it was undone (`to_redo`) or redone.
    fn push_moved(&mut self, scope: &Scope, mut entry: Arc<Entry>, to_redo: bool) {
        let tick = self.tick();
        let (max_steps, max_bytes) = (self.max_steps, self.max_bytes);
        let inner = Arc::make_mut(&mut entry);
        inner.tick = tick;
        // A redone or undone step is never extended by later typing.
        inner.typing = false;
        let stack = self.stack_mut(scope);
        if to_redo {
            stack.redo.push(entry);
        } else {
            stack.undo.push(entry);
        }
        stack.enforce(max_steps, max_bytes);
    }

    /// Forget everything, for example when a different set of notebooks is opened.
    pub fn clear(&mut self) {
        self.pages.clear();
        self.notebook = Stack::default();
    }
}

impl Session {
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Whether Undo has a step for the page shown now.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo(self.selection().page.as_deref())
    }

    /// Whether Redo has a step for the page shown now.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo(self.selection().page.as_deref())
    }

    /// Apply a transaction and record it as an undo step. If an operation cannot be applied
    /// (for example, it names a stroke that is not there), the ones already applied are
    /// reversed, nothing is recorded, and this returns false.
    pub fn commit(&mut self, tx: Transaction) -> bool {
        if tx.ops.is_empty() {
            return true;
        }
        if !ops::apply_all(self, tx.ops.iter()) {
            return false;
        }
        self.history.record(tx);
        self.mark_changed();
        true
    }

    /// Undo the most recent step that applies to the page shown now. Returns its label, or
    /// `None` when there is nothing to undo. A step that can no longer be reversed (the
    /// document changed underneath it) is dropped and `None` is returned.
    pub fn undo(&mut self) -> Option<String> {
        self.step(false)
    }

    /// Redo the most recently undone step. Returns its label.
    pub fn redo(&mut self) -> Option<String> {
        self.step(true)
    }

    fn step(&mut self, redo: bool) -> Option<String> {
        let page = self.selection().page.clone();
        let scope = self.history.pick(page.as_deref(), redo)?;
        let entry = self.history.pop(&scope, redo)?;
        let label = entry.label.clone();
        let applied = if redo {
            ops::apply_all(self, entry.ops.iter())
        } else {
            let inverse: Vec<Op> = entry.ops.iter().rev().map(Op::inverse).collect();
            ops::apply_all(self, inverse.iter())
        };
        if !applied {
            tracing::warn!("dropped a history step that could no longer be applied: {label}");
            return None;
        }
        self.history.push_moved(&scope, entry, !redo);
        self.mark_changed();
        Some(label)
    }

    /// Move a page of the notebook shown to `section` at `index` (counted with the page already
    /// taken out; past the end means last), as one notebook-level undo step. The engine call
    /// the structure commands (PR 1.13) make; it is not a command itself. Returns false,
    /// changing nothing, when the page or section is not in the notebook shown.
    pub fn move_page(&mut self, page: &str, section: &str, index: usize) -> bool {
        let Some(notebook) = self.current_notebook() else {
            return false;
        };
        let Some(from) = notebook.section_of_page(page) else {
            return false;
        };
        let Some(from_index) = from.pages.iter().position(|p| p.id == page) else {
            return false;
        };
        let Some(target) = notebook.section(section) else {
            return false;
        };
        // Pages in the target once this one is out of it.
        let others = target.pages.len() - usize::from(target.id == from.id);
        let index = index.min(others);
        let op = Op::MovePage {
            notebook: notebook.id.clone(),
            page: page.to_owned(),
            from: Place {
                section: from.id.clone(),
                index: from_index,
            },
            to: Place {
                section: section.to_owned(),
                index,
            },
        };
        self.commit(Transaction::new(Scope::Notebook, "Move page").with(op))
    }

    /// Replace `remove_len` bytes at byte offset `at` of page `page`'s markdown source with
    /// `inserted`, as one undo step. With `typing` set, keystrokes coalesce into word-sized
    /// steps (see [`Merge::Typing`]): consecutive typing, Backspace, or Delete in one place
    /// undoes together, and a new word, a different place, or any other step starts a new one.
    ///
    /// This is the engine call container text editing (PR 1.5) makes from its commands. It is
    /// not a command itself. Returns false, changing nothing, when the range is out of bounds
    /// or splits a character, or when there is nothing to change.
    pub fn edit_page_text(
        &mut self,
        page: &str,
        at: usize,
        remove_len: usize,
        inserted: &str,
        typing: bool,
    ) -> bool {
        let text = self.page_text(page).unwrap_or_default();
        let end = at.saturating_add(remove_len);
        let Some(removed) = text.get(at..end) else {
            return false;
        };
        if removed.is_empty() && inserted.is_empty() {
            return false;
        }
        let label = if typing { "Typing" } else { "Edit text" };
        let op = Op::ReplaceText {
            page: page.to_owned(),
            at,
            removed: removed.to_owned(),
            inserted: inserted.to_owned(),
        };
        let tx = Transaction::new(Scope::Page(page.to_owned()), label).with(op);
        self.commit(if typing { tx.typing() } else { tx })
    }

    /// Record a stroke added to the end of a page's ink, as one undo step. The helper for
    /// `ink.add_stroke`; returns false when the page cannot hold the stroke.
    pub(crate) fn commit_add_stroke(&mut self, page: &str, stroke: Stroke) -> bool {
        let index = self.ink().layer(page).map_or(0, |layer| layer.len());
        let tx =
            Transaction::new(Scope::Page(page.to_owned()), "Ink stroke").with(Op::InsertStroke {
                page: page.to_owned(),
                index,
                stroke,
            });
        self.commit(tx)
    }

    /// A new eraser drag starts: the next erase is a step of its own.
    pub(crate) fn end_erase_chain(&mut self) {
        self.history.erase_chain = false;
    }

    /// Record the ops of one erase as one undo step. With `join` set, and when the step just
    /// before was an erase too (an earlier part of the same eraser drag), the ops join that
    /// step so the whole drag undoes at once. Anything else in between, such as another
    /// command's step or an undo, ends the chain. `ops` must not be empty.
    pub(crate) fn commit_erase(&mut self, page: &str, ops: Vec<Op>, join: bool) -> bool {
        let mut tx = Transaction::new(Scope::Page(page.to_owned()), "Erase ink");
        tx.ops = ops;
        if join && self.history.erase_chain {
            tx = tx.merge_into_previous();
        }
        let done = self.commit(tx);
        self.history.erase_chain = done;
        done
    }
}

#[cfg(test)]
mod tests;
