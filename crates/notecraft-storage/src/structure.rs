//! File system steps for editing a notebook's structure (docs/markdown.md section 2): create,
//! rename, and move folders and page files, and write the order files.
//!
//! Every step refuses to overwrite: a target that already exists is an error, never replaced.
//! Settings files (`notebook.toml`, `.section.toml`) are written atomically through
//! `craft-atomic`. Callers pass paths they have already checked to stay inside the notebook.
//! Nothing here deletes user content; removal is a move into the trash folder, which the
//! engine arranges (docs/architecture.md section 5).

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use toml::{Table, Value};

use crate::notebook_toml::{NotebookConfig, MAX_CONFIG_BYTES};
use crate::section_toml::{SectionConfig, SectionKind};

/// The trash folder inside `.notecraft`, relative to the notebook (docs/markdown.md section 10).
pub const TRASH_DIR: &str = ".notecraft/trash";

/// Why a structure step did not happen. Nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructureError {
    /// The target of a create, rename, or move already exists.
    Exists(String),
    /// The source of a rename, move, or copy is not there.
    Missing(String),
    /// Any other failure, as a short message.
    Io(String),
}

impl fmt::Display for StructureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exists(name) => write!(f, "{name} already exists"),
            Self::Missing(name) => write!(f, "{name} is missing"),
            Self::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for StructureError {}

fn io_error(path: &Path, err: &io::Error) -> StructureError {
    match err.kind() {
        io::ErrorKind::AlreadyExists => StructureError::Exists(show(path)),
        io::ErrorKind::NotFound => StructureError::Missing(show(path)),
        _ => StructureError::Io(format!("{}: {err}", show(path))),
    }
}

fn show(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Create the folder `path` (and missing parents). Fails if `path` itself already exists.
pub fn create_dir_new(path: &Path) -> Result<(), StructureError> {
    if path.exists() {
        return Err(StructureError::Exists(show(path)));
    }
    fs::create_dir_all(path).map_err(|err| io_error(path, &err))
}

/// Create the file `path` holding `text`, atomically. Fails if `path` already exists.
pub fn create_file_new(path: &Path, text: &str) -> Result<(), StructureError> {
    if path.exists() {
        return Err(StructureError::Exists(show(path)));
    }
    ensure_parent(path)?;
    craft_atomic::atomic_write(path, text.as_bytes()).map_err(|err| io_error(path, &err))
}

/// Copy the file `from` to `to`. Fails if `to` exists or `from` is not a file.
pub fn copy_file_new(from: &Path, to: &Path) -> Result<(), StructureError> {
    if !from.is_file() {
        return Err(StructureError::Missing(show(from)));
    }
    if to.exists() {
        return Err(StructureError::Exists(show(to)));
    }
    ensure_parent(to)?;
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|err| io_error(to, &err))
}

fn ensure_parent(path: &Path) -> Result<(), StructureError> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() && !parent.exists() => {
            fs::create_dir_all(parent).map_err(|err| io_error(parent, &err))
        }
        _ => Ok(()),
    }
}

/// Rename or move `from` (a file or folder) to `to`, creating the folders above `to`. Fails,
/// changing nothing, if `to` exists or `from` does not. A change of letter case alone
/// (`notes` to `Notes`) is allowed, also on volumes that ignore case.
pub fn rename_new(from: &Path, to: &Path) -> Result<(), StructureError> {
    if fs::symlink_metadata(from).is_err() {
        return Err(StructureError::Missing(show(from)));
    }
    let case_only = from.parent() == to.parent()
        && from != to
        && show(from).to_lowercase() == show(to).to_lowercase();
    if case_only {
        return rename_case_only(from, to);
    }
    if fs::symlink_metadata(to).is_ok() {
        return Err(StructureError::Exists(show(to)));
    }
    ensure_parent(to)?;
    fs::rename(from, to).map_err(|err| io_error(to, &err))
}

/// Case-only rename through a temporary name, so a volume that ignores case does not see the
/// target as already existing, and a volume that cares about case still refuses to replace a
/// different entry.
fn rename_case_only(from: &Path, to: &Path) -> Result<(), StructureError> {
    let parent = from.parent().unwrap_or(Path::new(""));
    let temp = parent.join(format!(".nc-rename-{}", std::process::id()));
    if fs::symlink_metadata(&temp).is_ok() {
        return Err(StructureError::Exists(show(&temp)));
    }
    fs::rename(from, &temp).map_err(|err| io_error(from, &err))?;
    if fs::symlink_metadata(to).is_ok() {
        // A different entry already has that name: put the original back.
        let _ = fs::rename(&temp, from);
        return Err(StructureError::Exists(show(to)));
    }
    match fs::rename(&temp, to) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::rename(&temp, from);
            Err(io_error(to, &err))
        }
    }
}

/// The text of `path`, or `None` when it does not exist, is unreadable, is not UTF-8, or is
/// over the settings size cap. Used to remember a settings file so an edit can be undone.
pub fn read_text(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_CONFIG_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

/// Write `text` to the settings file `path` atomically, or remove the file when `text` is
/// `None` (a file that is already gone is fine).
pub fn write_text(path: &Path, text: Option<&str>) -> Result<(), StructureError> {
    match text {
        Some(text) => {
            ensure_parent(path)?;
            craft_atomic::atomic_write(path, text.as_bytes()).map_err(|err| io_error(path, &err))
        }
        None => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(io_error(path, &err)),
        },
    }
}

fn color_text(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0x00FF_FFFF)
}

fn render(table: &Table) -> String {
    toml::to_string(table).unwrap_or_default()
}

/// `notebook.toml` text for `config`. Unknown keys are written back as they were read.
pub fn render_notebook_config(config: &NotebookConfig) -> String {
    let mut table = config.extra.clone();
    table.insert(
        "format".to_owned(),
        Value::Integer(config.format.unwrap_or(1)),
    );
    if let Some(id) = &config.id {
        table.insert("id".to_owned(), Value::String(id.clone()));
    }
    if let Some(name) = &config.name {
        table.insert("name".to_owned(), Value::String(name.clone()));
    }
    if let Some(color) = config.color {
        table.insert("color".to_owned(), Value::String(color_text(color)));
    }
    table.insert("sections".to_owned(), names(&config.sections));
    render(&table)
}

/// `.section.toml` text for `config`. A section writes its `pages`, a group its `sections`.
pub fn render_section_config(config: &SectionConfig) -> String {
    let mut table = config.extra.clone();
    table.insert(
        "format".to_owned(),
        Value::Integer(config.format.unwrap_or(1)),
    );
    let group = config.kind == Some(SectionKind::Group);
    let kind = if group { "group" } else { "section" };
    table.insert("kind".to_owned(), Value::String(kind.to_owned()));
    if let Some(color) = config.color {
        table.insert("color".to_owned(), Value::String(color_text(color)));
    }
    if group {
        table.insert("sections".to_owned(), names(&config.sections));
    } else {
        let pages = config
            .pages
            .iter()
            .map(|page| {
                let mut entry = Table::new();
                entry.insert("file".to_owned(), Value::String(page.file.clone()));
                entry.insert("level".to_owned(), Value::Integer(i64::from(page.level)));
                if page.collapsed {
                    entry.insert("collapsed".to_owned(), Value::Boolean(true));
                }
                Value::Table(entry)
            })
            .collect();
        table.insert("pages".to_owned(), Value::Array(pages));
    }
    render(&table)
}

/// The `origin.toml` note kept beside a trashed item so it can be put back: where it was
/// (`original`, relative to the notebook), what it was (`page`, `section`, or `group`), its
/// position and level, and when it was deleted (seconds since the Unix epoch).
pub fn render_trash_note(
    original: &str,
    kind: &str,
    index: usize,
    level: u8,
    deleted: u64,
) -> String {
    let mut table = Table::new();
    table.insert("original".to_owned(), Value::String(original.to_owned()));
    table.insert("kind".to_owned(), Value::String(kind.to_owned()));
    table.insert(
        "index".to_owned(),
        Value::Integer(i64::try_from(index).unwrap_or(0)),
    );
    table.insert("level".to_owned(), Value::Integer(i64::from(level)));
    table.insert(
        "deleted".to_owned(),
        Value::Integer(i64::try_from(deleted).unwrap_or(0)),
    );
    render(&table)
}

fn names(list: &[String]) -> Value {
    Value::Array(list.iter().cloned().map(Value::String).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::section_toml::PageListing;

    fn dir() -> tempfile::TempDir {
        tempfile::TempDir::new().unwrap()
    }

    #[test]
    fn create_steps_refuse_to_overwrite() {
        let d = dir();
        let file = d.path().join("A/B.md");
        create_file_new(&file, "x").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "x");
        assert!(matches!(
            create_file_new(&file, "y"),
            Err(StructureError::Exists(_))
        ));
        assert_eq!(fs::read_to_string(&file).unwrap(), "x");
        assert!(matches!(
            create_dir_new(&d.path().join("A")),
            Err(StructureError::Exists(_))
        ));
        create_dir_new(&d.path().join("C/D")).unwrap();
    }

    #[test]
    fn rename_never_replaces_an_existing_entry() {
        let d = dir();
        fs::write(d.path().join("a.md"), "a").unwrap();
        fs::write(d.path().join("b.md"), "b").unwrap();
        let err = rename_new(&d.path().join("a.md"), &d.path().join("b.md")).unwrap_err();
        assert!(matches!(err, StructureError::Exists(_)));
        assert_eq!(fs::read_to_string(d.path().join("a.md")).unwrap(), "a");
        assert_eq!(fs::read_to_string(d.path().join("b.md")).unwrap(), "b");
        let err = rename_new(&d.path().join("none.md"), &d.path().join("c.md")).unwrap_err();
        assert!(matches!(err, StructureError::Missing(_)));
        rename_new(&d.path().join("a.md"), &d.path().join("deep/er/c.md")).unwrap();
        assert!(d.path().join("deep/er/c.md").is_file());
    }

    #[test]
    fn a_case_only_rename_works() {
        let d = dir();
        fs::write(d.path().join("notes.md"), "n").unwrap();
        rename_new(&d.path().join("notes.md"), &d.path().join("Notes.md")).unwrap();
        let names: Vec<String> = fs::read_dir(d.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, ["Notes.md"]);
        assert_eq!(fs::read_to_string(d.path().join("Notes.md")).unwrap(), "n");
    }

    #[test]
    fn copy_keeps_the_original_and_refuses_to_overwrite() {
        let d = dir();
        fs::write(d.path().join("a.bin"), "a").unwrap();
        copy_file_new(&d.path().join("a.bin"), &d.path().join("x/a.bin")).unwrap();
        assert!(d.path().join("a.bin").is_file() && d.path().join("x/a.bin").is_file());
        assert!(copy_file_new(&d.path().join("a.bin"), &d.path().join("x/a.bin")).is_err());
        assert!(copy_file_new(&d.path().join("no"), &d.path().join("y")).is_err());
    }

    #[test]
    fn write_text_writes_and_removes() {
        let d = dir();
        let path = d.path().join(".notecraft/notebook.toml");
        write_text(&path, Some("a = 1\n")).unwrap();
        assert_eq!(read_text(&path).as_deref(), Some("a = 1\n"));
        write_text(&path, None).unwrap();
        assert_eq!(read_text(&path), None);
        write_text(&path, None).unwrap();
    }

    #[test]
    fn rendered_configs_read_back_the_same() {
        let section = SectionConfig {
            kind: Some(SectionKind::Section),
            color: Some(0x3C8DBC),
            pages: vec![
                PageListing {
                    file: "A \"q\".md".into(),
                    level: 0,
                    collapsed: true,
                },
                PageListing {
                    file: "B.notecraft".into(),
                    level: 2,
                    collapsed: false,
                },
            ],
            ..SectionConfig::default()
        };
        let back = SectionConfig::parse(&render_section_config(&section)).unwrap();
        assert_eq!(back.kind, section.kind);
        assert_eq!(back.color, section.color);
        assert_eq!(back.pages, section.pages);
        let group = SectionConfig {
            kind: Some(SectionKind::Group),
            sections: vec!["Z".into(), "A".into()],
            ..SectionConfig::default()
        };
        let back = SectionConfig::parse(&render_section_config(&group)).unwrap();
        assert_eq!(back.kind, Some(SectionKind::Group));
        assert_eq!(back.sections, group.sections);
        let mut notebook = NotebookConfig {
            id: Some("nb_1".into()),
            name: Some("My".into()),
            color: Some(0x7A4FB5),
            sections: vec!["Work".into()],
            ..NotebookConfig::default()
        };
        notebook.extra.insert("future".into(), Value::Integer(3));
        let back = NotebookConfig::parse(&render_notebook_config(&notebook)).unwrap();
        assert_eq!(back.id, notebook.id);
        assert_eq!(back.name, notebook.name);
        assert_eq!(back.color, notebook.color);
        assert_eq!(back.sections, notebook.sections);
        assert_eq!(back.extra.get("future"), Some(&Value::Integer(3)));
    }
}
