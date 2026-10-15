//! The installed font families, for the ribbon's font box. Only each file's table directory
//! and `name` table are read, never the whole font, and the scan runs on a background thread
//! so a large font folder never holds up the window. Until it finishes, or if it finds
//! nothing, the font box offers a short fixed list.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use skrifa::raw::tables::name::Name;
use skrifa::raw::{FontData, FontRead};

/// Most font files looked at in one scan.
const MAX_FILES: usize = 10_000;
/// Most tables a font's directory may list; real fonts have a few dozen.
const MAX_TABLES: u16 = 512;
/// Most fonts read from one collection file.
const MAX_IN_COLLECTION: u32 = 64;
/// Largest `name` table read.
const MAX_NAME_TABLE: u32 = 1024 * 1024;
/// Deepest folder level searched under a font folder.
const MAX_DEPTH: usize = 6;

/// The family list, filled in by a background scan.
#[derive(Clone, Default)]
pub struct FontList {
    families: Arc<Mutex<Option<Vec<String>>>>,
}

impl std::fmt::Debug for FontList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontList")
            .field("families", &self.families().map(|list| list.len()))
            .finish()
    }
}

impl FontList {
    /// An empty list that is never filled (tests, and builds with no font folders).
    pub fn none() -> Self {
        Self::default()
    }

    /// A list that is already known (tests).
    pub fn from_families(families: Vec<String>) -> Self {
        Self {
            families: Arc::new(Mutex::new(Some(families))),
        }
    }

    /// Start scanning the OS font folders on a background thread. `on_done` runs when the
    /// list is ready (the app asks for a repaint).
    pub fn scan_in_background(on_done: impl FnOnce() + Send + 'static) -> Self {
        let list = Self::default();
        let slot = Arc::clone(&list.families);
        let spawned = std::thread::Builder::new()
            .name("font-list".to_owned())
            .spawn(move || {
                let families = scan(&font_folders());
                tracing::info!(families = families.len(), "installed fonts listed");
                if let Ok(mut slot) = slot.lock() {
                    *slot = Some(families);
                }
                on_done();
            });
        if let Err(err) = spawned {
            tracing::warn!(error = %err, "font list scan not started; using the short list");
        }
        list
    }

    /// The families found, sorted, or `None` while the scan runs or when it found nothing.
    pub fn families(&self) -> Option<Vec<String>> {
        let slot = self.families.lock().ok()?;
        slot.as_ref().filter(|list| !list.is_empty()).cloned()
    }
}

/// The family names in every font file under `folders`, sorted and without repeats.
pub fn scan(folders: &[PathBuf]) -> Vec<String> {
    let mut files = Vec::new();
    for folder in folders {
        collect(folder, 0, &mut files);
    }
    let mut names = BTreeSet::new();
    for file in &files {
        for name in families_in(file) {
            names.insert(name);
        }
    }
    let mut sorted: Vec<String> = names.into_iter().collect();
    sorted.sort_by_key(|name| name.to_lowercase());
    sorted
}

fn collect(folder: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MAX_FILES {
            return;
        }
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => collect(&path, depth + 1, out),
            Ok(_) if is_font_file(&path) => out.push(path),
            _ => {}
        }
    }
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["ttf", "otf", "ttc", "otc"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

/// The family names of the fonts in one file; empty when it cannot be read.
pub fn families_in(path: &Path) -> Vec<String> {
    let Ok(mut file) = File::open(path) else {
        return Vec::new();
    };
    let Some(header) = read_at(&mut file, 0, 12) else {
        return Vec::new();
    };
    let offsets = if header.get(0..4) == Some(b"ttcf") {
        let count = be_u32(&header, 8).unwrap_or(0).min(MAX_IN_COLLECTION);
        let Some(table) = read_at(&mut file, 12, count as usize * 4) else {
            return Vec::new();
        };
        (0..count as usize)
            .filter_map(|i| be_u32(&table, i * 4))
            .collect()
    } else {
        vec![0]
    };
    offsets
        .into_iter()
        .filter_map(|offset| family_at(&mut file, u64::from(offset)))
        .collect()
}

/// The family name of the font whose table directory starts at `offset`.
fn family_at(file: &mut File, offset: u64) -> Option<String> {
    let header = read_at(file, offset, 12)?;
    let tables = be_u16(&header, 4)?.min(MAX_TABLES);
    let records = read_at(file, offset + 12, usize::from(tables) * 16)?;
    let (start, length) = records.as_chunks::<16>().0.iter().find_map(|record| {
        (record.get(0..4) == Some(b"name")).then(|| (be_u32(record, 8), be_u32(record, 12)))
    })?;
    let (start, length) = (start?, length?.min(MAX_NAME_TABLE));
    let table = read_at(file, u64::from(start), length as usize)?;
    family_name(&table)
}

/// The best family name in a `name` table: the typographic family (16) before the legacy
/// family (1), English (Windows or Mac) before any other language. Names starting with a
/// dot are hidden system fonts.
pub fn family_name(table: &[u8]) -> Option<String> {
    let name = Name::read(FontData::new(table)).ok()?;
    let data = name.string_data();
    let mut best: Option<(u8, String)> = None;
    for record in name.name_record() {
        let id = record.name_id().to_u16();
        if id != 1 && id != 16 {
            continue;
        }
        let english = matches!(
            (record.platform_id(), record.language_id()),
            (3, 0x0409) | (1, 0)
        );
        let rank = u8::from(id == 16) * 2 + u8::from(english);
        if best.as_ref().is_some_and(|(have, _)| *have >= rank) {
            continue;
        }
        let Ok(text) = record.string(data) else {
            continue;
        };
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        let text = text.trim();
        if !text.is_empty() && !text.starts_with('.') {
            best = Some((rank, text.to_owned()));
        }
    }
    best.map(|(_, name)| name)
}

fn read_at(file: &mut File, offset: u64, len: usize) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut buffer = vec![0; len];
    file.read_exact(&mut buffer).ok()?;
    Some(buffer)
}

fn be_u16(bytes: &[u8], at: usize) -> Option<u16> {
    let pair = bytes.get(at..at + 2)?;
    Some(u16::from_be_bytes([*pair.first()?, *pair.get(1)?]))
}

fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let quad: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
    Some(u32::from_be_bytes(quad))
}

/// Where the OS keeps fonts, for every user and for this one.
fn font_folders() -> Vec<PathBuf> {
    let mut folders = Vec::new();
    let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
    if cfg!(target_os = "windows") {
        if let Some(windows) = env("WINDIR").or_else(|| env("SystemRoot")) {
            folders.push(windows.join("Fonts"));
        }
        if let Some(local) = env("LOCALAPPDATA") {
            folders.push(local.join("Microsoft").join("Windows").join("Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        folders.push(PathBuf::from("/System/Library/Fonts"));
        folders.push(PathBuf::from("/Library/Fonts"));
        if let Some(home) = env("HOME") {
            folders.push(home.join("Library").join("Fonts"));
        }
    } else {
        folders.push(PathBuf::from("/usr/share/fonts"));
        folders.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = env("HOME") {
            folders.push(home.join(".local").join("share").join("fonts"));
            folders.push(home.join(".fonts"));
        }
    }
    folders
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `name` table with the given (platform, language, name id, UTF-16 text) records.
    fn name_table(records: &[(u16, u16, u16, &str)]) -> Vec<u8> {
        let mut strings = Vec::new();
        let mut table = Vec::new();
        let count = records.len() as u16;
        table.extend(0u16.to_be_bytes());
        table.extend(count.to_be_bytes());
        table.extend((6 + 12 * count).to_be_bytes());
        for (platform, language, id, text) in records {
            let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
            let encoding: u16 = if *platform == 3 { 1 } else { 0 };
            for field in [
                *platform,
                encoding,
                *language,
                *id,
                bytes.len() as u16,
                strings.len() as u16,
            ] {
                table.extend(field.to_be_bytes());
            }
            strings.extend(bytes);
        }
        table.extend(strings);
        table
    }

    #[test]
    fn prefers_the_typographic_english_family() {
        let table = name_table(&[
            (3, 0x0407, 1, "Familie"),
            (3, 0x0409, 1, "Example Light"),
            (3, 0x0409, 16, "Example"),
        ]);
        assert_eq!(family_name(&table).as_deref(), Some("Example"));
        let legacy = name_table(&[(3, 0x0409, 1, "Only Legacy")]);
        assert_eq!(family_name(&legacy).as_deref(), Some("Only Legacy"));
    }

    #[test]
    fn hidden_empty_and_damaged_names_are_skipped() {
        assert_eq!(family_name(&name_table(&[(3, 0x0409, 1, ".SF NS")])), None);
        assert_eq!(family_name(&name_table(&[(3, 0x0409, 1, "  ")])), None);
        assert_eq!(family_name(&[0, 0, 0, 9]), None);
        assert_eq!(family_name(&[]), None);
    }

    #[test]
    fn unreadable_files_give_no_families() {
        let dir = std::env::temp_dir().join(format!("notecraft-font-list-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let junk = dir.join("junk.ttf");
        let _ = std::fs::write(&junk, b"not a font at all");
        assert!(families_in(&junk).is_empty());
        assert!(families_in(&dir.join("missing.ttf")).is_empty());
        assert!(scan(&[dir.join("no such folder")]).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn this_machines_fonts_list_without_failing() {
        let families = scan(&font_folders());
        let mut sorted = families.clone();
        sorted.sort_by_key(|name| name.to_lowercase());
        assert_eq!(families, sorted);
    }
}
