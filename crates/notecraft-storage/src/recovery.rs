//! The recovery journal: a copy of each page's unsaved text, kept in the settings folder so a
//! crash or power loss does not lose edits (docs/architecture.md section 3.4).
//!
//! One small JSON file per page, so a newer copy replaces an older one. The file is named
//! `fnv-<16 hex digits>.json`, where the digits are the FNV-1a 64 hash of a canonical form of
//! the page path (see [`canonical_key`]), which is stable across Rust releases and across
//! spellings of the same path (`a/../b.md`, a relative path, a different case on a
//! case-insensitive system). Older versions named files by `DefaultHasher`, which is not stable
//! across Rust releases; those entries still load, and they are matched by the path stored
//! inside, not by name, when the page is saved, discarded, or rewritten.
//!
//! Entries are written with `craft-atomic`. The journal is lenient: an entry that does not
//! parse, is too large, or has the wrong version is skipped (and logged), never an error for
//! the caller, and the number of entries read is capped. [`Journal::prune`] deletes what can
//! never be offered back.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::attachments::{fnv1a64, strip_verbatim};
use crate::page_file::MAX_PAGE_BYTES;

/// Journal entries offered at most; the oldest beyond this are dropped by [`Journal::prune`].
pub const MAX_ENTRIES: usize = 2000;
/// Journal files looked at in one scan; more are left alone.
const MAX_SCANNED: usize = MAX_ENTRIES * 2;
const VERSION: u32 = 1;
const EXTENSION: &str = "json";
/// File name prefix of entries named by the stable hash. Other `.json` files are old-style.
const NAME_PREFIX: &str = "fnv-";

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    /// The page file the text belongs to (absolute, as the user sees it).
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

/// One journal file that parsed.
struct Scanned {
    file: PathBuf,
    entry: RecoveryEntry,
    key: String,
    modified: SystemTime,
    stable_name: bool,
}

/// What [`Journal::scan`] found.
struct Scan {
    good: Vec<Scanned>,
    /// Files that can never be offered back.
    bad: Vec<PathBuf>,
}

impl Journal {
    /// A journal in `dir`; the folder is created on the first write.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn file_for_key(&self, key: &str) -> PathBuf {
        self.dir.join(format!(
            "{NAME_PREFIX}{:016x}.{EXTENSION}",
            fnv1a64(key.as_bytes())
        ))
    }

    /// Record `text` as the unsaved text of the page file at `path`. Any older entry for the
    /// same page under an old-style name is removed.
    pub fn write(&self, path: &Path, text: &str) -> io::Result<()> {
        let shown = normalize(&absolute(path));
        let key = key_text(path);
        let stored = Stored {
            version: VERSION,
            path: shown.to_string_lossy().into_owned(),
            text: text.to_owned(),
        };
        let bytes = serde_json::to_vec(&stored).map_err(io::Error::other)?;
        fs::create_dir_all(&self.dir)?;
        craft_atomic::atomic_write(&self.file_for_key(&key), &bytes)?;
        self.remove_old_style(&key);
        Ok(())
    }

    /// Forget the entry for the page file at `path` (it was saved or discarded), whatever the
    /// entry file is called.
    pub fn remove(&self, path: &Path) {
        let key = key_text(path);
        remove_file(&self.file_for_key(&key));
        self.remove_old_style(&key);
    }

    /// Delete old-style entry files whose stored path maps to `key`.
    fn remove_old_style(&self, key: &str) {
        for file in self.files().into_iter().filter(|f| !is_stable_name(f)) {
            if let Some(entry) = read_entry(&file) {
                if key_text(&entry.path) == key {
                    remove_file(&file);
                }
            }
        }
    }

    /// The journal's `.json` files in name order, at most [`MAX_SCANNED`].
    fn files(&self) -> Vec<PathBuf> {
        let Ok(read) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = read
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == EXTENSION))
            .collect();
        files.sort();
        files.truncate(MAX_SCANNED);
        files
    }

    fn scan(&self) -> Scan {
        let mut good = Vec::new();
        let mut bad = Vec::new();
        for file in self.files() {
            let Ok(meta) = fs::metadata(&file) else {
                bad.push(file);
                continue;
            };
            // Folders and the like are not ours to judge.
            if !meta.is_file() {
                continue;
            }
            match read_entry(&file) {
                Some(entry) => good.push(Scanned {
                    key: key_text(&entry.path),
                    modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    stable_name: is_stable_name(&file),
                    file,
                    entry,
                }),
                None => bad.push(file),
            }
        }
        Scan { good, bad }
    }

    /// Split the readable entries into the newest per page (at most [`MAX_ENTRIES`], the
    /// newest kept) and the files to drop (older duplicates, then the oldest over the cap).
    fn pick(good: Vec<Scanned>) -> (Vec<Scanned>, Vec<PathBuf>) {
        let mut by_key: BTreeMap<String, Scanned> = BTreeMap::new();
        let mut dropped = Vec::new();
        for item in good {
            match by_key.remove(&item.key) {
                None => {
                    by_key.insert(item.key.clone(), item);
                }
                Some(other) => {
                    let (keep, drop) = if wins(&item, &other) {
                        (item, other)
                    } else {
                        (other, item)
                    };
                    dropped.push(drop.file);
                    by_key.insert(keep.key.clone(), keep);
                }
            }
        }
        let mut kept: Vec<Scanned> = by_key.into_values().collect();
        if kept.len() > MAX_ENTRIES {
            kept.sort_by(|a, b| {
                b.modified
                    .cmp(&a.modified)
                    .then_with(|| a.file.cmp(&b.file))
            });
            dropped.extend(kept.split_off(MAX_ENTRIES).into_iter().map(|s| s.file));
        }
        kept.sort_by(|a, b| a.file.cmp(&b.file));
        (kept, dropped)
    }

    /// Every readable entry, one per page (the newest), in file name order. Bad entries are
    /// skipped.
    pub fn load_all(&self) -> Vec<RecoveryEntry> {
        let scan = self.scan();
        for file in &scan.bad {
            tracing::warn!(file = %file.display(), "skipped a bad recovery entry");
        }
        let (kept, _) = Self::pick(scan.good);
        kept.into_iter().map(|s| s.entry).collect()
    }

    /// Delete the files that can never be offered back: damaged or unreadable ones, ones over
    /// the size cap, ones with an empty path or a wrong version, older duplicates for the
    /// same page, and the oldest entries beyond [`MAX_ENTRIES`]. A valid, newest entry is
    /// never deleted, including one for a page whose file is gone (that raises the "moved or
    /// deleted" question). Returns how many files were deleted.
    pub fn prune(&self) -> usize {
        let scan = self.scan();
        let (_, mut doomed) = Self::pick(scan.good);
        doomed.extend(scan.bad);
        for file in &doomed {
            remove_file(file);
        }
        doomed.len()
    }
}

/// Whether `a` should be kept over `b` when both are for the same page: newer, then the
/// stable name, then the later file name.
fn wins(a: &Scanned, b: &Scanned) -> bool {
    (a.modified, a.stable_name, &a.file) > (b.modified, b.stable_name, &b.file)
}

fn is_stable_name(file: &Path) -> bool {
    file.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with(NAME_PREFIX))
}

fn remove_file(file: &Path) {
    if let Err(err) = fs::remove_file(file) {
        if err.kind() != io::ErrorKind::NotFound {
            tracing::warn!("cannot remove a recovery entry: {err}");
        }
    }
}

/// `path` made absolute without touching the disk (relative paths are joined to the current
/// folder). Falls back to the path as given.
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

/// Resolve `.` and `..` components without touching the disk.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(part);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// The canonical form of a page path, used to key entries: the canonical path when the file
/// exists; otherwise the canonical parent folder joined with the file name; otherwise the
/// path as given (made absolute and normalized lexically).
///
/// A `..` is resolved on disk when the path exists (so a link in the path is followed), and
/// lexically when it does not.
pub fn canonical_key(path: &Path) -> PathBuf {
    let abs = absolute(path);
    if let Ok(real) = fs::canonicalize(&abs) {
        return strip_verbatim(&real);
    }
    let abs = normalize(&abs);
    if let (Some(parent), Some(name)) = (abs.parent(), abs.file_name()) {
        if let Ok(real) = fs::canonicalize(parent) {
            return strip_verbatim(&real).join(name);
        }
    }
    abs
}

/// The key as text; folded to lower case on systems whose file systems ignore case.
fn key_text(path: &Path) -> String {
    let text = canonical_key(path).to_string_lossy().into_owned();
    if cfg!(any(windows, target_os = "macos")) {
        text.to_lowercase()
    } else {
        text
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

    fn entry_files(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn old_style(dir: &Path, name: &str, path: &Path, text: &str) {
        let stored = Stored {
            version: VERSION,
            path: path.to_string_lossy().into_owned(),
            text: text.to_owned(),
        };
        std::fs::write(dir.join(name), serde_json::to_vec(&stored).unwrap()).unwrap();
    }

    fn set_age(file: &Path, secs_ago: u64) {
        let when = std::time::SystemTime::now() - std::time::Duration::from_secs(secs_ago);
        let f = std::fs::File::options().write(true).open(file).unwrap();
        f.set_modified(when).unwrap();
    }

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

    #[test]
    fn entry_names_use_fnv_1a_and_are_stable() {
        let dir = tempfile::TempDir::new().unwrap();
        let journal = Journal::new(dir.path().join("j"));
        let page = dir.path().join("A.md");
        journal.write(&page, "x").unwrap();
        let key = key_text(&page);
        let expected = format!("fnv-{:016x}.json", fnv1a64(key.as_bytes()));
        assert_eq!(entry_files(&dir.path().join("j")), vec![expected]);
        // Known FNV-1a 64 vectors pin the algorithm.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn old_style_entries_load_and_are_removed_by_stored_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let j = dir.path().join("j");
        std::fs::create_dir_all(&j).unwrap();
        let page = dir.path().join("A.md");
        old_style(&j, "0123456789abcdef.json", &page, "old");
        let journal = Journal::new(&j);
        let all = journal.load_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].text, "old");
        // Remove matches by the stored path, whatever the file is called.
        journal.remove(&page);
        assert!(journal.load_all().is_empty());
        assert!(entry_files(&j).is_empty());
    }

    #[test]
    fn writing_replaces_an_old_style_entry_for_the_same_page() {
        let dir = tempfile::TempDir::new().unwrap();
        let j = dir.path().join("j");
        std::fs::create_dir_all(&j).unwrap();
        let page = dir.path().join("A.md");
        let other = dir.path().join("B.md");
        old_style(&j, "1111111111111111.json", &page, "old");
        old_style(&j, "2222222222222222.json", &other, "other");
        let journal = Journal::new(&j);
        journal.write(&page, "new").unwrap();
        let all = journal.load_all();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|e| e.path == page && e.text == "new"));
        assert!(all.iter().any(|e| e.path == other && e.text == "other"));
        assert_eq!(entry_files(&j).len(), 2);
    }

    #[test]
    fn different_spellings_of_a_path_share_one_entry() {
        let dir = tempfile::TempDir::new().unwrap();
        let sec = dir.path().join("Sec");
        std::fs::create_dir_all(sec.join("sub")).unwrap();
        let page = sec.join("A.md");
        std::fs::write(&page, "disk").unwrap();
        let journal = Journal::new(dir.path().join("j"));
        journal.write(&page, "one").unwrap();
        let dotted = sec.join("sub").join("..").join("A.md");
        journal.write(&dotted, "two").unwrap();
        let all = journal.load_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].text, "two");
        // The path the user sees is stored (tidied, not canonical).
        assert_eq!(all[0].path, page);
        journal.remove(&dotted);
        assert!(journal.load_all().is_empty());
    }

    #[test]
    fn a_missing_file_is_keyed_through_its_parent() {
        let dir = tempfile::TempDir::new().unwrap();
        let sec = dir.path().join("Sec");
        std::fs::create_dir_all(sec.join("x")).unwrap();
        let gone = sec.join("Gone.md");
        let dotted = sec.join("x").join("..").join("Gone.md");
        assert_eq!(canonical_key(&gone), canonical_key(&dotted));
        let journal = Journal::new(dir.path().join("j"));
        journal.write(&gone, "a").unwrap();
        journal.write(&dotted, "b").unwrap();
        assert_eq!(journal.load_all().len(), 1);
        // Neither the file nor its folder exists: the path as given is the key.
        let nowhere = std::path::PathBuf::from("/no_such_root_for_notecraft/q/../n/Z.md");
        if cfg!(unix) {
            assert_eq!(
                canonical_key(&nowhere),
                Path::new("/no_such_root_for_notecraft/n/Z.md")
            );
        }
    }

    #[test]
    fn a_relative_path_maps_to_the_absolute_entry() {
        let dir = tempfile::TempDir::new().unwrap();
        let journal = Journal::new(dir.path().join("j"));
        let cwd = std::env::current_dir().unwrap();
        let rel = Path::new("relative_page_for_journal_test.md");
        journal.write(rel, "r").unwrap();
        journal.write(&cwd.join(rel), "abs").unwrap();
        let all = journal.load_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].text, "abs");
        assert!(all[0].path.is_absolute());
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn case_does_not_split_entries_on_case_insensitive_systems() {
        let dir = tempfile::TempDir::new().unwrap();
        let journal = Journal::new(dir.path().join("j"));
        journal.write(&dir.path().join("Page.md"), "a").unwrap();
        journal.write(&dir.path().join("PAGE.md"), "b").unwrap();
        assert_eq!(journal.load_all().len(), 1);
    }

    #[test]
    fn prune_removes_what_can_never_be_offered() {
        let dir = tempfile::TempDir::new().unwrap();
        let j = dir.path().join("j");
        let journal = Journal::new(&j);
        let live = dir.path().join("Live.md");
        let gone = dir.path().join("Vanished.md");
        std::fs::write(&live, "disk").unwrap();
        journal.write(&live, "live").unwrap();
        // The page file is gone: its entry stays so the "moved or deleted" question is raised.
        journal.write(&gone, "unsaved").unwrap();
        std::fs::write(j.join("junk.json"), b"{not json").unwrap();
        std::fs::write(
            j.join("empty.json"),
            br#"{"version":1,"path":"","text":"t"}"#,
        )
        .unwrap();
        std::fs::write(j.join("v9.json"), br#"{"version":9,"path":"x","text":"t"}"#).unwrap();
        std::fs::write(j.join("note.txt"), b"keep").unwrap();
        std::fs::create_dir_all(j.join("dir.json")).unwrap();
        // An older duplicate under an old-style name.
        old_style(&j, "3333333333333333.json", &live, "older");
        set_age(&j.join("3333333333333333.json"), 3600);
        assert_eq!(journal.prune(), 4);
        let all = journal.load_all();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|e| e.path == live && e.text == "live"));
        assert!(all.iter().any(|e| e.path == gone && e.text == "unsaved"));
        assert!(j.join("note.txt").exists());
        assert!(j.join("dir.json").is_dir());
        // Nothing more to do.
        assert_eq!(journal.prune(), 0);
    }

    #[test]
    fn prune_is_bounded_and_drops_the_oldest_over_the_cap() {
        let dir = tempfile::TempDir::new().unwrap();
        let j = dir.path().join("j");
        std::fs::create_dir_all(&j).unwrap();
        let journal = Journal::new(&j);
        for i in 0..MAX_ENTRIES + 3 {
            let page = dir.path().join(format!("p{i}.md"));
            let name = format!("fnv-{i:016x}.json");
            old_style(&j, &name, &page, "t");
            set_age(&j.join(&name), 100_000 - u64::try_from(i).unwrap_or(0));
        }
        assert_eq!(journal.prune(), 3);
        let all = journal.load_all();
        assert_eq!(all.len(), MAX_ENTRIES);
        // The three oldest (p0, p1, p2) are the ones gone.
        assert!(all.iter().all(|e| !e.path.ends_with("p0.md")));
        assert!(all.iter().any(|e| e.path.ends_with("p3.md")));
    }
}
