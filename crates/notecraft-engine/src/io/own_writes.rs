//! Remembering the page files NoteCraft just wrote, so the folder watcher's echo of its own
//! save does not make the notebook index refresh (a full folder diff) after every auto-save.
//!
//! An event is ignored only when the file still holds exactly the bytes NoteCraft wrote and
//! the write is recent, or when it is a temporary sibling of such a file (the atomic write
//! creates one). An outside edit therefore still marks the notebook stale.

use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::{Duration, Instant};

/// How long after a save its watcher events are treated as the save's own.
pub(super) const OWN_WRITE_WINDOW: Duration = Duration::from_secs(5);

struct Written {
    /// Name of the page file.
    file_name: String,
    /// Name of the folder holding it.
    folder_name: String,
    hash: u64,
    at: Instant,
}

/// Recent saves by page id.
#[derive(Default)]
pub(super) struct OwnWrites {
    recent: BTreeMap<String, Written>,
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn name_of(path: &Path) -> Option<String> {
    path.file_name().map(|n| n.to_string_lossy().into_owned())
}

impl OwnWrites {
    /// Note that the file of page `page_id` was just written at `path`.
    pub(super) fn record(&mut self, page_id: &str, path: &Path, now: Instant) {
        self.recent
            .retain(|_, w| now.saturating_duration_since(w.at) < OWN_WRITE_WINDOW);
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let (Some(file_name), Some(folder_name)) = (name_of(path), path.parent().and_then(name_of))
        else {
            return;
        };
        self.recent.insert(
            page_id.to_owned(),
            Written {
                file_name,
                folder_name,
                hash: hash_bytes(&bytes),
                at: now,
            },
        );
    }

    /// Whether the change at `changed` (the file of page `page_id`, if it is a known page) is
    /// the echo of NoteCraft's own save.
    pub(super) fn is_echo(&self, page_id: Option<&str>, changed: &Path, now: Instant) -> bool {
        let fresh = |w: &Written| now.saturating_duration_since(w.at) < OWN_WRITE_WINDOW;
        if let Some(written) = page_id.and_then(|id| self.recent.get(id)) {
            if fresh(written) {
                return std::fs::read(changed).is_ok_and(|b| hash_bytes(&b) == written.hash);
            }
        }
        // A temporary sibling of a page file that was just written.
        let (Some(name), Some(folder)) = (name_of(changed), changed.parent().and_then(name_of))
        else {
            return false;
        };
        self.recent.values().any(|w| {
            fresh(w)
                && w.folder_name == folder
                && name != w.file_name
                && name.contains(&w.file_name)
        })
    }
}
