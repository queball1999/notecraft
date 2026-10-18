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
use notecraft_storage::index::refresh_or_rebuild;
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
}

/// The refresh threads of every open notebook folder.
#[derive(Default)]
pub(super) struct IndexSync {
    slots: BTreeMap<PathBuf, Slot>,
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
            });
            if let Some(job) = &slot.job {
                match job.rx.try_recv() {
                    Ok(result) => {
                        let job = slot.job.take();
                        if let (Ok(done), Some(job)) = (result, job) {
                            apply(session, slot, job, done);
                        }
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => slot.job = None,
                }
            }
            if slot.job.is_none() && slot.stale {
                slot.stale = false;
                slot.job = spawn(folder, id, session.tree_generation(id), wake);
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
    if !session.apply_index_refresh(job.generation, notebook, done.titles) {
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
    let header = read_notebook_header(root);
    Ok(Refreshed {
        notebook: notebook_from_sidebar(&header, &sidebar),
        titles: titles_of(&sidebar),
    })
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
