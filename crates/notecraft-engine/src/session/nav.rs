//! Navigation titles and index results: the session side of reading the notebook index
//! (build_plan/03 PR 1.2, docs/architecture.md).
//!
//! The navigation first shows the tree read from the index (or the folder when there is no
//! usable index). A background refresh (`io/index_sync.rs`) later brings the index in line
//! with the folder and hands back a fresh tree, which replaces the one shown only when it
//! differs. Frontmatter titles come from the index; they are remembered here so a tree read
//! from the folder after a structure edit keeps showing them.

use std::collections::BTreeMap;

use notecraft_model::{Notebook, SectionNode};

use super::Session;

/// Deepest group nesting walked when titles are applied.
const MAX_DEPTH: usize = 64;

/// Replace page titles with the frontmatter titles known for them.
pub(crate) fn apply_titles(notebook: &mut Notebook, titles: &BTreeMap<String, String>) {
    if titles.is_empty() {
        return;
    }
    apply_to_nodes(&mut notebook.sections, titles, 0);
}

fn apply_to_nodes(nodes: &mut [SectionNode], titles: &BTreeMap<String, String>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    for node in nodes {
        match node {
            SectionNode::Section(section) => {
                for page in &mut section.pages {
                    if let Some(title) = titles.get(&page.id) {
                        page.title.clone_from(title);
                    }
                }
            }
            SectionNode::Group(group) => apply_to_nodes(&mut group.children, titles, depth + 1),
        }
    }
}

impl Session {
    /// Remember the frontmatter titles of notebook `notebook` (page id to title), replacing
    /// what was remembered. The tree in memory is not touched.
    pub fn remember_frontmatter_titles(
        &mut self,
        notebook: &str,
        titles: BTreeMap<String, String>,
    ) {
        let prefix = format!("{notebook}/");
        self.frontmatter_titles
            .retain(|id, _| !id.starts_with(&prefix));
        self.frontmatter_titles.extend(titles);
    }

    /// Goes up whenever the tree of `notebook` is read from the folder again or the notebook
    /// is opened again. A background index result built before the change is out of date.
    pub fn tree_generation(&self, notebook: &str) -> u64 {
        self.tree_generations.get(notebook).copied().unwrap_or(0)
    }

    pub(crate) fn bump_tree_generation(&mut self, notebook: &str) {
        let slot = self
            .tree_generations
            .entry(notebook.to_owned())
            .or_insert(0);
        *slot = slot.wrapping_add(1);
    }

    /// Show the tree the background index refresh produced, if nothing has changed the tree
    /// since the refresh began (`generation`) and the new tree differs from the one shown.
    /// Returns whether the result was current; a stale one is dropped and the caller
    /// refreshes again.
    pub fn apply_index_refresh(
        &mut self,
        generation: u64,
        notebook: Notebook,
        titles: BTreeMap<String, String>,
    ) -> bool {
        if generation != self.tree_generation(&notebook.id) {
            return false;
        }
        let id = notebook.id.clone();
        self.remember_frontmatter_titles(&id, titles);
        let Some(slot) = self.notebooks.iter_mut().find(|nb| nb.id == id) else {
            return true;
        };
        if *slot == notebook {
            return true;
        }
        *slot = notebook;
        self.reselect(&id);
        self.mark_changed();
        true
    }

    /// The id and folder of each notebook read from disk.
    pub fn notebook_roots(&self) -> impl Iterator<Item = (&str, &std::path::Path)> {
        self.notebook_roots
            .iter()
            .map(|(id, path)| (id.as_str(), path.as_path()))
    }
}

#[cfg(test)]
mod tests {
    use notecraft_model::{PageRef, Section};

    use super::*;

    fn notebook(title: &str) -> Notebook {
        Notebook {
            id: "nb".to_owned(),
            name: "NB".to_owned(),
            color: 1,
            sections: vec![SectionNode::Section(Section {
                id: "nb/S".to_owned(),
                name: "S".to_owned(),
                color: 2,
                pages: vec![PageRef::new("nb/S/A.md", title, 0)],
            })],
        }
    }

    fn title_of(session: &Session) -> String {
        session.notebooks()[0]
            .all_sections()
            .first()
            .and_then(|s| s.pages.first())
            .map(|p| p.title.clone())
            .unwrap_or_default()
    }

    #[test]
    fn titles_replace_page_titles() {
        let mut nb = notebook("A");
        let titles = BTreeMap::from([("nb/S/A.md".to_owned(), "Fancy".to_owned())]);
        apply_titles(&mut nb, &titles);
        assert_eq!(nb.all_sections()[0].pages[0].title, "Fancy");
    }

    #[test]
    fn a_current_result_replaces_the_tree_and_a_stale_one_does_not() {
        let mut session = Session::new();
        session.open_notebook(notebook("A"));
        let generation = session.tree_generation("nb");
        let titles = BTreeMap::from([("nb/S/A.md".to_owned(), "Fancy".to_owned())]);
        session.bump_tree_generation("nb");
        assert!(!session.apply_index_refresh(generation, notebook("Fancy"), titles.clone()));
        assert_eq!(title_of(&session), "A");
        let generation = session.tree_generation("nb");
        assert!(session.apply_index_refresh(generation, notebook("Fancy"), titles));
        assert_eq!(title_of(&session), "Fancy");
    }
}
