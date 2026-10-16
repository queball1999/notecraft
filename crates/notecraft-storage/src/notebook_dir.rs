//! Scan a notebook folder into a tree of groups, sections, and pages
//! (docs/markdown.md section 2).
//!
//! The scan only reads directory listings. It never opens a page file, never follows
//! symbolic links, and never fails on odd content: anything it cannot use is skipped and
//! reported as a [`ScanWarning`]. It is bounded by [`ScanLimits`] so a hostile or huge
//! folder cannot exhaust memory or the stack.
//!
//! The tree uses its own plain types because this crate sits below the model crate
//! (AGENTS.md section 5). The engine converts a [`NotebookTree`] into model types.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::notebook_toml::{read_notebook_config, NotebookConfig, ReadOutcome};
use crate::section_toml::{read_section_config, SectionConfig, SectionKind, SECTION_FILE_NAME};

/// The folder inside a notebook that holds NoteCraft's own data (docs/markdown.md 2).
pub const NOTEBOOK_DIR_NAME: &str = ".notecraft";
/// The notebook settings file inside [`NOTEBOOK_DIR_NAME`].
pub const NOTEBOOK_FILE_NAME: &str = "notebook.toml";
/// The attachments folder inside a section. It is never a section itself.
pub const ATTACHMENTS_DIR_NAME: &str = "attachments";
/// Page file extensions, lowercase (docs/markdown.md section 2.3).
pub const PAGE_EXTENSIONS: [&str; 2] = ["md", "notecraft"];

/// Folder depth limit: folders deeper than this below the notebook are ignored.
pub const MAX_DEPTH: usize = 8;
/// A notebook with more directory entries than this gets a warning.
pub const FILE_COUNT_WARNING: usize = 50_000;
/// The scan stops after this many directory entries, so a runaway folder stays bounded.
pub const MAX_ENTRIES: usize = 500_000;

/// The bounds a scan obeys. The defaults are the limits above; tests use smaller ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanLimits {
    pub max_depth: usize,
    pub warn_entries: usize,
    pub max_entries: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_depth: MAX_DEPTH,
            warn_entries: FILE_COUNT_WARNING,
            max_entries: MAX_ENTRIES,
        }
    }
}

/// Which kind of file a page is (docs/markdown.md section 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    /// A `.md` page: text only.
    Markdown,
    /// A `.notecraft` page: markdown plus canvas data.
    Canvas,
}

/// One page file in a section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageEntry {
    /// Path from the notebook folder, with `/` separators.
    pub rel_path: String,
    pub file_name: String,
    /// The file name without its extension. Page content is read in a later PR, so the
    /// frontmatter title does not show yet.
    pub title: String,
    /// 0 for a page, 1 for a subpage, 2 for a sub-subpage.
    pub level: u8,
    pub kind: PageKind,
}

/// A folder of pages.
#[derive(Clone, Debug, PartialEq)]
pub struct SectionEntry {
    /// Path from the notebook folder, with `/` separators.
    pub rel_path: String,
    /// The folder name, which is the section name.
    pub name: String,
    /// Tab color as 0xRRGGBB, when `.section.toml` sets one.
    pub color: Option<u32>,
    pub pages: Vec<PageEntry>,
    /// The folder's `.section.toml` as read (defaults when missing or bad).
    pub config: SectionConfig,
}

/// A folder of sections and further groups.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupEntry {
    pub rel_path: String,
    pub name: String,
    pub color: Option<u32>,
    pub children: Vec<TreeNode>,
    /// The folder's `.section.toml` as read (defaults when missing or bad).
    pub config: SectionConfig,
}

/// One entry in a notebook's or group's list.
#[derive(Clone, Debug, PartialEq)]
pub enum TreeNode {
    Section(SectionEntry),
    Group(GroupEntry),
}

impl TreeNode {
    pub fn name(&self) -> &str {
        match self {
            Self::Section(section) => &section.name,
            Self::Group(group) => &group.name,
        }
    }
}

/// Why something was skipped or cut short. The notebook still opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WarningKind {
    /// A folder deeper than the depth limit was ignored.
    TooDeep,
    /// The notebook has more entries than [`ScanLimits::warn_entries`].
    ManyFiles,
    /// The scan stopped at [`ScanLimits::max_entries`]; the rest was not read.
    ScanStopped,
    /// A folder could not be read.
    Unreadable,
    /// A file or folder name is not valid UTF-8 and was skipped.
    BadName,
    /// A configuration file could not be used; defaults apply.
    BadConfig,
    /// Two page files in one section share a base name, so links to it are ambiguous.
    DuplicatePage,
}

/// One problem found while scanning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanWarning {
    /// Path from the notebook folder with `/` separators; empty for the notebook itself.
    pub path: String,
    pub kind: WarningKind,
}

impl fmt::Display for ScanWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let place = if self.path.is_empty() {
            "the notebook"
        } else {
            &self.path
        };
        match self.kind {
            WarningKind::TooDeep => {
                write!(
                    f,
                    "{place}: folders nested deeper than {MAX_DEPTH} levels are ignored"
                )
            }
            WarningKind::ManyFiles => write!(
                f,
                "{place}: more than {FILE_COUNT_WARNING} files; opening may be slow"
            ),
            WarningKind::ScanStopped => write!(f, "{place}: too many files; the rest was not read"),
            WarningKind::Unreadable => write!(f, "{place}: could not be read"),
            WarningKind::BadName => write!(f, "{place}: name is not valid text and was skipped"),
            WarningKind::BadConfig => {
                write!(f, "{place}: settings file could not be read; defaults used")
            }
            WarningKind::DuplicatePage => {
                write!(f, "{place}: another page in this section has the same name")
            }
        }
    }
}

/// Why a folder could not be opened as a notebook at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanError {
    NotADirectory(PathBuf),
    Io { path: PathBuf, message: String },
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotADirectory(path) => write!(f, "{} is not a folder", path.display()),
            Self::Io { path, message } => write!(f, "cannot read {}: {message}", path.display()),
        }
    }
}

impl std::error::Error for ScanError {}

/// A notebook folder read into a tree.
#[derive(Clone, Debug, PartialEq)]
pub struct NotebookTree {
    pub root: PathBuf,
    /// Stable id: from `notebook.toml` when it has one, else derived from the folder path.
    pub id: String,
    /// Display name: from `notebook.toml`, else the folder name.
    pub name: String,
    /// Notebook color as 0xRRGGBB, when `notebook.toml` sets one.
    pub color: Option<u32>,
    pub nodes: Vec<TreeNode>,
    /// `.notecraft/notebook.toml` as read (defaults when missing or bad).
    pub config: NotebookConfig,
    pub warnings: Vec<ScanWarning>,
    /// Directory entries seen, counting the ones that are not pages.
    pub entry_count: usize,
}

impl NotebookTree {
    /// Every section, depth first through groups.
    pub fn sections(&self) -> Vec<&SectionEntry> {
        let mut out = Vec::new();
        collect_sections(&self.nodes, &mut out);
        out
    }

    pub fn page_count(&self) -> usize {
        self.sections().iter().map(|s| s.pages.len()).sum()
    }
}

fn collect_sections<'a>(nodes: &'a [TreeNode], out: &mut Vec<&'a SectionEntry>) {
    for node in nodes {
        match node {
            TreeNode::Section(section) => out.push(section),
            TreeNode::Group(group) => collect_sections(&group.children, out),
        }
    }
}

/// Scan `root` with the default limits.
pub fn scan_notebook(root: &Path) -> Result<NotebookTree, ScanError> {
    scan_notebook_with(root, ScanLimits::default())
}

/// Scan `root` with explicit limits.
pub fn scan_notebook_with(root: &Path, limits: ScanLimits) -> Result<NotebookTree, ScanError> {
    let meta = fs::metadata(root).map_err(|err| ScanError::Io {
        path: root.to_path_buf(),
        message: err.to_string(),
    })?;
    if !meta.is_dir() {
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }
    let mut scanner = Scanner {
        limits,
        entries: 0,
        warned_many: false,
        stopped: false,
        warnings: Vec::new(),
    };
    let listing = scanner.list(root, "").ok_or_else(|| ScanError::Io {
        path: root.to_path_buf(),
        message: "the folder could not be listed".to_owned(),
    })?;
    let (config, outcome) =
        read_notebook_config(&root.join(NOTEBOOK_DIR_NAME).join(NOTEBOOK_FILE_NAME));
    if outcome == ReadOutcome::Bad {
        scanner.warn(
            &format!("{NOTEBOOK_DIR_NAME}/{NOTEBOOK_FILE_NAME}"),
            WarningKind::BadConfig,
        );
    }
    let nodes = scanner.child_nodes(root, "", &listing.dirs, &config.sections, 1);
    Ok(NotebookTree {
        root: root.to_path_buf(),
        id: config.id.clone().unwrap_or_else(|| derived_id(root)),
        name: config.name.clone().unwrap_or_else(|| folder_name(root)),
        color: config.color,
        config,
        nodes,
        warnings: scanner.warnings,
        entry_count: scanner.entries,
    })
}

/// The notebook id used when `notebook.toml` has none: `dir-` and 16 hex digits of an
/// FNV-1a hash of the folder path, so it is the same on every open of the same path.
pub fn derived_id(root: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in root.to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("dir-{hash:016x}")
}

fn folder_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Notebook".to_owned())
}

/// What one folder holds, by name, in directory order.
struct Listing {
    dirs: Vec<String>,
    files: Vec<String>,
}

struct Scanner {
    limits: ScanLimits,
    entries: usize,
    warned_many: bool,
    stopped: bool,
    warnings: Vec<ScanWarning>,
}

impl Scanner {
    fn warn(&mut self, path: &str, kind: WarningKind) {
        self.warnings.push(ScanWarning {
            path: path.to_owned(),
            kind,
        });
    }

    /// List `dir`. Hidden names (leading dot) are skipped; symbolic links are skipped so a
    /// link cannot make the tree loop or leave the notebook.
    fn list(&mut self, dir: &Path, rel: &str) -> Option<Listing> {
        let read = match fs::read_dir(dir) {
            Ok(read) => read,
            Err(err) => {
                tracing::warn!(path = %dir.display(), %err, "cannot read folder");
                self.warn(rel, WarningKind::Unreadable);
                return None;
            }
        };
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        for entry in read {
            if self.stopped {
                break;
            }
            let Ok(entry) = entry else { continue };
            self.entries += 1;
            if self.entries > self.limits.max_entries {
                self.stopped = true;
                self.warn(rel, WarningKind::ScanStopped);
                break;
            }
            if self.entries > self.limits.warn_entries && !self.warned_many {
                self.warned_many = true;
                self.warn("", WarningKind::ManyFiles);
            }
            let Ok(name) = entry.file_name().into_string() else {
                self.warn(&join(rel, "?"), WarningKind::BadName);
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                dirs.push(name);
            } else if kind.is_file() {
                files.push(name);
            }
        }
        Some(Listing { dirs, files })
    }

    /// The sections and groups for the folders `dirs` of `parent`, ordered with `listed`
    /// first. `depth` is the depth of those folders (1 for top-level ones).
    fn child_nodes(
        &mut self,
        parent: &Path,
        rel: &str,
        dirs: &[String],
        listed: &[String],
        depth: usize,
    ) -> Vec<TreeNode> {
        let names: Vec<&String> = dirs
            .iter()
            .filter(|name| !is_attachments(name.as_str()))
            .collect();
        let ordered = order_names(names.iter().map(|s| s.as_str()), listed);
        let mut nodes = Vec::new();
        for name in ordered {
            let child_rel = join(rel, &name);
            if depth > self.limits.max_depth {
                self.warn(&child_rel, WarningKind::TooDeep);
                continue;
            }
            if self.stopped {
                break;
            }
            if let Some(node) = self.folder(&parent.join(&name), &child_rel, &name, depth) {
                nodes.push(node);
            }
        }
        nodes
    }

    /// One folder as a section or a group.
    fn folder(&mut self, dir: &Path, rel: &str, name: &str, depth: usize) -> Option<TreeNode> {
        let listing = self.list(dir, rel)?;
        let (config, outcome) = read_section_config(&dir.join(SECTION_FILE_NAME));
        if outcome == ReadOutcome::Bad {
            self.warn(&join(rel, SECTION_FILE_NAME), WarningKind::BadConfig);
        }
        let pages = page_names(&listing.files);
        let has_subfolders = listing.dirs.iter().any(|d| !is_attachments(d));
        let is_group = match config.kind {
            Some(SectionKind::Group) => true,
            Some(SectionKind::Section) => false,
            None => pages.is_empty() && has_subfolders,
        };
        if is_group {
            let children = self.child_nodes(dir, rel, &listing.dirs, &config.sections, depth + 1);
            return Some(TreeNode::Group(GroupEntry {
                rel_path: rel.to_owned(),
                name: name.to_owned(),
                color: config.color,
                children,
                config,
            }));
        }
        let ordered = order_names(
            pages.iter().map(String::as_str),
            &config
                .pages
                .iter()
                .map(|p| p.file.clone())
                .collect::<Vec<_>>(),
        );
        let mut seen = BTreeSet::new();
        let mut entries = Vec::new();
        for file in ordered {
            // The first listing of a file decides its level.
            let level = config
                .pages
                .iter()
                .find(|listing| listing.file == file)
                .map_or(0, |listing| listing.level);
            let Some(entry) = page_entry(rel, &file, level) else {
                continue;
            };
            if !seen.insert(entry.title.to_lowercase()) {
                self.warn(&entry.rel_path, WarningKind::DuplicatePage);
            }
            entries.push(entry);
        }
        Some(TreeNode::Section(SectionEntry {
            rel_path: rel.to_owned(),
            name: name.to_owned(),
            color: config.color,
            pages: entries,
            config,
        }))
    }
}

fn is_attachments(name: &str) -> bool {
    name.eq_ignore_ascii_case(ATTACHMENTS_DIR_NAME)
}

fn join(rel: &str, name: &str) -> String {
    if rel.is_empty() {
        name.to_owned()
    } else {
        format!("{rel}/{name}")
    }
}

/// The page file names in `files` (`.md` or `.notecraft`, any case, non-empty base name).
fn page_names(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|name| page_kind(name).is_some())
        .cloned()
        .collect()
}

/// The page kind of file `name`, or `None` when it is not a page file.
pub fn page_kind(name: &str) -> Option<PageKind> {
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    if ext.eq_ignore_ascii_case("md") {
        Some(PageKind::Markdown)
    } else if ext.eq_ignore_ascii_case("notecraft") {
        Some(PageKind::Canvas)
    } else {
        None
    }
}

fn page_entry(section_rel: &str, file: &str, level: u8) -> Option<PageEntry> {
    let kind = page_kind(file)?;
    let (stem, _) = file.rsplit_once('.')?;
    Some(PageEntry {
        rel_path: join(section_rel, file),
        file_name: file.to_owned(),
        title: stem.to_owned(),
        level: level.min(MAX_PAGE_LEVEL),
        kind,
    })
}

/// Deepest subpage level: a page, a subpage, and a sub-subpage (as in OneNote).
pub const MAX_PAGE_LEVEL: u8 = 2;

/// `names` ordered with the ones in `listed` first, in list order (a name listed twice
/// counts once; names in the list that do not exist are dropped), then the rest sorted by
/// name, ignoring case first.
pub fn order_names<'a>(names: impl IntoIterator<Item = &'a str>, listed: &[String]) -> Vec<String> {
    let mut rest: Vec<&str> = names.into_iter().collect();
    let mut out: Vec<String> = Vec::with_capacity(rest.len());
    for want in listed {
        if let Some(pos) = rest.iter().position(|name| *name == want.as_str()) {
            out.push(rest.remove(pos).to_owned());
        }
    }
    rest.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    out.extend(rest.into_iter().map(str::to_owned));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(root: &Path, rel: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "x").unwrap();
    }

    fn names(nodes: &[TreeNode]) -> Vec<&str> {
        nodes.iter().map(TreeNode::name).collect()
    }

    fn titles(section: &SectionEntry) -> Vec<&str> {
        section.pages.iter().map(|p| p.title.as_str()).collect()
    }

    #[test]
    fn scans_sections_groups_and_pages() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        touch(root, "Work/b.md");
        touch(root, "Work/A.notecraft");
        touch(root, "Work/notes.txt");
        touch(root, "Work/attachments/pic.png");
        touch(root, "Projects/Alpha/one.md");
        touch(root, "Projects/Beta/two.md");
        touch(root, ".notecraft/index.sqlite");
        touch(root, "stray.md");
        let tree = scan_notebook(root).unwrap();
        assert_eq!(names(&tree.nodes), ["Projects", "Work"]);
        let TreeNode::Group(group) = &tree.nodes[0] else {
            panic!("Projects is a group");
        };
        assert_eq!(names(&group.children), ["Alpha", "Beta"]);
        let TreeNode::Section(work) = &tree.nodes[1] else {
            panic!("Work is a section");
        };
        assert_eq!(titles(work), ["A", "b"]);
        assert_eq!(work.pages[0].kind, PageKind::Canvas);
        assert_eq!(work.pages[1].kind, PageKind::Markdown);
        assert_eq!(work.pages[0].rel_path, "Work/A.notecraft");
        assert_eq!(tree.sections().len(), 3);
        assert_eq!(tree.page_count(), 4);
        assert!(tree.warnings.is_empty(), "{:?}", tree.warnings);
    }

    #[test]
    fn empty_folder_is_an_empty_section_and_hidden_names_are_skipped() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("Empty")).unwrap();
        touch(dir.path(), "Empty/.hidden.md");
        touch(dir.path(), "Empty/.md");
        let tree = scan_notebook(dir.path()).unwrap();
        let TreeNode::Section(section) = &tree.nodes[0] else {
            panic!("an empty folder is a section");
        };
        assert!(section.pages.is_empty());
    }

    #[test]
    fn depth_is_limited() {
        let dir = TempDir::new().unwrap();
        // A section eight folders deep is kept; anything below it is ignored.
        touch(dir.path(), "a/b/c/d/e/f/g/h/page.md");
        touch(dir.path(), "1/2/3/4/5/6/7/8/9/deep.md");
        let tree = scan_notebook(dir.path()).unwrap();
        let sections = tree.sections();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].rel_path, "a/b/c/d/e/f/g/h");
        assert!(tree
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::TooDeep && w.path == "1/2/3/4/5/6/7/8/9"));
    }

    #[test]
    fn many_files_warn_and_huge_folders_stop() {
        let dir = TempDir::new().unwrap();
        for i in 0..20 {
            touch(dir.path(), &format!("S/p{i:02}.md"));
        }
        let limits = ScanLimits {
            warn_entries: 5,
            max_entries: 1000,
            ..ScanLimits::default()
        };
        let tree = scan_notebook_with(dir.path(), limits).unwrap();
        assert_eq!(tree.page_count(), 20);
        assert!(tree
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::ManyFiles));
        assert_eq!(
            tree.warnings
                .iter()
                .filter(|w| w.kind == WarningKind::ManyFiles)
                .count(),
            1
        );

        let limits = ScanLimits {
            max_entries: 10,
            ..ScanLimits::default()
        };
        let tree = scan_notebook_with(dir.path(), limits).unwrap();
        assert!(tree.page_count() < 20);
        assert!(tree
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::ScanStopped));
    }

    #[test]
    fn duplicate_base_names_warn() {
        let dir = TempDir::new().unwrap();
        touch(dir.path(), "S/Plan.md");
        touch(dir.path(), "S/Plan.notecraft");
        let tree = scan_notebook(dir.path()).unwrap();
        assert_eq!(tree.page_count(), 2);
        assert!(tree
            .warnings
            .iter()
            .any(|w| w.kind == WarningKind::DuplicatePage));
    }

    #[test]
    fn missing_and_non_folder_roots_are_errors() {
        let dir = TempDir::new().unwrap();
        assert!(matches!(
            scan_notebook(&dir.path().join("nope")),
            Err(ScanError::Io { .. })
        ));
        touch(dir.path(), "file.md");
        assert!(matches!(
            scan_notebook(&dir.path().join("file.md")),
            Err(ScanError::NotADirectory(_))
        ));
    }

    #[test]
    fn ids_are_stable_for_a_path() {
        let a = derived_id(Path::new("/notes/Work"));
        assert_eq!(a, derived_id(Path::new("/notes/Work")));
        assert_ne!(a, derived_id(Path::new("/notes/Home")));
        assert!(a.starts_with("dir-") && a.len() == 20);
    }

    #[test]
    fn order_puts_listed_names_first() {
        let listed = vec![
            "c".to_owned(),
            "gone".to_owned(),
            "a".to_owned(),
            "c".to_owned(),
        ];
        assert_eq!(
            order_names(["b", "a", "c", "B", "d"], &listed),
            ["c", "a", "B", "b", "d"]
        );
        assert_eq!(order_names(["b", "A", "c"], &[]), ["A", "b", "c"]);
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    #[test]
    fn notebook_toml_sets_identity_and_section_order() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        write(
            root,
            ".notecraft/notebook.toml",
            "id = \"nb_1\"\nname = \"Home\"\ncolor = \"#102030\"\nsections = [\"Zed\", \"Mid\"]\nextra = 1\n",
        );
        touch(root, "Alpha/a.md");
        touch(root, "Mid/m.md");
        touch(root, "Zed/z.md");
        let tree = scan_notebook(root).unwrap();
        assert_eq!(tree.id, "nb_1");
        assert_eq!(tree.name, "Home");
        assert_eq!(tree.color, Some(0x102030));
        assert_eq!(names(&tree.nodes), ["Zed", "Mid", "Alpha"]);
        assert!(tree.config.extra.contains_key("extra"));
    }

    #[test]
    fn missing_notebook_toml_uses_the_folder() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().join("My Notes");
        touch(&root, "S/a.md");
        let tree = scan_notebook(&root).unwrap();
        assert_eq!(tree.name, "My Notes");
        assert_eq!(tree.id, derived_id(&root));
        assert_eq!(tree.color, None);
        assert!(tree.warnings.is_empty());
    }

    #[test]
    fn section_toml_orders_pages_and_sets_levels() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        write(
            root,
            "Work/.section.toml",
            "color = \"#3C8DBC\"\npages = [\n { file = \"c.md\", level = 1 },\n { file = \"gone.md\", level = 1 },\n { file = \"a.notecraft\" },\n { file = \"c.md\", level = 2 },\n]\n",
        );
        touch(root, "Work/a.notecraft");
        touch(root, "Work/b.md");
        touch(root, "Work/c.md");
        touch(root, "Work/d.md");
        let tree = scan_notebook(root).unwrap();
        let TreeNode::Section(work) = &tree.nodes[0] else {
            panic!("section");
        };
        assert_eq!(work.color, Some(0x3C8DBC));
        let got: Vec<(&str, u8)> = work
            .pages
            .iter()
            .map(|p| (p.title.as_str(), p.level))
            .collect();
        assert_eq!(got, [("c", 1), ("a", 0), ("b", 0), ("d", 0)]);
    }

    #[test]
    fn kind_in_section_toml_overrides_the_guess() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        // Has only folders, but says it is a section.
        write(root, "AsSection/.section.toml", "kind = \"section\"\n");
        touch(root, "AsSection/Inner/x.md");
        // Has a page and a folder, but says it is a group.
        write(
            root,
            "AsGroup/.section.toml",
            "kind = \"group\"\nsections = [\"Two\", \"One\"]\n",
        );
        touch(root, "AsGroup/stray.md");
        touch(root, "AsGroup/One/o.md");
        touch(root, "AsGroup/Two/t.md");
        let tree = scan_notebook(root).unwrap();
        let TreeNode::Group(group) = &tree.nodes[0] else {
            panic!("AsGroup is a group");
        };
        assert_eq!(group.name, "AsGroup");
        assert_eq!(names(&group.children), ["Two", "One"]);
        let TreeNode::Section(section) = &tree.nodes[1] else {
            panic!("AsSection is a section");
        };
        assert!(section.pages.is_empty());
    }

    #[test]
    fn bad_settings_files_warn_and_default() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        write(root, ".notecraft/notebook.toml", "= = broken");
        write(root, "S/.section.toml", "pages = [[");
        touch(root, "S/b.md");
        touch(root, "S/a.md");
        let tree = scan_notebook(root).unwrap();
        assert_eq!(tree.name, root.file_name().unwrap().to_string_lossy());
        assert_eq!(tree.page_count(), 2);
        let bad: Vec<&str> = tree
            .warnings
            .iter()
            .filter(|w| w.kind == WarningKind::BadConfig)
            .map(|w| w.path.as_str())
            .collect();
        assert_eq!(bad, [".notecraft/notebook.toml", "S/.section.toml"]);
    }

    #[test]
    fn hostile_names_in_settings_never_escape_the_listing() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        write(
            root,
            "S/.section.toml",
            "pages = [{ file = \"../../secret.md\" }, { file = \"/etc/passwd.md\" }]\n",
        );
        touch(root, "S/a.md");
        let tree = scan_notebook(root).unwrap();
        assert_eq!(tree.page_count(), 1);
    }

    #[test]
    fn page_kinds() {
        assert_eq!(page_kind("a.md"), Some(PageKind::Markdown));
        assert_eq!(page_kind("A.MD"), Some(PageKind::Markdown));
        assert_eq!(page_kind("a.NoteCraft"), Some(PageKind::Canvas));
        for bad in [".md", "md", "a.txt", "a.md.bak", "a."] {
            assert_eq!(page_kind(bad), None, "{bad}");
        }
    }
}
