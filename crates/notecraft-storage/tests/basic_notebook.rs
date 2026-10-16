//! Scans the sample notebook in `testdata/notebooks/basic` and checks the whole tree.

// A failing test is supposed to stop.
#![allow(clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use notecraft_storage::{scan_notebook, NotebookTree, PageKind, SectionEntry, TreeNode};

fn sample() -> NotebookTree {
    let root: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "testdata",
        "notebooks",
        "basic",
    ]
    .iter()
    .collect();
    scan_notebook(&root).expect("the sample notebook scans")
}

fn pages(section: &SectionEntry) -> Vec<(&str, u8)> {
    section
        .pages
        .iter()
        .map(|p| (p.title.as_str(), p.level))
        .collect()
}

#[test]
fn identity_comes_from_notebook_toml() {
    let tree = sample();
    assert_eq!(tree.id, "nb_basic_sample");
    assert_eq!(tree.name, "Basic Sample");
    assert_eq!(tree.color, Some(0x7A4FB5));
    assert!(tree.warnings.is_empty(), "{:?}", tree.warnings);
}

#[test]
fn top_level_follows_the_listed_order() {
    let tree = sample();
    let names: Vec<&str> = tree.nodes.iter().map(TreeNode::name).collect();
    assert_eq!(names, ["Journal", "Projects", "Reference"]);
}

#[test]
fn journal_has_ordered_pages_with_levels() {
    let tree = sample();
    let TreeNode::Section(journal) = &tree.nodes[0] else {
        panic!("Journal is a section");
    };
    assert_eq!(journal.color, Some(0x3C8DBC));
    assert_eq!(
        pages(journal),
        [
            ("Welcome", 0),
            ("Daily log", 0),
            ("Monday", 1),
            ("Morning run", 2),
            ("Tuesday", 1),
            ("Sketch ideas", 0),
        ]
    );
    let sketch = journal.pages.last().expect("a last page");
    assert_eq!(sketch.kind, PageKind::Canvas);
    assert_eq!(sketch.rel_path, "Journal/Sketch ideas.notecraft");
}

#[test]
fn projects_is_a_group_with_ordered_sections() {
    let tree = sample();
    let TreeNode::Group(group) = &tree.nodes[1] else {
        panic!("Projects is a group");
    };
    let names: Vec<&str> = group.children.iter().map(TreeNode::name).collect();
    assert_eq!(names, ["Beta", "Alpha"]);
    let TreeNode::Section(alpha) = &group.children[1] else {
        panic!("Alpha is a section");
    };
    assert_eq!(pages(alpha), [("Plan", 0), ("Risks", 0)]);
}

#[test]
fn reference_pages_sort_by_name_and_skip_other_files() {
    let tree = sample();
    let TreeNode::Section(reference) = &tree.nodes[2] else {
        panic!("Reference is a section");
    };
    assert_eq!(
        pages(reference),
        [("Archive", 0), ("Reading list", 0), ("Recipes", 0)]
    );
}

#[test]
fn totals() {
    let tree = sample();
    assert_eq!(tree.sections().len(), 4);
    assert_eq!(tree.page_count(), 12);
}
