//! Keeping the notebook index current off the UI thread (build_plan/03 PR 1.2).
//!
//! For each open notebook folder there is at most one refresh thread at a time. A refresh runs
//! when the notebook is first seen, after its tree was read again (a structure edit), and when
//! the folder watcher reports a change. The thread refreshes `.notecraft/index.sqlite`, reads
//! the frontmatter title of every page that is new or changed, and sends back the tree built
//! from the index rows. [`Session::apply_index_refresh`] shows it only if it still matches the
//! folder state the refresh began with.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

use notecraft_model::Notebook;
use notecraft_storage::index::{refresh_or_rebuild, Index};
use notecraft_storage::notebook_dir::NOTEBOOK_DIR_NAME;
use notecraft_storage::{index_path, read_notebook_header};

use crate::open::{notebook_from_sidebar, titles_of};
use crate::session::Session;

/// Most of a page file read to find its frontmatter title.
const FRONTMATTER_READ_LIMIT: u64 = 64 * 1024;

/// What a finished refresh hands back.
struct Refreshed {
    notebook: Option<Notebook>,
    titles: BTreeMap<String, String>,
    /// Frontmatter aliases by page id, for resolving page links on the canvas.
    aliases: BTreeMap<String, Vec<String>>,
    /// Pages that vanished and came back at another path with the same `notecraft.id`:
    /// `(old path, new path)` from the notebook folder (renamed or moved outside NoteCraft).
    moves: Vec<(String, String)>,
}

/// One running refresh.
struct Job {
    rx: Receiver<Result<Refreshed, String>>,
    /// The notebook's tree generation when the refresh began.
    generation: u64,
    notebook_id: String,
}

#[derive(Default)]
struct Slot {
    job: Option<Job>,
    /// A refresh is wanted (a change came in, or none has run yet).
    stale: bool,
    /// Outside renames found by refreshes whose tree was not shown yet.
    moves: Vec<(String, String)>,
}

/// The refresh threads of every open notebook folder.
#[derive(Default)]
pub(super) struct IndexSync {
    slots: BTreeMap<PathBuf, Slot>,
    /// Refreshes started so far (tests count them).
    #[cfg(test)]
    pub(super) started: usize,
}

impl IndexSync {
    /// Note that something under `folder` changed, so its index needs a refresh. Changes
    /// inside the notebook's own `.notecraft` folder (the index itself) are ignored.
    pub(super) fn note_change(&mut self, folder: &Path, changed: &Path) {
        let inside_settings = changed
            .strip_prefix(folder)
            .ok()
            .and_then(|rel| rel.components().next())
            .is_some_and(|first| first.as_os_str() == NOTEBOOK_DIR_NAME);
        if inside_settings {
            return;
        }
        if let Some(slot) = self.slots.get_mut(folder) {
            slot.stale = true;
        }
    }

    /// Apply finished refreshes and start the ones that are due. Call once per frame.
    pub(super) fn pump(&mut self, session: &mut Session, wake: &Arc<dyn Fn() + Send + Sync>) {
        let open: BTreeMap<PathBuf, String> = session
            .notebook_roots()
            .map(|(id, path)| (path.to_path_buf(), id.to_owned()))
            .collect();
        self.slots.retain(|folder, _| open.contains_key(folder));
        for (folder, id) in &open {
            let slot = self.slots.entry(folder.clone()).or_insert_with(|| Slot {
                job: None,
                stale: true,
                moves: Vec::new(),
            });
            if let Some(job) = &slot.job {
                match job.rx.try_recv() {
                    Ok(result) => {
                        let job = slot.job.take();
                        match (result, job) {
                            (Ok(done), Some(job)) => apply(session, slot, job, done),
                            (Err(err), _) => {
                                tracing::warn!(
                                    folder = %folder.display(),
                                    "cannot refresh the notebook index: {err}"
                                );
                            }
                            _ => {}
                        }
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => slot.job = None,
                }
            }
            if slot.job.is_none() && slot.stale {
                slot.stale = false;
                slot.job = spawn(folder, id, session.tree_generation(id), wake);
                #[cfg(test)]
                {
                    self.started += 1;
                }
            }
        }
    }
}

fn apply(session: &mut Session, slot: &mut Slot, job: Job, done: Refreshed) {
    let Some(notebook) = done.notebook else {
        return;
    };
    if notebook.id != job.notebook_id {
        return;
    }
    session.remember_link_aliases(&notebook.id, done.aliases);
    slot.moves.extend(done.moves);
    let id = notebook.id.clone();
    if session.apply_index_refresh(job.generation, notebook, done.titles) {
        // The tree shows the new paths now: see whether links to renamed pages can be fixed.
        let moves = std::mem::take(&mut slot.moves);
        session.consider_outside_renames(&id, moves);
    } else {
        // The tree changed while the refresh ran, so go again.
        slot.stale = true;
    }
}

fn spawn(
    folder: &Path,
    notebook_id: &str,
    generation: u64,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> Option<Job> {
    let (tx, rx) = channel();
    let root = folder.to_path_buf();
    let wake = Arc::clone(wake);
    std::thread::Builder::new()
        .name("notecraft-nav-index".to_owned())
        .spawn(move || {
            let result = refresh(&root);
            let _ = tx.send(result);
            wake();
        })
        .ok()?;
    Some(Job {
        rx,
        generation,
        notebook_id: notebook_id.to_owned(),
    })
}

/// Refresh the index of the notebook at `root`, fill in frontmatter titles for new and
/// changed pages, and build the tree from the rows.
fn refresh(root: &Path) -> Result<Refreshed, String> {
    // The page ids known before the refresh drops the rows of pages that are gone.
    let known_ids = Index::open(&index_path(root))
        .and_then(|index| index.frontmatter_ids())
        .unwrap_or_default();
    let (mut index, report) =
        refresh_or_rebuild(root, &index_path(root)).map_err(|err| err.to_string())?;
    let mut sidebar = index.sidebar().map_err(|err| err.to_string())?;
    let wanted: BTreeSet<&String> = report
        .stats
        .added
        .iter()
        .chain(report.stats.changed.iter())
        .collect();
    if !wanted.is_empty() {
        let paths: HashMap<&str, &str> = sidebar
            .pages
            .iter()
            .map(|p| (p.id.as_str(), p.path.as_str()))
            .collect();
        let found: Vec<(String, Option<String>)> = wanted
            .iter()
            .filter_map(|id| {
                let rel = paths.get(id.as_str())?;
                Some(((*id).clone(), frontmatter_title(&root.join(rel))))
            })
            .collect();
        index
            .set_frontmatter_titles(&found)
            .map_err(|err| err.to_string())?;
        sidebar = index.sidebar().map_err(|err| err.to_string())?;
    }
    let moves = update_page_ids(root, &mut index, &known_ids)?;
    // Search text follows the same refresh: pages that are new or changed get their rows.
    crate::search::update_text_index(root, &mut index)?;
    // Page links follow the same refresh. Links resolve against every page, so they are
    // resolved again whenever a page was added, removed, renamed, or edited.
    let pages_changed = !(report.stats.added.is_empty()
        && report.stats.changed.is_empty()
        && report.stats.removed.is_empty());
    crate::link_index::update_link_index(root, &mut index, pages_changed)?;
    let aliases = crate::link_index::stored_aliases(&index)?;
    // Note tags follow the same way (PR 2.5). A failure only leaves them out of date.
    if let Err(err) = crate::tags_index::update_tag_index(root, &mut index) {
        tracing::warn!("note tag index not updated: {err}");
    }
    let header = read_notebook_header(root);
    Ok(Refreshed {
        notebook: notebook_from_sidebar(&header, &sidebar),
        titles: titles_of(&sidebar),
        aliases,
        moves,
    })
}

/// Read the `notecraft.id` of every page whose id was not read yet, store them, and find the
/// pages that were renamed or moved outside NoteCraft: a page in `known` (the ids before the
/// refresh) that is gone, whose id now belongs to exactly one page that was not there before.
/// Returns `(old path, new path)` pairs.
fn update_page_ids(
    root: &Path,
    index: &mut Index,
    known: &[(String, String, String)],
) -> Result<Vec<(String, String)>, String> {
    let pending = index.pages_needing_fm_id().map_err(|err| err.to_string())?;
    let read: Vec<(String, Option<String>)> = pending
        .iter()
        .map(|(id, rel)| {
            (
                id.clone(),
                Some(frontmatter_id(&root.join(rel)).unwrap_or_default()),
            )
        })
        .collect();
    index
        .set_frontmatter_ids(&read)
        .map_err(|err| err.to_string())?;
    let now = index.frontmatter_ids().map_err(|err| err.to_string())?;
    let mut moves = Vec::new();
    for (old_id, old_path, fm_id) in known {
        if now.iter().any(|(id, _, _)| id == old_id) {
            continue;
        }
        let arrived: Vec<&(String, String, String)> = now
            .iter()
            .filter(|(id, _, other)| other == fm_id && !known.iter().any(|(k, _, _)| k == id))
            .collect();
        // An id shared by two pages (a page copied outside NoteCraft) pairs nothing: not when
        // several pages held it before, and not when several hold it now.
        let sole = now.iter().filter(|(_, _, other)| other == fm_id).count() == 1
            && known.iter().filter(|(_, _, other)| other == fm_id).count() == 1;
        if let ([(_, new_path, _)], true) = (arrived.as_slice(), sole) {
            moves.push((old_path.clone(), new_path.clone()));
        }
    }
    Ok(moves)
}

/// The `notecraft.id` in the frontmatter at the top of a page file, if it has one.
fn frontmatter_id(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(FRONTMATTER_READ_LIMIT)
        .read_to_end(&mut bytes)
        .ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let front = notecraft_markdown::parse_frontmatter(&text)?;
    let id = front.notecraft("id")?.trim();
    (!id.is_empty()).then(|| id.to_owned())
}

/// The `title` in the frontmatter at the top of a page file, if it has one.
fn frontmatter_title(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(FRONTMATTER_READ_LIMIT)
        .read_to_end(&mut bytes)
        .ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let front = notecraft_markdown::parse_frontmatter(&text)?;
    let title = front.title()?.trim();
    (!title.is_empty()).then(|| title.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, text).expect("write");
    }

    #[test]
    fn the_refresh_reads_frontmatter_titles_and_builds_the_tree() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(
            dir.path(),
            "Sec/A.md",
            "---\ntitle: Fancy Title\n---\nBody\n",
        );
        write(dir.path(), "Sec/B.md", "no frontmatter\n");
        let done = refresh(dir.path()).expect("refresh");
        let notebook = done.notebook.expect("tree");
        let titles: Vec<String> = notebook.all_sections()[0]
            .pages
            .iter()
            .map(|p| p.title.clone())
            .collect();
        assert!(titles.contains(&"Fancy Title".to_owned()));
        assert!(titles.contains(&"B".to_owned()));
        assert_eq!(done.titles.len(), 1);
        // A second refresh keeps the title without reading the page again.
        let again = refresh(dir.path()).expect("refresh");
        assert_eq!(again.titles, done.titles);
    }

    #[test]
    fn the_refresh_keeps_the_search_text_current() {
        use notecraft_storage::index::{Index, TextScope};

        let dir = tempfile::TempDir::new().expect("temp dir");
        write(
            dir.path(),
            "Sec/A.md",
            "---\ntitle: Fancy Title\n---\n# Heading\nSome **bold** words\n",
        );
        refresh(dir.path()).expect("refresh");
        let find = |text: &str| {
            let index = Index::open(&index_path(dir.path())).expect("index");
            index.search_text(text, TextScope::All, 10).expect("search")
        };
        // Markdown syntax and frontmatter keys are not searchable; the text and title are.
        assert_eq!(find("bold").len(), 1);
        assert_eq!(find("heading").len(), 1);
        assert_eq!(find("fancy")[0].title, "Fancy Title");
        assert!(find("**").is_empty());
        write(dir.path(), "Sec/A.md", "now about kayaks and more text\n");
        refresh(dir.path()).expect("refresh");
        assert!(find("bold").is_empty());
        assert_eq!(find("kayak").len(), 1);
        std::fs::remove_file(dir.path().join("Sec/A.md")).expect("rm");
        refresh(dir.path()).expect("refresh");
        assert!(find("kayak").is_empty());
    }

    #[test]
    fn editing_the_title_updates_it_on_the_next_refresh() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(dir.path(), "Sec/A.md", "---\ntitle: One\n---\nBody\n");
        let first = refresh(dir.path()).expect("refresh");
        assert!(first.titles.values().any(|t| t == "One"));
        write(dir.path(), "Sec/A.md", "---\ntitle: Two!\n---\nBody\n");
        let second = refresh(dir.path()).expect("refresh");
        assert!(second.titles.values().any(|t| t == "Two!"));
        write(dir.path(), "Sec/A.md", "plain now\n");
        let third = refresh(dir.path()).expect("refresh");
        assert!(third.titles.is_empty());
    }

    #[test]
    fn a_page_renamed_outside_is_found_again_by_its_id() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let with_id = |id: &str| format!("---\nnotecraft:\n  id: {id}\n---\nbody\n");
        write(dir.path(), "Sec/Old.md", &with_id("pg_1"));
        write(dir.path(), "Sec/Other.md", &with_id("pg_2"));
        write(dir.path(), "Sec/NoId.md", "plain\n");
        let first = refresh(dir.path()).expect("refresh");
        assert!(first.moves.is_empty());
        std::fs::rename(dir.path().join("Sec/Old.md"), dir.path().join("Sec/New.md"))
            .expect("rename");
        std::fs::rename(
            dir.path().join("Sec/NoId.md"),
            dir.path().join("Sec/Gone.md"),
        )
        .expect("rename");
        let second = refresh(dir.path()).expect("refresh");
        assert_eq!(
            second.moves,
            [("Sec/Old.md".to_owned(), "Sec/New.md".to_owned())]
        );
        // Nothing is reported again.
        assert!(refresh(dir.path()).expect("refresh").moves.is_empty());
    }

    #[test]
    fn a_copy_with_the_same_id_is_not_a_rename() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(dir.path(), "Sec/A.md", "---\nnotecraft:\n  id: pg_1\n---\n");
        refresh(dir.path()).expect("refresh");
        write(dir.path(), "Sec/B.md", "---\nnotecraft:\n  id: pg_1\n---\n");
        assert!(refresh(dir.path()).expect("refresh").moves.is_empty());
    }

    #[test]
    fn a_copy_that_is_then_renamed_or_removed_pairs_nothing() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let text = "---\nnotecraft:\n  id: pg_1\n---\n";
        write(dir.path(), "Sec/A.md", text);
        write(dir.path(), "Sec/B.md", text);
        refresh(dir.path()).expect("refresh");
        // Both copies renamed at once, or one renamed while the other stays: no pairing.
        std::fs::rename(dir.path().join("Sec/A.md"), dir.path().join("Sec/A2.md")).expect("rename");
        assert!(refresh(dir.path()).expect("refresh").moves.is_empty());
        std::fs::remove_file(dir.path().join("Sec/B.md")).expect("rm");
        std::fs::rename(dir.path().join("Sec/A2.md"), dir.path().join("Sec/A3.md"))
            .expect("rename");
        write(dir.path(), "Sec/C.md", text);
        assert!(refresh(dir.path()).expect("refresh").moves.is_empty());
    }

    #[test]
    fn a_page_saved_by_notecraft_is_found_again_after_an_outside_rename() {
        use crate::open::open_folder;
        use crate::stamp::{Clock, Stamp};

        let dir = tempfile::TempDir::new().expect("temp dir");
        write(dir.path(), "Sec/Plan.notecraft", "# Plan\n");
        write(dir.path(), "Sec/Notes.md", "see [[Plan]] and [[Plan]]\n");
        write(dir.path(), "Sec/Other.notecraft", "# Other\n");
        let mut session = Session::new();
        session.set_clock(Clock::fixed(Stamp::new(2026, 11, 5, 15, 7).expect("stamp")));
        let opened = open_folder(dir.path()).expect("scan");
        let nb = opened.notebook.id.clone();
        session.open_notebook(opened.notebook);
        session.set_notebook_root(&nb, dir.path());
        let page = format!("{nb}/Sec/Plan.notecraft");
        assert!(session.select_page_by_id(&page));
        session.load_current_page();
        assert!(session.edit_page_text(&page, 6, 0, "more\n", false));
        assert_eq!(session.save_page(&page), crate::session::SaveOutcome::Saved);
        let saved = std::fs::read_to_string(dir.path().join("Sec/Plan.notecraft")).expect("read");
        assert!(saved.contains("  id: pg_"), "{saved}");
        // Every page without an id is never paired.
        refresh(dir.path()).expect("refresh");
        std::fs::rename(
            dir.path().join("Sec/Plan.notecraft"),
            dir.path().join("Sec/Roadmap.notecraft"),
        )
        .expect("rename");
        std::fs::rename(
            dir.path().join("Sec/Other.notecraft"),
            dir.path().join("Sec/Else.notecraft"),
        )
        .expect("rename");
        let done = refresh(dir.path()).expect("refresh");
        assert_eq!(
            done.moves,
            [(
                "Sec/Plan.notecraft".to_owned(),
                "Sec/Roadmap.notecraft".to_owned()
            )]
        );
        // The offer reaches the session through the PR 2.3 path.
        let opened = open_folder(dir.path()).expect("scan");
        session.open_notebook(opened.notebook);
        session.consider_outside_renames(&nb, done.moves);
        assert_eq!(session.link_fix_offer().map(|o| o.links), Some(2));
    }

    #[test]
    fn a_corrupt_index_is_rebuilt() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        write(dir.path(), "Sec/A.md", "---\ntitle: Kept\n---\n");
        write(
            dir.path(),
            ".notecraft/index.sqlite",
            "this is not a database",
        );
        let done = refresh(dir.path()).expect("refresh");
        assert!(done.titles.values().any(|t| t == "Kept"));
    }
}
