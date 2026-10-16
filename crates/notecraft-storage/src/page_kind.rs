//! Changing a page between `.md` and `.notecraft` (docs/markdown.md sections 2.3 and 3.5).
//!
//! A `.md` page is promoted to `.notecraft` the first time it needs canvas data (ink today),
//! and a `.notecraft` page can be converted back to plain markdown. Both write the new file
//! atomically first, then remove the old one, so a crash leaves at least one complete copy. If
//! the section's `.section.toml` lists the page by file name, only that name is rewritten
//! (the quoted string), so the rest of the file keeps its bytes.

use std::io;
use std::path::{Component, Path};

use crate::notebook_dir::PAGE_EXTENSIONS;
use crate::section_toml::SECTION_FILE_NAME;

/// The extension of a page file, in lower case, if it is a page extension.
fn page_ext(rel: &str) -> Option<&'static str> {
    let ext = Path::new(rel).extension()?.to_str()?;
    PAGE_EXTENSIONS
        .iter()
        .copied()
        .find(|e| e.eq_ignore_ascii_case(ext))
}

fn with_ext(rel: &str, ext: &str) -> Option<String> {
    let stem_len = rel
        .len()
        .checked_sub(Path::new(rel).extension()?.len() + 1)?;
    Some(format!("{}.{ext}", rel.get(..stem_len)?))
}

/// Rewrite one quoted file name in `.section.toml` text. `None` when it does not appear.
fn rename_in_section_toml(text: &str, old: &str, new: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let from = format!("{quote}{old}{quote}");
        if text.contains(&from) {
            return Some(text.replace(&from, &format!("{quote}{new}{quote}")));
        }
    }
    None
}

/// Write `text` as the page at `rel` (a `.md` or `.notecraft` path under `root`) with the
/// other extension, then delete the old file. Returns the new path (with `/` separators).
/// Refuses (`AlreadyExists`) when the new file is already there, and refuses a path that is
/// not a plain relative page path.
pub fn change_page_kind(root: &Path, rel: &str, text: &str) -> io::Result<String> {
    let bad = |why: &str| io::Error::new(io::ErrorKind::InvalidInput, why.to_owned());
    if !Path::new(rel)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(bad("the page path is not valid"));
    }
    let new_ext = match page_ext(rel) {
        Some("md") => "notecraft",
        Some(_) => "md",
        None => return Err(bad("not a page file")),
    };
    let new_rel = with_ext(rel, new_ext).ok_or_else(|| bad("not a page file"))?;
    let (old_path, new_path) = (root.join(rel), root.join(&new_rel));
    if new_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "a page with that name already exists",
        ));
    }
    craft_atomic::atomic_write(&new_path, text.as_bytes())?;
    // The new file is complete; a failure from here leaves both files, never neither.
    let section_toml = old_path.parent().map(|dir| dir.join(SECTION_FILE_NAME));
    if let (Some(toml_path), Some(old_name), Some(new_name)) = (
        section_toml,
        Path::new(rel).file_name().and_then(|n| n.to_str()),
        Path::new(&new_rel).file_name().and_then(|n| n.to_str()),
    ) {
        if let Ok(existing) = std::fs::read_to_string(&toml_path) {
            if let Some(updated) = rename_in_section_toml(&existing, old_name, new_name) {
                craft_atomic::atomic_write(&toml_path, updated.as_bytes())?;
            }
        }
    }
    std::fs::remove_file(&old_path)?;
    Ok(new_rel)
}

/// Keep a copy of a page file whose canvas block could not be read, at
/// `.notecraft/history/<page path>/canvas-unreadable.notecraft` under `root`
/// (docs/markdown.md section 3.4). Returns the path written.
pub fn keep_unreadable_copy(root: &Path, rel: &Path, text: &str) -> io::Result<std::path::PathBuf> {
    let folder: String = rel
        .to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let dir = root
        .join(crate::notebook_dir::NOTEBOOK_DIR_NAME)
        .join("history")
        .join(folder);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("canvas-unreadable.notecraft");
    craft_atomic::atomic_write(&path, text.as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn unreadable_copies_are_kept_byte_for_byte() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = super::keep_unreadable_copy(
            dir.path(),
            std::path::Path::new("S/A.notecraft"),
            "x\r\ny",
        )
        .unwrap();
        assert!(path.starts_with(dir.path().join(".notecraft/history")));
        assert_eq!(std::fs::read(path).unwrap(), b"x\r\ny");
    }
}

#[cfg(test)]
mod tests_kind {
    use super::*;

    #[test]
    fn promotes_and_converts_back() {
        let dir = tempfile::TempDir::new().unwrap();
        let sec = dir.path().join("Sec");
        std::fs::create_dir_all(&sec).unwrap();
        std::fs::write(sec.join("A.md"), "old").unwrap();
        std::fs::write(
            sec.join(".section.toml"),
            "# mine\npages = [\n  { file = \"A.md\", level = 1 },\n  \"B.md\",\n]\n",
        )
        .unwrap();
        let rel = change_page_kind(dir.path(), "Sec/A.md", "new").unwrap();
        assert_eq!(rel, "Sec/A.notecraft");
        assert!(!sec.join("A.md").exists());
        assert_eq!(
            std::fs::read_to_string(sec.join("A.notecraft")).unwrap(),
            "new"
        );
        assert_eq!(
            std::fs::read_to_string(sec.join(".section.toml")).unwrap(),
            "# mine\npages = [\n  { file = \"A.notecraft\", level = 1 },\n  \"B.md\",\n]\n"
        );
        let back = change_page_kind(dir.path(), "Sec/A.notecraft", "plain").unwrap();
        assert_eq!(back, "Sec/A.md");
        assert_eq!(std::fs::read_to_string(sec.join("A.md")).unwrap(), "plain");
    }

    #[test]
    fn refuses_clashes_and_bad_paths() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("A.md"), "x").unwrap();
        std::fs::write(dir.path().join("A.notecraft"), "y").unwrap();
        let err = change_page_kind(dir.path(), "A.md", "z").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("A.md")).unwrap(),
            "x"
        );
        assert!(change_page_kind(dir.path(), "../A.md", "z").is_err());
        assert!(change_page_kind(dir.path(), "A.txt", "z").is_err());
        assert!(change_page_kind(dir.path(), "Missing.md", "z").is_err());
    }
}
