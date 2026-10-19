//! A background thread that opens the index and refreshes it, so the sidebar can show the
//! last known state at once while the folder is compared (docs/markdown.md section 8).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::thread::JoinHandle;

use super::{Index, IndexError, RefreshStats};

/// What a finished background refresh reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerReport {
    pub stats: RefreshStats,
    /// True when the old database was unusable and was deleted and rebuilt.
    pub rebuilt: bool,
}

/// A running background refresh.
pub struct IndexWorker {
    rx: Receiver<Result<WorkerReport, String>>,
    handle: Option<JoinHandle<()>>,
}

impl IndexWorker {
    /// Start refreshing the index at `db_path` from the notebook at `root`.
    /// Never panics; a failure arrives as an error message.
    pub fn spawn(root: PathBuf, db_path: PathBuf) -> Self {
        let (tx, rx) = channel();
        let handle = std::thread::Builder::new()
            .name("notecraft-index".to_owned())
            .spawn(move || {
                let result = refresh_or_rebuild(&root, &db_path)
                    .map(|(_, report)| report)
                    .map_err(|err| err.to_string());
                let _ = tx.send(result);
            })
            .ok();
        Self { rx, handle }
    }

    /// The result if the refresh has finished, without waiting. `None` while it runs.
    pub fn try_result(&self) -> Option<Result<WorkerReport, String>> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("the index thread stopped".to_owned())),
        }
    }

    /// Wait for the refresh to finish.
    pub fn wait(mut self) -> Result<WorkerReport, String> {
        let result = self
            .rx
            .recv()
            .unwrap_or_else(|_| Err("the index thread stopped".to_owned()));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        result
    }
}

/// Open (rebuilding if needed) and refresh, returning the open index with the report. A
/// database error during the refresh also deletes the file and tries once more from scratch.
pub fn refresh_or_rebuild(
    root: &Path,
    db_path: &Path,
) -> Result<(Index, WorkerReport), IndexError> {
    let (mut index, mut rebuilt) = Index::open_or_rebuild(db_path)?;
    match index.refresh(root) {
        Ok(stats) => Ok((index, WorkerReport { stats, rebuilt })),
        Err(IndexError::Db(err)) => {
            tracing::warn!("index refresh failed, rebuilding: {err}");
            drop(index);
            super::delete_database_files(db_path);
            rebuilt = true;
            let (mut fresh, _) = Index::open_or_rebuild(db_path)?;
            let stats = fresh.refresh(root)?;
            Ok((fresh, WorkerReport { stats, rebuilt }))
        }
        Err(other) => Err(other),
    }
}
