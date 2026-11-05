//! Reading and writing one page file: bounded and lenient reads, crash-safe writes
//! (docs/markdown.md sections 1 and 9).
//!
//! The read never loads more than the size cap into memory. A file over the cap is read up to
//! the cap and flagged `truncated`, so the caller can show it read-only as plain text. Pages
//! are UTF-8. A file that is not (UTF-16 with or without a byte order mark, or any other
//! bytes, read as Windows-1252) is decoded correctly for display and flagged by its
//! [`PageEncoding`]; the caller opens it read-only and never writes it back until the user
//! converts it with a backup ([`keep_original_bytes`], [`write_page_text`]). A missing or
//! unreadable file is a [`PageReadError`], never a panic.
//!
//! Writes go through `craft-atomic` (temp file, flush, rename), so a crash or a full disk never
//! leaves a half-written page. The text comes from `notecraft-markdown`; this module only moves
//! bytes.
//!
//! The read is synchronous. Pages are read when they are shown, so a slow disk can stall the
//! UI for one frame; moving it to a worker comes with auto-save and outside-edit watching
//! (PR 1.7).

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Page file size cap: 64 MiB (docs/markdown.md section 9).
pub const MAX_PAGE_BYTES: u64 = 64 * 1024 * 1024;

/// How the bytes of a page file were encoded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageEncoding {
    /// UTF-8, the format of every page NoteCraft writes.
    #[default]
    Utf8,
    /// UTF-16, little endian (what Notepad writes as "Unicode").
    Utf16Le,
    /// UTF-16, big endian.
    Utf16Be,
    /// Anything else that is not UTF-8, read as Windows-1252 (every byte value maps to a
    /// character, so reading never fails and converting never loses a byte).
    Windows1252,
}

impl PageEncoding {
    /// A short name for messages.
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Windows1252 => "Windows-1252",
        }
    }
}

/// The notice shown on a page that is not UTF-8.
pub const NOT_UTF8_NOTICE: &str =
    "This page is not saved as UTF-8 text, so NoteCraft opened it read-only.";

/// The text of a page file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageText {
    /// The whole file as text (frontmatter included), at most the cap in bytes.
    pub text: String,
    /// How the file was encoded. Anything but UTF-8 was decoded for display only.
    pub encoding: PageEncoding,
    /// The file was over the cap; only its first bytes are in `text`.
    pub truncated: bool,
}

impl PageText {
    /// The page must not be edited or saved over: it was cut short, or it is not UTF-8.
    pub fn read_only(&self) -> bool {
        self.truncated || self.encoding != PageEncoding::Utf8
    }

    /// A plain-text warning for the user when the page did not read cleanly, else `None`.
    pub fn warning(&self) -> Option<String> {
        let legacy = self.encoding != PageEncoding::Utf8;
        match (self.truncated, legacy) {
            (true, true) => Some(format!(
                "The page is very large and was cut short. {NOT_UTF8_NOTICE}"
            )),
            (true, false) => Some("The page is very large and was cut short.".to_owned()),
            (false, true) => Some(NOT_UTF8_NOTICE.to_owned()),
            (false, false) => None,
        }
    }
}

/// Windows-1252 characters for the bytes 0x80 to 0x9F. The five undefined bytes map to the
/// control characters of the same value, so every byte round-trips.
const CP1252_HIGH: [char; 32] = [
    '\u{20AC}', '\u{81}', '\u{201A}', '\u{192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{2C6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8D}', '\u{17D}', '\u{8F}',
    '\u{90}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{2DC}', '\u{2122}', '\u{161}', '\u{203A}', '\u{153}', '\u{9D}', '\u{17E}', '\u{178}',
];

fn decode_cp1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => CP1252_HIGH
                .get(usize::from(b) - 0x80)
                .copied()
                .unwrap_or('\u{FFFD}'),
            _ => char::from(b),
        })
        .collect()
}

fn decode_utf16(bytes: &[u8], big_endian: bool) -> String {
    let units = bytes.chunks_exact(2).map(|pair| {
        let a = pair.first().copied().unwrap_or(0);
        let b = pair.get(1).copied().unwrap_or(0);
        if big_endian {
            u16::from_be_bytes([a, b])
        } else {
            u16::from_le_bytes([a, b])
        }
    });
    char::decode_utf16(units)
        .map(|c| c.unwrap_or('\u{FFFD}'))
        .collect()
}

/// UTF-16 without a byte order mark, guessed from text that is mostly ASCII: NUL bytes fill
/// one byte position of every pair and never the other. Needs at least 4 pairs. Returns
/// `Some(big_endian)`.
fn guess_utf16(bytes: &[u8]) -> Option<bool> {
    let pairs = bytes.len() / 2;
    if pairs < 4 {
        return None;
    }
    let (mut even, mut odd) = (0usize, 0usize);
    for pair in bytes.chunks_exact(2) {
        even += usize::from(pair.first() == Some(&0));
        odd += usize::from(pair.get(1) == Some(&0));
    }
    // Little endian ASCII has its NULs in the second byte of a pair, big endian in the first.
    if even == 0 && odd * 10 >= pairs * 3 {
        Some(false)
    } else if odd == 0 && even * 10 >= pairs * 3 {
        Some(true)
    } else {
        None
    }
}

/// Decode the bytes of a page file. Never fails: UTF-16 when there is a byte order mark (or
/// the bytes look like BOM-less UTF-16), else UTF-8 when the bytes are valid, else
/// Windows-1252. A UTF-16 byte order mark is not part of the text.
pub fn decode_page_bytes(bytes: &[u8]) -> (String, PageEncoding) {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return (decode_utf16(rest, false), PageEncoding::Utf16Le);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return (decode_utf16(rest, true), PageEncoding::Utf16Be);
    }
    if let Some(big_endian) = guess_utf16(bytes) {
        let encoding = if big_endian {
            PageEncoding::Utf16Be
        } else {
            PageEncoding::Utf16Le
        };
        return (decode_utf16(bytes, big_endian), encoding);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => (text.to_owned(), PageEncoding::Utf8),
        Err(_) => (decode_cp1252(bytes), PageEncoding::Windows1252),
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
    // A cap that cuts a UTF-8 character in half is the cap's doing, not bad input.
    if truncated && guess_utf16(&bytes).is_none() {
        if let Err(utf8) = std::str::from_utf8(&bytes) {
            if utf8.error_len().is_none() {
                bytes.truncate(utf8.valid_up_to());
            }
        }
    }
    let (text, encoding) = decode_page_bytes(&bytes);
    Ok(PageText {
        text,
        encoding,
        truncated,
    })
}

/// The raw bytes of the page file at `path`, or `None` when it is over the size cap (so a
/// caller never rewrites a file it did not read whole).
pub fn read_page_bytes(path: &Path) -> Result<Option<Vec<u8>>, PageReadError> {
    let file = File::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => PageReadError::NotFound,
        _ => PageReadError::Io(err),
    })?;
    if !file.metadata().map_err(PageReadError::Io)?.is_file() {
        return Err(PageReadError::NotAFile);
    }
    let mut bytes = Vec::new();
    file.take(MAX_PAGE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(PageReadError::Io)?;
    Ok((u64::try_from(bytes.len()).is_ok_and(|len| len <= MAX_PAGE_BYTES)).then_some(bytes))
}

/// A stable 64-bit hash (FNV-1a) of `bytes`, for backup file names.
fn content_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Keep the original bytes of the page file `file` before it is rewritten as UTF-8. For a
/// page in a notebook (`notebook` is its folder and the page path inside it) the copy goes to
/// `.notecraft/history/<page path>/original-<hash>.bak`; otherwise beside the file as
/// `<name>.original-<hash>.bak`. The hash is of the bytes, so the same bytes keep one copy.
/// Returns the path of the copy.
pub fn keep_original_bytes(
    file: &Path,
    notebook: Option<(&Path, &Path)>,
    bytes: &[u8],
) -> io::Result<PathBuf> {
    let hash = format!("{:016x}", content_hash(bytes));
    let path = match notebook {
        Some((root, rel)) => {
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
            dir.join(format!("original-{hash}.bak"))
        }
        None => {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "page".to_owned());
            file.with_file_name(format!("{name}.original-{hash}.bak"))
        }
    };
    craft_atomic::atomic_write(&path, bytes)?;
    Ok(path)
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
        assert!(!page.read_only());
        assert_eq!(page.warning(), None);
    }

    #[test]
    fn invalid_utf8_is_read_as_windows_1252() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write(&dir, "a.md", b"caf\xe9 \x93q\x94 \x80 \x81 end");
        let page = read_page_file(&path).unwrap();
        assert_eq!(page.text, "caf\u{e9} \u{201c}q\u{201d} \u{20ac} \u{81} end");
        assert_eq!(page.encoding, PageEncoding::Windows1252);
        assert!(page.read_only() && !page.truncated);
        assert_eq!(page.warning().as_deref(), Some(NOT_UTF8_NOTICE));
    }

    #[test]
    fn utf16_with_a_bom_is_decoded() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut le = vec![0xFF, 0xFE];
        let mut be = vec![0xFE, 0xFF];
        for unit in "# Hi\r\ncaf\u{e9} \u{1F600}".encode_utf16() {
            le.extend(unit.to_le_bytes());
            be.extend(unit.to_be_bytes());
        }
        let page = read_page_file(&write(&dir, "le.md", &le)).unwrap();
        assert_eq!(page.text, "# Hi\r\ncaf\u{e9} \u{1F600}");
        assert_eq!(page.encoding, PageEncoding::Utf16Le);
        let page = read_page_file(&write(&dir, "be.md", &be)).unwrap();
        assert_eq!(page.text, "# Hi\r\ncaf\u{e9} \u{1F600}");
        assert_eq!(page.encoding, PageEncoding::Utf16Be);
    }

    #[test]
    fn utf16_without_a_bom_is_guessed_from_ascii_text() {
        let le: Vec<u8> = "hello world"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let be: Vec<u8> = "hello world"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        assert_eq!(
            decode_page_bytes(&le),
            ("hello world".to_owned(), PageEncoding::Utf16Le)
        );
        assert_eq!(
            decode_page_bytes(&be),
            ("hello world".to_owned(), PageEncoding::Utf16Be)
        );
        // Short or ordinary text is not guessed.
        assert_eq!(decode_page_bytes(b"a\0b").1, PageEncoding::Utf8);
        assert_eq!(decode_page_bytes(b"plain text").1, PageEncoding::Utf8);
    }

    #[test]
    fn hostile_bytes_never_panic() {
        let mut seed = 12345u64;
        for len in 0..200usize {
            let bytes: Vec<u8> = (0..len)
                .map(|_| {
                    seed = seed
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    (seed >> 33) as u8
                })
                .collect();
            let _ = decode_page_bytes(&bytes);
        }
        let _ = decode_page_bytes(&[0xFF]);
        let _ = decode_page_bytes(&[0xFF, 0xFE, 0x00]);
        let _ = decode_page_bytes(&[0xFF, 0xFE, 0x00, 0xD8]);
        let (text, _) = decode_page_bytes(&[0xFE, 0xFF, 0xD8, 0x00]);
        assert_eq!(text, "\u{FFFD}");
    }

    #[test]
    fn original_bytes_are_kept_by_hash() {
        let dir = tempfile::TempDir::new().unwrap();
        let bytes = b"caf\xe9";
        let kept = keep_original_bytes(
            &dir.path().join("S/A.md"),
            Some((dir.path(), Path::new("S/A.md"))),
            bytes,
        )
        .unwrap();
        assert!(kept.starts_with(dir.path().join(".notecraft/history/S_A.md")));
        assert_eq!(std::fs::read(&kept).unwrap(), bytes);
        let loose = dir.path().join("note.md");
        let beside = keep_original_bytes(&loose, None, bytes).unwrap();
        assert_eq!(beside.parent(), loose.parent());
        assert!(beside
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("note.md.original-") && n.ends_with(".bak")));
        assert_eq!(std::fs::read(beside).unwrap(), bytes);
    }

    #[test]
    fn a_file_over_the_cap_is_cut_and_flagged() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write(&dir, "big.md", &[b'a'; 1000]);
        let page = read_page_file_with(&path, 100).unwrap();
        assert_eq!(page.text.len(), 100);
        assert!(page.truncated && page.encoding == PageEncoding::Utf8);
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
        assert!(page.truncated && page.encoding == PageEncoding::Utf8);
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
