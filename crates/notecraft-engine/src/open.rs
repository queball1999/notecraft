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

    #[test]
    fn a_missing_folder_is_an_error() {
        let missing = sample_path().join("no-such-folder");
        assert!(open_folder(&missing).is_err());
    }
}
