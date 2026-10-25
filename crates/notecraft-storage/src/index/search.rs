//! Full-text search over page text (docs/markdown.md section 8).
//!
//! Storage cannot parse markdown, so the engine strips a page to plain text and hands it to
//! [`Index::set_page_texts`]. A page needs text when its content hash differs from the hash
//! its text was built from ([`Index::pages_needing_text`]), so the text follows the same
//! refresh path as the rest of the index. Titles weigh ten times a body word in the ranking.

use rusqlite::params;

use super::Index;

/// Marks the start of a matched word in a [`TextHit::snippet`].
pub const MARK_START: char = '\u{2}';
/// Marks the end of a matched word in a [`TextHit::snippet`].
pub const MARK_END: char = '\u{3}';

/// Most words taken from a search text.
const MAX_TERMS: usize = 16;
/// Longest word kept from a search text, in characters.
const MAX_TERM_CHARS: usize = 64;
/// Most hits one search returns.
pub const MAX_HITS: usize = 500;
/// Words of context in a snippet.
const SNIPPET_WORDS: i64 = 20;
/// Weight of a title match against a body match.
const TITLE_WEIGHT: f64 = 10.0;

/// The plain text of one page, ready for the full-text table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageBody {
    /// The page id (`<notebook id>/<path>`).
    pub id: String,
    pub title: String,
    /// The page text without markdown syntax.
    pub body: String,
}

/// Which pages a search looks at, within one notebook's index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextScope<'a> {
    /// Every page in the index.
    All,
    /// One page, by id.
    Page(&'a str),
    /// The pages of one section, by section path.
    Section(&'a str),
    /// The pages of every section under a group path (and the group itself).
    Group(&'a str),
}

/// One page that matched.
#[derive(Clone, Debug, PartialEq)]
pub struct TextHit {
    pub id: String,
    pub path: String,
    /// Path of the section that holds the page.
    pub section: String,
    /// The title to show (the frontmatter title, else the file name).
    pub title: String,
    /// File modified time, milliseconds since the Unix epoch.
    pub modified: i64,
    /// Relevance score: lower is better (SQLite `bm25`, with titles weighted higher).
    pub rank: f64,
    /// A piece of the text around the match, with [`MARK_START`] and [`MARK_END`] around
    /// each matched word.
    pub snippet: String,
}

/// The words of a search text as an FTS5 match expression, or `None` when it has no words.
///
/// Words are letters and digits; everything else separates them. Every word matches as a
/// prefix, so a search works while it is being typed. Text inside double quotes is a phrase
/// whose last word also matches as a prefix. All parts must match.
pub fn match_expression(text: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for (at, segment) in text.split('"').enumerate() {
        let words = words_of(segment);
        if words.is_empty() {
            continue;
        }
        // Odd segments sit between a pair of quotes. An unclosed quote still counts.
        if at % 2 == 1 {
            parts.push(format!("\"{}\"*", words.join(" ")));
        } else {
            parts.extend(words.into_iter().map(|w| format!("\"{w}\"*")));
        }
        if parts.len() >= MAX_TERMS {
            parts.truncate(MAX_TERMS);
            break;
        }
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn words_of(segment: &str) -> Vec<String> {
    segment
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.chars().take(MAX_TERM_CHARS).collect())
        .collect()
}

/// Escape `%`, `_`, and `\` for a `LIKE ... ESCAPE '\'` pattern.
pub(super) fn like_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

impl Index {
    /// Pages whose text is missing or was built from older content: new pages and pages
    /// whose file changed since the last [`Index::set_page_texts`]. Returns `(id, path, title)`
    /// triples: path relative to the notebook folder, and the title to show.
    pub fn pages_needing_text(&self) -> rusqlite::Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, path, COALESCE(NULLIF(trim(fm_title), ''), title) FROM pages WHERE text_hash IS NULL OR text_hash != hash ORDER BY ord",
        )?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Replace the full-text rows of pages and mark their text as current. Unknown ids are
    /// ignored. One transaction.
    pub fn set_page_texts(&mut self, pages: &[PageBody]) -> rusqlite::Result<()> {
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
            tx.execute("DELETE FROM fts_pages WHERE page = ?1", [&page.id])?;
            tx.execute(
                "INSERT INTO fts_pages (page, title, body) VALUES (?1, ?2, ?3)",
                params![page.id, page.title, page.body],
            )?;
            tx.execute(
                "UPDATE pages SET text_hash = hash WHERE id = ?1",
                [&page.id],
            )?;
        }
        tx.commit()
    }

    /// Search page titles and text. Best matches first (titles weigh more than body text),
    /// at most `limit` (capped at [`MAX_HITS`]). An empty or wordless `text` finds nothing.
    pub fn search_text(
        &self,
        text: &str,
        scope: TextScope<'_>,
        limit: usize,
    ) -> rusqlite::Result<Vec<TextHit>> {
        let Some(expression) = match_expression(text) else {
            return Ok(Vec::new());
        };
        let (kind, value, pattern): (i64, &str, String) = match scope {
            TextScope::All => (0, "", String::new()),
            TextScope::Page(id) => (1, id, String::new()),
            TextScope::Section(path) => (2, path, String::new()),
            TextScope::Group(path) => (3, path, format!("{}/%", like_escape(path))),
        };
        let limit = i64::try_from(limit.min(MAX_HITS)).unwrap_or(0);
        let mut stmt = self.conn.prepare_cached(
            "SELECT p.id, p.path, p.section,
                    COALESCE(NULLIF(trim(p.fm_title), ''), p.title), p.modified,
                    bm25(fts_pages, 0.0, ?7, 1.0) AS rank,
                    snippet(fts_pages, 2, ?5, ?6, '...', ?8)
             FROM fts_pages JOIN pages p ON p.id = fts_pages.page
             WHERE fts_pages MATCH ?1
               AND (?2 = 0
                    OR (?2 = 1 AND p.id = ?3)
                    OR (?2 = 2 AND p.section = ?3)
                    OR (?2 = 3 AND (p.section = ?3 OR p.section LIKE ?4 ESCAPE '\\')))
             ORDER BY rank, p.id
             LIMIT ?9",
        )?;
        let rows = stmt
            .query_map(
                params![
                    expression,
                    kind,
                    value,
                    pattern,
                    MARK_START.to_string(),
                    MARK_END.to_string(),
                    TITLE_WEIGHT,
                    SNIPPET_WORDS,
                    limit
                ],
                |row| {
                    Ok(TextHit {
                        id: row.get(0)?,
                        path: row.get(1)?,
                        section: row.get(2)?,
                        title: row.get(3)?,
                        modified: row.get(4)?,
                        rank: row.get(5)?,
                        snippet: row.get(6)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn expression_quotes_words_and_keeps_phrases() {
        assert_eq!(match_expression("tent"), Some("\"tent\"*".to_owned()));
        assert_eq!(
            match_expression("pack  the-tent!"),
            Some("\"pack\"* \"the\"* \"tent\"*".to_owned())
        );
        assert_eq!(
            match_expression("a \"red tent\" b"),
            Some("\"a\"* \"red tent\"* \"b\"*".to_owned())
        );
        assert_eq!(match_expression("   !!! "), None);
        assert_eq!(match_expression(""), None);
        // FTS operators and a lone quote are plain separators.
        assert_eq!(
            match_expression("NOT x OR \"y"),
            Some("\"NOT\"* \"x\"* \"OR\"* \"y\"*".to_owned())
        );
    }

    #[test]
    fn expression_is_bounded() {
        let many = (0..100)
            .map(|i| format!("w{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let expr = match_expression(&many).expect("expr");
        assert_eq!(expr.matches('*').count(), MAX_TERMS);
        let long = "x".repeat(1000);
        let expr = match_expression(&long).expect("expr");
        assert!(expr.len() < 100);
    }

    fn notebook() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("nb");
        for (rel, text) in [
            ("Grp/Work/Plan.md", "plan the launch"),
            ("Grp/Work/Notes.md", "meeting notes about the tent budget"),
            ("Grp/Deep/Retro.md", "retro notes"),
            ("Home/Trip.md", "pack the tent and the stove"),
        ] {
            let path = root.join(rel);
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(path, text).expect("write");
        }
        (dir, root)
    }

    fn load(index: &mut Index, root: &Path) {
        index.refresh(root).expect("refresh");
        let pending = index.pages_needing_text().expect("pending");
        let texts: Vec<PageBody> = pending
            .iter()
            .map(|(id, path, _)| {
                let body = fs::read_to_string(root.join(path)).expect("read");
                let title = Path::new(path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                PageBody {
                    id: id.clone(),
                    title,
                    body,
                }
            })
            .collect();
        index.set_page_texts(&texts).expect("texts");
    }

    fn paths(hits: &[TextHit]) -> Vec<String> {
        hits.iter().map(|h| h.path.clone()).collect()
    }

    #[test]
    fn pages_need_text_until_it_is_stored() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        index.refresh(&root).expect("refresh");
        assert_eq!(index.pages_needing_text().expect("pending").len(), 4);
        load(&mut index, &root);
        assert!(index.pages_needing_text().expect("pending").is_empty());
        // An untouched refresh keeps the text current.
        index.refresh(&root).expect("refresh");
        assert!(index.pages_needing_text().expect("pending").is_empty());
        // An edit makes only that page need text again.
        fs::write(root.join("Home/Trip.md"), "pack the kayak, longer").expect("edit");
        index.refresh(&root).expect("refresh");
        let pending = index.pages_needing_text().expect("pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].1, "Home/Trip.md");
        load(&mut index, &root);
        let hits = index
            .search_text("kayak", TextScope::All, 10)
            .expect("search");
        assert_eq!(paths(&hits), ["Home/Trip.md"]);
        assert!(index
            .search_text("stove", TextScope::All, 10)
            .expect("search")
            .is_empty());
    }

    #[test]
    fn a_removed_page_leaves_the_results() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        load(&mut index, &root);
        assert_eq!(
            index
                .search_text("stove", TextScope::All, 10)
                .expect("s")
                .len(),
            1
        );
        fs::remove_file(root.join("Home/Trip.md")).expect("rm");
        index.refresh(&root).expect("refresh");
        assert!(index
            .search_text("stove", TextScope::All, 10)
            .expect("s")
            .is_empty());
    }

    #[test]
    fn matches_prefixes_and_marks_the_words() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        load(&mut index, &root);
        let hits = index
            .search_text("ten", TextScope::All, 10)
            .expect("search");
        assert_eq!(hits.len(), 2);
        assert!(hits
            .iter()
            .all(|h| h.snippet.contains(&format!("{MARK_START}tent{MARK_END}"))));
    }

    #[test]
    fn titles_rank_above_body_words() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        index.refresh(&root).expect("refresh");
        let pages = [
            (
                "Other",
                "the tent tent tent tent tent is in the body of this page",
            ),
            ("Tent", "short"),
        ];
        let rows: Vec<PageBody> = index
            .pages_needing_text()
            .expect("pending")
            .into_iter()
            .zip(pages)
            .map(|((id, _, _), (title, body))| PageBody {
                id,
                title: title.to_owned(),
                body: body.to_owned(),
            })
            .collect();
        index.set_page_texts(&rows).expect("texts");
        let hits = index
            .search_text("tent", TextScope::All, 10)
            .expect("search");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, rows[1].id);
        assert!(hits[0].rank <= hits[1].rank);
    }

    #[test]
    fn scopes_limit_the_pages() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        load(&mut index, &root);
        let all = index.search_text("notes", TextScope::All, 10).expect("s");
        assert_eq!(all.len(), 2);
        let section = index
            .search_text("notes", TextScope::Section("Grp/Work"), 10)
            .expect("s");
        assert_eq!(paths(&section), ["Grp/Work/Notes.md"]);
        let group = index
            .search_text("notes", TextScope::Group("Grp"), 10)
            .expect("s");
        assert_eq!(group.len(), 2);
        let other = index
            .search_text("notes", TextScope::Group("Home"), 10)
            .expect("s");
        assert!(other.is_empty());
        // A group path is a whole folder name, not a prefix.
        let prefix = index
            .search_text("notes", TextScope::Group("Gr"), 10)
            .expect("s");
        assert!(prefix.is_empty());
        let page = index
            .search_text("notes", TextScope::Page(&all[0].id), 10)
            .expect("s");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, all[0].id);
    }

    #[test]
    fn like_characters_in_a_group_path_are_literal() {
        assert_eq!(like_escape("a_b%c\\d"), "a\\_b\\%c\\\\d");
    }

    #[test]
    fn a_wordless_search_and_the_limit() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        load(&mut index, &root);
        assert!(index
            .search_text("  ", TextScope::All, 10)
            .expect("s")
            .is_empty());
        assert_eq!(
            index
                .search_text("the", TextScope::All, 1)
                .expect("s")
                .len(),
            1
        );
    }

    #[test]
    fn a_frontmatter_title_is_what_the_hit_shows() {
        let (_dir, root) = notebook();
        let mut index = Index::open_in_memory().expect("open");
        load(&mut index, &root);
        let id = index.search_text("launch", TextScope::All, 5).expect("s")[0]
            .id
            .clone();
        index
            .set_frontmatter_titles(&[(id, Some("Big Launch".to_owned()))])
            .expect("titles");
        let hits = index.search_text("launch", TextScope::All, 5).expect("s");
        assert_eq!(hits[0].title, "Big Launch");
    }
}
