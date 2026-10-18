//! Attachments: files embedded in a page (docs/markdown.md section 2.4).
//!
//! A file is copied into the `attachments/` folder next to the page, under a name made of a
//! short content-hash prefix and the sanitized original name (`3f9a2c-budget.xlsx`). A copy
//! never overwrites anything: the name is reserved with `create_new`, and a taken name gets a
//! number (`3f9a2c-budget 2.xlsx`). If the very same bytes are already stored under the name,
//! that file is reused. A file over the link threshold (100 MiB by default) is not copied;
//! the page links to it in place.
//!
//! This module moves bytes and builds the link text. It never edits the page; the engine does
//! that, so the whole insert is one undo step.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::notebook_dir::ATTACHMENTS_DIR_NAME;
use crate::paths::{sanitize_file_name, MAX_FILE_NAME_BYTES};

/// Files over this size are linked in place instead of copied (100 MiB).
pub const LINK_THRESHOLD_BYTES: u64 = 100 * 1024 * 1024;
/// Most bytes accepted for an attachment made from clipboard or dropped bytes (256 MiB).
pub const MAX_BYTES_ATTACHMENT: usize = 256 * 1024 * 1024;
/// Most names in the attachments folder looked at when choosing a free name.
const MAX_NAMES_LISTED: usize = 100_000;
/// Most numbered variants of one name tried before giving up.
const MAX_VARIANTS: usize = 10_000;
/// Longest stored name in bytes, leaving room for the hash prefix and a number.
const MAX_STORED_NAME_BYTES: usize = MAX_FILE_NAME_BYTES - 16;

/// Why a file could not be attached.
#[derive(Debug)]
pub enum AttachError {
    /// The source does not exist or is not a regular file.
    NotAFile,
    /// The bytes are too large to take from memory.
    TooLarge,
    /// No free name was found in the attachments folder.
    NoFreeName,
    Io(io::Error),
}

impl fmt::Display for AttachError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAFile => f.write_str("the file does not exist or is not a file"),
            Self::TooLarge => f.write_str("the file is too large"),
            Self::NoFreeName => f.write_str("no free name in the attachments folder"),
            Self::Io(err) => write!(f, "the file could not be copied: {err}"),
        }
    }
}

impl std::error::Error for AttachError {}

impl From<io::Error> for AttachError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// A file that was attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredAttachment {
    /// Where the bytes are now: the copy in `attachments/`, or the original when linked.
    pub path: PathBuf,
    /// The link destination for the page, percent-encoded: `attachments/3f9a2c-budget.xlsx`,
    /// or the absolute path of a linked file.
    pub link: String,
    /// The original file name, for link text.
    pub display_name: String,
    pub size: u64,
    /// The file was not copied; the page links to the original.
    pub linked: bool,
}

/// Copy `source` into `section_dir/attachments/` (see the module docs), or link it in place if
/// it is over [`LINK_THRESHOLD_BYTES`].
pub fn store_attachment(
    section_dir: &Path,
    source: &Path,
) -> Result<StoredAttachment, AttachError> {
    store_attachment_with(section_dir, source, LINK_THRESHOLD_BYTES)
}

/// [`store_attachment`] with a different link threshold, in bytes.
pub fn store_attachment_with(
    section_dir: &Path,
    source: &Path,
    link_over: u64,
) -> Result<StoredAttachment, AttachError> {
    let meta = fs::metadata(source).map_err(|_| AttachError::NotAFile)?;
    if !meta.is_file() {
        return Err(AttachError::NotAFile);
    }
    let size = meta.len();
    let display_name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if size > link_over {
        let absolute = fs::canonicalize(source).unwrap_or_else(|_| source.to_path_buf());
        let absolute = strip_verbatim(&absolute);
        let link = encode_link_path(&absolute.to_string_lossy().replace('\\', "/"));
        return Ok(StoredAttachment {
            path: absolute,
            link,
            display_name,
            size,
            linked: true,
        });
    }
    let hash = hash_file(source)?;
    let mut reader = File::open(source)?;
    store_stream(section_dir, &display_name, hash, size, &mut reader)
}

/// Store `bytes` (a pasted picture, say) as an attachment named `name`.
pub fn store_attachment_bytes(
    section_dir: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<StoredAttachment, AttachError> {
    if bytes.len() > MAX_BYTES_ATTACHMENT {
        return Err(AttachError::TooLarge);
    }
    let mut hash = FNV_OFFSET;
    fnv_update(&mut hash, bytes);
    let mut reader = bytes;
    store_stream(section_dir, name, hash, bytes.len() as u64, &mut reader)
}

fn store_stream(
    section_dir: &Path,
    name: &str,
    hash: u64,
    size: u64,
    reader: &mut dyn Read,
) -> Result<StoredAttachment, AttachError> {
    let dir = section_dir.join(ATTACHMENTS_DIR_NAME);
    fs::create_dir_all(&dir)?;
    let base = stored_name(hash, name);
    let (stem, ext) = split_ext(&base);
    let mut taken: Vec<String> = Vec::new();
    for entry in fs::read_dir(&dir)?.take(MAX_NAMES_LISTED).flatten() {
        taken.push(entry.file_name().to_string_lossy().to_lowercase());
    }
    for n in 1..=MAX_VARIANTS {
        let candidate = if n == 1 {
            base.clone()
        } else {
            format!("{stem} {n}{ext}")
        };
        let path = dir.join(&candidate);
        if taken.contains(&candidate.to_lowercase()) {
            // The same bytes under this name: reuse the stored file.
            if n == 1 && same_content(&path, size, hash) {
                return Ok(done(path, &candidate, name, size));
            }
            continue;
        }
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let copied = io::copy(&mut reader.take(size), &mut file)
                    .and_then(|_| file.flush())
                    .and_then(|_| file.sync_all());
                if let Err(err) = copied {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(err.into());
                }
                return Ok(done(path, &candidate, name, size));
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err.into()),
        }
    }
    Err(AttachError::NoFreeName)
}

fn done(path: PathBuf, stored: &str, display: &str, size: u64) -> StoredAttachment {
    StoredAttachment {
        path,
        link: format!("{ATTACHMENTS_DIR_NAME}/{}", encode_link_path(stored)),
        display_name: display.to_owned(),
        size,
        linked: false,
    }
}

/// `3f9a2c-budget.xlsx`: the first 24 bits of the content hash, then the sanitized name.
fn stored_name(hash: u64, name: &str) -> String {
    // Cut the stem before sanitizing, so a very long name keeps its extension.
    let (stem, ext) = split_ext(name);
    let ext = match ext.strip_prefix('.') {
        Some(rest) if !rest.is_empty() => format!(".{}", sanitize_file_name(rest)),
        _ => String::new(),
    };
    let room = MAX_STORED_NAME_BYTES.saturating_sub(ext.len());
    let stem = sanitize_file_name(cut(stem, room));
    let stem = cut(&stem, room);
    format!("{:06x}-{stem}{ext}", hash >> 40)
}

/// Split `name` into the stem and the extension with its dot (empty when there is none).
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(at) if at > 0 && name.len() - at <= 16 => name.split_at(at),
        _ => (name, ""),
    }
}

fn cut(text: &str, max_bytes: usize) -> &str {
    let mut end = max_bytes.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.get(..end).unwrap_or("")
}

fn same_content(path: &Path, size: u64, hash: u64) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    meta.is_file() && meta.len() == size && hash_file(path).is_ok_and(|h| h == hash)
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv_update(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
}

/// FNV-1a (64 bits) over a file, read in chunks.
pub fn hash_file(path: &Path) -> io::Result<u64> {
    let mut file = File::open(path)?;
    let mut hash = FNV_OFFSET;
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(hash);
        }
        fnv_update(&mut hash, buf.get(..n).unwrap_or_default());
    }
}

/// Drop the `\\?\` prefix Windows adds to canonical paths, so links stay readable.
fn strip_verbatim(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

/// Percent-encode a link destination: letters, digits, and `-._~/:` stay; every other byte
/// becomes `%XX`, so spaces and parentheses never break the markdown link.
pub fn encode_link_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/:".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Undo [`encode_link_path`]. A `%` that is not followed by two hex digits is kept as is, and
/// bytes that are not UTF-8 are replaced.
pub fn decode_link_path(link: &str) -> String {
    let bytes = link.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&byte) = bytes.get(i) {
        let hex = |at: usize| bytes.get(at).and_then(|b| char::from(*b).to_digit(16));
        if byte == b'%' {
            if let (Some(hi), Some(lo)) = (hex(i + 1), hex(i + 2)) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(byte);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether the file name has a picture extension NoteCraft draws on the canvas (PNG, JPEG,
/// WebP, SVG).
pub fn is_picture_name(name: &str) -> bool {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    matches!(
        ext.as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "svg")
    )
}

/// File extensions that run code when opened. Opening an attachment with one of these is
/// refused; the user can open the folder and decide there.
const RUNNABLE: &[&str] = &[
    // Windows programs, installers, scripts, and files that run or load code when opened.
    "exe",
    "com",
    "bat",
    "cmd",
    "msi",
    "msp",
    "mst",
    "scr",
    "pif",
    "lnk",
    "url",
    "scf",
    "ps1",
    "psm1",
    "psd1",
    "ps1xml",
    "ps2",
    "ps2xml",
    "psc1",
    "psc2",
    "msh",
    "msh1",
    "msh2",
    "mshxml",
    "msh1xml",
    "msh2xml",
    "vb",
    "vbs",
    "vbe",
    "js",
    "jse",
    "ws",
    "wsf",
    "wsh",
    "wsc",
    "sct",
    "hta",
    "cpl",
    "msc",
    "reg",
    "inf",
    "dll",
    "ocx",
    "sys",
    "drv",
    "chm",
    "hlp",
    "gadget",
    "application",
    "appref-ms",
    "xbap",
    "appx",
    "appxbundle",
    "msix",
    "msixbundle",
    "appinstaller",
    "settingcontent-ms",
    "library-ms",
    "search-ms",
    "searchconnector-ms",
    "website",
    "diagcab",
    "diagcfg",
    "diagpkg",
    "xll",
    "mde",
    "ade",
    "adp",
    "iso",
    "img",
    "vhd",
    "vhdx",
    // Interpreters that run a script on double-click when installed.
    "jar",
    "jnlp",
    "py",
    "pyw",
    "pyc",
    "pyo",
    "pyz",
    "pyzw",
    "pl",
    "rb",
    "php",
    "tcl",
    // macOS and Linux.
    "sh",
    "bash",
    "zsh",
    "csh",
    "ksh",
    "fish",
    "command",
    "terminal",
    "tool",
    "app",
    "pkg",
    "mpkg",
    "dmg",
    "scpt",
    "scptd",
    "applescript",
    "workflow",
    "action",
    "fileloc",
    "desktop",
    "appimage",
    "run",
    "bin",
    "deb",
    "rpm",
    "snap",
    "flatpak",
    "flatpakref",
];

/// Whether opening a file with this name could run code, so it must not be opened for the user.
/// Windows drops trailing dots and spaces from a name and `:` starts a data stream, so
/// `evil.exe.` and `evil.exe:x` count as `.exe`.
pub fn is_runnable_name(name: &str) -> bool {
    let name = name.split(':').next().unwrap_or_default();
    let name = name.trim_end_matches(['.', ' ']);
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    ext.is_some_and(|ext| RUNNABLE.contains(&ext.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("temp dir")
    }

    fn file(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, bytes).expect("write");
        path
    }

    fn name_of(path: &Path) -> String {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    #[test]
    fn a_file_is_copied_under_a_hash_prefixed_name() {
        let src = dir();
        let section = dir();
        let path = file(&src, "budget.xlsx", b"numbers");
        let stored = store_attachment(section.path(), &path).expect("store");
        assert!(!stored.linked);
        assert_eq!(stored.display_name, "budget.xlsx");
        assert_eq!(stored.size, 7);
        let name = name_of(&stored.path);
        assert!(name.ends_with("-budget.xlsx"), "{name}");
        assert_eq!(name.len(), "000000-budget.xlsx".len());
        assert_eq!(stored.link, format!("attachments/{name}"));
        assert_eq!(fs::read(&stored.path).expect("read"), b"numbers");
        assert!(stored.path.starts_with(section.path().join("attachments")));
    }

    #[test]
    fn the_same_bytes_reuse_the_stored_file_and_other_bytes_never_overwrite() {
        let src = dir();
        let section = dir();
        let a = file(&src, "note.txt", b"one");
        let first = store_attachment(section.path(), &a).expect("first");
        let again = store_attachment(section.path(), &a).expect("again");
        assert_eq!(first.path, again.path);
        // A file the user changed under the name this one would get is not touched.
        fs::write(&first.path, b"mine").expect("user edit");
        let next = store_attachment(section.path(), &a).expect("again after edit");
        assert_ne!(next.path, first.path);
        assert_eq!(fs::read(&first.path).expect("read"), b"mine");
        assert_eq!(fs::read(&next.path).expect("read"), b"one");
        assert_eq!(
            name_of(&next.path),
            name_of(&first.path).replace(".txt", " 2.txt")
        );
    }

    #[test]
    fn names_are_sanitized_and_kept_short() {
        let src = dir();
        let section = dir();
        let long = format!("{}.pdf", "x".repeat(400));
        let path = file(&src, "plain", b"a");
        let stored = store_attachment_bytes(section.path(), "a<b>:c.txt", b"hi").expect("bytes");
        let name = name_of(&stored.path);
        assert!(name.ends_with("-a-b--c.txt"), "{name}");
        let stored = store_attachment_bytes(section.path(), &long, b"hi").expect("long");
        let name = name_of(&stored.path);
        assert!(
            name.len() <= MAX_FILE_NAME_BYTES && name.ends_with(".pdf"),
            "{name}"
        );
        let stored = store_attachment(section.path(), &path).expect("no extension");
        assert!(name_of(&stored.path).ends_with("-plain"));
        let con = store_attachment_bytes(section.path(), "CON", b"x").expect("device name");
        assert!(name_of(&con.path).ends_with("-CON_"));
    }

    #[test]
    fn big_files_are_linked_not_copied() {
        let src = dir();
        let section = dir();
        let path = file(&src, "big file (1).bin", &[7_u8; 64]);
        let stored = store_attachment_with(section.path(), &path, 10).expect("link");
        assert!(stored.linked);
        assert!(!section.path().join("attachments").exists());
        assert!(
            stored.link.contains("big%20file%20%281%29.bin"),
            "{}",
            stored.link
        );
        assert!(!stored.link.contains('\\'));
        assert_eq!(
            name_of(Path::new(&decode_link_path(&stored.link))),
            "big file (1).bin"
        );
    }

    #[test]
    fn missing_sources_and_folders_are_errors() {
        let section = dir();
        let missing = section.path().join("nope.txt");
        assert!(matches!(
            store_attachment(section.path(), &missing),
            Err(AttachError::NotAFile)
        ));
        assert!(matches!(
            store_attachment(section.path(), section.path()),
            Err(AttachError::NotAFile)
        ));
    }

    #[test]
    fn link_paths_round_trip() {
        for text in [
            "attachments/a.png",
            "attachments/a b (c).png",
            "attachments/caf\u{e9} \u{4e2d}.txt",
            "C:/Users/me/50%.txt",
        ] {
            assert_eq!(decode_link_path(&encode_link_path(text)), text);
        }
        assert_eq!(encode_link_path("a b"), "a%20b");
        // Stray and truncated escapes are kept.
        assert_eq!(decode_link_path("100%"), "100%");
        assert_eq!(decode_link_path("%zz%4"), "%zz%4");
    }

    #[test]
    fn picture_and_runnable_names() {
        assert!(
            is_picture_name("A.PNG")
                && is_picture_name("x.jpeg")
                && is_picture_name("y.webp")
                && is_picture_name("z.SVG")
        );
        assert!(!is_picture_name("x") && !is_picture_name("x.txt"));
        assert!(is_runnable_name("setup.EXE") && is_runnable_name("a.sh"));
        assert!(!is_runnable_name("a.xlsx") && !is_runnable_name("exe"));
        for sneaky in [
            "evil.exe.",
            "evil.exe . .",
            "evil.exe:zone",
            "invoice.pdf.scr",
            "link.URL",
            "run.py",
            "disk.iso",
        ] {
            assert!(is_runnable_name(sneaky), "{sneaky}");
        }
        for fine in ["notes.pdf", "photo.png", "budget.xlsx", "a.txt."] {
            assert!(!is_runnable_name(fine), "{fine}");
        }
    }
}
