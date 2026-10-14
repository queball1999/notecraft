//! The editing session: the state commands read and change. Phase 0 has no notes yet, so
//! the session only counts changes; open notebooks, the current page, and undo history land
//! in Phase 1.

/// Everything a command can see and change. Commands run on the thread that owns it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    revision: u64,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    /// Goes up by one every time a command changes the session.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Record that a command changed the session.
    pub fn mark_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}
