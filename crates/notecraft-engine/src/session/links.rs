//! Page links on the session: the pages a link can point to, resolving a link written on the
//! page shown, and the page picker (build_plan/04 PR 2.2, docs/markdown.md section 5).

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use notecraft_markdown::{fold_link_text, LinkIndex, LinkPage};
use notecraft_model::{Notebook, SectionNode};

use super::Session;
use crate::links::{
    link_pages, page_path, LinkPicker, LinkPickerView, LinkResolution, PickerEntry,
    MAX_PICKER_ENTRIES,
};

/// Deepest group nesting walked when listing pages for the picker.
const MAX_DEPTH: usize = 64;

impl Session {
    /// Remember the frontmatter aliases of the pages of notebook `notebook` (page id to
    /// aliases), replacing what was remembered. They come from the notebook index.
    pub fn remember_link_aliases(
        &mut self,
        notebook: &str,
        aliases: BTreeMap<String, Vec<String>>,
    ) {
        let prefix = format!("{notebook}/");
        let before = self.link_aliases.clone();
        self.link_aliases.retain(|id, _| !id.starts_with(&prefix));
        self.link_aliases.extend(aliases);
        if self.link_aliases != before {
            self.mark_changed();
        }
    }

    /// The pages of the notebook shown, as targets for page links.
    pub fn link_pages(&self) -> Vec<LinkPage> {
        self.current_notebook().map_or_else(Vec::new, |notebook| {
            link_pages(notebook, &|id| {
                self.link_aliases.get(id).cloned().unwrap_or_default()
            })
        })
    }

    /// The resolver for the notebook shown. Building it costs a pass over the page list, so
    /// the front end keeps it while [`Session::link_fingerprint`] stays the same.
    pub fn link_index(&self) -> LinkIndex {
        LinkIndex::new(self.link_pages())
    }

    /// A number that changes when the notebook shown gets or loses a page, a page is renamed
    /// or moved, or an alias changes: the times the answer of a link can change. Cheap, so
    /// the front end can ask every frame.
    pub fn link_fingerprint(&self) -> u64 {
        let Some(notebook) = self.current_notebook() else {
            return 0;
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        notebook.id.hash(&mut hasher);
        fingerprint_nodes(&notebook.sections, &self.link_aliases, 0, &mut hasher);
        hasher.finish()
    }

    /// The path from the notebook folder of the page shown (with its extension), the place
    /// links on it are resolved from.
    pub fn current_page_path(&self) -> Option<String> {
        let notebook = self.current_notebook()?;
        let page = self.selection().page.as_deref()?;
        page_path(&notebook.id, page).map(str::to_owned)
    }

    /// Where `target`, written on the page shown, points.
    pub fn resolve_page_link(&self, target: &str) -> LinkResolution {
        let from = self.current_page_path().unwrap_or_default();
        let index = self.link_index();
        LinkResolution::from_resolved(&index.resolve(target, &from))
    }

    /// The page picker, while one is open.
    pub fn link_picker(&self) -> Option<&LinkPicker> {
        self.link_picker.as_ref()
    }

    /// Open the picker for the caret in the page shown, where `at` is the byte offset just
    /// after a `[[`. Not a command: the commands call it after they validate.
    pub(crate) fn open_link_picker(&mut self, page: &str, at: usize) {
        self.link_picker = Some(LinkPicker {
            page: page.to_owned(),
            at,
            selected: 0,
        });
        self.mark_changed();
    }

    /// Close the picker. Returns whether one was open.
    pub fn close_link_picker(&mut self) -> bool {
        let was_open = self.link_picker.take().is_some();
        if was_open {
            self.mark_changed();
        }
        was_open
    }

    /// Move the highlighted picker entry by `delta`, wrapping around.
    pub(crate) fn move_link_picker(&mut self, delta: i64) {
        let Some(count) = self.link_picker_view().map(|view| view.entries.len()) else {
            return;
        };
        let Some(picker) = self.link_picker.as_mut() else {
            return;
        };
        if count == 0 {
            return;
        }
        let count = i64::try_from(count).unwrap_or(i64::MAX);
        let now = i64::try_from(picker.selected).unwrap_or(0);
        let next = (now + delta).rem_euclid(count);
        picker.selected = usize::try_from(next).unwrap_or(0);
        self.mark_changed();
    }

    /// The text typed after `[[` while the picker is open and still applies: the caret is in
    /// the same page, after the brackets, on the same line, and nothing that ends a link has
    /// been typed. `None` once any of that stops being true.
    pub fn link_picker_query(&self) -> Option<String> {
        let picker = self.link_picker.as_ref()?;
        if self.selection().page.as_deref() != Some(picker.page.as_str()) {
            return None;
        }
        let cursor = self.text_cursor()?;
        if cursor.anchor != cursor.head || cursor.head < picker.at {
            return None;
        }
        let text = self.page_text(&picker.page)?;
        if picker.at < 2 || text.get(picker.at - 2..picker.at) != Some("[[") {
            return None;
        }
        let typed = text.get(picker.at..cursor.head)?;
        if typed.contains(['\n', '\r', '|', '[', ']']) {
            return None;
        }
        Some(typed.to_owned())
    }

    /// What the picker lists now: the pages of the notebook shown that match the typed text,
    /// best matches first, at most [`MAX_PICKER_ENTRIES`]. `None` when no picker applies.
    pub fn link_picker_view(&self) -> Option<LinkPickerView> {
        let query = self.link_picker_query()?;
        let picker = self.link_picker.as_ref()?;
        let notebook = self.current_notebook()?;
        let wanted = fold_link_text(query.trim());
        let mut found: Vec<(u8, PickerEntry)> = Vec::new();
        let mut visit = |place: &str, id: &str, title: &str| {
            if id == picker.page {
                return;
            }
            let path = page_path(&notebook.id, id).unwrap_or(id);
            let rank = if wanted.is_empty() {
                Some(0)
            } else {
                let title_f = fold_link_text(title);
                if title_f.starts_with(&wanted) {
                    Some(0)
                } else if title_f.contains(&wanted) {
                    Some(1)
                } else if fold_link_text(path).contains(&wanted) {
                    Some(2)
                } else {
                    None
                }
            };
            if let Some(rank) = rank {
                found.push((
                    rank,
                    PickerEntry {
                        page: id.to_owned(),
                        title: title.to_owned(),
                        place: place.to_owned(),
                    },
                ));
            }
        };
        list_pages(notebook, &notebook.sections, 0, &mut visit);
        found.sort_by_key(|(rank, _)| *rank);
        found.truncate(MAX_PICKER_ENTRIES);
        let entries: Vec<PickerEntry> = found.into_iter().map(|(_, entry)| entry).collect();
        let selected = picker.selected.min(entries.len().saturating_sub(1));
        Some(LinkPickerView {
            query,
            entries,
            selected,
        })
    }
}

fn list_pages(
    notebook: &Notebook,
    nodes: &[SectionNode],
    depth: usize,
    visit: &mut dyn FnMut(&str, &str, &str),
) {
    if depth > MAX_DEPTH {
        return;
    }
    for node in nodes {
        match node {
            SectionNode::Section(section) => {
                for page in &section.pages {
                    visit(&section.name, &page.id, page.display_title());
                }
            }
            SectionNode::Group(group) => list_pages(notebook, &group.children, depth + 1, visit),
        }
    }
}

fn fingerprint_nodes(
    nodes: &[SectionNode],
    aliases: &BTreeMap<String, Vec<String>>,
    depth: usize,
    hasher: &mut impl Hasher,
) {
    if depth > MAX_DEPTH {
        return;
    }
    for node in nodes {
        match node {
            SectionNode::Section(section) => {
                for page in &section.pages {
                    page.id.hash(hasher);
                    if let Some(found) = aliases.get(&page.id) {
                        found.hash(hasher);
                    }
                }
            }
            SectionNode::Group(group) => {
                fingerprint_nodes(&group.children, aliases, depth + 1, hasher);
            }
        }
    }
}
