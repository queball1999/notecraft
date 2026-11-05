//! Resolving a page link target to a page (docs/markdown.md section 5.3).
//!
//! Pure functions over a list of the pages in one notebook: no file access, no index. The
//! caller (the engine for the canvas, the link index builder for `index.sqlite`) supplies the
//! pages, each with its path relative to the notebook folder and its frontmatter aliases.
//!
//! The order for `[[target]]`, written from the page at `from`:
//!
//! 1. a page whose path from the notebook root, without extension, equals `target`;
//! 2. a page whose path from the folder of `from` equals `target`;
//! 3. a page whose base name equals `target` exactly;
//! 4. the same, ignoring case and Unicode normalization (NFC);
//! 5. a page whose frontmatter `aliases` contains `target`.
//!
//! Steps 1 and 2 name one file. When step 3, 4, or 5 finds more than one page, the page
//! closest to `from` in the folder tree is chosen and the link is ambiguous; the other pages
//! stay in the candidate list. When nothing matches, the link is unresolved.

use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

/// Most candidates kept for an ambiguous link.
pub const MAX_CANDIDATES: usize = 32;

/// One page the resolver can point at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkPage {
    /// The page id (`<notebook id>/<path>` in the app).
    pub id: String,
    /// Path from the notebook folder, with `/` separators and the file extension.
    pub path: String,
    /// The `aliases` of the page's frontmatter.
    pub aliases: Vec<String>,
}

impl LinkPage {
    pub fn new(id: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            aliases: Vec::new(),
        }
    }

    /// This page with the given aliases.
    pub fn with_aliases<I, S>(mut self, aliases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.aliases = aliases.into_iter().map(Into::into).collect();
        self
    }

    /// The path without its extension.
    pub fn stem(&self) -> &str {
        strip_extension(&self.path)
    }

    /// The file name without its extension (the page's base name).
    pub fn base(&self) -> &str {
        base_of(self.stem())
    }
}

/// What a link target points at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved<'a> {
    /// Exactly one page.
    Page(&'a LinkPage),
    /// More than one page matched at step 3, 4, or 5. `chosen` is the closest to the linking
    /// page; `candidates` lists every match, closest first (so `chosen` is the first).
    Ambiguous {
        chosen: &'a LinkPage,
        candidates: Vec<&'a LinkPage>,
    },
    /// No page matches.
    Unresolved,
}

impl<'a> Resolved<'a> {
    /// The page a click goes to, if any.
    pub fn page(&self) -> Option<&'a LinkPage> {
        match self {
            Self::Page(page) => Some(page),
            Self::Ambiguous { chosen, .. } => Some(chosen),
            Self::Unresolved => None,
        }
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous { .. })
    }
}

/// The path without a final `.ext` (a leading dot is not an extension).
fn strip_extension(path: &str) -> &str {
    let name_start = path.rfind('/').map_or(0, |i| i + 1);
    let name = path.get(name_start..).unwrap_or(path);
    match name.rfind('.') {
        Some(dot) if dot > 0 => path.get(..name_start + dot).unwrap_or(path),
        _ => path,
    }
}

fn base_of(stem: &str) -> &str {
    stem.rsplit('/').next().unwrap_or(stem)
}

fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// The key for step 4: NFC, lower case, NFC again (lower casing can denormalize).
pub fn fold(text: &str) -> String {
    let lowered: String = text.nfc().collect::<String>().to_lowercase();
    lowered.nfc().collect()
}

/// Steps between two folders in the folder tree: up from one to the shared parent, then down.
fn folder_distance(a: &str, b: &str) -> usize {
    let a: Vec<&str> = a.split('/').filter(|s| !s.is_empty()).collect();
    let b: Vec<&str> = b.split('/').filter(|s| !s.is_empty()).collect();
    let shared = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    (a.len() - shared) + (b.len() - shared)
}

/// The pages of one notebook, indexed for resolving links.
#[derive(Clone, Debug, Default)]
pub struct LinkIndex {
    pages: Vec<LinkPage>,
    by_stem: HashMap<String, Vec<usize>>,
    by_base: HashMap<String, Vec<usize>>,
    by_folded: HashMap<String, Vec<usize>>,
    by_alias: HashMap<String, Vec<usize>>,
}

impl LinkIndex {
    pub fn new(pages: Vec<LinkPage>) -> Self {
        let mut index = Self::default();
        for (at, page) in pages.iter().enumerate() {
            index
                .by_stem
                .entry(page.stem().to_owned())
                .or_default()
                .push(at);
            index
                .by_base
                .entry(page.base().to_owned())
                .or_default()
                .push(at);
            // Step 4 matches the base name, the path, and the path from a folder alike, so
            // the key set holds both the base name and the whole path.
            let folded_base = fold(page.base());
            let folded_stem = fold(page.stem());
            if folded_stem != folded_base {
                index.by_folded.entry(folded_stem).or_default().push(at);
            }
            index.by_folded.entry(folded_base).or_default().push(at);
            for alias in &page.aliases {
                let alias = alias.trim();
                if !alias.is_empty() {
                    index.by_alias.entry(alias.to_owned()).or_default().push(at);
                }
            }
        }
        index.pages = pages;
        index
    }

    pub fn pages(&self) -> &[LinkPage] {
        &self.pages
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// The page with this id.
    pub fn page(&self, id: &str) -> Option<&LinkPage> {
        self.pages.iter().find(|p| p.id == id)
    }

    /// Where `target`, written on the page at path `from` (relative to the notebook folder),
    /// points. A `from` of `""` means "not in any page", for example when checking a link
    /// before it is placed.
    pub fn resolve(&self, target: &str, from: &str) -> Resolved<'_> {
        let target = target.trim();
        if target.is_empty() {
            return Resolved::Unresolved;
        }
        let section = folder_of(from);
        // Step 1: the path from the notebook root.
        let by_path = |key: &str| self.collect(self.by_stem.get(key));
        let hits = by_path(target);
        if !hits.is_empty() {
            return self.pick(hits, section);
        }
        // Step 2: the path from the folder of the linking page.
        if !section.is_empty() {
            let hits = by_path(&format!("{section}/{target}"));
            if !hits.is_empty() {
                return self.pick(hits, section);
            }
        }
        // Step 3: the base name, exactly.
        let hits = self.collect(self.by_base.get(target));
        if !hits.is_empty() {
            return self.pick(hits, section);
        }
        // Step 4: ignoring case and Unicode normalization. A path from a folder is matched
        // too, so `[[section/page]]` finds `Section/Page`.
        let folded = fold(target);
        let mut hits = self.collect(self.by_folded.get(&folded));
        if !section.is_empty() {
            let relative = fold(&format!("{section}/{target}"));
            for at in self.collect(self.by_folded.get(&relative)) {
                if !hits.contains(&at) {
                    hits.push(at);
                }
            }
        }
        if !hits.is_empty() {
            return self.pick(hits, section);
        }
        // Step 5: a frontmatter alias.
        let hits = self.collect(self.by_alias.get(target));
        if !hits.is_empty() {
            return self.pick(hits, section);
        }
        Resolved::Unresolved
    }

    fn collect(&self, found: Option<&Vec<usize>>) -> Vec<usize> {
        found.cloned().unwrap_or_default()
    }

    /// One page, or the closest of several.
    fn pick(&self, mut hits: Vec<usize>, section: &str) -> Resolved<'_> {
        hits.sort_unstable();
        hits.dedup();
        let mut pages: Vec<&LinkPage> = hits.iter().filter_map(|&at| self.pages.get(at)).collect();
        pages.sort_by(|a, b| {
            let near = |p: &LinkPage| folder_distance(section, folder_of(&p.path));
            near(a)
                .cmp(&near(b))
                .then_with(|| a.path.len().cmp(&b.path.len()))
                .then_with(|| a.path.cmp(&b.path))
        });
        pages.truncate(MAX_CANDIDATES);
        match pages.as_slice() {
            [] => Resolved::Unresolved,
            [only] => Resolved::Page(only),
            [first, ..] => Resolved::Ambiguous {
                chosen: first,
                candidates: pages.clone(),
            },
        }
    }

    /// The shortest text for a link from the page at `from` to `page` that resolves to that
    /// page and no other: the base name when that is enough, else the path from the
    /// notebook folder. (Docs/markdown.md section 5.2.)
    pub fn shortest_form(&self, page: &LinkPage, from: &str) -> String {
        let base = page.base();
        if matches!(self.resolve(base, from), Resolved::Page(found) if found.id == page.id) {
            return base.to_owned();
        }
        let stem = page.stem();
        if stem != base {
            // Relative to the folder of the linking page, when that is shorter.
            let section = folder_of(from);
            if !section.is_empty() {
                if let Some(rest) = stem
                    .strip_prefix(section)
                    .and_then(|rest| rest.strip_prefix('/'))
                {
                    if matches!(self.resolve(rest, from), Resolved::Page(found) if found.id == page.id)
                    {
                        return rest.to_owned();
                    }
                }
            }
        }
        stem.to_owned()
    }
}
