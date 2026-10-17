//! Structure editing: create, rename, move, and delete notebooks, section groups, sections, and
//! pages (docs/architecture.md section 5, build_plan/03 PR 1.13).
//!
//! Every edit builds one [`Transaction`] of [`FsOp`]s and commits it, so it is one undo step
//! that includes its effects on disk. The folder is the truth: after a step is applied, undone,
//! or redone, the notebook is read from disk again and replaces the tree in memory. Ids are
//! paths (`<notebook id>/<path>`), so a rename or move also re-keys everything stored per id
//! (page text, styles, ink, notices, the selection, and the history).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use notecraft_model::{Notebook, Section, SectionGroup, SectionNode};
use notecraft_storage::notebook_toml::read_notebook_config;
use notecraft_storage::paths::{page_file_name, sanitize_file_name, unique_file_name};
use notecraft_storage::section_toml::{read_section_config, PageListing, SectionKind};
use notecraft_storage::structure::{
    read_text, render_notebook_config, render_section_config, TRASH_DIR,
};

use super::{Selection, Session};
use crate::open::{open_folder, DEFAULT_SECTION_COLORS};
use crate::undo::{Content, FsOp, Op, Scope, Transaction};

/// Why a structure edit did not happen. Nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditError(pub String);

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for EditError {}

fn fail<T>(message: impl Into<String>) -> Result<T, EditError> {
    Err(EditError(message.into()))
}

/// Deepest nesting of folders below the notebook (docs/markdown.md section 2.2).
const MAX_NESTING: usize = 8;
/// Attachment links followed when a page moves to another section.
const MAX_ATTACHMENT_LINKS: usize = 256;
/// The default page name, as in OneNote.
const NEW_PAGE_TITLE: &str = "Untitled Page";

// ---------------------------------------------------------------------------------------
// Plumbing used by the file operations and by undo.

impl Session {
    /// The folder a notebook was read from, if it came from disk.
    pub fn notebook_root(&self, notebook: &str) -> Option<&Path> {
        self.notebook_roots.get(notebook).map(PathBuf::as_path)
    }

    /// The notebook must be read from disk again when the current step is over.
    pub(crate) fn mark_dirty(&mut self, notebook: &str) {
        self.dirty.insert(notebook.to_owned());
    }

    /// Read every notebook marked dirty from disk again. Called after a step is applied,
    /// rolled back, undone, or redone, so the tree in memory follows the files whatever
    /// happened half way.
    pub(crate) fn resync_dirty(&mut self) {
        let dirty = std::mem::take(&mut self.dirty);
        for id in dirty {
            self.resync_notebook(&id);
        }
    }

    fn resync_notebook(&mut self, id: &str) {
        let Some(root) = self.notebook_roots.get(id).cloned() else {
            return;
        };
        let opened = match open_folder(&root) {
            Ok(opened) if opened.notebook.id == id => opened,
            Ok(_) => {
                tracing::warn!(
                    notebook = id,
                    "notebook id changed on disk; tree not refreshed"
                );
                return;
            }
            Err(err) => {
                tracing::warn!(notebook = id, "cannot read notebook again: {err}");
                return;
            }
        };
        let Some(slot) = self.notebooks.iter_mut().find(|nb| nb.id == id) else {
            return;
        };
        *slot = opened.notebook;
        self.reselect(id);
    }

    /// Make the selection name things that exist in notebook `id`.
    fn reselect(&mut self, id: &str) {
        if self.selection.notebook.as_deref() != Some(id) {
            return;
        }
        let Some(notebook) = self.notebooks.iter().find(|nb| nb.id == id) else {
            return;
        };
        let section = self
            .selection
            .section
            .as_deref()
            .and_then(|s| notebook.section(s))
            .or_else(|| notebook.first_section());
        let page = self
            .selection
            .page
            .as_deref()
            .and_then(|p| section.and_then(|s| s.page(p)))
            .or_else(|| section.and_then(|s| s.pages.first()));
        self.selection = Selection {
            notebook: Some(id.to_owned()),
            section: section.map(|s| s.id.clone()),
            page: page.map(|p| p.id.clone()),
        };
    }

    /// Everything stored per page or section id follows `from` to `to` (both paths inside
    /// notebook `notebook`) when a file or folder is renamed or moved.
    pub(crate) fn rekey_ids(&mut self, notebook: &str, from: &str, to: &str) {
        let old = format!("{notebook}/{from}");
        let new = format!("{notebook}/{to}");
        let map = move |id: &str| -> Option<String> {
            if id == old {
                return Some(new.clone());
            }
            let rest = id.strip_prefix(&old)?.strip_prefix('/')?;
            Some(format!("{new}/{rest}"))
        };
        let rekey_map = |id: &mut Option<String>| {
            if let Some(new) = id.as_deref().and_then(&map) {
                *id = Some(new);
            }
        };
        rekey_map(&mut self.selection.section);
        rekey_map(&mut self.selection.page);
        rekey_keys(&mut self.page_sources, &map);
        rekey_keys(&mut self.page_styles, &map);
        rekey_keys(&mut self.page_notices, &map);
        rekey_keys(&mut self.ink.layers, &map);
        rekey_keys(&mut self.containers, &map);
        if let Some((page, _)) = self.text_cursor.as_mut() {
            if let Some(new) = map(page) {
                *page = new;
            }
        }
        self.history.rekey(&map);
    }

    /// Take a notebook out of the open list (undoable close). Its folder is not touched.
    pub(crate) fn unlist_notebook(&mut self, id: &str, index: usize) -> Result<(), String> {
        let _ = index;
        let Some(at) = self.notebooks.iter().position(|nb| nb.id == id) else {
            return Err("the notebook is not open".to_owned());
        };
        self.notebooks.remove(at);
        if self.selection.notebook.as_deref() == Some(id) {
            let next = self.notebooks.get(at).or_else(|| self.notebooks.first());
            let section = next.and_then(Notebook::first_section);
            self.selection = Selection {
                notebook: next.map(|nb| nb.id.clone()),
                section: section.map(|s| s.id.clone()),
                page: section.and_then(|s| s.pages.first()).map(|p| p.id.clone()),
            };
        }
        Ok(())
    }

    /// Put a closed notebook back at `index`, read from `root` again, and show it.
    pub(crate) fn relist_notebook(
        &mut self,
        id: &str,
        root: &Path,
        index: usize,
    ) -> Result<(), String> {
        if self.notebooks.iter().any(|nb| nb.id == id) {
            return Err("the notebook is already open".to_owned());
        }
        let opened = open_folder(root).map_err(|err| err.to_string())?;
        if opened.notebook.id != id {
            return Err("the folder holds a different notebook now".to_owned());
        }
        let at = index.min(self.notebooks.len());
        self.notebooks.insert(at, opened.notebook);
        self.notebook_roots
            .insert(id.to_owned(), root.to_path_buf());
        let section = self.notebooks.get(at).and_then(Notebook::first_section);
        self.selection = Selection {
            notebook: Some(id.to_owned()),
            section: section.map(|s| s.id.clone()),
            page: section.and_then(|s| s.pages.first()).map(|p| p.id.clone()),
        };
        Ok(())
    }
}

fn rekey_keys<V>(map: &mut BTreeMap<String, V>, rename: &dyn Fn(&str) -> Option<String>) {
    let moved: Vec<String> = map
        .keys()
        .filter(|k| rename(k).is_some())
        .cloned()
        .collect();
    for key in moved {
        if let (Some(value), Some(new)) = (map.remove(&key), rename(&key)) {
            map.insert(new, value);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Looking things up in the model.

fn base(id: &str) -> &str {
    id.rsplit_once('/').map_or(id, |(_, name)| name)
}

fn node_id(node: &SectionNode) -> &str {
    match node {
        SectionNode::Section(s) => &s.id,
        SectionNode::Group(g) => &g.id,
    }
}

/// Where a section or group sits.
struct Found<'a> {
    node: &'a SectionNode,
    /// The group holding it, `None` at the top level.
    parent: Option<&'a SectionGroup>,
    siblings: &'a [SectionNode],
    index: usize,
}

fn find_node<'a>(
    nodes: &'a [SectionNode],
    parent: Option<&'a SectionGroup>,
    id: &str,
    depth: usize,
) -> Option<Found<'a>> {
    if depth > MAX_NESTING + 8 {
        return None;
    }
    for (index, node) in nodes.iter().enumerate() {
        if node_id(node) == id {
            return Some(Found {
                node,
                parent,
                siblings: nodes,
                index,
            });
        }
        if let SectionNode::Group(group) = node {
            if let Some(found) = find_node(&group.children, Some(group), id, depth + 1) {
                return Some(found);
            }
        }
    }
    None
}

/// How many folder levels `node` and what is inside it span (1 for a lone section).
fn height(node: &SectionNode, depth: usize) -> usize {
    match node {
        SectionNode::Section(_) => 1,
        SectionNode::Group(_) if depth > MAX_NESTING + 8 => 1,
        SectionNode::Group(group) => {
            1 + group
                .children
                .iter()
                .map(|c| height(c, depth + 1))
                .max()
                .unwrap_or(0)
        }
    }
}

fn names_of(nodes: &[SectionNode]) -> Vec<String> {
    nodes.iter().map(|n| base(node_id(n)).to_owned()).collect()
}

fn listings(section: &Section) -> Vec<PageListing> {
    section
        .pages
        .iter()
        .map(|p| PageListing {
            file: base(&p.id).to_owned(),
            level: p.level,
        })
        .collect()
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

/// Names of everything in the folder, hidden entries included (for choosing a free name).
fn names_on_disk(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|read| {
            read.filter_map(|e| e.ok()?.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// `stem`, or `stem 2`, `stem 3`, ... for a folder name not in `taken` (ignoring case).
fn unique_dir_name(stem: &str, taken: &[String]) -> String {
    let is_taken = |name: &str| {
        taken.iter().any(|t| t.eq_ignore_ascii_case(name))
            || name.eq_ignore_ascii_case("attachments")
    };
    if !is_taken(stem) {
        return stem.to_owned();
    }
    for n in 2..=taken.len().saturating_add(3) {
        let candidate = format!("{stem} {n}");
        if !is_taken(&candidate) {
            return candidate;
        }
    }
    format!("{stem} {}", taken.len().saturating_add(4))
}

// ---------------------------------------------------------------------------------------
// Building a step.

/// The notebook an edit works in, with the ops collected so far.
struct Plan {
    notebook: Notebook,
    root: PathBuf,
    ops: Vec<Op>,
    slots: Vec<String>,
}

impl Plan {
    fn new(session: &Session, notebook: &str) -> Result<Self, EditError> {
        let Some(nb) = session.notebooks.iter().find(|nb| nb.id == notebook) else {
            return fail("that notebook is not open");
        };
        let Some(root) = session.notebook_roots.get(notebook) else {
            return fail("this is a sample notebook; open or create a notebook folder first");
        };
        Ok(Self {
            notebook: nb.clone(),
            root: root.clone(),
            ops: Vec::new(),
            slots: Vec::new(),
        })
    }

    /// The plan for the notebook that holds `id` (a page, section, or group id).
    fn owning(session: &Session, id: &str) -> Result<Self, EditError> {
        let owner = session.notebooks.iter().find(|nb| {
            id.strip_prefix(nb.id.as_str())
                .is_some_and(|r| r.starts_with('/'))
        });
        // Sample notebooks use short ids that do not start with the notebook id.
        let owner = owner.or_else(|| {
            session
                .notebooks
                .iter()
                .find(|nb| nb.section(id).is_some() || nb.section_of_page(id).is_some())
        });
        match owner {
            Some(nb) => Self::new(session, &nb.id),
            None => fail("nothing with that id is open"),
        }
    }

    fn id(&self) -> &str {
        &self.notebook.id
    }

    fn rel<'a>(&self, id: &'a str) -> &'a str {
        id.strip_prefix(self.id())
            .and_then(|r| r.strip_prefix('/'))
            .unwrap_or(id)
    }

    fn push(&mut self, op: FsOp) {
        self.ops.push(Op::Fs(op));
    }

    /// A free spot in the trash for `name`: `<trash>/<token>/<name>`.
    fn slot(&mut self, name: &str) -> (String, String) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        for n in 0u32.. {
            let dir = format!("{TRASH_DIR}/{secs}-{n}");
            if self.slots.contains(&dir) || self.root.join(&dir).exists() {
                continue;
            }
            self.slots.push(dir.clone());
            let slot = format!("{dir}/{name}");
            return (dir, slot);
        }
        let dir = format!("{TRASH_DIR}/{secs}");
        (dir.clone(), format!("{dir}/{name}"))
    }

    fn create(&mut self, rel: &str, content: Content) {
        let (_, slot) = self.slot(base(rel));
        let notebook = self.id().to_owned();
        self.push(FsOp::Create {
            notebook,
            rel: rel.to_owned(),
            content,
            slot,
        });
    }

    fn relocate(&mut self, from: &str, to: &str) {
        let notebook = self.id().to_owned();
        self.push(FsOp::Move {
            notebook,
            from: from.to_owned(),
            to: to.to_owned(),
        });
    }

    fn write(&mut self, rel: String, before: Option<String>, after: Option<String>) {
        let notebook = self.id().to_owned();
        self.push(FsOp::WriteText {
            notebook,
            rel,
            before,
            after,
        });
    }

    /// Rewrite the `.section.toml` of folder `dir` (a section or group).
    fn section_config(
        &mut self,
        dir: &str,
        edit: impl FnOnce(&mut notecraft_storage::SectionConfig),
    ) {
        let rel = join(dir, notecraft_storage::section_toml::SECTION_FILE_NAME);
        let path = self.root.join(&rel);
        let before = read_text(&path);
        let (mut config, _) = read_section_config(&path);
        edit(&mut config);
        let after = render_section_config(&config);
        self.write(rel, before, Some(after));
    }

    /// Rewrite `.notecraft/notebook.toml`, adding the id and name if it is new.
    fn notebook_config(&mut self, edit: impl FnOnce(&mut notecraft_storage::NotebookConfig)) {
        let rel = ".notecraft/notebook.toml".to_owned();
        let path = self.root.join(&rel);
        let before = read_text(&path);
        let (mut config, _) = read_notebook_config(&path);
        config.id.get_or_insert_with(|| self.notebook.id.clone());
        config
            .name
            .get_or_insert_with(|| self.notebook.name.clone());
        edit(&mut config);
        let after = render_notebook_config(&config);
        self.write(rel, before, Some(after));
    }

    /// Write the display order of the children of group `parent` (`None` for the notebook).
    fn child_order(&mut self, parent: Option<&str>, names: Vec<String>) {
        match parent {
            None => self.notebook_config(|c| c.sections = names),
            Some(dir) => self.section_config(dir, |c| {
                c.kind = Some(SectionKind::Group);
                c.sections = names;
            }),
        }
    }

    /// Write the page order and levels of section folder `dir`.
    fn page_order(&mut self, dir: &str, pages: Vec<PageListing>) {
        self.section_config(dir, |c| {
            c.kind.get_or_insert(SectionKind::Section);
            c.pages = pages;
        });
    }

    /// Move `from` into the trash with a note of where it came from, so it can be put back.
    fn trash(&mut self, from: &str, kind: &str, index: usize, level: u8) {
        let (dir, slot) = self.slot(base(from));
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let note = notecraft_storage::structure::render_trash_note(from, kind, index, level, secs);
        self.write(format!("{dir}/origin.toml"), None, Some(note));
        self.relocate(from, &slot);
    }
}

impl Session {
    fn commit_plan(&mut self, plan: Plan, label: &str) -> Result<(), EditError> {
        let tx = plan
            .ops
            .into_iter()
            .fold(Transaction::new(Scope::Notebook, label), Transaction::with);
        if self.commit(tx) {
            Ok(())
        } else {
            fail("the change could not be made; nothing was changed")
        }
    }

    fn page_parts(&self, page: &str) -> Result<(Plan, Section, usize), EditError> {
        let plan = Plan::owning(self, page)?;
        let Some(section) = plan.notebook.section_of_page(page) else {
            return fail("no page with that id");
        };
        let Some(index) = section.pages.iter().position(|p| p.id == page) else {
            return fail("no page with that id");
        };
        let section = section.clone();
        Ok((plan, section, index))
    }

    // ---- pages ----------------------------------------------------------------------

    /// Add a page titled `title` (`Untitled Page` when none) at the end of `section`, show it,
    /// and return its id. One undo step.
    pub fn create_page(&mut self, section: &str, title: Option<&str>) -> Result<String, EditError> {
        let mut plan = Plan::owning(self, section)?;
        let Some(target) = plan.notebook.section(section).cloned() else {
            return fail("no section with that id");
        };
        let dir = plan.rel(section).to_owned();
        let title = title.map(str::trim).filter(|t| !t.is_empty());
        let wanted = page_file_name(title.unwrap_or(NEW_PAGE_TITLE), "md");
        let stem = wanted.strip_suffix(".md").unwrap_or(&wanted);
        let taken = names_on_disk(&plan.root.join(&dir));
        let taken: Vec<&str> = taken.iter().map(String::as_str).collect();
        let file = unique_file_name(stem, "md", &taken);
        let rel = join(&dir, &file);
        plan.create(&rel, Content::Text(String::new()));
        let mut pages = listings(&target);
        pages.push(PageListing { file, level: 0 });
        plan.page_order(&dir, pages);
        let id = format!("{}/{rel}", plan.id());
        self.commit_plan(plan, "New page")?;
        self.select_page_id(&id);
        Ok(id)
    }

    /// Rename a page: its file is renamed to the new title (made safe for every system and
    /// numbered if the name is taken). Returns the new id. One undo step.
    pub fn rename_page(&mut self, page: &str, title: &str) -> Result<String, EditError> {
        let (mut plan, section, index) = self.page_parts(page)?;
        let dir = plan.rel(&section.id).to_owned();
        let old_file = base(page).to_owned();
        let ext = old_file
            .rsplit_once('.')
            .map_or("md", |(_, e)| e)
            .to_owned();
        let wanted = page_file_name(title, &ext);
        let stem = wanted
            .strip_suffix(&format!(".{ext}"))
            .unwrap_or(&wanted)
            .to_owned();
        if wanted == old_file {
            return Ok(page.to_owned());
        }
        let on_disk = names_on_disk(&plan.root.join(&dir));
        let taken: Vec<&str> = on_disk
            .iter()
            .map(String::as_str)
            .filter(|n| *n != old_file)
            .collect();
        let file = unique_file_name(&stem, &ext, &taken);
        if file == old_file {
            return Ok(page.to_owned());
        }
        plan.relocate(&join(&dir, &old_file), &join(&dir, &file));
        let mut pages = listings(&section);
        if let Some(entry) = pages.get_mut(index) {
            entry.file.clone_from(&file);
        }
        plan.page_order(&dir, pages);
        let id = format!("{}/{}", plan.id(), join(&dir, &file));
        self.commit_plan(plan, "Rename page")?;
        Ok(id)
    }

    /// Delete a page: its file moves to the notebook's trash folder (`.notecraft/trash`), from
    /// where Undo, and in Phase 3 the Recycle Bin, bring it back. One undo step.
    pub fn delete_page(&mut self, page: &str) -> Result<(), EditError> {
        let (mut plan, section, index) = self.page_parts(page)?;
        let dir = plan.rel(&section.id).to_owned();
        let level = section.pages.get(index).map_or(0, |p| p.level);
        let neighbor = section
            .pages
            .get(index + 1)
            .or_else(|| index.checked_sub(1).and_then(|i| section.pages.get(i)))
            .map(|p| p.id.clone());
        let from = plan.rel(page).to_owned();
        plan.trash(&from, "page", index, level);
        let mut pages = listings(&section);
        pages.remove(index);
        plan.page_order(&dir, pages);
        let was_shown = self.selection.page.as_deref() == Some(page);
        self.commit_plan(plan, "Delete page")?;
        if was_shown {
            self.selection.page = neighbor;
        }
        Ok(())
    }

    /// Move a page to `index` of `section` (counted with the page already taken out; past the
    /// end means last). Inside one section only the order changes; to another section the file
    /// moves with the attachments its text links to, which are copied. Returns the new id. One
    /// undo step. A sample (mock) notebook is changed in memory only.
    pub fn move_page_to(
        &mut self,
        page: &str,
        section: &str,
        index: usize,
    ) -> Result<String, EditError> {
        let has_disk = self
            .notebooks
            .iter()
            .any(|nb| page.starts_with(nb.id.as_str()) && self.notebook_roots.contains_key(&nb.id));
        if !has_disk {
            return if self.move_page(page, section, index) {
                Ok(page.to_owned())
            } else {
                fail("the page could not be moved there")
            };
        }
        let (mut plan, from, from_index) = self.page_parts(page)?;
        let Some(target) = plan.notebook.section(section).cloned() else {
            return fail("no section with that id");
        };
        if !target.id.starts_with(plan.id()) {
            return fail("a page cannot move to another notebook");
        }
        let from_dir = plan.rel(&from.id).to_owned();
        let to_dir = plan.rel(&target.id).to_owned();
        if from.id == target.id {
            let mut pages = listings(&from);
            let moved = pages.remove(from_index);
            pages.insert(index.min(pages.len()), moved);
            plan.page_order(&from_dir, pages);
            self.commit_plan(plan, "Move page")?;
            return Ok(page.to_owned());
        }
        let file = base(page).to_owned();
        let ext = file.rsplit_once('.').map_or("md", |(_, e)| e);
        let stem = file.strip_suffix(&format!(".{ext}")).unwrap_or(&file);
        let on_disk = names_on_disk(&plan.root.join(&to_dir));
        let taken: Vec<&str> = on_disk.iter().map(String::as_str).collect();
        let new_file = unique_file_name(stem, ext, &taken);
        // Attachments the page links to are copied along (other pages may still use them).
        let text = match self.page_text(page) {
            Some(text) => text.to_owned(),
            None => read_text_bounded(&plan.root.join(plan.rel(page))),
        };
        for name in attachment_links(&text) {
            let source = join(&join(&from_dir, "attachments"), &name);
            let copy = join(&join(&to_dir, "attachments"), &name);
            let exists_there = plan.root.join(&copy).exists();
            if plan.root.join(&source).is_file() && !exists_there {
                plan.create(&copy, Content::CopyOf(source));
            }
        }
        plan.relocate(&join(&from_dir, &file), &join(&to_dir, &new_file));
        let mut from_pages = listings(&from);
        from_pages.remove(from_index);
        plan.page_order(&from_dir, from_pages);
        let mut to_pages = listings(&target);
        to_pages.insert(
            index.min(to_pages.len()),
            PageListing {
                file: new_file.clone(),
                level: 0,
            },
        );
        plan.page_order(&to_dir, to_pages);
        let id = format!("{}/{}", plan.id(), join(&to_dir, &new_file));
        let was_shown = self.selection.page.as_deref() == Some(page);
        self.commit_plan(plan, "Move page")?;
        if was_shown {
            self.select_page_id(&id);
        }
        Ok(id)
    }

    /// Set how far a page is indented as a subpage (0 page, 1 subpage, 2 sub-subpage). The
    /// first page of a section stays at level 0 and a page is at most one level deeper than
    /// the page before it. Returns the level that was set. One undo step.
    pub fn set_page_level(&mut self, page: &str, level: u8) -> Result<u8, EditError> {
        let (mut plan, section, index) = self.page_parts(page)?;
        let before = index.checked_sub(1).and_then(|i| section.pages.get(i));
        let ceiling = before.map_or(0, |p| p.level + 1);
        let level = level
            .min(notecraft_model::page::MAX_PAGE_LEVEL)
            .min(ceiling);
        if section.pages.get(index).map(|p| p.level) == Some(level) {
            return Ok(level);
        }
        let dir = plan.rel(&section.id).to_owned();
        let mut pages = listings(&section);
        // Subpages below follow a parent that moves out: keep them below it.
        if let Some(entry) = pages.get_mut(index) {
            entry.level = level;
        }
        let mut limit = level + 1;
        for entry in pages.iter_mut().skip(index + 1) {
            entry.level = entry.level.min(limit);
            limit = entry.level + 1;
        }
        plan.page_order(&dir, pages);
        self.commit_plan(plan, "Change page level")?;
        Ok(level)
    }

    // ---- sections and groups ----------------------------------------------------------

    /// Add a section named `name` (`New Section` when none) to the group `parent`, or to the
    /// notebook `notebook` when `parent` is `None`, and show it. Returns its id. One undo step.
    pub fn create_section(
        &mut self,
        notebook: &str,
        parent: Option<&str>,
        name: Option<&str>,
    ) -> Result<String, EditError> {
        self.create_folder_node(notebook, parent, name, false)
    }

    /// Add a section group; see [`Self::create_section`].
    pub fn create_section_group(
        &mut self,
        notebook: &str,
        parent: Option<&str>,
        name: Option<&str>,
    ) -> Result<String, EditError> {
        self.create_folder_node(notebook, parent, name, true)
    }

    fn create_folder_node(
        &mut self,
        notebook: &str,
        parent: Option<&str>,
        name: Option<&str>,
        group: bool,
    ) -> Result<String, EditError> {
        let mut plan = Plan::new(self, notebook)?;
        let nb = plan.notebook.clone();
        let (children, parent_dir): (&[SectionNode], Option<String>) = match parent {
            None => (&nb.sections, None),
            Some(id) => {
                let found = find_node(&nb.sections, None, id, 0);
                match found.map(|f| f.node) {
                    Some(SectionNode::Group(g)) => (&g.children, Some(plan.rel(id).to_owned())),
                    Some(SectionNode::Section(_)) => return fail("a section cannot hold sections"),
                    None => return fail("no section group with that id"),
                }
            }
        };
        let depth = parent_dir.as_deref().map_or(0, |d| d.split('/').count());
        if depth + 1 > MAX_NESTING {
            return fail("section groups can be nested 8 levels deep at most");
        }
        let default_name = if group {
            "New Section Group"
        } else {
            "New Section"
        };
        let wanted = sanitize_file_name(
            name.map(str::trim)
                .filter(|n| !n.is_empty())
                .unwrap_or(default_name),
        );
        let parent_path = parent_dir.clone().unwrap_or_default();
        let mut taken = names_on_disk(&if parent_path.is_empty() {
            plan.root.clone()
        } else {
            plan.root.join(&parent_path)
        });
        taken.extend(names_of(children));
        let folder = unique_dir_name(&wanted, &taken);
        let rel = join(&parent_path, &folder);
        let mut names = names_of(children);
        names.push(folder);
        let color = DEFAULT_SECTION_COLORS
            .get(children.len() % DEFAULT_SECTION_COLORS.len())
            .copied()
            .unwrap_or(0x9FD3E8);
        let id = format!("{}/{rel}", plan.id());
        plan.create(&rel, Content::Dir);
        // A new section starts with one untitled page, as in OneNote, in the same step.
        let first_page = format!("{NEW_PAGE_TITLE}.md");
        plan.section_config(&rel, |c| {
            c.kind = Some(if group {
                SectionKind::Group
            } else {
                SectionKind::Section
            });
            c.color = Some(color);
            if !group {
                c.pages = vec![PageListing {
                    file: first_page.clone(),
                    level: 0,
                }];
            }
        });
        if !group {
            plan.create(&join(&rel, &first_page), Content::Text(String::new()));
        }
        plan.child_order(parent_dir.as_deref(), names);
        let label = if group {
            "New section group"
        } else {
            "New section"
        };
        self.commit_plan(plan, label)?;
        if !group {
            self.select_section_id(&id);
        }
        Ok(id)
    }

    /// Rename a section or section group (its folder is renamed). Returns the new id. One undo
    /// step.
    pub fn rename_section(&mut self, id: &str, name: &str) -> Result<String, EditError> {
        let mut plan = Plan::owning(self, id)?;
        let nb = plan.notebook.clone();
        let Some(found) = find_node(&nb.sections, None, id, 0) else {
            return fail("no section with that id");
        };
        let old = base(id).to_owned();
        let parent_rel = found.parent.map(|g| plan.rel(&g.id).to_owned());
        let wanted = sanitize_file_name(name);
        if wanted == old {
            return Ok(id.to_owned());
        }
        let parent_path = parent_rel.clone().unwrap_or_default();
        let mut taken = names_on_disk(&plan.root.join(&parent_path));
        taken.retain(|n| *n != old);
        let folder = unique_dir_name(&wanted, &taken);
        if folder == old {
            return Ok(id.to_owned());
        }
        let mut names = names_of(found.siblings);
        if let Some(slot) = names.get_mut(found.index) {
            slot.clone_from(&folder);
        }
        let from = plan.rel(id).to_owned();
        let to = join(&parent_path, &folder);
        plan.relocate(&from, &to);
        plan.child_order(parent_rel.as_deref(), names);
        let new_id = format!("{}/{to}", plan.id());
        self.commit_plan(plan, "Rename section")?;
        Ok(new_id)
    }

    /// Set a section's tab color. One undo step.
    pub fn set_section_color(&mut self, id: &str, color: u32) -> Result<(), EditError> {
        let mut plan = Plan::owning(self, id)?;
        let nb = plan.notebook.clone();
        let Some(found) = find_node(&nb.sections, None, id, 0) else {
            return fail("no section with that id");
        };
        let group = matches!(found.node, SectionNode::Group(_));
        let current = match found.node {
            SectionNode::Section(s) => s.color,
            SectionNode::Group(_) => u32::MAX,
        };
        if current == color & 0x00FF_FFFF {
            return Ok(());
        }
        let dir = plan.rel(id).to_owned();
        plan.section_config(&dir, |c| {
            if group {
                c.kind = Some(SectionKind::Group);
            } else {
                c.kind.get_or_insert(SectionKind::Section);
            }
            c.color = Some(color & 0x00FF_FFFF);
        });
        self.commit_plan(plan, "Section color")
    }

    /// Delete a section or section group: its folder moves to the trash folder. One undo step.
    pub fn delete_section(&mut self, id: &str) -> Result<(), EditError> {
        let mut plan = Plan::owning(self, id)?;
        let nb = plan.notebook.clone();
        let Some(found) = find_node(&nb.sections, None, id, 0) else {
            return fail("no section with that id");
        };
        let parent_rel = found.parent.map(|g| plan.rel(&g.id).to_owned());
        let mut names = names_of(found.siblings);
        let index = found.index;
        names.remove(index);
        let kind = if matches!(found.node, SectionNode::Group(_)) {
            "group"
        } else {
            "section"
        };
        let from = plan.rel(id).to_owned();
        let neighbor = self.neighbor_section(&nb, id);
        plan.trash(&from, kind, index, 0);
        plan.child_order(parent_rel.as_deref(), names);
        let shown = self
            .selection
            .section
            .as_deref()
            .is_some_and(|s| s == id || s.strip_prefix(id).is_some_and(|r| r.starts_with('/')));
        self.commit_plan(plan, "Delete section")?;
        if shown {
            self.selection.section = neighbor.clone();
            self.selection.page = None;
            self.reselect_current();
        }
        Ok(())
    }

    /// The section to show instead once section or group `id` is gone.
    fn neighbor_section(&self, notebook: &Notebook, id: &str) -> Option<String> {
        let inside = |s: &str| s == id || s.strip_prefix(id).is_some_and(|r| r.starts_with('/'));
        let all: Vec<&str> = notebook
            .all_sections()
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        let first_in = all.iter().position(|s| inside(s))?;
        all.iter()
            .skip(first_in)
            .find(|s| !inside(s))
            .or_else(|| all.iter().take(first_in).rev().find(|s| !inside(s)))
            .map(|s| (*s).to_owned())
    }

    /// Move a section or group into the group `parent` (`None` for the notebook's top level)
    /// at `index` among its children (counted with the node taken out; past the end means
    /// last). Returns the new id. One undo step.
    pub fn move_section_to(
        &mut self,
        id: &str,
        parent: Option<&str>,
        index: usize,
    ) -> Result<String, EditError> {
        let mut plan = Plan::owning(self, id)?;
        let nb = plan.notebook.clone();
        let Some(found) = find_node(&nb.sections, None, id, 0) else {
            return fail("no section with that id");
        };
        let from_parent = found.parent.map(|g| plan.rel(&g.id).to_owned());
        let height = height(found.node, 0);
        let (to_children, to_parent): (&[SectionNode], Option<String>) = match parent {
            None => (&nb.sections, None),
            Some(p) => {
                if p == id || p.strip_prefix(id).is_some_and(|r| r.starts_with('/')) {
                    return fail("a section group cannot move into itself");
                }
                match find_node(&nb.sections, None, p, 0).map(|f| f.node) {
                    Some(SectionNode::Group(g)) => (&g.children, Some(plan.rel(p).to_owned())),
                    Some(SectionNode::Section(_)) => return fail("a section cannot hold sections"),
                    None => return fail("no section group with that id"),
                }
            }
        };
        let depth = to_parent.as_deref().map_or(0, |d| d.split('/').count());
        if depth + height > MAX_NESTING {
            return fail("section groups can be nested 8 levels deep at most");
        }
        let old_name = base(id).to_owned();
        let mut from_names = names_of(found.siblings);
        from_names.remove(found.index);
        if from_parent == to_parent {
            let mut names = names_of(found.siblings);
            let moved = names.remove(found.index);
            names.insert(index.min(names.len()), moved);
            plan.child_order(to_parent.as_deref(), names);
            self.commit_plan(plan, "Move section")?;
            return Ok(id.to_owned());
        }
        let to_path = to_parent.clone().unwrap_or_default();
        let mut taken = names_on_disk(&plan.root.join(&to_path));
        taken.extend(names_of(to_children));
        let folder = unique_dir_name(&old_name, &taken);
        let mut to_names = names_of(to_children);
        to_names.insert(index.min(to_names.len()), folder.clone());
        let from = plan.rel(id).to_owned();
        let to = join(&to_path, &folder);
        plan.relocate(&from, &to);
        plan.child_order(from_parent.as_deref(), from_names);
        plan.child_order(to_parent.as_deref(), to_names);
        let new_id = format!("{}/{to}", plan.id());
        self.commit_plan(plan, "Move section")?;
        Ok(new_id)
    }

    // ---- notebooks --------------------------------------------------------------------

    /// Rename a notebook: the display name in `notebook.toml` changes; the folder keeps its
    /// name. One undo step.
    pub fn rename_notebook(&mut self, notebook: &str, name: &str) -> Result<(), EditError> {
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_control) || name.chars().count() > 256 {
            return fail("a notebook name is 1 to 256 characters with no control characters");
        }
        let mut plan = Plan::new(self, notebook)?;
        if plan.notebook.name == name {
            return Ok(());
        }
        plan.notebook_config(|c| c.name = Some(name.to_owned()));
        self.commit_plan(plan, "Rename notebook")
    }

    /// Set a notebook's color. One undo step.
    pub fn set_notebook_color(&mut self, notebook: &str, color: u32) -> Result<(), EditError> {
        let mut plan = Plan::new(self, notebook)?;
        if plan.notebook.color == color & 0x00FF_FFFF {
            return Ok(());
        }
        plan.notebook_config(|c| c.color = Some(color & 0x00FF_FFFF));
        self.commit_plan(plan, "Notebook color")
    }

    /// Close a notebook: it leaves the list; its folder and files stay where they are, so it
    /// can be opened again (OneNote's Close This Notebook). One undo step.
    pub fn close_notebook(&mut self, notebook: &str) -> Result<(), EditError> {
        let Some(index) = self.notebooks.iter().position(|nb| nb.id == notebook) else {
            return fail("that notebook is not open");
        };
        let Some(root) = self.notebook_roots.get(notebook).cloned() else {
            // A sample notebook has no folder to come back from.
            self.notebooks.remove(index);
            self.mark_changed();
            return Ok(());
        };
        let op = FsOp::Unlist {
            notebook: notebook.to_owned(),
            root,
            index,
        };
        let tx = Transaction::new(Scope::Notebook, "Close notebook").with(Op::Fs(op));
        if self.commit(tx) {
            Ok(())
        } else {
            fail("the notebook could not be closed")
        }
    }

    /// Create a notebook in `folder` (made if it does not exist) and open it. An existing
    /// folder is kept as it is and only gets its `notebook.toml`. Not an undo step: like
    /// opening, it adds something the user can close again.
    pub fn create_notebook(&mut self, folder: &Path) -> Result<String, EditError> {
        if folder.exists() && !folder.is_dir() {
            return fail("that path is not a folder");
        }
        std::fs::create_dir_all(folder).map_err(|err| EditError(err.to_string()))?;
        let file = folder.join(".notecraft").join("notebook.toml");
        if !file.exists() {
            let name = folder
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "Notebook".to_owned());
            let id = notecraft_storage::notebook_dir::derived_id(folder);
            let config = notecraft_storage::NotebookConfig {
                id: Some(id),
                name: Some(name),
                ..Default::default()
            };
            notecraft_storage::structure::write_text(&file, Some(&render_notebook_config(&config)))
                .map_err(|err| EditError(err.to_string()))?;
        }
        let opened = open_folder(folder).map_err(|err| EditError(err.to_string()))?;
        let id = opened.notebook.id.clone();
        self.open_notebook(opened.notebook);
        self.set_notebook_root(&id, folder);
        self.mark_changed();
        Ok(id)
    }

    // ---- selection helpers -----------------------------------------------------------

    fn select_page_id(&mut self, page: &str) {
        let found = self.notebooks.iter().find_map(|nb| {
            nb.section_of_page(page)
                .map(|s| (nb.id.clone(), s.id.clone()))
        });
        if let Some((notebook, section)) = found {
            self.selection = Selection {
                notebook: Some(notebook),
                section: Some(section),
                page: Some(page.to_owned()),
            };
        }
    }

    fn select_section_id(&mut self, section: &str) {
        let found = self.notebooks.iter().find_map(|nb| {
            nb.section(section).map(|s| {
                (
                    nb.id.clone(),
                    s.id.clone(),
                    s.pages.first().map(|p| p.id.clone()),
                )
            })
        });
        if let Some((notebook, section, page)) = found {
            self.selection = Selection {
                notebook: Some(notebook),
                section: Some(section),
                page,
            };
        }
    }

    /// Fix a selection whose section or page is gone.
    fn reselect_current(&mut self) {
        if let Some(id) = self.selection.notebook.clone() {
            self.reselect(&id);
        }
    }
}

/// The text of a page file for scanning its links: up to 4 MiB, empty if it cannot be read.
fn read_text_bounded(path: &Path) -> String {
    match notecraft_storage::read_page_file(path) {
        Ok(page) => page.text,
        Err(_) => String::new(),
    }
}

/// File names linked from `attachments/` in page text, such as `](attachments/3f9a2c-a.png)`.
/// Lenient and bounded: names with `/`, `..`, or control characters are skipped.
fn attachment_links(text: &str) -> Vec<String> {
    const MARK: &str = "attachments/";
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        let Some(after) = rest.get(at + MARK.len()..) else {
            break;
        };
        let end = after
            .find(|c: char| matches!(c, ')' | '>' | '"' | '\'' | '#' | '?') || c.is_control())
            .unwrap_or(after.len());
        let name = after.get(..end).unwrap_or_default().replace("%20", " ");
        let ok = !name.is_empty() && !name.contains(['/', '\\']) && name != ".." && name != ".";
        if ok && !found.contains(&name) {
            found.push(name);
            if found.len() >= MAX_ATTACHMENT_LINKS {
                break;
            }
        }
        rest = after.get(end..).unwrap_or_default();
    }
    found
}

#[cfg(test)]
mod tests;
