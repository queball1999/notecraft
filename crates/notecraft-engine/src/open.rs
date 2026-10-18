//! Opening a notebook folder: scan it with the storage layer and turn the tree into model
//! types the session and the navigation use.
//!
//! Ids are made unique across open notebooks by prefixing the notebook id: a section is
//! `<notebook id>/<folder path>` and a page is `<notebook id>/<file path>`, so they are stable
//! as long as the files stay where they are.

use std::path::Path;

use notecraft_model::{Notebook, PageRef, Section, SectionGroup, SectionNode};
use notecraft_storage::{
    scan_notebook, GroupEntry, NotebookTree, ScanError, SectionEntry, TreeNode,
};

/// Notebook swatch when `notebook.toml` sets no color.
pub const DEFAULT_NOTEBOOK_COLOR: u32 = 0x7E57C2;

/// Section tab colors for sections whose `.section.toml` sets none, used in turn.
pub const DEFAULT_SECTION_COLORS: [u32; 6] =
    [0xF6C28B, 0x9FD3E8, 0xC9B6E4, 0xB5DCA6, 0xF3A9B8, 0xE8D87A];

/// A notebook read from disk, with anything the scan wants the user to know.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenedNotebook {
    pub notebook: Notebook,
    /// Plain-text warnings from the scan (skipped folders, bad settings files, ...).
    pub warnings: Vec<String>,
}

/// Scan the notebook folder at `path`.
pub fn open_folder(path: &Path) -> Result<OpenedNotebook, ScanError> {
    let tree = scan_notebook(path)?;
    Ok(OpenedNotebook {
        warnings: tree.warnings.iter().map(ToString::to_string).collect(),
        notebook: notebook_from_tree(&tree),
    })
}

/// The model notebook for a scanned tree.
pub fn notebook_from_tree(tree: &NotebookTree) -> Notebook {
    let mut colors = 0;
    Notebook {
        id: tree.id.clone(),
        name: tree.name.clone(),
        color: tree.color.unwrap_or(DEFAULT_NOTEBOOK_COLOR),
        sections: nodes(&tree.id, &tree.nodes, &mut colors),
    }
}

/// What opening a notebook for the navigation found.
#[derive(Clone, Debug, PartialEq)]
pub struct NavOpened {
    pub opened: OpenedNotebook,
    /// Frontmatter titles by page id, when the tree came from the index.
    pub titles: std::collections::BTreeMap<String, String>,
}

/// Open the notebook at `path` for the navigation. When the folder has a usable index with
/// pages in it, the tree comes from the index (page order and frontmatter titles) and the
/// folder is not listed; the background refresh (`SessionIo`) catches the index up. With no
/// index, an empty one, or one that cannot be read, the folder reader is the fallback. The
/// index is never repaired here, so a large database cannot slow the open.
pub fn open_folder_for_navigation(path: &Path) -> Result<NavOpened, ScanError> {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(found) = open_from_index(path) {
        return Ok(found);
    }
    Ok(NavOpened {
        opened: open_folder(path)?,
        titles: std::collections::BTreeMap::new(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn open_from_index(path: &Path) -> Option<NavOpened> {
    use notecraft_storage::index::Index;
    use notecraft_storage::{index_path, read_notebook_header};

    let db = index_path(path);
    if !path.is_dir() || !db.is_file() {
        return None;
    }
    let sidebar = Index::open(&db).and_then(|index| index.sidebar()).ok()?;
    let header = read_notebook_header(path);
    let notebook = notebook_from_sidebar(&header, &sidebar)?;
    let titles = titles_of(&sidebar);
    Some(NavOpened {
        opened: OpenedNotebook {
            notebook,
            warnings: Vec::new(),
        },
        titles,
    })
}

/// Page id to frontmatter title for the pages that have one.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn titles_of(
    sidebar: &notecraft_storage::index::Sidebar,
) -> std::collections::BTreeMap<String, String> {
    sidebar
        .pages
        .iter()
        .filter_map(|page| Some((page.id.clone(), page.frontmatter_title.clone()?)))
        .collect()
}

/// Deepest group nesting followed when a tree is built from index rows.
#[cfg(not(target_arch = "wasm32"))]
const MAX_GROUP_DEPTH: usize = 64;

/// The model notebook for index rows, or `None` when the index holds no sections (it has
/// not been filled yet). The rows are in sidebar order, so the result matches
/// [`notebook_from_tree`] for the same folder.
#[cfg(not(target_arch = "wasm32"))]
pub fn notebook_from_sidebar(
    header: &notecraft_storage::NotebookHeader,
    sidebar: &notecraft_storage::index::Sidebar,
) -> Option<Notebook> {
    use std::collections::HashMap;

    if sidebar.sections.is_empty() {
        return None;
    }
    // Rows built under another notebook id (notebook.toml changed) are out of date.
    let prefix = format!("{}/", header.id);
    if sidebar.pages.iter().any(|p| !p.id.starts_with(&prefix)) {
        return None;
    }
    let mut children: HashMap<&str, Vec<usize>> = HashMap::new();
    for (at, row) in sidebar.sections.iter().enumerate() {
        children.entry(row.parent.as_str()).or_default().push(at);
    }
    let mut pages: HashMap<&str, Vec<&notecraft_storage::index::PageRow>> = HashMap::new();
    for page in &sidebar.pages {
        pages.entry(page.section.as_str()).or_default().push(page);
    }
    struct Build<'a> {
        notebook: &'a str,
        sidebar: &'a notecraft_storage::index::Sidebar,
        children: HashMap<&'a str, Vec<usize>>,
        pages: HashMap<&'a str, Vec<&'a notecraft_storage::index::PageRow>>,
        colors: usize,
    }
    impl Build<'_> {
        fn level(&mut self, parent: &str, depth: usize) -> Vec<SectionNode> {
            if depth > MAX_GROUP_DEPTH {
                return Vec::new();
            }
            let rows = self.children.get(parent).cloned().unwrap_or_default();
            let mut out = Vec::new();
            for at in rows {
                let Some(row) = self.sidebar.sections.get(at) else {
                    continue;
                };
                let id = format!("{}/{}", self.notebook, row.path);
                if row.kind == "group" {
                    let children = self.level(&row.path, depth + 1);
                    out.push(SectionNode::Group(SectionGroup {
                        id,
                        name: row.name.clone(),
                        children,
                    }));
                } else {
                    let color = row.color.unwrap_or_else(|| next_color(&mut self.colors));
                    let pages = self
                        .pages
                        .get(row.path.as_str())
                        .map(|rows| {
                            rows.iter()
                                .map(|p| PageRef::new(p.id.clone(), p.title.clone(), p.level))
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push(SectionNode::Section(Section {
                        id,
                        name: row.name.clone(),
                        color,
                        pages,
                    }));
                }
            }
            out
        }
    }
    let mut build = Build {
        notebook: &header.id,
        sidebar,
        children,
        pages,
        colors: 0,
    };
    let sections = build.level("", 0);
    Some(Notebook {
        id: header.id.clone(),
        name: header.name.clone(),
        color: header.color.unwrap_or(DEFAULT_NOTEBOOK_COLOR),
        sections,
    })
}

fn nodes(notebook: &str, nodes: &[TreeNode], colors: &mut usize) -> Vec<SectionNode> {
    nodes
        .iter()
        .map(|node| match node {
            TreeNode::Section(section) => {
                SectionNode::Section(section_of(notebook, section, colors))
            }
            TreeNode::Group(group) => SectionNode::Group(group_of(notebook, group, colors)),
        })
        .collect()
}

fn section_of(notebook: &str, section: &SectionEntry, colors: &mut usize) -> Section {
    Section {
        id: format!("{notebook}/{}", section.rel_path),
        name: section.name.clone(),
        color: section.color.unwrap_or_else(|| next_color(colors)),
        pages: section
            .pages
            .iter()
            .map(|page| {
                PageRef::new(
                    format!("{notebook}/{}", page.rel_path),
                    page.title.clone(),
                    page.level,
                )
                .with_collapsed(page.collapsed)
            })
            .collect(),
    }
}

fn group_of(notebook: &str, group: &GroupEntry, colors: &mut usize) -> SectionGroup {
    SectionGroup {
        id: format!("{notebook}/{}", group.rel_path),
        name: group.name.clone(),
        children: nodes(notebook, &group.children, colors),
    }
}

fn next_color(colors: &mut usize) -> u32 {
    let color = DEFAULT_SECTION_COLORS
        .get(*colors % DEFAULT_SECTION_COLORS.len())
        .copied()
        .unwrap_or(DEFAULT_NOTEBOOK_COLOR);
    *colors += 1;
    color
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn sample_path() -> PathBuf {
        [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "testdata",
            "notebooks",
            "basic",
        ]
        .iter()
        .collect()
    }

    #[test]
    fn the_sample_notebook_becomes_model_types() {
        let opened = open_folder(&sample_path()).unwrap();
        assert!(opened.warnings.is_empty());
        let nb = opened.notebook;
        assert_eq!(nb.id, "nb_basic_sample");
        assert_eq!(nb.name, "Basic Sample");
        assert_eq!(nb.color, 0x7A4FB5);
        let sections: Vec<&str> = nb.all_sections().iter().map(|s| s.name.as_str()).collect();
        assert_eq!(sections, ["Journal", "Beta", "Alpha", "Reference"]);
        let journal = nb.first_section().unwrap();
        assert_eq!(journal.id, "nb_basic_sample/Journal");
        assert_eq!(journal.color, 0x3C8DBC);
        let monday = journal.pages.iter().find(|p| p.title == "Monday").unwrap();
        assert_eq!(monday.id, "nb_basic_sample/Journal/Monday.md");
        assert_eq!(monday.level, 1);
        // Sections without a color take the defaults in turn.
        let beta = nb.section("nb_basic_sample/Projects/Beta").unwrap();
        assert_eq!(beta.color, DEFAULT_SECTION_COLORS[0]);
        assert!(matches!(nb.sections[1], SectionNode::Group(_)));
    }

    #[test]
    fn ids_are_unique() {
        let nb = open_folder(&sample_path()).unwrap().notebook;
        let mut ids = std::collections::BTreeSet::new();
        for section in nb.all_sections() {
            assert!(ids.insert(section.id.clone()));
            for page in &section.pages {
                assert!(ids.insert(page.id.clone()), "{}", page.id);
            }
        }
    }

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    #[test]
    fn the_index_gives_the_same_tree_as_the_folder_reader() {
        let dir = tempfile::TempDir::new().unwrap();
        copy_dir(&sample_path(), dir.path());
        // No index yet: the folder reader is the fallback.
        let before = open_folder_for_navigation(dir.path()).unwrap();
        assert!(before.titles.is_empty());
        let db = notecraft_storage::index_path(dir.path());
        assert!(!db.exists());
        notecraft_storage::index::refresh_or_rebuild(dir.path(), &db).unwrap();
        let from_index = open_folder_for_navigation(dir.path()).unwrap();
        assert_eq!(from_index.opened.notebook, before.opened.notebook);
    }

    #[test]
    fn a_corrupt_index_falls_back_to_the_folder() {
        let dir = tempfile::TempDir::new().unwrap();
        copy_dir(&sample_path(), dir.path());
        let db = notecraft_storage::index_path(dir.path());
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        std::fs::write(&db, "garbage").unwrap();
        let opened = open_folder_for_navigation(dir.path()).unwrap();
        assert_eq!(opened.opened.notebook.name, "Basic Sample");
    }

    #[test]
    fn a_missing_folder_is_an_error() {
        let missing = sample_path().join("no-such-folder");
        assert!(open_folder(&missing).is_err());
    }
}
