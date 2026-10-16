//! Reading and writing one page file: bounded and lenient reads, crash-safe writes
//! (docs/markdown.md sections 1 and 9).
//!
//! The read never loads more than the size cap into memory. A file over the cap is read up to
//! the cap and flagged `truncated`, so the caller can show it read-only as plain text. Bytes
//! that are not valid UTF-8 are replaced with U+FFFD and flagged `lossy`. A missing or
//! unreadable file is a [`PageReadError`], never a panic.
//!
//! Writes go through `craft-atomic` (temp file, flush, rename), so a crash or a full disk never
//! leaves a half-written page. The text comes from `notecraft-markdown`; this module only moves
//! bytes.
//!
//! The read is synchronous. Pages are read when they are shown, so a slow disk can stall the
//! UI for one frame; moving it to a worker comes with auto-save and outside-edit watching
//! (PR 1.7).

use std::borrow::Cow;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Page file size cap: 64 MiB (docs/markdown.md section 9).
pub const MAX_PAGE_BYTES: u64 = 64 * 1024 * 1024;

/// The text of a page file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageText {
    /// The whole file as text (frontmatter included), at most the cap in bytes.
    pub text: String,
    /// The file held bytes that are not valid UTF-8; they were replaced with U+FFFD.
    pub lossy: bool,
    /// The file was over the cap; only its first bytes are in `text`.
    pub truncated: bool,
}

impl PageText {
    /// A plain-text warning for the user when the page did not read cleanly, else `None`.
    pub fn warning(&self) -> Option<String> {
        match (self.truncated, self.lossy) {
            (true, true) => Some(
                "The page is very large and was cut short, and some characters could not be read."
                    .to_owned(),
            ),
            (true, false) => Some("The page is very large and was cut short.".to_owned()),
            (false, true) => {
                Some("Some characters in the page could not be read and were replaced.".to_owned())
            }
            (false, false) => None,
        }
    }
}

/// Why a page file could not be read.
#[derive(Debug)]
pub enum PageReadError {
    NotFound,
    NotAFile,
    Io(std::io::Error),
}

impl fmt::Display for PageReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("the file does not exist"),
            Self::NotAFile => f.write_str("it is not a file"),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for PageReadError {}

/// Read the page file at `path` with the default cap.
pub fn read_page_file(path: &Path) -> Result<PageText, PageReadError> {
    read_page_file_with(path, MAX_PAGE_BYTES)
}

/// Read the page file at `path`, taking at most `max_bytes` of it.
pub fn read_page_file_with(path: &Path, max_bytes: u64) -> Result<PageText, PageReadError> {
    let file = File::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => PageReadError::NotFound,
        _ => PageReadError::Io(err),
    })?;
    let meta = file.metadata().map_err(PageReadError::Io)?;
    if !meta.is_file() {
        return Err(PageReadError::NotAFile);
    }
    let mut bytes = Vec::new();
    // One byte past the cap tells a file that is exactly the cap from one that is over it.
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(PageReadError::Io)?;
    let truncated = u64::try_from(bytes.len()).map_or(true, |len| len > max_bytes);
    if truncated {
        bytes.truncate(usize::try_from(max_bytes).unwrap_or(usize::MAX));
    }
    Ok(match String::from_utf8(bytes) {
        Ok(text) => PageText {
            text,
            lossy: false,
            truncated,
        },
        Err(err) => {
            let utf8 = err.utf8_error();
            let mut bytes = err.into_bytes();
            // A cap that cuts a character in half is the cap's doing, not bad input.
            if truncated && utf8.error_len().is_none() {
                bytes.truncate(utf8.valid_up_to());
            }
            let text = String::from_utf8_lossy(&bytes);
            PageText {
                lossy: matches!(text, Cow::Owned(_)),
                text: text.into_owned(),
                truncated,
            }
        }
    })
}

/// What [`write_page_text`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    /// The file already held exactly these bytes; nothing was written.
    Unchanged,
    /// The file was written.
    Written,
}

/// Write `text` to the page file at `path` atomically. If the file already holds exactly
/// these bytes it is left alone (no write, no new modified time).
pub fn write_page_text(path: &Path, text: &str) -> io::Result<WriteOutcome> {
    if let Ok(existing) = std::fs::read(path) {
        if existing == text.as_bytes() {
            return Ok(WriteOutcome::Unchanged);
        }
    }
    craft_atomic::atomic_write(path, text.as_bytes())?;
    Ok(WriteOutcome::Written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn reads_text_as_is() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write(&dir, "a.md", "# Hi\r\ncaf\u{e9}\n".as_bytes());
        let page = read_page_file(&path).unwrap();
        assert_eq!(page.text, "# Hi\r\ncaf\u{e9}\n");
        assert!(!page.lossy && !page.truncated);
        assert_eq!(page.warning(), None);
    }

    #[test]
    fn invalid_utf8_is_replaced_with_a_warning() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write(&dir, "a.md", b"ok \xff\xfe end");
        let page = read_page_file(&path).unwrap();
        assert_eq!(page.text, "ok \u{fffd}\u{fffd} end");
        assert!(page.lossy && !page.truncated);
        assert!(page.warning().is_some());
    }

    #[test]
    fn a_file_over_the_cap_is_cut_and_flagged() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write(&dir, "big.md", &[b'a'; 1000]);
        let page = read_page_file_with(&path, 100).unwrap();
        assert_eq!(page.text.len(), 100);
        assert!(page.truncated && !page.lossy);
        let exact = read_page_file_with(&path, 1000).unwrap();
        assert!(!exact.truncated);
    }

    #[test]
    fn a_cut_inside_a_character_is_not_bad_input() {
        let dir = tempfile::TempDir::new().unwrap();
        // Each character is two bytes; a cap of 5 cuts the third one in half.
        let path = write(&dir, "u.md", "\u{e9}\u{e9}\u{e9}\u{e9}".as_bytes());
        let page = read_page_file_with(&path, 5).unwrap();
        assert_eq!(page.text, "\u{e9}\u{e9}");
        assert!(page.truncated && !page.lossy);
    }

    #[test]
    fn missing_and_non_files_are_errors() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(matches!(
            read_page_file(&dir.path().join("no.md")),
            Err(PageReadError::NotFound)
        ));
        assert!(matches!(
            read_page_file(dir.path()),
            Err(PageReadError::NotAFile | PageReadError::Io(_))
        ));
    }

    #[test]
    fn write_new_and_replace_existing() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("page.md");
        assert_eq!(
            write_page_text(
                &path, "# A
"
            )
            .unwrap(),
            WriteOutcome::Written
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"# A
"
        );
        assert_eq!(
            write_page_text(
                &path, "# B
"
            )
            .unwrap(),
            WriteOutcome::Written
        );
        assert_eq!(
            read_page_file(&path).unwrap().text,
            "# B
"
        );
    }

    #[test]
    fn identical_bytes_are_not_rewritten() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write(
            &dir, "page.md", b"same
",
        );
        assert_eq!(
            write_page_text(
                &path, "same
"
            )
            .unwrap(),
            WriteOutcome::Unchanged
        );
    }

    #[test]
    fn no_temp_files_are_left_behind() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("page.md");
        write_page_text(&path, "x").unwrap();
        write_page_text(&path, "y").unwrap();
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("page.md")]);
    }
}
