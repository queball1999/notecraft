//! The `tags` table: where each note tag is in each page (docs/markdown.md sections 4 and 8).
//!
//! Storage cannot parse markdown, so the engine reads pages the index lists in
//! [`Index::pages_needing_tags`], finds their tags, and hands the rows back through
//! [`Index::set_page_tags`]. Queries by scope ([`Index::tagged_items`]) feed the Find Tags
//! pane, the tags summary page, and the command line.

use rusqlite::params;

use super::Index;

/// Where tagged items are looked for. One index belongs to one notebook, so "all notebooks"
/// is the union of the answers from each notebook's index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagScope {
    /// Every page of the notebook.
    Notebook,
    /// The pages of a section, or of a section group and everything in it. The path is the
    /// section or group path from the notebook folder, with `/` separators.
    Folder(String),
    /// One page, by id.
    Page(String),
}

/// One tag found in a page, as the engine hands it to the index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagRow {
    /// The tag name without the `#`, lowercase (`todo` for a task list item).
    pub tag: String,
    /// The source bytes of the `#name` tag (or of the `[ ]` box of a task item).
    pub start_byte: usize,
    pub end_byte: usize,
    /// The source bytes of the paragraph or list item that holds the tag.
    pub holder_start: usize,
    pub holder_end: usize,
    /// The holder's text without markdown syntax or tag names.
    pub text: String,
    /// For a To Do: whether it is checked. `None` for other tags.
    pub checked: Option<bool>,
}

/// One tagged item as a query returns it, in notebook order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaggedRow {
    pub page_id: String,
    pub page_path: String,
    /// Path of the section that holds the page.
    pub section: String,
    pub page_title: String,
    /// Page file modified time, milliseconds since the Unix epoch.
    pub modified: i64,
    pub tag: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub holder_start: usize,
    pub holder_end: usize,
    pub text: String,
    pub checked: Option<bool>,
}

/// Most rows one query returns.
pub const MAX_TAGGED_ROWS: usize = 50_000;

fn to_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn to_usize(n: i64) -> usize {
    usize::try_from(n).unwrap_or(0)
}

impl Index {
    /// Markdown pages whose tags were not read from the current file content: new pages,
    /// changed pages, and every page after the tags table was added. Returns (id, path from
    /// the notebook folder).
    pub fn pages_needing_tags(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, path FROM pages
             WHERE kind != 'canvas' AND (tags_hash IS NULL OR tags_hash != hash)
             ORDER BY ord",
        )?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Replace the tags of a page and record that they match the page's current content.
    /// An unknown page id is ignored.
    pub fn set_page_tags(&mut self, page: &str, rows: &[TagRow]) -> rusqlite::Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM tags WHERE page = ?1", [page])?;
        {
            let mut insert = tx.prepare_cached(
                "INSERT INTO tags (page, tag, start_byte, end_byte, holder_start, holder_end, body, checked)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for row in rows {
                insert.execute(params![
                    page,
                    row.tag,
                    to_i64(row.start_byte),
                    to_i64(row.end_byte),
                    to_i64(row.holder_start),
                    to_i64(row.holder_end),
                    row.text,
                    row.checked.map(i64::from),
                ])?;
            }
        }
        tx.execute("UPDATE pages SET tags_hash = hash WHERE id = ?1", [page])?;
        tx.commit()
    }

    /// The tagged items in `scope`, in notebook order (section, page, position in the page).
    /// `tags` limits the answer to those tag names (lowercase); empty means every tag.
    /// At most [`MAX_TAGGED_ROWS`] rows.
    pub fn tagged_items(
        &self,
        scope: &TagScope,
        tags: &[String],
    ) -> rusqlite::Result<Vec<TaggedRow>> {
        let (filter, value) = match scope {
            TagScope::Notebook => ("1 = 1", String::new()),
            TagScope::Folder(path) => (
                "(p.section = ?1 OR substr(p.section, 1, length(?1) + 1) = ?1 || '/')",
                path.trim_matches('/').to_owned(),
            ),
            TagScope::Page(id) => ("p.id = ?1", id.clone()),
        };
        let sql = format!(
            "SELECT p.id, p.path, p.section,
                    COALESCE(NULLIF(trim(p.fm_title), ''), p.title), p.modified,
                    t.tag, t.start_byte, t.end_byte, t.holder_start, t.holder_end, t.body, t.checked
             FROM tags t
             JOIN pages p ON p.id = t.page
             LEFT JOIN sections s ON s.path = p.section
             WHERE {filter}
             ORDER BY COALESCE(s.ord, 0), p.ord, t.start_byte
             LIMIT {MAX_TAGGED_ROWS}"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let map = |row: &rusqlite::Row<'_>| {
            Ok(TaggedRow {
                page_id: row.get(0)?,
                page_path: row.get(1)?,
                section: row.get(2)?,
                page_title: row.get(3)?,
                modified: row.get(4)?,
                tag: row.get(5)?,
                start_byte: to_usize(row.get(6)?),
                end_byte: to_usize(row.get(7)?),
                holder_start: to_usize(row.get(8)?),
                holder_end: to_usize(row.get(9)?),
                text: row.get(10)?,
                checked: row.get::<_, Option<i64>>(11)?.map(|c| c != 0),
            })
        };
        let rows = if matches!(scope, TagScope::Notebook) {
            stmt.query_map([], map)?
        } else {
            stmt.query_map([value], map)?
        };
        let mut out = Vec::new();
        for row in rows {
            let row = row?;
            if tags.is_empty() || tags.iter().any(|t| t.eq_ignore_ascii_case(&row.tag)) {
                out.push(row);
            }
        }
        Ok(out)
    }

    /// How many times each tag is used in `scope`, most used first, then by name.
    pub fn tag_counts(&self, scope: &TagScope) -> rusqlite::Result<Vec<(String, usize)>> {
        let mut counts: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for row in self.tagged_items(scope, &[])? {
            *counts.entry(row.tag).or_default() += 1;
        }
        let mut out: Vec<(String, usize)> = counts.into_iter().collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(tag: &str, at: usize, text: &str, checked: Option<bool>) -> TagRow {
        TagRow {
            tag: tag.to_owned(),
            start_byte: at,
            end_byte: at + tag.len() + 1,
            holder_start: at,
            holder_end: at + 40,
            text: text.to_owned(),
            checked,
        }
    }

    /// An index with sections `Work` and `Work/Sub` and `Home`, one page each.
    fn index_with_pages() -> Index {
        let index = Index::open_in_memory().expect("open");
        for (path, parent, ord) in [("Work", "", 0), ("Work/Sub", "Work", 1), ("Home", "", 2)] {
            index
                .conn
                .execute(
                    "INSERT INTO sections (path, parent, name, kind, color, ord)
                     VALUES (?1, ?2, ?1, 'section', NULL, ?3)",
                    params![path, parent, ord],
                )
                .expect("section");
        }
        for (n, (section, kind)) in [
            ("Work", "markdown"),
            ("Work/Sub", "markdown"),
            ("Home", "markdown"),
            ("Home", "canvas"),
        ]
        .into_iter()
        .enumerate()
        {
            index
                .conn
                .execute(
                    "INSERT INTO pages (id, path, section, title, kind, level, ord, created, modified, size, hash)
                     VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, 0, 100, 10, 7)",
                    params![
                        format!("nb/p{n}"),
                        format!("{section}/p{n}.md"),
                        section,
                        format!("Page {n}"),
                        kind,
                        n as i64
                    ],
                )
                .expect("page");
        }
        index
    }

    #[test]
    fn markdown_pages_need_tags_until_they_are_recorded() {
        let mut index = index_with_pages();
        let need = index.pages_needing_tags().expect("need");
        assert_eq!(need.len(), 3, "the canvas page is never listed");
        index
            .set_page_tags("nb/p0", &[row("idea", 5, "Think", None)])
            .expect("set");
        assert_eq!(index.pages_needing_tags().expect("need").len(), 2);
        // A changed page (new hash) is listed again.
        index
            .conn
            .execute("UPDATE pages SET hash = 8 WHERE id = 'nb/p0'", [])
            .expect("hash");
        assert_eq!(index.pages_needing_tags().expect("need").len(), 3);
    }

    #[test]
    fn setting_tags_replaces_the_old_rows() {
        let mut index = index_with_pages();
        index
            .set_page_tags(
                "nb/p0",
                &[row("idea", 5, "a", None), row("todo", 60, "b", Some(true))],
            )
            .expect("set");
        index
            .set_page_tags("nb/p0", &[row("question", 9, "c", None)])
            .expect("set again");
        let found = index.tagged_items(&TagScope::Notebook, &[]).expect("q");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].tag, "question");
        assert_eq!(found[0].text, "c");
        assert_eq!(found[0].page_title, "Page 0");
        assert_eq!(found[0].section, "Work");
        assert_eq!((found[0].start_byte, found[0].end_byte), (9, 18));
    }

    #[test]
    fn queries_follow_scope_and_tag_filters() {
        let mut index = index_with_pages();
        index
            .set_page_tags("nb/p0", &[row("idea", 5, "work idea", None)])
            .expect("p0");
        index
            .set_page_tags(
                "nb/p1",
                &[
                    row("todo", 1, "sub task", Some(false)),
                    row("idea", 50, "sub idea", None),
                ],
            )
            .expect("p1");
        index
            .set_page_tags("nb/p2", &[row("idea", 3, "home idea", None)])
            .expect("p2");
        let texts = |scope: &TagScope, tags: &[&str]| -> Vec<String> {
            let tags: Vec<String> = tags.iter().map(|t| (*t).to_owned()).collect();
            index
                .tagged_items(scope, &tags)
                .expect("query")
                .into_iter()
                .map(|r| r.text)
                .collect()
        };
        // Notebook order: section order, then page order, then position.
        assert_eq!(
            texts(&TagScope::Notebook, &[]),
            ["work idea", "sub task", "sub idea", "home idea"]
        );
        assert_eq!(
            texts(&TagScope::Folder("Work".into()), &[]),
            ["work idea", "sub task", "sub idea"],
            "a group takes its sections"
        );
        assert_eq!(
            texts(&TagScope::Folder("Work/Sub".into()), &[]),
            ["sub task", "sub idea"]
        );
        assert_eq!(texts(&TagScope::Folder("Wor".into()), &[]).len(), 0);
        assert_eq!(texts(&TagScope::Page("nb/p2".into()), &[]), ["home idea"]);
        assert_eq!(
            texts(&TagScope::Notebook, &["IDEA"]),
            ["work idea", "sub idea", "home idea"]
        );
        assert_eq!(texts(&TagScope::Notebook, &["todo"]), ["sub task"]);
        let todo = index
            .tagged_items(&TagScope::Notebook, &["todo".to_owned()])
            .expect("todo");
        assert_eq!(todo[0].checked, Some(false));
        assert_eq!(
            index.tag_counts(&TagScope::Notebook).expect("counts"),
            [("idea".to_owned(), 3), ("todo".to_owned(), 1)]
        );
    }

    #[test]
    fn removing_a_page_removes_its_tags_on_refresh() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("nb");
        std::fs::create_dir_all(root.join("S")).expect("mkdir");
        std::fs::write(root.join("S/a.md"), "#idea one\n").expect("a");
        std::fs::write(root.join("S/b.md"), "#idea two\n").expect("b");
        let db = root.join(".notecraft/index.sqlite");
        let (mut index, _) = super::super::refresh_or_rebuild(&root, &db).expect("refresh");
        for (id, _) in index.pages_needing_tags().expect("need") {
            index
                .set_page_tags(&id, &[row("idea", 0, "x", None)])
                .expect("set");
        }
        assert_eq!(
            index
                .tagged_items(&TagScope::Notebook, &[])
                .expect("q")
                .len(),
            2
        );
        assert!(index.pages_needing_tags().expect("need").is_empty());
        std::fs::remove_file(root.join("S/b.md")).expect("rm");
        std::fs::write(root.join("S/a.md"), "#idea changed content\n").expect("edit");
        let (index, _) = super::super::refresh_or_rebuild(&root, &db).expect("refresh");
        assert_eq!(
            index
                .tagged_items(&TagScope::Notebook, &[])
                .expect("q")
                .len(),
            1
        );
        assert_eq!(
            index.pages_needing_tags().expect("need").len(),
            1,
            "the edited page is read again"
        );
    }
}
