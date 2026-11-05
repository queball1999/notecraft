//! Changing a page between `.md` and `.notecraft` (docs/markdown.md sections 2.3 and 3.5).
//!
//! A `.md` page is promoted to `.notecraft` the first time it needs canvas data (ink today),
//! and a `.notecraft` page can be converted back to plain markdown. Both write the new file
//! atomically first, then remove the old one, so a crash leaves at least one complete copy. If
//! the section's `.section.toml` lists the page by file name, only that name in the `pages`
//! list is rewritten (the quoted string), so the rest of the file keeps its bytes.

use std::io;
use std::path::{Component, Path, PathBuf};

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

/// The byte span of the `[ ... ]` value of the top-level `pages` key, if there is one.
fn pages_list_span(text: &str) -> Option<(usize, usize)> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let after = line
            .trim_start()
            .strip_prefix("pages")
            .map(str::trim_start)
            .and_then(|rest| rest.strip_prefix('='));
        if let Some(after) = after {
            let value = offset + (line.len() - after.len());
            let open = value + text.get(value..)?.find(|c: char| !c.is_whitespace())?;
            if !text.get(open..)?.starts_with('[') {
                return None;
            }
            return Some((open, matching_bracket(text, open)?));
        }
        offset += line.len();
    }
    None
}

/// The index just past the `]` that closes the `[` at `open`. Strings and comments are
/// skipped.
fn matching_bracket(text: &str, open: usize) -> Option<usize> {
    let (mut depth, mut quote, mut comment, mut escaped) = (0usize, None::<char>, false, false);
    for (i, c) in text.get(open..)?.char_indices() {
        if comment {
            comment = c != '\n';
        } else if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' && q == '"' {
                escaped = true;
            } else if c == q || c == '\n' {
                quote = None;
            }
        } else {
            match c {
                '"' | '\'' => quote = Some(c),
                '#' => comment = true,
                '[' => depth += 1,
                ']' => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(open + i + 1);
                    }
                }
                _ => {}
            }
        }
    }
    None
}

/// Rewrite one quoted file name in the `pages` list of `.section.toml` text. Other keys, and
/// the same text elsewhere in the file, are left alone. `None` when it is not in the list.
fn rename_in_section_toml(text: &str, old: &str, new: &str) -> Option<String> {
    let (start, end) = pages_list_span(text)?;
    let list = text.get(start..end)?;
    for quote in ['"', '\''] {
        let from = format!("{quote}{old}{quote}");
        if list.contains(&from) {
            let replaced = list.replace(&from, &format!("{quote}{new}{quote}"));
            return Some(format!(
                "{}{replaced}{}",
                text.get(..start)?,
                text.get(end..)?
            ));
        }
    }
    None
}

/// `swapped` (the page path with its new extension), or the same folder with a numbered name
/// when a file with that name (any case) is already there.
fn unique_target(root: &Path, swapped: &str, ext: &str) -> String {
    let path = Path::new(swapped);
    let (Some(stem), Some(dir)) = (path.file_stem().and_then(|s| s.to_str()), path.parent()) else {
        return swapped.to_owned();
    };
    let existing: Vec<String> = std::fs::read_dir(root.join(dir))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    let refs: Vec<&str> = existing.iter().map(String::as_str).collect();
    let name = crate::paths::unique_file_name(stem, ext, &refs);
    let dir = dir.to_string_lossy().replace('\\', "/");
    if dir.is_empty() {
        name
    } else {
        format!("{dir}/{name}")
    }
}

/// Write `text` as the page at `rel` (a `.md` or `.notecraft` path under `root`) with the
/// other extension, then delete the old file. Returns the new path (with `/` separators),
/// which is a numbered name (`Name 2.notecraft`) when a file with the plain name is already
/// there. Refuses a path that is not a plain relative page path. If the section list cannot
/// be rewritten or the old file cannot be removed, the new file (and the list) are put back,
/// so a failure never leaves two copies of the page.
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
    let old_path = root.join(rel);
    if !old_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the page file is missing",
        ));
    }
    let swapped = with_ext(rel, new_ext).ok_or_else(|| bad("not a page file"))?;
    let new_rel = unique_target(root, &swapped, new_ext);
    let new_path = root.join(&new_rel);
    craft_atomic::atomic_write(&new_path, text.as_bytes())?;
    let undo_new = |err: io::Error| {
        let _ = std::fs::remove_file(&new_path);
        err
    };
    // Rewrite the section list first, so a failure to remove the old file can undo it.
    let mut restore: Option<(PathBuf, String)> = None;
    let names = (
        Path::new(rel).file_name().and_then(|n| n.to_str()),
        Path::new(&new_rel).file_name().and_then(|n| n.to_str()),
    );
    if let (Some(dir), (Some(old_name), Some(new_name))) = (old_path.parent(), names) {
        let toml_path = dir.join(SECTION_FILE_NAME);
        if let Ok(existing) = std::fs::read_to_string(&toml_path) {
            if let Some(updated) = rename_in_section_toml(&existing, old_name, new_name) {
                craft_atomic::atomic_write(&toml_path, updated.as_bytes()).map_err(undo_new)?;
                restore = Some((toml_path, existing));
            }
        }
    }
    if let Err(err) = std::fs::remove_file(&old_path) {
        if let Some((toml_path, original)) = restore {
            let _ = craft_atomic::atomic_write(&toml_path, original.as_bytes());
        }
        return Err(undo_new(err));
    }
    Ok(new_rel)
}

/// 64-bit FNV-1a as 16 hex digits: a stable name for a snapshot of some text.
fn content_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Keep a copy of a page file whose canvas block could not be read, at
/// `.notecraft/history/<page path>/canvas-unreadable-<hash>.notecraft` under `root`
/// (docs/markdown.md section 3.4). Returns the snapshot path; a snapshot with the same bytes
/// that is already there is not written again.
pub fn keep_unreadable_copy(root: &Path, rel: &Path, text: &str) -> io::Result<PathBuf> {
    keep_snapshot(root, rel, text, "canvas-unreadable")
}

/// Keep a copy of a page file as it is before converting it to plain markdown (which drops
/// the canvas data), at `.notecraft/history/<page path>/before-convert-<hash>.notecraft`.
pub fn keep_snapshot_before_convert(root: &Path, rel: &Path, text: &str) -> io::Result<PathBuf> {
    keep_snapshot(root, rel, text, "before-convert")
}

fn keep_snapshot(root: &Path, rel: &Path, text: &str, label: &str) -> io::Result<PathBuf> {
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
    let path = dir.join(format!("{label}-{}.notecraft", content_hash(text)));
    if path.is_file() {
        return Ok(path);
    }
    std::fs::create_dir_all(&dir)?;
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

    #[test]
    fn the_same_text_is_kept_once_and_new_text_gets_its_own_snapshot() {
        let dir = tempfile::TempDir::new().unwrap();
        let rel = std::path::Path::new("S/A.notecraft");
        let first = super::keep_unreadable_copy(dir.path(), rel, "one").unwrap();
        let again = super::keep_unreadable_copy(dir.path(), rel, "one").unwrap();
        assert_eq!(first, again);
        let second = super::keep_unreadable_copy(dir.path(), rel, "two").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read(&first).unwrap(), b"one");
        assert_eq!(std::fs::read(&second).unwrap(), b"two");
        assert_eq!(
            std::fs::read_dir(first.parent().unwrap()).unwrap().count(),
            2
        );
    }

    #[test]
    fn a_convert_snapshot_does_not_clash_with_an_unreadable_one() {
        let dir = tempfile::TempDir::new().unwrap();
        let rel = std::path::Path::new("S/A.notecraft");
        let a = super::keep_unreadable_copy(dir.path(), rel, "x").unwrap();
        let b = super::keep_snapshot_before_convert(dir.path(), rel, "x").unwrap();
        assert_ne!(a, b);
        assert_eq!(std::fs::read(b).unwrap(), b"x");
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
    fn a_stray_file_with_the_new_name_gets_a_numbered_name_instead() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("A.md"), "x").unwrap();
        std::fs::write(dir.path().join("A.notecraft"), "y").unwrap();
        std::fs::write(dir.path().join(".section.toml"), "pages = [\"A.md\"]\n").unwrap();
        let rel = change_page_kind(dir.path(), "A.md", "z").unwrap();
        assert_eq!(rel, "A 2.notecraft");
        assert!(!dir.path().join("A.md").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("A.notecraft")).unwrap(),
            "y"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("A 2.notecraft")).unwrap(),
            "z"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".section.toml")).unwrap(),
            "pages = [\"A 2.notecraft\"]\n"
        );
    }

    #[test]
    fn a_failure_to_remove_the_old_file_rolls_everything_back() {
        let dir = tempfile::TempDir::new().unwrap();
        // A folder named like a page cannot be removed as a file.
        std::fs::create_dir_all(dir.path().join("A.md")).unwrap();
        let toml = "pages = [\"A.md\"]\n";
        std::fs::write(dir.path().join(".section.toml"), toml).unwrap();
        assert!(change_page_kind(dir.path(), "A.md", "z").is_err());
        assert!(!dir.path().join("A.notecraft").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".section.toml")).unwrap(),
            toml
        );
    }

    #[test]
    fn only_the_page_list_is_rewritten() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("A.md"), "x").unwrap();
        let toml = "note = \"A.md\"\n# pages = [\"A.md\"]\npages = [\n  \"B.md\", # \"]\"\n  { file = \"A.md\", level = 1 },\n]\nother = [\"A.md\"]\n";
        std::fs::write(dir.path().join(".section.toml"), toml).unwrap();
        change_page_kind(dir.path(), "A.md", "z").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".section.toml")).unwrap(),
            toml.replace("{ file = \"A.md\"", "{ file = \"A.notecraft\"")
        );
    }

    #[test]
    fn refuses_bad_paths() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(change_page_kind(dir.path(), "../A.md", "z").is_err());
        assert!(change_page_kind(dir.path(), "A.txt", "z").is_err());
        assert!(change_page_kind(dir.path(), "Missing.md", "z").is_err());
    }
}
