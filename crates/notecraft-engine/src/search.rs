//! Search across notebooks (build_plan/04 PR 2.4): scopes, ranking, and sorting.
//!
//! The search reads each notebook's `.notecraft/index.sqlite` (see `notecraft-storage`), so it
//! never touches the page files. It takes plain inputs (folders, the current selection, a
//! query), so the command line search (PR 2.11) calls the same [`search`] the app does.

use std::cmp::Ordering;
use std::path::PathBuf;

use crate::session::Selection;

/// Where a search looks, as in OneNote's scope list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchScope {
    /// The page shown.
    Page,
    /// The section shown.
    Section,
    /// The section group that holds the section shown (the notebook when it has none).
    Group,
    /// The notebook shown.
    Notebook,
    /// Every open notebook.
    #[default]
    All,
}

impl SearchScope {
    /// Every scope, narrowest first (the order of the scope list).
    pub const LIST: [SearchScope; 5] = [
        Self::Page,
        Self::Section,
        Self::Group,
        Self::Notebook,
        Self::All,
    ];

    /// The id used in command arguments.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Section => "section",
            Self::Group => "group",
            Self::Notebook => "notebook",
            Self::All => "all",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::LIST.into_iter().find(|scope| scope.id() == id)
    }

    /// The text of the scope list.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Page => "This Page",
            Self::Section => "This Section",
            Self::Group => "This Section Group",
            Self::Notebook => "This Notebook",
            Self::All => "All Notebooks",
        }
    }
}

/// How the results are ordered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchSort {
    /// Best match first.
    #[default]
    Relevance,
    /// By notebook, then section, then title.
    Section,
    /// By page title.
    Title,
    /// Most recently modified first.
    Modified,
}

impl SearchSort {
    pub const LIST: [SearchSort; 4] = [Self::Relevance, Self::Section, Self::Title, Self::Modified];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Section => "section",
            Self::Title => "title",
            Self::Modified => "modified",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::LIST.into_iter().find(|sort| sort.id() == id)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Relevance => "Relevance",
            Self::Section => "Section",
            Self::Title => "Title",
            Self::Modified => "Date Modified",
        }
    }
}

/// What a result is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HitKind {
    /// A page whose title or text matched.
    #[default]
    Page,
    /// A file attached to a page whose name or type matched.
    Attachment,
}

/// The file behind an attachment result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentInfo {
    /// Path from the notebook folder.
    pub path: String,
    pub mime: String,
    pub size: u64,
}

/// One result: a page whose title or text matched, or an attachment whose name or type did.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub kind: HitKind,
    /// Set for an attachment.
    pub attachment: Option<AttachmentInfo>,
    pub notebook_id: String,
    pub notebook_name: String,
    /// The section id (`<notebook id>/<folder path>`), as the navigation uses it.
    pub section_id: String,
    /// The section's folder path inside the notebook, such as `Group/Section`.
    pub section_path: String,
    /// The last part of `section_path`.
    pub section_name: String,
    /// The page; for an attachment, the first page that links to it (empty if none does).
    pub page_id: String,
    /// The page title, or the attachment name.
    pub title: String,
    /// File modified time, milliseconds since the Unix epoch.
    pub modified: i64,
    /// Lower is a better match.
    pub rank: f64,
    /// Text around the match, without markers.
    pub snippet: String,
    /// Byte ranges of `snippet` to highlight, in order.
    pub highlights: Vec<(usize, usize)>,
}

/// What a search found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchResults {
    pub query: String,
    pub scope: SearchScope,
    pub sort: SearchSort,
    pub hits: Vec<SearchHit>,
    /// More pages matched than the list holds.
    pub truncated: bool,
}

/// What the search window shows and remembers (the session holds one).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchState {
    /// Whether the search box and results pane are shown.
    pub open: bool,
    /// Goes up each time the box is opened, so the front end can put the caret in it.
    pub open_serial: u64,
    pub query: String,
    pub scope: SearchScope,
    pub sort: SearchSort,
    pub results: Option<SearchResults>,
}

/// An open notebook to search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRoot {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
}

/// Most results a search returns in all.
pub const MAX_RESULTS: usize = 500;

/// The selection narrows the scopes: which notebook, section, and page "this" means.
pub type SearchContext = Selection;

/// The folder path of a section id, given its notebook id.
fn section_path<'a>(notebook: &str, section_id: &'a str) -> Option<&'a str> {
    section_id
        .strip_prefix(notebook)
        .and_then(|rest| rest.strip_prefix('/'))
}

/// Cut a snippet with word markers into plain text and highlight ranges.
pub fn split_marks(marked: &str) -> (String, Vec<(usize, usize)>) {
    let mut text = String::with_capacity(marked.len());
    let mut ranges = Vec::new();
    let mut start = None;
    for c in marked.chars() {
        match c {
            '\u{2}' => start = Some(text.len()),
            '\u{3}' => {
                if let Some(from) = start.take() {
                    if text.len() > from {
                        ranges.push((from, text.len()));
                    }
                }
            }
            c if c.is_control() && c != '\n' && c != '\t' => {}
            '\n' | '\t' => text.push(' '),
            c => text.push(c),
        }
    }
    (text, ranges)
}

fn order(sort: SearchSort, a: &Sortable, b: &Sortable) -> Ordering {
    let by_rank = a.rank.partial_cmp(&b.rank).unwrap_or(Ordering::Equal);
    let by_title = a
        .title
        .to_lowercase()
        .cmp(&b.title.to_lowercase())
        .then_with(|| a.title.cmp(&b.title));
    let by_section = a
        .notebook_at
        .cmp(&b.notebook_at)
        .then_with(|| {
            a.section_path
                .to_lowercase()
                .cmp(&b.section_path.to_lowercase())
        })
        .then(by_title);
    let by_id = a.id.cmp(&b.id);
    match sort {
        SearchSort::Relevance => by_rank.then(by_id),
        SearchSort::Section => by_section.then(by_id),
        SearchSort::Title => by_title.then(by_section).then(by_id),
        SearchSort::Modified => b.modified.cmp(&a.modified).then(by_title).then(by_id),
    }
}

/// The fields a sort reads.
struct Sortable {
    notebook_at: usize,
    section_path: String,
    title: String,
    modified: i64,
    rank: f64,
    id: String,
}

impl SearchHit {
    fn sortable(&self, notebook_at: usize) -> Sortable {
        Sortable {
            notebook_at,
            section_path: self.section_path.clone(),
            title: self.title.clone(),
            modified: self.modified,
            rank: self.rank,
            id: self.page_id.clone(),
        }
    }
}

/// A search text split into what pages are matched on and what attachments are matched on.
///
/// `type:pdf` (or `type:image`, `type:png`, a mime type) limits the search to attachments of
/// that type. The other words match page titles and text, and attachment names.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedQuery {
    /// The query without `type:` words.
    text: String,
    /// Lowercase words an attachment name must contain.
    words: Vec<String>,
    /// Lowercase types, all of which an attachment must match.
    types: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ParsedQuery {
    fn new(query: &str) -> Self {
        let mut text = Vec::new();
        let mut types = Vec::new();
        for token in query.split_whitespace() {
            let lower = token.to_lowercase();
            match lower.strip_prefix("type:") {
                Some(kind) => {
                    let kind = kind.trim_start_matches('.');
                    if !kind.is_empty() && types.len() < 4 {
                        types.push(kind.to_owned());
                    }
                }
                None => text.push(token),
            }
        }
        let text = text.join(" ");
        let words = text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .take(16)
            .map(str::to_owned)
            .collect();
        Self { text, words, types }
    }

    /// Whether an attachment named `name` with mime type `mime` matches.
    fn matches_attachment(&self, name: &str, mime: &str) -> bool {
        if self.words.is_empty() && self.types.is_empty() {
            return false;
        }
        let lower = name.to_lowercase();
        let ext = lower.rsplit_once('.').map_or("", |(_, ext)| ext);
        let mime = mime.to_lowercase();
        self.words.iter().all(|w| lower.contains(w.as_str()))
            && self.types.iter().all(|t| {
                ext == t
                    || mime == *t
                    || mime
                        .split_once('/')
                        .is_some_and(|(major, _)| major == t || mime.ends_with(&format!("/{t}")))
            })
    }
}

/// "2 KB" style size.
#[cfg(not(target_arch = "wasm32"))]
fn size_text(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} bytes")
    }
}

/// The page and attachment hits of one notebook index.
#[cfg(not(target_arch = "wasm32"))]
fn notebook_hits(
    root: &SearchRoot,
    index: &notecraft_storage::index::Index,
    scope: notecraft_storage::index::TextScope<'_>,
    query: &ParsedQuery,
    out: &mut Vec<SearchHit>,
) -> Result<(), String> {
    // A type filter is about files, so pages are not searched.
    if query.types.is_empty() {
        let pages = index
            .search_text(&query.text, scope, MAX_RESULTS)
            .map_err(|err| err.to_string())?;
        for hit in pages {
            let (snippet, highlights) = split_marks(&hit.snippet);
            out.push(SearchHit {
                kind: HitKind::Page,
                attachment: None,
                notebook_id: root.id.clone(),
                notebook_name: root.name.clone(),
                section_id: format!("{}/{}", root.id, hit.section),
                section_name: last_part(&hit.section),
                section_path: hit.section,
                page_id: hit.id,
                title: hit.title,
                modified: hit.modified,
                rank: hit.rank,
                snippet,
                highlights,
            });
        }
    }
    if query.words.is_empty() && query.types.is_empty() {
        return Ok(());
    }
    let files = index.attachments(scope).map_err(|err| err.to_string())?;
    for file in files
        .into_iter()
        .filter(|f| query.matches_attachment(&f.name, &f.mime))
        .take(MAX_RESULTS)
    {
        let size = u64::try_from(file.size).unwrap_or(0);
        let kind = file
            .name
            .rsplit_once('.')
            .map_or_else(|| file.mime.clone(), |(_, ext)| ext.to_lowercase());
        out.push(SearchHit {
            kind: HitKind::Attachment,
            section_id: if file.section.is_empty() {
                String::new()
            } else {
                format!("{}/{}", root.id, file.section)
            },
            section_name: if file.section.is_empty() {
                root.name.clone()
            } else {
                last_part(&file.section)
            },
            attachment: Some(AttachmentInfo {
                path: file.path,
                mime: file.mime,
                size,
            }),
            notebook_id: root.id.clone(),
            notebook_name: root.name.clone(),
            section_path: file.section,
            page_id: file.page.unwrap_or_default(),
            title: file.name,
            modified: file.modified,
            // Pages score below zero, so a name match lists after the page matches.
            rank: 0.0,
            snippet: format!("{kind} file, {}", size_text(size)),
            highlights: Vec::new(),
        });
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn last_part(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

/// Search `roots` for `query` within `scope`, relative to `context`, ordered by `sort`.
///
/// A notebook whose index cannot be read is skipped. An empty query, or a scope that needs a
/// page or section that is not selected, finds nothing.
#[cfg(not(target_arch = "wasm32"))]
pub fn search(
    roots: &[SearchRoot],
    context: &SearchContext,
    query: &str,
    scope: SearchScope,
    sort: SearchSort,
) -> SearchResults {
    use notecraft_storage::index::{Index, TextScope};
    use notecraft_storage::index_path;

    let mut results = SearchResults {
        query: query.to_owned(),
        scope,
        sort,
        ..SearchResults::default()
    };
    let parsed = ParsedQuery::new(query);
    let mut found: Vec<(usize, SearchHit)> = Vec::new();
    for (at, root) in roots.iter().enumerate() {
        let current = context.notebook.as_deref() == Some(root.id.as_str());
        let section = context
            .section
            .as_deref()
            .and_then(|id| section_path(&root.id, id));
        let text_scope = match scope {
            SearchScope::All => Some(TextScope::All),
            SearchScope::Notebook => current.then_some(TextScope::All),
            SearchScope::Page => context
                .page
                .as_deref()
                .filter(|_| current)
                .map(TextScope::Page),
            SearchScope::Section => section.filter(|_| current).map(TextScope::Section),
            SearchScope::Group => section.filter(|_| current).map(|path| {
                path.rsplit_once('/')
                    .map_or(TextScope::All, |(group, _)| TextScope::Group(group))
            }),
        };
        let Some(text_scope) = text_scope else {
            continue;
        };
        let db = index_path(&root.path);
        if !db.is_file() {
            continue;
        }
        let index = match Index::open(&db) {
            Ok(index) => index,
            Err(err) => {
                tracing::warn!("search in {} failed: {err}", root.name);
                continue;
            }
        };
        let mut hits = Vec::new();
        if let Err(err) = notebook_hits(root, &index, text_scope, &parsed, &mut hits) {
            tracing::warn!("search in {} failed: {err}", root.name);
            continue;
        }
        found.extend(hits.into_iter().map(|hit| (at, hit)));
    }
    let mut keyed: Vec<(Sortable, SearchHit)> = found
        .into_iter()
        .map(|(at, hit)| (hit.sortable(at), hit))
        .collect();
    keyed.sort_by(|a, b| order(sort, &a.0, &b.0));
    results.truncated = keyed.len() > MAX_RESULTS;
    keyed.truncate(MAX_RESULTS);
    results.hits = keyed.into_iter().map(|(_, hit)| hit).collect();
    results
}

/// Most of a page file read to build its search text.
#[cfg(not(target_arch = "wasm32"))]
const MAX_INDEX_READ: u64 = 4 * 1024 * 1024;
/// Most bytes of page text kept in the full-text table.
#[cfg(not(target_arch = "wasm32"))]
const MAX_INDEX_TEXT: usize = 256 * 1024;

/// The title and plain text of a page file's source, for the full-text table: the frontmatter
/// and the canvas block are left out, markdown syntax is dropped.
#[cfg(not(target_arch = "wasm32"))]
pub fn page_search_text(source: &str) -> (Option<String>, String) {
    let visible = match notecraft_storage::find_canvas_block(source) {
        Some(block) => source.get(..block.block.start).unwrap_or(source),
        None => source,
    };
    let doc = notecraft_markdown::parse(visible);
    let title = doc
        .frontmatter
        .as_ref()
        .and_then(|front| front.title())
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty());
    (title, doc.plain_text_up_to(MAX_INDEX_TEXT))
}

/// Build the full-text rows of every page in `index` that has none or has an old one (new
/// pages and pages whose file changed). Call it after [`Index::refresh`]; it is cheap when
/// nothing changed. Returns how many pages were indexed. A page that cannot be read gets an
/// empty text, so it is not tried again until it changes.
///
/// [`Index::refresh`]: notecraft_storage::index::Index::refresh
#[cfg(not(target_arch = "wasm32"))]
pub fn update_text_index(
    root: &std::path::Path,
    index: &mut notecraft_storage::index::Index,
) -> Result<usize, String> {
    use std::io::Read;

    use notecraft_storage::index::PageBody;

    let pending = index.pages_needing_text().map_err(|err| err.to_string())?;
    let mut rows = Vec::with_capacity(pending.len());
    let mut refs = Vec::with_capacity(pending.len());
    for (id, path, title) in pending {
        let mut bytes = Vec::new();
        let read = std::fs::File::open(root.join(&path))
            .and_then(|file| file.take(MAX_INDEX_READ).read_to_end(&mut bytes));
        let (found_title, body, links) = match read {
            Ok(_) => {
                let source = String::from_utf8_lossy(&bytes);
                let (title, body) = page_search_text(&source);
                (title, body, attachment_links(&source, &path))
            }
            Err(_) => (None, String::new(), Vec::new()),
        };
        refs.push((id.clone(), links));
        rows.push(PageBody {
            id,
            title: found_title.unwrap_or(title),
            body,
        });
    }
    let count = rows.len();
    index.set_page_texts(&rows).map_err(|err| err.to_string())?;
    index
        .set_attachment_refs(&refs)
        .map_err(|err| err.to_string())?;
    Ok(count)
}

/// Most attachment links read from one page.
#[cfg(not(target_arch = "wasm32"))]
const MAX_PAGE_ATTACHMENT_LINKS: usize = 1_000;

/// The attachments a page file links to, as paths from the notebook folder. An insert writes
/// `attachments/<stored name>` next to the page (percent-encoded), so a link is that text
/// right after `(`, a quote, `<`, `[`, or white space.
#[cfg(not(target_arch = "wasm32"))]
fn attachment_links(source: &str, page_path: &str) -> Vec<String> {
    const MARKER: &str = "attachments/";
    let folder = page_path.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut out: Vec<String> = Vec::new();
    let mut from = 0;
    while let Some(found) = source.get(from..).and_then(|rest| rest.find(MARKER)) {
        let at = from + found;
        from = at + MARKER.len();
        let before = source.get(..at).and_then(|s| s.chars().next_back());
        if before.is_some_and(|c| !(c.is_whitespace() || matches!(c, '(' | '"' | '\'' | '<' | '[')))
        {
            continue;
        }
        let tail = source.get(from..).unwrap_or_default();
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, ')' | '"' | '\'' | '>' | ']' | '<'))
            .unwrap_or(tail.len());
        let name = percent_decode(tail.get(..end).unwrap_or_default());
        if name.is_empty() || name.contains("..") || name.contains('\\') {
            continue;
        }
        let path = if folder.is_empty() {
            format!("{MARKER}{name}")
        } else {
            format!("{folder}/{MARKER}{name}")
        };
        if !out.contains(&path) {
            out.push(path);
            if out.len() >= MAX_PAGE_ATTACHMENT_LINKS {
                break;
            }
        }
    }
    out
}

/// `%20` style escapes undone; a bad escape stays as it is.
#[cfg(not(target_arch = "wasm32"))]
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let hex = |b: Option<&u8>| {
            b.and_then(|b| char::from(*b).to_digit(16))
                .and_then(|d| u8::try_from(d).ok())
        };
        if byte == b'%' {
            if let (Some(high), Some(low)) = (hex(bytes.get(at + 1)), hex(bytes.get(at + 2))) {
                out.push(high * 16 + low);
                at += 3;
                continue;
            }
        }
        out.push(byte);
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Bring the notebook index at `root` in line with the folder, text included, without the
/// app running (the command line `index` and `search` call this). Returns the number of
/// pages whose search text was built.
#[cfg(not(target_arch = "wasm32"))]
pub fn refresh_search_index(root: &std::path::Path) -> Result<usize, String> {
    let (mut index, _) =
        notecraft_storage::index::refresh_or_rebuild(root, &notecraft_storage::index_path(root))
            .map_err(|err| err.to_string())?;
    let count = update_text_index(root, &mut index)?;
    // Page links (PR 2.2) are filled in the same pass, and resolved against all pages.
    crate::link_index::update_link_index(root, &mut index, true)?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_become_ranges() {
        let (text, ranges) = split_marks("pack the \u{2}tent\u{3} and \u{2}stove\u{3}\nnow");
        assert_eq!(text, "pack the tent and stove now");
        assert_eq!(ranges, [(9, 13), (18, 23)]);
        assert_eq!(&text[9..13], "tent");
        // Stray or unclosed marks are dropped.
        let (text, ranges) = split_marks("a \u{3}b \u{2}c");
        assert_eq!(text, "a b c");
        assert!(ranges.is_empty());
    }

    #[test]
    fn ids_round_trip() {
        for scope in SearchScope::LIST {
            assert_eq!(SearchScope::from_id(scope.id()), Some(scope));
        }
        for sort in SearchSort::LIST {
            assert_eq!(SearchSort::from_id(sort.id()), Some(sort));
        }
        assert_eq!(SearchScope::from_id("nope"), None);
    }

    #[test]
    fn section_paths_come_from_ids() {
        assert_eq!(section_path("nb", "nb/Grp/Sec"), Some("Grp/Sec"));
        assert_eq!(section_path("nb", "other/Sec"), None);
        assert_eq!(section_path("nb", "nbx/Sec"), None);
    }

    #[test]
    fn type_words_limit_a_search_to_attachments() {
        let q = ParsedQuery::new("budget type:PDF");
        assert_eq!(q.text, "budget");
        assert!(q.matches_attachment("budget-2026.pdf", "application/pdf"));
        assert!(!q.matches_attachment("budget-2026.png", "image/png"));
        assert!(!q.matches_attachment("notes.pdf", "application/pdf"));
        let q = ParsedQuery::new("type:image");
        assert!(q.matches_attachment("a.png", "image/png"));
        assert!(!q.matches_attachment("a.pdf", "application/pdf"));
        let q = ParsedQuery::new("   ");
        assert!(!q.matches_attachment("a.pdf", "application/pdf"));
    }

    #[test]
    fn attachment_links_are_read_from_page_text() {
        let links = attachment_links(
            "see ![x](attachments/ab12-my%20file.pdf) and <attachments/c.png> not xattachments/d",
            "Sec/page.notecraft",
        );
        assert_eq!(
            links,
            ["Sec/attachments/ab12-my file.pdf", "Sec/attachments/c.png"]
        );
        assert!(attachment_links("(attachments/../x)", "p.notecraft").is_empty());
    }

    #[test]
    fn sizes_read_in_units() {
        assert_eq!(size_text(12), "12 bytes");
        assert_eq!(size_text(2048), "2 KB");
        assert_eq!(size_text(3 * 1024 * 1024), "3 MB");
    }
}
