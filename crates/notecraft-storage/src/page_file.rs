//! Reading and writing one page file (docs/markdown.md section 1, principle 6).
//!
//! Every write goes through `craft-atomic` (temp file, flush, rename), so a crash or a full
//! disk never leaves a half-written page. The text comes from `notecraft-markdown`, which
//! applies edits as span replacements; this module only moves the bytes.

use std::io;
use std::path::Path;

/// Most bytes of a page file read. A larger file is refused instead of read into memory.
pub const MAX_PAGE_BYTES: u64 = 64 * 1024 * 1024;

/// What `write_page_text` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    /// The file already held exactly these bytes; nothing was written.
    Unchanged,
    /// The file was written.
    Written,
}

/// Read a page file as text. Bytes that are not valid UTF-8 are replaced, never an error, so a
/// damaged page still opens. A file over [`MAX_PAGE_BYTES`] gives an `InvalidData` error.
pub fn read_page_text(path: &Path) -> io::Result<String> {
    let len = std::fs::metadata(path)?.len();
    if len > MAX_PAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "page file is too large",
        ));
    }
    let bytes = std::fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
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
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn writes_new_and_replaces_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.md");
        assert_eq!(
            write_page_text(&path, "# A\r\n").unwrap(),
            WriteOutcome::Written
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"# A\r\n");
        assert_eq!(
            write_page_text(&path, "# B\n").unwrap(),
            WriteOutcome::Written
        );
        assert_eq!(read_page_text(&path).unwrap(), "# B\n");
    }

    #[test]
    fn identical_bytes_are_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.md");
        std::fs::write(&path, "same\n").unwrap();
        assert_eq!(
            write_page_text(&path, "same\n").unwrap(),
            WriteOutcome::Unchanged
        );
    }

    #[test]
    fn no_temp_files_are_left_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.md");
        write_page_text(&path, "x").unwrap();
        write_page_text(&path, "y").unwrap();
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("page.md")]);
    }

    #[test]
    fn invalid_utf8_is_replaced_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.md");
        std::fs::write(&path, [b'a', 0xff, b'b']).unwrap();
        assert_eq!(read_page_text(&path).unwrap(), "a\u{fffd}b");
    }

    #[test]
    fn missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_page_text(&dir.path().join("none.md")).is_err());
    }
}
