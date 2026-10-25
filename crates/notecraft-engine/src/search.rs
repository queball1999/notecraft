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

/// One result: a page whose title or text matched.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    pub notebook_id: String,
    pub notebook_name: String,
    /// The section id (`<notebook id>/<folder path>`), as the navigation uses it.
    pub section_id: String,
    /// The section's folder path inside the notebook, such as `Group/Section`.
    pub section_path: String,
    /// The last part of `section_path`.
    pub section_name: String,
    pub page_id: String,
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
        let hits =
            Index::open(&db).and_then(|index| index.search_text(query, text_scope, MAX_RESULTS));
        let hits = match hits {
            Ok(hits) => hits,
            Err(err) => {
                tracing::warn!("search in {} failed: {err}", root.name);
                continue;
            }
        };
        for hit in hits {
            let (snippet, highlights) = split_marks(&hit.snippet);
            let section_name = hit
                .section
                .rsplit('/')
                .next()
                .unwrap_or(hit.section.as_str())
                .to_owned();
            found.push((
                at,
                SearchHit {
                    notebook_id: root.id.clone(),
                    notebook_name: root.name.clone(),
                    section_id: format!("{}/{}", root.id, hit.section),
                    section_path: hit.section,
                    section_name,
                    page_id: hit.id,
                    title: hit.title,
                    modified: hit.modified,
                    rank: hit.rank,
                    snippet,
                    highlights,
                },
            ));
        }
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
    for (id, path, title) in pending {
        let mut bytes = Vec::new();
        let read = std::fs::File::open(root.join(&path))
            .and_then(|file| file.take(MAX_INDEX_READ).read_to_end(&mut bytes));
        let (found_title, body) = match read {
            Ok(_) => page_search_text(&String::from_utf8_lossy(&bytes)),
            Err(_) => (None, String::new()),
        };
        rows.push(PageBody {
            id,
            title: found_title.unwrap_or(title),
            body,
        });
    }
    let count = rows.len();
    index.set_page_texts(&rows).map_err(|err| err.to_string())?;
    Ok(count)
}

/// Bring the notebook index at `root` in line with the folder, text included, without the
/// app running (the command line `index` and `search` call this). Returns the number of
/// pages whose search text was built.
#[cfg(not(target_arch = "wasm32"))]
pub fn refresh_search_index(root: &std::path::Path) -> Result<usize, String> {
    let (mut index, _) =
        notecraft_storage::index::refresh_or_rebuild(root, &notecraft_storage::index_path(root))
            .map_err(|err| err.to_string())?;
    update_text_index(root, &mut index)
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
}
