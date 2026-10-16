//! Recently opened notebooks and pages, newest first, as the start page and File > Open list
//! them. The front end saves the list in the settings file and restores it at startup, so
//! everything restored is checked again here.

/// Longest recent list, as in OneNote's recent notebooks.
pub const MAX_RECENT: usize = 10;
/// Longest path kept, in characters. Longer paths are not real paths.
pub const MAX_PATH_CHARS: usize = 4096;

/// Whether a recent entry is a notebook folder or a page file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RecentKind {
    Notebook,
    Page,
}

impl RecentKind {
    pub const ALL: [Self; 2] = [Self::Notebook, Self::Page];

    /// The settings and command output value: `notebook` or `page`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Notebook => "notebook",
            Self::Page => "page",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.id() == id)
    }
}

/// One recent notebook or page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentItem {
    pub kind: RecentKind,
    pub path: String,
}

impl RecentItem {
    /// An entry for `path`, or `None` if the path is empty, too long, or has control
    /// characters. Surrounding spaces are trimmed.
    pub fn new(kind: RecentKind, path: &str) -> Option<Self> {
        clean_path(path).map(|path| Self { kind, path })
    }

    /// The last part of the path (the folder or file name), for display.
    pub fn name(&self) -> &str {
        let trimmed = self.path.trim_end_matches(['/', '\\']);
        trimmed
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.path)
    }
}

/// `path` trimmed, or `None` if it is empty, longer than [`MAX_PATH_CHARS`], or contains a
/// control character (including NUL).
pub fn clean_path(path: &str) -> Option<String> {
    let path = path.trim();
    let ok = !path.is_empty()
        && path.chars().count() <= MAX_PATH_CHARS
        && !path.chars().any(char::is_control);
    ok.then(|| path.to_owned())
}

/// The recent list: newest first, no duplicates, at most [`MAX_RECENT`] entries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recents {
    items: Vec<RecentItem>,
}

impl Recents {
    /// A list restored from settings. Bad entries are dropped, duplicates keep their first
    /// (newest) place, and the list is cut to [`MAX_RECENT`].
    pub fn from_items(items: impl IntoIterator<Item = (RecentKind, String)>) -> Self {
        let mut recents = Self::default();
        for (kind, path) in items {
            if recents.items.len() == MAX_RECENT {
                break;
            }
            if let Some(item) = RecentItem::new(kind, &path) {
                if !recents.items.contains(&item) {
                    recents.items.push(item);
                }
            }
        }
        recents
    }

    pub fn items(&self) -> &[RecentItem] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The newest recent notebook, if any.
    pub fn latest_notebook(&self) -> Option<&RecentItem> {
        self.items
            .iter()
            .find(|item| item.kind == RecentKind::Notebook)
    }

    /// Move `item` to the top, adding it if it is new.
    pub fn push(&mut self, item: RecentItem) {
        self.items.retain(|old| *old != item);
        self.items.insert(0, item);
        self.items.truncate(MAX_RECENT);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notebook(path: &str) -> RecentItem {
        RecentItem::new(RecentKind::Notebook, path).unwrap()
    }

    #[test]
    fn push_moves_to_the_top_without_duplicates() {
        let mut recents = Recents::default();
        recents.push(notebook("a"));
        recents.push(notebook("b"));
        recents.push(notebook("a"));
        let paths: Vec<&str> = recents.items().iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, ["a", "b"]);
        // The same path as a page is a different entry.
        recents.push(RecentItem::new(RecentKind::Page, "a").unwrap());
        assert_eq!(recents.items().len(), 3);
    }

    #[test]
    fn list_is_capped() {
        let mut recents = Recents::default();
        for i in 0..25 {
            recents.push(notebook(&format!("nb{i}")));
        }
        assert_eq!(recents.items().len(), MAX_RECENT);
        assert_eq!(
            recents.items().first().map(|i| i.path.as_str()),
            Some("nb24")
        );
        recents.clear();
        assert!(recents.is_empty());
    }

    #[test]
    fn bad_paths_are_rejected() {
        assert_eq!(clean_path("  C:\\Notes  ").as_deref(), Some("C:\\Notes"));
        assert_eq!(clean_path(""), None);
        assert_eq!(clean_path("   "), None);
        assert_eq!(clean_path("a\u{0}b"), None);
        assert_eq!(clean_path("a\nb"), None);
        assert_eq!(clean_path(&"x".repeat(MAX_PATH_CHARS + 1)), None);
        assert!(clean_path(&"x".repeat(MAX_PATH_CHARS)).is_some());
    }

    #[test]
    fn restore_drops_bad_and_duplicate_entries() {
        let mut saved = vec![
            (RecentKind::Notebook, "work".to_owned()),
            (RecentKind::Notebook, "".to_owned()),
            (RecentKind::Page, "notes.md".to_owned()),
            (RecentKind::Notebook, " work ".to_owned()),
        ];
        saved.extend((0..40).map(|i| (RecentKind::Page, format!("p{i}.md"))));
        let recents = Recents::from_items(saved);
        assert_eq!(recents.items().len(), MAX_RECENT);
        assert_eq!(recents.items().first(), Some(&notebook("work")));
        assert_eq!(recents.latest_notebook(), Some(&notebook("work")));
        assert_eq!(
            recents.items().get(1).map(|i| i.path.as_str()),
            Some("notes.md")
        );
    }

    #[test]
    fn names_are_the_last_path_part() {
        assert_eq!(notebook("C:\\Users\\q\\Work Notes").name(), "Work Notes");
        assert_eq!(notebook("/home/q/notes/").name(), "notes");
        assert_eq!(notebook("plain").name(), "plain");
        assert_eq!(notebook("/").name(), "/");
    }

    #[test]
    fn kind_ids_round_trip() {
        for kind in RecentKind::ALL {
            assert_eq!(RecentKind::from_id(kind.id()), Some(kind));
        }
        assert_eq!(RecentKind::from_id("folder"), None);
    }
}
