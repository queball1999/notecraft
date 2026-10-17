//! Notebooks on disk, for the subcommands that read one: finding the folder and scanning it
//! with the folder reader.

use std::path::PathBuf;

use notecraft_storage::notebook_dir::{NOTEBOOK_DIR_NAME, NOTEBOOK_FILE_NAME};
use notecraft_storage::{scan_notebook, NotebookTree, ScanError};

use crate::code;

/// A command that did not work: the exit code and a plain-text reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub code: u8,
    pub message: String,
}

impl Failure {
    pub fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// The notebook folder: `--notebook` if given, else the current directory when it is a
/// notebook (it has `.notecraft/notebook.toml`).
pub fn locate(explicit: Option<&str>) -> Result<PathBuf, Failure> {
    if let Some(path) = explicit {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Ok(path);
        }
        return Err(Failure::new(
            code::NOT_FOUND,
            format!("notebook folder not found: {}", path.display()),
        ));
    }
    let here = std::env::current_dir()
        .map_err(|e| Failure::new(code::ERROR, format!("cannot read the current folder: {e}")))?;
    if here
        .join(NOTEBOOK_DIR_NAME)
        .join(NOTEBOOK_FILE_NAME)
        .is_file()
    {
        return Ok(here);
    }
    Err(Failure::new(
        code::NOT_FOUND,
        "no notebook here: run inside a notebook folder or pass --notebook <path>",
    ))
}

/// A notebook folder and what the folder reader found in it.
pub struct Notebook {
    pub root: PathBuf,
    pub tree: NotebookTree,
}

/// Find the notebook folder and scan it.
pub fn open(explicit: Option<&str>) -> Result<Notebook, Failure> {
    let root = locate(explicit)?;
    let tree = scan_notebook(&root).map_err(|e| match e {
        ScanError::NotADirectory(_) => Failure::new(code::NOT_FOUND, e.to_string()),
        ScanError::Io { .. } => Failure::new(code::ERROR, e.to_string()),
    })?;
    Ok(Notebook { root, tree })
}
