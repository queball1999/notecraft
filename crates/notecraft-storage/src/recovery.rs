//! The recovery journal: a copy of each page's unsaved text, kept in the settings folder so a
//! crash or power loss does not lose edits (docs/architecture.md section 3.4).
//!
//! One small JSON file per page, named by a hash of the page file's path, so a newer copy
//! replaces an older one. Entries are written with `craft-atomic`. The journal is lenient: an
//! entry that does not parse, is too large, or has the wrong version is skipped (and logged),
//! never an error for the caller, and the number of entries read is capped.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::page_file::MAX_PAGE_BYTES;

/// Journal entries read at most; more are ignored.
pub const MAX_ENTRIES: usize = 2000;
const VERSION: u32 = 1;
const EXTENSION: &str = "json";

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    /// The page file the text belongs to (absolute).
    path: String,
    text: String,
}

/// Unsaved text of one page file, found in the journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryEntry {
    pub path: PathBuf,
    pub text: String,
}

/// The journal folder.
#[derive(Clone, Debug)]
pub struct Journal {
    dir: PathBuf,
}

impl Journal {
    /// A journal in `dir`; the folder is created on the first write.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn file_for(&self, path: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        self.dir
            .join(format!("{:016x}.{EXTENSION}", hasher.finish()))
    }

    /// Record `text` as the unsaved text of the page file at `path`.
    pub fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        let stored = Stored {
            version: VERSION,
            path: path.to_string_lossy().into_owned(),
            text: text.to_owned(),
        };
        let bytes = serde_json::to_vec(&stored).map_err(io::Error::other)?;
        fs::create_dir_all(&self.dir)?;
        craft_atomic::atomic_write(&self.file_for(path), &bytes)
    }

    /// Forget the entry for the page file at `path` (it was saved or discarded).
    pub fn remove(&self, path: &Path) {
        if let Err(err) = fs::remove_file(self.file_for(path)) {
            if err.kind() != io::ErrorKind::NotFound {
                tracing::warn!("cannot remove a recovery entry: {err}");
            }
        }
    }

    /// Every readable entry, in file name order. Bad entries are skipped.
    pub fn load_all(&self) -> Vec<RecoveryEntry> {
        let Ok(read) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = read
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == EXTENSION))
            .collect();
        files.sort();
        let mut out = Vec::new();
        for file in files.into_iter().take(MAX_ENTRIES) {
            match read_entry(&file) {
                Some(entry) => out.push(entry),
                None => tracing::warn!(file = %file.display(), "skipped a bad recovery entry"),
            }
        }
        out
    }
}

fn read_entry(file: &Path) -> Option<RecoveryEntry> {
    let meta = fs::metadata(file).ok()?;
    // JSON escaping can roughly double the text, so allow twice the page cap.
    if !meta.is_file() || meta.len() > MAX_PAGE_BYTES.saturating_mul(2) {
        return None;
    }
    let bytes = fs::read(file).ok()?;
    let stored: Stored = serde_json::from_slice(&bytes).ok()?;
    if stored.version != VERSION || stored.path.is_empty() {
        return None;
    }
    Some(RecoveryEntry {
        path: PathBuf::from(stored.path),
        text: stored.text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_round_trip_and_are_replaced_and_removed() {
        let dir = tempfile::TempDir::new().unwrap();
        let journal = Journal::new(dir.path().join("recovery"));
        assert!(journal.load_all().is_empty());
        let page = dir.path().join("Sec").join("A.md");
        journal.write(&page, "one \u{e9}\n\"q\"").unwrap();
        journal.write(&page, "two").unwrap();
        let other = dir.path().join("B.md");
        journal.write(&other, "b").unwrap();
        let all = journal.load_all();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|e| e.path == page && e.text == "two"));
        journal.remove(&page);
        journal.remove(&page);
        assert_eq!(journal.load_all().len(), 1);
    }

    #[test]
    fn damaged_entries_are_skipped() {
        let dir = tempfile::TempDir::new().unwrap();
        let journal = Journal::new(dir.path());
        journal.write(&dir.path().join("A.md"), "ok").unwrap();
        std::fs::write(dir.path().join("junk.json"), b"{not json").unwrap();
        std::fs::write(
            dir.path().join("old.json"),
            br#"{"version":99,"path":"x","text":"t"}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("note.txt"), b"x").unwrap();
        let all = journal.load_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].text, "ok");
    }
}
