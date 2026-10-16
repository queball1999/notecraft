//! The rebuildable notebook index, `.notecraft/index.sqlite` (docs/markdown.md section 8).
//!
//! The index is a cache built from the files. Deleting it is always safe. It is native
//! only: SQLite compiles from C, so this module is not built for `wasm32`.

mod schema;

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::notebook_dir::PageKind;

pub use schema::SCHEMA_VERSION;

/// One section or group row, in sidebar order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionRow {
    /// Path from the notebook folder, with `/` separators.
    pub path: String,
    /// Path of the enclosing group; empty at the top level.
    pub parent: String,
    pub name: String,
    /// `section` or `group`.
    pub kind: String,
    /// Tab color as 0xRRGGBB.
    pub color: Option<u32>,
}

/// One page row, in sidebar order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRow {
    /// `<notebook id>/<path>`, the same id the engine uses.
    pub id: String,
    pub path: String,
    /// Path of the section that holds the page.
    pub section: String,
    pub title: String,
    pub kind: PageKind,
    /// 0 for a page, 1 for a subpage, 2 for a sub-subpage.
    pub level: u8,
    /// File modified time, milliseconds since the Unix epoch.
    pub modified: i64,
}

/// Everything the navigation needs, read from the index without touching the notebook.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sidebar {
    pub sections: Vec<SectionRow>,
    pub pages: Vec<PageRow>,
}

/// An open index database.
pub struct Index {
    conn: Connection,
}

impl Index {
    /// Open (creating if needed) the database at `path` and bring it to the current schema.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::init(conn)
    }

    /// A throwaway index in memory.
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> rusqlite::Result<Self> {
        // The index is a cache that can be rebuilt, so trade durability for speed.
        conn.pragma_update(None, "synchronous", "OFF")?;
        schema::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// The schema version recorded in the database.
    pub fn schema_version(&self) -> rusqlite::Result<i64> {
        schema::version(&self.conn)
    }

    /// The sections and pages for the sidebar, in order.
    pub fn sidebar(&self) -> rusqlite::Result<Sidebar> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT path, parent, name, kind, color FROM sections ORDER BY ord",
        )?;
        let sections = stmt
            .query_map([], |row| {
                Ok(SectionRow {
                    path: row.get(0)?,
                    parent: row.get(1)?,
                    name: row.get(2)?,
                    kind: row.get(3)?,
                    color: row.get::<_, Option<i64>>(4)?.and_then(|c| u32::try_from(c).ok()),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, path, section, title, kind, level, modified FROM pages ORDER BY ord",
        )?;
        let pages = stmt
            .query_map([], |row| {
                let kind: String = row.get(4)?;
                let level: i64 = row.get(5)?;
                Ok(PageRow {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    section: row.get(2)?,
                    title: row.get(3)?,
                    kind: if kind == "canvas" {
                        PageKind::Canvas
                    } else {
                        PageKind::Markdown
                    },
                    level: u8::try_from(level).unwrap_or(0),
                    modified: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Sidebar { sections, pages })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables(index: &Index) -> Vec<String> {
        let mut stmt = index
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .expect("prepare");
        stmt.query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .filter_map(Result::ok)
            .collect()
    }

    #[test]
    fn new_database_gets_every_table() {
        let index = Index::open_in_memory().expect("open");
        assert_eq!(index.schema_version().expect("version"), SCHEMA_VERSION);
        let names = tables(&index);
        for want in [
            "attachment_refs",
            "attachments",
            "fts_pages",
            "links",
            "pages",
            "sections",
            "tags",
        ] {
            assert!(names.iter().any(|n| n == want), "missing table {want}");
        }
        assert_eq!(index.sidebar().expect("sidebar"), Sidebar::default());
    }

    #[test]
    fn fts_table_accepts_and_matches_text() {
        let index = Index::open_in_memory().expect("open");
        index
            .conn
            .execute(
                "INSERT INTO fts_pages (page, title, body) VALUES ('a', 'Trip', 'pack the tent')",
                [],
            )
            .expect("insert");
        let hits: i64 = index
            .conn
            .query_row(
                "SELECT count(*) FROM fts_pages WHERE fts_pages MATCH 'tent'",
                [],
                |row| row.get(0),
            )
            .expect("match");
        assert_eq!(hits, 1);
    }

    #[test]
    fn reopening_keeps_version_and_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.sqlite");
        {
            let index = Index::open(&path).expect("open");
            index
                .conn
                .execute(
                    "INSERT INTO sections (path, parent, name, kind, color, ord)
                     VALUES ('Work', '', 'Work', 'section', 255, 0)",
                    [],
                )
                .expect("insert");
        }
        let index = Index::open(&path).expect("reopen");
        assert_eq!(index.schema_version().expect("version"), SCHEMA_VERSION);
        let sidebar = index.sidebar().expect("sidebar");
        assert_eq!(sidebar.sections.len(), 1);
        assert_eq!(sidebar.sections[0].color, Some(255));
    }

    #[test]
    fn migrates_an_old_version_forward() {
        // A database at version 0 with no tables is migrated to the current version.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.sqlite");
        Connection::open(&path).expect("create");
        let index = Index::open(&path).expect("open");
        assert_eq!(index.schema_version().expect("version"), SCHEMA_VERSION);
    }

    #[test]
    fn a_newer_schema_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("index.sqlite");
        {
            let conn = Connection::open(&path).expect("create");
            conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .expect("set version");
        }
        assert!(Index::open(&path).is_err());
    }
}
