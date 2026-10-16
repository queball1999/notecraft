//! The index schema and its migrations (docs/markdown.md section 8).
//!
//! The schema version is the SQLite `user_version` pragma. Each entry of [`MIGRATIONS`]
//! takes the database from version `n` to `n + 1`; a new schema adds an entry and never
//! edits an old one. The index is a cache, so a database from a newer NoteCraft is not
//! migrated down: the caller deletes and rebuilds it.

use rusqlite::Connection;

/// Version 1: every table of the section 8 schema. Phase 1 fills `sections`, `pages`, and
/// `attachments`; Phase 2 fills `links`, `tags`, and `fts_pages`.
const V1: &str = "
CREATE TABLE sections (
    path    TEXT PRIMARY KEY NOT NULL,
    parent  TEXT NOT NULL,
    name    TEXT NOT NULL,
    kind    TEXT NOT NULL,
    color   INTEGER,
    ord     INTEGER NOT NULL
);
CREATE TABLE pages (
    id       TEXT PRIMARY KEY NOT NULL,
    path     TEXT NOT NULL UNIQUE,
    section  TEXT NOT NULL,
    title    TEXT NOT NULL,
    kind     TEXT NOT NULL,
    level    INTEGER NOT NULL,
    ord      INTEGER NOT NULL,
    created  INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    size     INTEGER NOT NULL,
    hash     INTEGER NOT NULL
);
CREATE INDEX pages_by_section ON pages (section, ord);
CREATE TABLE links (
    source_page   TEXT NOT NULL,
    target_text   TEXT NOT NULL,
    resolved_page TEXT,
    start_byte    INTEGER NOT NULL,
    end_byte      INTEGER NOT NULL
);
CREATE INDEX links_by_source ON links (source_page);
CREATE TABLE tags (
    page       TEXT NOT NULL,
    tag        TEXT NOT NULL,
    start_byte INTEGER NOT NULL,
    end_byte   INTEGER NOT NULL
);
CREATE INDEX tags_by_page ON tags (page);
CREATE TABLE attachments (
    path  TEXT PRIMARY KEY NOT NULL,
    size  INTEGER NOT NULL,
    mime  TEXT NOT NULL,
    hash  INTEGER
);
CREATE TABLE attachment_refs (
    attachment TEXT NOT NULL,
    page       TEXT NOT NULL,
    PRIMARY KEY (attachment, page)
);
CREATE VIRTUAL TABLE fts_pages USING fts5(page UNINDEXED, title, body);
";

/// Migrations in order. Index `n` upgrades version `n` to `n + 1`.
pub(super) const MIGRATIONS: &[&str] = &[V1];

/// The schema version this build writes.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// The version recorded in the database (0 for a new one).
pub(super) fn version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

/// Run every migration the database has not seen yet, each in its own transaction.
/// A database from a newer schema is an error, because it cannot be read safely.
pub(super) fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let mut current = version(conn)?;
    if current > SCHEMA_VERSION {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_SCHEMA),
            Some(format!(
                "index schema {current} is newer than {SCHEMA_VERSION}"
            )),
        ));
    }
    while current < SCHEMA_VERSION {
        let Some(sql) = usize::try_from(current)
            .ok()
            .and_then(|n| MIGRATIONS.get(n))
        else {
            break;
        };
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        current += 1;
        tx.pragma_update(None, "user_version", current)?;
        tx.commit()?;
    }
    Ok(())
}
