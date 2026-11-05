//! Page links in the engine (build_plan/04 PR 2.2): the pages a link can point to, how the
//! open page's links resolve, and the page picker that opens when `[[` is typed.
//!
//! The resolution rules live in `notecraft_markdown::LinkIndex` (docs/markdown.md section
//! 5.3). This module feeds it from the open notebooks' trees and from the index.

use notecraft_markdown::LinkPage;
use notecraft_model::{Notebook, SectionNode};

/// Most pages the picker lists at once.
pub const MAX_PICKER_ENTRIES: usize = 50;

/// Deepest group nesting walked when listing pages.
const MAX_DEPTH: usize = 64;

/// The path of a page from its notebook folder: the page id without the notebook id prefix.
pub fn page_path<'a>(notebook: &str, page_id: &'a str) -> Option<&'a str> {
    page_id
        .strip_prefix(notebook)
        .and_then(|rest| rest.strip_prefix('/'))
}

/// Every page of a notebook as a link target, in navigation order. `aliases` maps a page id
/// to the aliases of its frontmatter.
pub fn link_pages(notebook: &Notebook, aliases: &dyn Fn(&str) -> Vec<String>) -> Vec<LinkPage> {
    let mut out = Vec::new();
    collect(notebook, &notebook.sections, aliases, 0, &mut out);
    out
}

fn collect(
    notebook: &Notebook,
    nodes: &[SectionNode],
    aliases: &dyn Fn(&str) -> Vec<String>,
    depth: usize,
    out: &mut Vec<LinkPage>,
) {
    if depth > MAX_DEPTH {
        return;
    }
    for node in nodes {
        match node {
            SectionNode::Section(section) => {
                for page in &section.pages {
                    if let Some(path) = page_path(&notebook.id, &page.id) {
                        out.push(
                            LinkPage::new(page.id.clone(), path.to_owned())
                                .with_aliases(aliases(&page.id)),
                        );
                    }
                }
            }
            SectionNode::Group(group) => {
                collect(notebook, &group.children, aliases, depth + 1, out)
            }
        }
    }
}

/// What a link target resolved to, with owned data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkResolution {
    /// The page a click goes to; for an ambiguous link, the closest candidate.
    pub page: Option<String>,
    /// More than one page matched.
    pub ambiguous: bool,
    /// Every matching page, closest first, when the link is ambiguous.
    pub candidates: Vec<LinkCandidate>,
}

/// A page an ambiguous link could mean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkCandidate {
    pub page: String,
    /// Its path from the notebook folder without the extension, for showing.
    pub label: String,
}

impl LinkResolution {
    pub fn unresolved() -> Self {
        Self {
            page: None,
            ambiguous: false,
            candidates: Vec::new(),
        }
    }

    /// Own the answer of the markdown resolver.
    pub fn from_resolved(found: &notecraft_markdown::Resolved<'_>) -> Self {
        use notecraft_markdown::Resolved;
        match found {
            Resolved::Page(page) => Self {
                page: Some(page.id.clone()),
                ambiguous: false,
                candidates: Vec::new(),
            },
            Resolved::Ambiguous { chosen, candidates } => Self {
                page: Some(chosen.id.clone()),
                ambiguous: true,
                candidates: candidates
                    .iter()
                    .map(|page| LinkCandidate {
                        page: page.id.clone(),
                        label: page.stem().to_owned(),
                    })
                    .collect(),
            },
            Resolved::Unresolved => Self::unresolved(),
        }
    }
}

/// The page picker opened by typing `[[` (docs/markdown.md section 5.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkPicker {
    /// The page being edited.
    pub page: String,
    /// Byte offset in the page text just after the `[[`.
    pub at: usize,
    /// Which listed entry is highlighted.
    pub selected: usize,
}

/// One page the picker lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerEntry {
    pub page: String,
    pub title: String,
    /// The section that holds the page.
    pub place: String,
}

/// What the picker shows now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkPickerView {
    /// The text typed after `[[`.
    pub query: String,
    pub entries: Vec<PickerEntry>,
    /// Index into `entries` of the highlighted one.
    pub selected: usize,
}
