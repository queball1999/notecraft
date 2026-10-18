//! Notebooks on disk, for the subcommands that read one: finding the folder, listing its
//! pages, and turning what the user typed into a page (`<page>` in docs/cli.md).

use std::path::PathBuf;

use notecraft_storage::notebook_dir::{NOTEBOOK_DIR_NAME, NOTEBOOK_FILE_NAME};
use notecraft_storage::{scan_notebook, NotebookTree, PageEntry, ScanError};

use crate::code;

/// A command that did not work: the exit code and a plain-text reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: u8,
    pub message: String,
}

impl Failure {
    pub fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// The notebook folder: `--notebook` if given, else the current directory when it is a
/// notebook (it has `.notecraft/notebook.toml`).
pub fn locate(explicit: Option<&str>) -> Result<PathBuf, Failure> {
    if let Some(path) = explicit {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Ok(path);
        }
        return Err(Failure::new(
            code::NOT_FOUND,
            format!("notebook folder not found: {}", path.display()),
        ));
    }
    let here = std::env::current_dir()
        .map_err(|e| Failure::new(code::ERROR, format!("cannot read the current folder: {e}")))?;
    if here
        .join(NOTEBOOK_DIR_NAME)
        .join(NOTEBOOK_FILE_NAME)
        .is_file()
    {
        return Ok(here);
    }
    Err(Failure::new(
        code::NOT_FOUND,
        "no notebook here: run inside a notebook folder or pass --notebook <path>",
    ))
}

/// A notebook folder and what the folder reader found in it.
pub struct Notebook {
    pub root: PathBuf,
    pub tree: NotebookTree,
}

/// Find the notebook folder and scan it.
pub fn open(explicit: Option<&str>) -> Result<Notebook, Failure> {
    let root = locate(explicit)?;
    let tree = scan_notebook(&root).map_err(|e| match e {
        ScanError::NotADirectory(_) => Failure::new(code::NOT_FOUND, e.to_string()),
        ScanError::Io { .. } => Failure::new(code::ERROR, e.to_string()),
    })?;
    Ok(Notebook { root, tree })
}

/// One page of the notebook, as the session names it.
#[derive(Debug)]
pub struct PageInfo<'a> {
    /// The session's page id: `<notebook id>/<path>`.
    pub id: String,
    pub entry: &'a PageEntry,
    /// The section folder, relative to the notebook (with `/` separators).
    pub section: &'a str,
}

impl PageInfo<'_> {
    /// The page's path without its extension: how a link names it.
    pub fn path_without_extension(&self) -> &str {
        strip_extension(&self.entry.rel_path)
    }

    /// The page's file name without its extension.
    pub fn stem(&self) -> &str {
        strip_extension(&self.entry.file_name)
    }
}

fn strip_extension(name: &str) -> &str {
    for ext in [".md", ".notecraft"] {
        if let Some(stem) = name.strip_suffix(ext) {
            return stem;
        }
    }
    name
}

impl Notebook {
    /// Every page, section by section in notebook order.
    pub fn pages(&self) -> Vec<PageInfo<'_>> {
        let mut out = Vec::new();
        for section in self.tree.sections() {
            for entry in &section.pages {
                out.push(PageInfo {
                    id: format!("{}/{}", self.tree.id, entry.rel_path),
                    entry,
                    section: &section.rel_path,
                });
            }
        }
        out
    }
}

/// The page `query` names (see [`resolve_page`]). Exit code 3 when nothing matches, 2 when
/// more than one page does.
pub fn resolve<'a, 'b>(
    pages: &'b [PageInfo<'a>],
    query: &str,
) -> Result<&'b PageInfo<'a>, Failure> {
    match resolve_page(pages, query, None) {
        Found::One(page) => Ok(page),
        Found::None => Err(Failure::new(
            code::NOT_FOUND,
            format!("no page matches `{}`", query.trim()),
        )),
        Found::Many(list) => {
            let names: Vec<&str> = list.iter().map(|p| p.entry.rel_path.as_str()).collect();
            Err(Failure::new(
                code::BAD_ARGS,
                format!(
                    "`{}` matches more than one page: {}",
                    query.trim(),
                    names.join(", ")
                ),
            ))
        }
    }
}

/// The result of looking a page name up.
#[derive(Debug)]
pub enum Found<'a, 'b> {
    One(&'b PageInfo<'a>),
    Many(Vec<&'b PageInfo<'a>>),
    None,
}

/// Look `query` up in `pages`, trying in order: the page id; the path relative to the
/// notebook; that path without its extension; the path relative to `section` (the section
/// folder of the page holding a link), if given; the file name without its extension; the
/// same, ignoring case. The first step that finds anything decides. Page aliases from the
/// frontmatter and Unicode normalization (docs/markdown.md section 5.3, steps 4 and 5) are
/// not applied yet.
pub fn resolve_page<'a, 'b>(
    pages: &'b [PageInfo<'a>],
    query: &str,
    section: Option<&str>,
) -> Found<'a, 'b> {
    let query = query.trim().replace('\\', "/");
    let query = query.strip_prefix("./").unwrap_or(&query);
    if query.is_empty() {
        return Found::None;
    }
    let lower = query.to_lowercase();
    let in_section = section.map(|s| format!("{s}/{query}"));
    let steps: [&dyn Fn(&PageInfo<'_>) -> bool; 6] = [
        &|p| p.id == query,
        &|p| p.entry.rel_path == query,
        &|p| p.path_without_extension() == query,
        &|p| in_section.as_deref() == Some(p.path_without_extension()),
        &|p| p.stem() == query,
        &|p| p.stem().to_lowercase() == lower || p.path_without_extension().to_lowercase() == lower,
    ];
    for step in steps {
        let mut hits: Vec<&PageInfo<'_>> = pages.iter().filter(|p| step(p)).collect();
        match hits.len() {
            0 => {}
            1 => {
                if let Some(page) = hits.pop() {
                    return Found::One(page);
                }
            }
            _ => return Found::Many(hits),
        }
    }
    Found::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use notecraft_storage::PageKind;

    fn entry(rel: &str) -> PageEntry {
        let file_name = rel.rsplit('/').next().unwrap_or(rel).to_owned();
        PageEntry {
            rel_path: rel.to_owned(),
            title: strip_extension(&file_name).to_owned(),
            file_name,
            level: 0,
            collapsed: false,
            kind: PageKind::Markdown,
        }
    }

    fn infos(entries: &[PageEntry]) -> Vec<PageInfo<'_>> {
        entries
            .iter()
            .map(|e| PageInfo {
                id: format!("nb/{}", e.rel_path),
                entry: e,
                section: "S",
            })
            .collect()
    }

    fn name(found: &Found<'_, '_>) -> String {
        match found {
            Found::One(p) => p.entry.rel_path.clone(),
            Found::Many(l) => format!("many:{}", l.len()),
            Found::None => "none".to_owned(),
        }
    }

    #[test]
    fn a_page_is_found_by_id_path_or_link_target() {
        let entries = [entry("Journal/Monday.md"), entry("Projects/Alpha/Plan.md")];
        let pages = infos(&entries);
        for query in [
            "nb/Journal/Monday.md",
            "Journal/Monday.md",
            "Journal\\Monday.md",
            "./Journal/Monday.md",
            "Journal/Monday",
            "Monday",
            "monday",
        ] {
            assert_eq!(
                name(&resolve_page(&pages, query, None)),
                "Journal/Monday.md",
                "{query}"
            );
        }
        assert_eq!(name(&resolve_page(&pages, "Alpha/Plan", None)), "none");
        // A link names a page relative to its own section folder first.
        assert_eq!(
            name(&resolve_page(&pages, "Alpha/Plan", Some("Projects"))),
            "Projects/Alpha/Plan.md"
        );
        assert_eq!(name(&resolve_page(&pages, "", None)), "none");
    }

    #[test]
    fn a_name_shared_by_two_pages_is_ambiguous() {
        let entries = [entry("A/Plan.md"), entry("B/Plan.md")];
        let pages = infos(&entries);
        assert_eq!(name(&resolve_page(&pages, "Plan", None)), "many:2");
        assert_eq!(name(&resolve_page(&pages, "B/Plan", None)), "B/Plan.md");
    }
}
