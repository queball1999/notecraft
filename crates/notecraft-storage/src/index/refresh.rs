//! Incremental refresh: bring the index in line with the folder (docs/markdown.md section 8).
//!
//! The folder is the source of truth. A refresh scans the tree (no page is read), compares
//! each page's modified time and size with its row, and reads and hashes only the pages
//! that differ. A page whose time changed but whose hash did not only gets a new time.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use rusqlite::{params, Transaction};

use super::Index;
use crate::notebook_dir::{
    scan_notebook, NotebookTree, PageEntry, PageKind, ScanError, TreeNode, ATTACHMENTS_DIR_NAME,
};

/// Largest page file that is read for hashing (docs/markdown.md section 9). A bigger file
/// is indexed by size and time only.
const MAX_HASH_BYTES: u64 = 64 * 1024 * 1024;
/// How deep the attachments folder is listed.
const ATTACHMENT_DEPTH: usize = 3;
/// Most attachment rows kept.
const MAX_ATTACHMENTS: usize = 100_000;

/// Why a refresh failed.
#[derive(Debug)]
pub enum IndexError {
    Db(rusqlite::Error),
    Scan(ScanError),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexError::Db(err) => write!(f, "index database: {err}"),
            IndexError::Scan(err) => write!(f, "notebook scan: {err}"),
        }
    }
}

impl std::error::Error for IndexError {}

impl From<rusqlite::Error> for IndexError {
    fn from(err: rusqlite::Error) -> Self {
        IndexError::Db(err)
    }
}

impl From<ScanError> for IndexError {
    fn from(err: ScanError) -> Self {
        IndexError::Scan(err)
    }
}

/// What one refresh did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefreshStats {
    /// Ids of pages seen for the first time.
    pub added: Vec<String>,
    /// Ids of pages whose content hash changed. Phase 2 reparses links, tags, and text for
    /// these and for `added`.
    pub changed: Vec<String>,
    /// Ids of pages no longer in the folder.
    pub removed: Vec<String>,
    /// Pages whose rows were kept as they were.
    pub unchanged: usize,
    /// Pages read from disk to compute a hash.
    pub read: usize,
}

struct Known {
    id: String,
    modified: i64,
    size: i64,
    hash: i64,
}

/// FNV-1a over the bytes, stored in SQLite as the same 64 bits read as a signed integer.
pub(super) fn content_hash(bytes: &[u8]) -> i64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    i64::from_ne_bytes(hash.to_ne_bytes())
}

fn millis(time: std::io::Result<std::time::SystemTime>) -> i64 {
    time.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

fn size_i64(size: u64) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

fn read_hash(path: &Path, size: u64) -> Option<i64> {
    if size > MAX_HASH_BYTES {
        return Some(0);
    }
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_HASH_BYTES).read_to_end(&mut bytes).ok()?;
    Some(content_hash(&bytes))
}

fn mime_for(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("txt") => "text/plain",
        Some("md") => "text/markdown",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("mp4") => "video/mp4",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

impl Index {
    /// Bring the index in line with the notebook folder at `root`.
    ///
    /// The whole update is one transaction, so a failure leaves the index as it was.
    pub fn refresh(&mut self, root: &Path) -> Result<RefreshStats, IndexError> {
        let tree = scan_notebook(root)?;
        let mut known = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT path, id, modified, size, hash FROM pages")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    Known {
                        id: row.get(1)?,
                        modified: row.get(2)?,
                        size: row.get(3)?,
                        hash: row.get(4)?,
                    },
                ))
            })?;
            for (path, entry) in rows.flatten() {
                known.insert(path, entry);
            }
        }
        let tx = self.conn.transaction()?;
        let stats = apply(&tx, &tree, root, known)?;
        refresh_attachments(&tx, root)?;
        tx.commit()?;
        Ok(stats)
    }
}

struct Walk<'a> {
    tx: &'a Transaction<'a>,
    tree: &'a NotebookTree,
    root: &'a Path,
    known: HashMap<String, Known>,
    stats: RefreshStats,
    section_ord: i64,
    page_ord: i64,
}

fn apply(
    tx: &Transaction<'_>,
    tree: &NotebookTree,
    root: &Path,
    known: HashMap<String, Known>,
) -> Result<RefreshStats, rusqlite::Error> {
    tx.execute("DELETE FROM sections", [])?;
    let mut walk = Walk {
        tx,
        tree,
        root,
        known,
        stats: RefreshStats::default(),
        section_ord: 0,
        page_ord: 0,
    };
    walk.nodes(&tree.nodes, "")?;
    // Whatever is left in `known` is gone from the folder.
    let gone: Vec<(String, Known)> = walk.known.drain().collect();
    for (path, old) in gone {
        tx.execute("DELETE FROM pages WHERE path = ?1", [&path])?;
        tx.execute("DELETE FROM links WHERE source_page = ?1", [&old.id])?;
        tx.execute("DELETE FROM tags WHERE page = ?1", [&old.id])?;
        tx.execute("DELETE FROM attachment_refs WHERE page = ?1", [&old.id])?;
        tx.execute("DELETE FROM fts_pages WHERE page = ?1", [&old.id])?;
        walk.stats.removed.push(old.id);
    }
    walk.stats.removed.sort();
    Ok(walk.stats)
}

impl Walk<'_> {
    fn nodes(&mut self, nodes: &[TreeNode], parent: &str) -> Result<(), rusqlite::Error> {
        for node in nodes {
            match node {
                TreeNode::Section(section) => {
                    self.insert_section(
                        &section.rel_path,
                        parent,
                        &section.name,
                        "section",
                        section.color,
                    )?;
                    for page in &section.pages {
                        self.page(&section.rel_path, page)?;
                    }
                }
                TreeNode::Group(group) => {
                    self.insert_section(
                        &group.rel_path,
                        parent,
                        &group.name,
                        "group",
                        group.color,
                    )?;
                    self.nodes(&group.children, &group.rel_path)?;
                }
            }
        }
        Ok(())
    }

    fn insert_section(
        &mut self,
        path: &str,
        parent: &str,
        name: &str,
        kind: &str,
        color: Option<u32>,
    ) -> Result<(), rusqlite::Error> {
        self.tx.execute(
            "INSERT OR REPLACE INTO sections (path, parent, name, kind, color, ord)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                path,
                parent,
                name,
                kind,
                color.map(i64::from),
                self.section_ord
            ],
        )?;
        self.section_ord += 1;
        Ok(())
    }

    fn page(&mut self, section: &str, page: &PageEntry) -> Result<(), rusqlite::Error> {
        let id = format!("{}/{}", self.tree.id, page.rel_path);
        let kind = match page.kind {
            PageKind::Markdown => "markdown",
            PageKind::Canvas => "canvas",
        };
        let ord = self.page_ord;
        self.page_ord += 1;
        let level = i64::from(page.level);
        let full = self.root.join(&page.rel_path);
        let meta = fs::metadata(&full).ok();
        let (modified, created, size) = match &meta {
            Some(m) => {
                let modified = millis(m.modified());
                let created = match millis(m.created()) {
                    0 => modified,
                    c => c,
                };
                (modified, created, size_i64(m.len()))
            }
            None => (0, 0, 0),
        };
        let old = self.known.remove(&page.rel_path);
        if let Some(old) = &old {
            if old.modified == modified && old.size == size && old.id == id {
                self.tx.execute(
                    "UPDATE pages SET section = ?1, title = ?2, kind = ?3, level = ?4, ord = ?5
                     WHERE path = ?6",
                    params![section, page.title, kind, level, ord, page.rel_path],
                )?;
                self.stats.unchanged += 1;
                return Ok(());
            }
        }
        let hash = if meta.is_some() {
            self.stats.read += 1;
            read_hash(&full, u64::try_from(size).unwrap_or(0)).unwrap_or(0)
        } else {
            0
        };
        let same_content = old
            .as_ref()
            .is_some_and(|o| o.hash == hash && o.size == size && o.id == id);
        self.tx.execute(
            "INSERT INTO pages (id, path, section, title, kind, level, ord, created, modified, size, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(path) DO UPDATE SET id = ?1, section = ?3, title = ?4, kind = ?5,
                level = ?6, ord = ?7, modified = ?9, size = ?10, hash = ?11,
                fm_title = CASE WHEN ?12 THEN fm_title ELSE NULL END",
            params![
                id,
                page.rel_path,
                section,
                page.title,
                kind,
                level,
                ord,
                created,
                modified,
                size,
                hash,
                same_content
            ],
        )?;
        match old {
            None => self.stats.added.push(id),
            Some(_) if same_content => self.stats.unchanged += 1,
            Some(old) if old.id != id => {
                self.stats.removed.push(old.id);
                self.stats.added.push(id);
            }
            Some(_) => self.stats.changed.push(id),
        }
        Ok(())
    }
}

fn refresh_attachments(tx: &Transaction<'_>, root: &Path) -> Result<(), rusqlite::Error> {
    tx.execute("DELETE FROM attachments", [])?;
    let mut count = 0;
    let mut stack = vec![(
        root.join(ATTACHMENTS_DIR_NAME),
        ATTACHMENTS_DIR_NAME.to_owned(),
        0,
    )];
    while let Some((dir, rel, depth)) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let child = format!("{rel}/{name}");
            if meta.is_dir() {
                if depth + 1 < ATTACHMENT_DEPTH {
                    stack.push((entry.path(), child, depth + 1));
                }
            } else if meta.is_file() && count < MAX_ATTACHMENTS {
                count += 1;
                tx.execute(
                    "INSERT OR REPLACE INTO attachments (path, size, mime, hash)
                     VALUES (?1, ?2, ?3, NULL)",
                    params![child, size_i64(meta.len()), mime_for(&name)],
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn notebook() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("nb");
        fs::create_dir_all(root.join("Work")).expect("mkdir");
        fs::create_dir_all(root.join("Home")).expect("mkdir");
        fs::write(root.join("Work/Plan.md"), "# Plan\n").expect("write");
        fs::write(root.join("Work/Notes.md"), "notes\n").expect("write");
        fs::write(root.join("Home/List.md"), "milk\n").expect("write");
        (dir, root)
    }

    #[test]
    fn first_refresh_adds_everything() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        let stats = index.refresh(&root).expect("refresh");
        assert_eq!(stats.added.len(), 3);
        assert_eq!(stats.read, 3);
        let sidebar = index.sidebar().expect("sidebar");
        assert_eq!(sidebar.sections.len(), 2);
        assert_eq!(sidebar.pages.len(), 3);
        assert!(sidebar.pages.iter().all(|p| p.id.ends_with(&p.path)));
    }

    #[test]
    fn frontmatter_title_shows_until_the_page_changes() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        index.refresh(&root).expect("refresh");
        let id = index.sidebar().expect("sidebar").pages[0].id.clone();
        index
            .set_frontmatter_titles(&[(id.clone(), Some("Shown".to_owned()))])
            .expect("titles");
        let title_of = |index: &Index| {
            let sidebar = index.sidebar().expect("sidebar");
            sidebar
                .pages
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.title.clone())
        };
        assert_eq!(title_of(&index).as_deref(), Some("Shown"));
        // An untouched refresh keeps it.
        index.refresh(&root).expect("refresh");
        assert_eq!(title_of(&index).as_deref(), Some("Shown"));
        // A changed file drops it until the engine reads the frontmatter again.
        let path = root.join(id.split_once('/').map(|(_, p)| p).unwrap_or(""));
        fs::write(&path, "changed content, longer\n").expect("edit");
        index.refresh(&root).expect("refresh");
        let title = title_of(&index).expect("page");
        assert_ne!(title, "Shown");
    }

    #[test]
    fn second_refresh_reads_nothing() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        index.refresh(&root).expect("refresh");
        let stats = index.refresh(&root).expect("refresh");
        assert_eq!(stats.unchanged, 3);
        assert_eq!(stats.read, 0);
        assert!(stats.added.is_empty() && stats.changed.is_empty() && stats.removed.is_empty());
    }

    #[test]
    fn edit_add_and_remove_are_picked_up() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        index.refresh(&root).expect("refresh");
        fs::write(root.join("Work/Plan.md"), "# Plan, longer now\n").expect("edit");
        fs::write(root.join("Home/New.md"), "x").expect("add");
        fs::remove_file(root.join("Work/Notes.md")).expect("remove");
        let stats = index.refresh(&root).expect("refresh");
        assert_eq!(stats.changed.len(), 1);
        assert!(stats.changed[0].ends_with("Work/Plan.md"));
        assert_eq!(stats.added.len(), 1);
        assert_eq!(stats.removed.len(), 1);
        assert!(stats.removed[0].ends_with("Work/Notes.md"));
        assert_eq!(index.sidebar().expect("sidebar").pages.len(), 3);
    }

    #[test]
    fn a_touched_file_with_the_same_content_is_not_changed() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        index.refresh(&root).expect("refresh");
        // Rewrite the same bytes; the modified time may move but the hash does not.
        fs::write(root.join("Work/Plan.md"), "# Plan\n").expect("rewrite");
        let stats = index.refresh(&root).expect("refresh");
        assert!(stats.changed.is_empty());
    }

    #[test]
    fn index_matches_a_fresh_scan_after_changes() {
        let (_dir, root) = notebook();
        let mut incremental = Index::open_in_memory().expect("open");
        incremental.refresh(&root).expect("refresh");
        fs::write(root.join("Home/Extra.md"), "e").expect("add");
        fs::remove_file(root.join("Home/List.md")).expect("remove");
        incremental.refresh(&root).expect("refresh");
        let mut fresh = Index::open_in_memory().expect("open");
        fresh.refresh(&root).expect("refresh");
        assert_eq!(
            incremental.sidebar().expect("a"),
            fresh.sidebar().expect("b")
        );
    }

    #[test]
    fn attachments_are_listed() {
        let (_dir, root) = notebook();
        fs::create_dir_all(root.join("attachments")).expect("mkdir");
        fs::write(root.join("attachments/ab12-photo.png"), [1, 2, 3]).expect("write");
        let mut index = Index::open_in_memory().expect("open");
        index.refresh(&root).expect("refresh");
        let (mime, size): (String, i64) = index
            .conn
            .query_row(
                "SELECT mime, size FROM attachments WHERE path = 'attachments/ab12-photo.png'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("row");
        assert_eq!(mime, "image/png");
        assert_eq!(size, 3);
    }

    #[test]
    fn hash_is_stable_and_content_sensitive() {
        assert_eq!(content_hash(b"abc"), content_hash(b"abc"));
        assert_ne!(content_hash(b"abc"), content_hash(b"abd"));
    }
}
