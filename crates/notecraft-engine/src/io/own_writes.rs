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
    /// The page's frontmatter title differs from the one saved before (or none was known), so
    /// the index and the navigation must still refresh for this save.
    title_changed: bool,
}

/// Recent saves by page id.
#[derive(Default)]
pub(super) struct OwnWrites {
    recent: BTreeMap<String, Written>,
    /// The frontmatter title of each page as last saved; absent for a page not saved yet.
    titles: BTreeMap<String, Option<String>>,
}

/// The frontmatter title in saved page text, trimmed; `None` when there is none.
fn title_of(bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(64 * 1024)];
    let text = String::from_utf8_lossy(head);
    let title = notecraft_markdown::parse_frontmatter(&text)?
        .title()?
        .trim()
        .to_owned();
    (!title.is_empty()).then_some(title)
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
        let title = title_of(&bytes);
        let title_changed = self.titles.get(page_id) != Some(&title);
        self.titles.insert(page_id.to_owned(), title);
        self.recent.insert(
            page_id.to_owned(),
            Written {
                file_name,
                folder_name,
                hash: hash_bytes(&bytes),
                at: now,
                title_changed,
            },
        );
    }

    /// Whether the change at `changed` (the file of page `page_id`, if it is a known page) is
    /// the echo of NoteCraft's own save.
    pub(super) fn is_echo(&self, page_id: Option<&str>, changed: &Path, now: Instant) -> bool {
        let fresh = |w: &Written| now.saturating_duration_since(w.at) < OWN_WRITE_WINDOW;
        if let Some(written) = page_id.and_then(|id| self.recent.get(id)) {
            if fresh(written) {
                return !written.title_changed
                    && std::fs::read(changed).is_ok_and(|b| hash_bytes(&b) == written.hash);
            }
        }
        // A temporary sibling of a page file that was just written, or its folder.
        let (Some(name), Some(folder)) = (name_of(changed), changed.parent().and_then(name_of))
        else {
            return false;
        };
        self.recent.values().any(|w| {
            let temp_sibling =
                w.folder_name == folder && name != w.file_name && name.contains(&w.file_name);
            // The write also reports the folder that holds the page.
            let its_folder = w.folder_name == name && changed.is_dir();
            fresh(w) && !w.title_changed && (temp_sibling || its_folder)
        })
    }
}
