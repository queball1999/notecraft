//! The page link index: the `links` and `page_aliases` tables (docs/markdown.md section 8).
//!
//! Storage cannot parse markdown, so the engine finds the links of a page and hands them to
//! [`Index::set_page_links`]. A page needs its links read when its content hash differs from
//! the hash they were built from ([`Index::pages_needing_links`]), so the rows follow the same
//! refresh path as the rest of the index. Resolving a link needs every page in the notebook,
//! so the engine does that as a second step and stores the answer with
//! [`Index::set_resolved_links`].

use rusqlite::params;

use super::Index;

/// Most alias rows kept for one page.
const MAX_ALIASES_PER_PAGE: usize = 64;

/// One link found in a page, before it is stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkRow {
    /// The text between `[[` and the optional `|`, trimmed.
    pub target_text: String,
    /// Byte range of the whole `[[...]]` in the page file.
    pub start_byte: usize,
    pub end_byte: usize,
}

/// Everything the link index keeps for one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageLinks {
    /// The page id (`<notebook id>/<path>`).
    pub id: String,
    pub links: Vec<LinkRow>,
    /// The `aliases` of the page's frontmatter.
    pub aliases: Vec<String>,
}

/// A link as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredLink {
    /// Row id, for [`Index::set_resolved_links`].
    pub row: i64,
    pub source_page: String,
    pub target_text: String,
    /// The page the link points to (the closest one, for an ambiguous link); `None` when no
    /// page matches or the link was not resolved yet.
    pub resolved_page: Option<String>,
    pub start_byte: usize,
    pub end_byte: usize,
}

fn to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

impl Index {
    /// Pages whose links were never read or were read from older content: new pages and
    /// pages whose file changed since the last [`Index::set_page_links`]. Returns `(id, path)`
    /// pairs, the path relative to the notebook folder.
    pub fn pages_needing_links(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, path FROM pages WHERE links_hash IS NULL OR links_hash != hash ORDER BY ord",
        )?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Replace the link rows and aliases of pages and mark them as built from the page's
    /// current content. The resolved page of each new row starts empty. Unknown ids are
    /// ignored. One transaction.
    pub fn set_page_links(&mut self, pages: &[PageLinks]) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        for page in pages {
            let known: i64 = tx.query_row(
                "SELECT count(*) FROM pages WHERE id = ?1",
                [&page.id],
                |row| row.get(0),
            )?;
            if known == 0 {
                continue;
            }
            tx.execute("DELETE FROM links WHERE source_page = ?1", [&page.id])?;
            tx.execute("DELETE FROM page_aliases WHERE page = ?1", [&page.id])?;
            for link in &page.links {
                tx.execute(
                    "INSERT INTO links (source_page, target_text, resolved_page, start_byte, end_byte)
                     VALUES (?1, ?2, NULL, ?3, ?4)",
                    params![
                        page.id,
                        link.target_text,
                        to_i64(link.start_byte),
                        to_i64(link.end_byte)
                    ],
                )?;
            }
            for alias in page.aliases.iter().take(MAX_ALIASES_PER_PAGE) {
                tx.execute(
                    "INSERT INTO page_aliases (page, alias) VALUES (?1, ?2)",
                    params![page.id, alias],
                )?;
            }
            tx.execute(
                "UPDATE pages SET links_hash = hash WHERE id = ?1",
                [&page.id],
            )?;
        }
        tx.commit()
    }

    /// Every page as `(id, path, aliases)`, in sidebar order: what link resolution needs.
    pub fn link_pages(&self) -> rusqlite::Result<Vec<(String, String, Vec<String>)>> {
        let mut aliases: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let mut stmt = self
            .conn
            .prepare_cached("SELECT page, alias FROM page_aliases ORDER BY rowid")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (page, alias) = row?;
            aliases.entry(page).or_default().push(alias);
        }
        let mut stmt = self
            .conn
            .prepare_cached("SELECT id, path FROM pages ORDER BY ord")?;
        let pages = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<(String, String)>>>()?;
        Ok(pages
            .into_iter()
            .map(|(id, path)| {
                let found = aliases.remove(&id).unwrap_or_default();
                (id, path, found)
            })
            .collect())
    }

    /// Every stored link in the index.
    pub fn all_links(&self) -> rusqlite::Result<Vec<StoredLink>> {
        self.links_where("1 = 1", &[])
    }

    /// The links written on one page, in the order they appear.
    pub fn page_links(&self, source_page: &str) -> rusqlite::Result<Vec<StoredLink>> {
        self.links_where("source_page = ?1", &[&source_page])
    }

    /// The links that currently resolve to one page (its backlinks, for the rename step).
    pub fn links_to(&self, target_page: &str) -> rusqlite::Result<Vec<StoredLink>> {
        self.links_where("resolved_page = ?1", &[&target_page])
    }

    fn links_where(
        &self,
        clause: &str,
        args: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<Vec<StoredLink>> {
        let sql = format!(
            "SELECT rowid, source_page, target_text, resolved_page, start_byte, end_byte
             FROM links WHERE {clause} ORDER BY source_page, start_byte"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(args, |row| {
                Ok(StoredLink {
                    row: row.get(0)?,
                    source_page: row.get(1)?,
                    target_text: row.get(2)?,
                    resolved_page: row.get(3)?,
                    start_byte: usize::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
                    end_byte: usize::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Store which page each link resolves to, by row id (`None` for an unresolved link).
    /// A row that no longer exists is ignored. One transaction.
    pub fn set_resolved_links(
        &mut self,
        resolved: &[(i64, Option<String>)],
    ) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        for (row, page) in resolved {
            tx.execute(
                "UPDATE links SET resolved_page = ?2 WHERE rowid = ?1",
                params![row, page],
            )?;
        }
        tx.commit()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    fn notebook(files: &[(&str, &str)]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("nb");
        for (rel, text) in files {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(path, text).expect("write");
        }
        (dir, root)
    }

    fn page_id(index: &Index, path: &str) -> String {
        index
            .link_pages()
            .expect("pages")
            .into_iter()
            .find(|(_, p, _)| p == path)
            .map(|(id, _, _)| id)
            .expect("page")
    }

    fn link(target: &str, start: usize, end: usize) -> LinkRow {
        LinkRow {
            target_text: target.to_owned(),
            start_byte: start,
            end_byte: end,
        }
    }

    fn read_links(index: &mut Index, root: &Path, path: &str, links: Vec<LinkRow>) {
        index.refresh(root).expect("refresh");
        let id = page_id(index, path);
        index
            .set_page_links(&[PageLinks {
                id,
                links,
                aliases: Vec::new(),
            }])
            .expect("links");
    }

    #[test]
    fn pages_need_links_until_they_are_stored() {
        let (_dir, root) = notebook(&[("S/A.md", "[[B]]"), ("S/B.md", "b")]);
        let mut index = Index::open_in_memory().expect("open");
        index.refresh(&root).expect("refresh");
        assert_eq!(index.pages_needing_links().expect("pending").len(), 2);
        let pending = index.pages_needing_links().expect("pending");
        let pages: Vec<PageLinks> = pending
            .iter()
            .map(|(id, _)| PageLinks {
                id: id.clone(),
                links: Vec::new(),
                aliases: Vec::new(),
            })
            .collect();
        index.set_page_links(&pages).expect("links");
        assert!(index.pages_needing_links().expect("pending").is_empty());
        // An untouched refresh keeps them current; an edit marks only that page.
        index.refresh(&root).expect("refresh");
        assert!(index.pages_needing_links().expect("pending").is_empty());
        fs::write(root.join("S/B.md"), "b changed, longer").expect("edit");
        index.refresh(&root).expect("refresh");
        let pending = index.pages_needing_links().expect("pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].1, "S/B.md");
    }

    #[test]
    fn links_are_replaced_per_page_and_resolved_separately() {
        let (_dir, root) = notebook(&[("S/A.md", "[[B]] and [[C]]"), ("S/B.md", "b")]);
        let mut index = Index::open_in_memory().expect("open");
        read_links(
            &mut index,
            &root,
            "S/A.md",
            vec![link("B", 0, 5), link("C", 10, 15)],
        );
        let a = page_id(&index, "S/A.md");
        let b = page_id(&index, "S/B.md");
        let stored = index.page_links(&a).expect("links");
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].target_text, "B");
        assert_eq!((stored[0].start_byte, stored[0].end_byte), (0, 5));
        assert_eq!(stored[0].resolved_page, None);
        index
            .set_resolved_links(&[(stored[0].row, Some(b.clone()))])
            .expect("resolve");
        let stored = index.page_links(&a).expect("links");
        assert_eq!(stored[0].resolved_page.as_deref(), Some(b.as_str()));
        assert_eq!(stored[1].resolved_page, None);
        assert_eq!(index.links_to(&b).expect("to").len(), 1);
        // Storing again replaces the rows.
        index
            .set_page_links(&[PageLinks {
                id: a.clone(),
                links: vec![link("B", 2, 7)],
                aliases: vec!["Alias".into()],
            }])
            .expect("links");
        let stored = index.page_links(&a).expect("links");
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].resolved_page, None);
        let pages = index.link_pages().expect("pages");
        let found = pages.iter().find(|(id, ..)| *id == a).expect("a");
        assert_eq!(found.2, ["Alias"]);
        assert_eq!(index.all_links().expect("all").len(), 1);
    }

    #[test]
    fn removed_pages_drop_their_links_and_aliases() {
        let (_dir, root) = notebook(&[("S/A.md", "[[B]]"), ("S/B.md", "b")]);
        let mut index = Index::open_in_memory().expect("open");
        read_links(&mut index, &root, "S/A.md", vec![link("B", 0, 5)]);
        let a = page_id(&index, "S/A.md");
        index
            .set_page_links(&[PageLinks {
                id: a.clone(),
                links: vec![link("B", 0, 5)],
                aliases: vec!["Alias".into()],
            }])
            .expect("links");
        fs::remove_file(root.join("S/A.md")).expect("rm");
        index.refresh(&root).expect("refresh");
        assert!(index.page_links(&a).expect("links").is_empty());
        assert!(index.all_links().expect("all").is_empty());
        let rows: i64 = index
            .conn
            .query_row("SELECT count(*) FROM page_aliases", [], |r| r.get(0))
            .expect("count");
        assert_eq!(rows, 0);
    }

    #[test]
    fn unknown_pages_are_ignored() {
        let mut index = Index::open_in_memory().expect("open");
        index
            .set_page_links(&[PageLinks {
                id: "nope".into(),
                links: vec![link("x", 0, 5)],
                aliases: Vec::new(),
            }])
            .expect("links");
        assert!(index.all_links().expect("all").is_empty());
        index
            .set_resolved_links(&[(42, Some("x".into()))])
            .expect("resolve");
    }
}
