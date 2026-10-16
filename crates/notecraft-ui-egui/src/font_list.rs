//! The installed font families, for the ribbon's font box. Only each file's table directory
//! and `name` table are read, never the whole font, and the scan runs on a background thread
//! so a large font folder never holds up the window. Until it finishes, or if it finds
//! nothing, the font box offers a short fixed list.
//!
//! The font box shows each family in its own face once that face is loaded into egui:
//! [`request_preview`] asks for one and [`FontList::load_previews`] loads a few per
//! frame, small files only and within a memory budget. Drawing checks that the face is
//! installed right now (a theme change reinstalls egui's fonts), so a row never names a
//! family egui does not have.

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use egui::{FontFamily, FontId};
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
/// Largest font file loaded for a preview; bigger faces (CJK) show in the UI font.
const MAX_PREVIEW_BYTES: u64 = 4 * 1024 * 1024;
/// Most preview faces read in one batch; a screenful of the list.
const PREVIEWS_PER_BATCH: usize = 16;
/// Most font data loaded for previews in one run.
const PREVIEW_BUDGET: u64 = 64 * 1024 * 1024;

/// Where a family's face is: the file and the face index inside a collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub path: PathBuf,
    pub index: u32,
}

/// What a scan found.
#[derive(Debug, Default)]
struct Found {
    names: Vec<String>,
    faces: BTreeMap<String, Face>,
}

/// The family list, filled in by a background scan.
#[derive(Clone, Default)]
pub struct FontList {
    found: Arc<Mutex<Option<Arc<Found>>>>,
    /// Faces a preview worker finished reading, waiting to be installed.
    ready: Arc<Mutex<Vec<PreviewRead>>>,
    /// A preview worker is running.
    busy: Arc<AtomicBool>,
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

    /// A list that is already known, with no faces to load (tests).
    pub fn from_families(families: Vec<String>) -> Self {
        Self::from_found(Found {
            names: families,
            faces: BTreeMap::new(),
        })
    }

    fn from_found(found: Found) -> Self {
        Self {
            found: Arc::new(Mutex::new(Some(Arc::new(found)))),
            ..Self::default()
        }
    }

    /// Start scanning the OS font folders on a background thread. `on_done` runs when the
    /// list is ready (the app asks for a repaint).
    pub fn scan_in_background(on_done: impl FnOnce() + Send + 'static) -> Self {
        let list = Self::default();
        let slot = Arc::clone(&list.found);
        let spawned = std::thread::Builder::new()
            .name("font-list".to_owned())
            .spawn(move || {
                let found = scan_faces(&font_folders());
                tracing::info!(families = found.names.len(), "installed fonts listed");
                if let Ok(mut slot) = slot.lock() {
                    *slot = Some(Arc::new(found));
                }
                on_done();
            });
        if let Err(err) = spawned {
            tracing::warn!(error = %err, "font list scan not started; using the short list");
        }
        list
    }

    fn found(&self) -> Option<Arc<Found>> {
        self.found.lock().ok()?.clone()
    }

    /// The families found, sorted, or `None` while the scan runs or when it found nothing.
    pub fn families(&self) -> Option<Vec<String>> {
        self.found()
            .filter(|found| !found.names.is_empty())
            .map(|found| found.names.clone())
    }

    /// Where `family`'s face is, if the scan found it.
    pub fn face(&self, family: &str) -> Option<Face> {
        self.found()?.faces.get(family).cloned()
    }

    /// Turn requested previews into installed faces (call once per frame). A worker thread
    /// reads and checks the files; this installs every face it finished in one frame, so egui
    /// rebuilds its fonts once per batch, then starts the next batch. A face that cannot be
    /// read, is too big, would pass the budget, or cannot draw its own name (symbol fonts) is
    /// marked failed and shows in the UI font.
    pub fn load_previews(&self, ctx: &egui::Context) {
        let ready = self
            .ready
            .lock()
            .map(|mut ready| std::mem::take(&mut *ready))
            .unwrap_or_default();
        let mut used = ctx.data(|data| data.get_temp::<u64>(budget_id()).unwrap_or(0));
        for read in ready {
            match read {
                PreviewRead::Face { name, bytes, index } => {
                    let size = bytes.len() as u64;
                    if used + size > PREVIEW_BUDGET {
                        mark(ctx, failed_id(), &name);
                        continue;
                    }
                    used += size;
                    install(ctx, &name, bytes, index);
                }
                PreviewRead::Failed(name) => mark(ctx, failed_id(), &name),
            }
        }
        ctx.data_mut(|data| data.insert_temp(budget_id(), used));

        if self.busy.load(Ordering::Acquire) {
            return;
        }
        let batch: Vec<(String, Option<Face>)> = ctx
            .data_mut(|data| {
                let queue = data.get_temp_mut_or_default::<Vec<String>>(queue_id());
                let take = queue.len().min(PREVIEWS_PER_BATCH);
                queue.drain(..take).collect::<Vec<String>>()
            })
            .into_iter()
            .filter_map(|name| match self.face(&name) {
                Some(face) => Some((name, Some(face))),
                // No file to read: settle it here, without a worker or a repaint.
                None => {
                    mark(ctx, failed_id(), &name);
                    None
                }
            })
            .collect();
        if batch.is_empty() {
            return;
        }
        self.busy.store(true, Ordering::Release);
        let (ready, busy, waker) = (Arc::clone(&self.ready), Arc::clone(&self.busy), ctx.clone());
        let names: Vec<String> = batch.iter().map(|(name, _)| name.clone()).collect();
        let spawned = std::thread::Builder::new()
            .name("font-preview".to_owned())
            .spawn(move || {
                let reads: Vec<PreviewRead> = batch
                    .into_iter()
                    .map(|(name, face)| read_face(name, face))
                    .collect();
                if let Ok(mut slot) = ready.lock() {
                    slot.extend(reads);
                }
                busy.store(false, Ordering::Release);
                waker.request_repaint();
            });
        if spawned.is_err() {
            self.busy.store(false, Ordering::Release);
            for name in names {
                mark(ctx, failed_id(), &name);
            }
        }
    }
}

/// A preview face the worker read, or a name it could not use.
enum PreviewRead {
    Face {
        name: String,
        bytes: Vec<u8>,
        index: u32,
    },
    Failed(String),
}

/// Read and check one face off the UI thread.
fn read_face(name: String, face: Option<Face>) -> PreviewRead {
    let bytes = face.and_then(|face| {
        let size = std::fs::metadata(&face.path).ok()?.len();
        if size > MAX_PREVIEW_BYTES {
            return None;
        }
        let bytes = std::fs::read(&face.path).ok()?;
        draws_name(&bytes, face.index, &name).then_some((bytes, face.index))
    });
    match bytes {
        Some((bytes, index)) => PreviewRead::Face { name, bytes, index },
        None => PreviewRead::Failed(name),
    }
}

/// Whether the face at `index` in `bytes` parses and has a glyph for every letter and digit
/// of `name`. Symbol fonts do not, and would draw an empty row.
fn draws_name(bytes: &[u8], index: u32, name: &str) -> bool {
    use skrifa::MetadataProvider;
    let Ok(font) = skrifa::FontRef::from_index(bytes, index) else {
        return false;
    };
    let charmap = font.charmap();
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .all(|c| charmap.map(c).is_some())
}

/// Add a preview face to egui under its own family.
fn install(ctx: &egui::Context, name: &str, bytes: Vec<u8>, index: u32) {
    let key = preview_key(name);
    let mut data = egui::FontData::from_owned(bytes);
    data.index = index;
    ctx.add_font(egui::epaint::text::FontInsert::new(
        &key,
        data,
        vec![egui::epaint::text::InsertFontFamily {
            family: FontFamily::Name(key.clone().into()),
            priority: egui::epaint::text::FontPriority::Highest,
        }],
    ));
}

fn queue_id() -> egui::Id {
    egui::Id::new("font_preview_queue")
}

fn failed_id() -> egui::Id {
    egui::Id::new("font_preview_failed")
}

fn budget_id() -> egui::Id {
    egui::Id::new("font_preview_budget")
}

fn mark(ctx: &egui::Context, set: egui::Id, name: &str) {
    ctx.data_mut(|data| {
        data.get_temp_mut_or_default::<HashSet<String>>(set)
            .insert(name.to_owned());
    });
}

fn preview_key(name: &str) -> String {
    format!("preview:{name}")
}

/// The font to draw `family`'s name in: its own face when egui has it installed now,
/// otherwise the UI font, and the face is requested (unless it failed before).
pub fn preview_font(ctx: &egui::Context, family: &str, size: f32) -> FontId {
    let key = FontFamily::Name(preview_key(family).into());
    if ctx.fonts(|fonts| fonts.definitions().families.contains_key(&key)) {
        return FontId::new(size, key);
    }
    ctx.data_mut(|data| {
        let failed = data
            .get_temp::<HashSet<String>>(failed_id())
            .is_some_and(|set| set.contains(family));
        let queue = data.get_temp_mut_or_default::<Vec<String>>(queue_id());
        if !failed && !queue.iter().any(|queued| queued == family) {
            queue.push(family.to_owned());
        }
    });
    FontId::proportional(size)
}

/// The family names in every font file under `folders`, sorted and without repeats.
pub fn scan(folders: &[PathBuf]) -> Vec<String> {
    scan_faces(folders).names
}

/// The families under `folders` and a face for each: the one in the file with the shortest
/// name, usually the regular face (`arial.ttf` before `arialbd.ttf`).
fn scan_faces(folders: &[PathBuf]) -> Found {
    let mut files = Vec::new();
    for folder in folders {
        collect(folder, 0, &mut files);
    }
    let mut faces: BTreeMap<String, Face> = BTreeMap::new();
    for path in &files {
        for (name, index) in faces_in(path) {
            let shorter = |old: &Face| {
                let len = |p: &Path| p.file_name().map_or(usize::MAX, |n| n.len());
                len(path) < len(&old.path)
            };
            if faces.get(&name).is_none_or(shorter) {
                faces.insert(
                    name,
                    Face {
                        path: path.clone(),
                        index,
                    },
                );
            }
        }
    }
    let mut names: Vec<String> = faces.keys().cloned().collect();
    names.sort_by_key(|name| name.to_lowercase());
    Found { names, faces }
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
    faces_in(path).into_iter().map(|(name, _)| name).collect()
}

/// The family name and face index of each font in one file.
fn faces_in(path: &Path) -> Vec<(String, u32)> {
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
        .zip(0_u32..)
        .filter_map(|(offset, index)| Some((family_at(&mut file, u64::from(offset))?, index)))
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

    /// One egui pass running `f`.
    fn pass(ctx: &egui::Context, mut f: impl FnMut(&egui::Context)) {
        ctx.begin_pass(egui::RawInput::default());
        f(ctx);
        // A test has no renderer to hand the font atlas to.
        ctx.end_pass().textures_delta.clear();
    }

    /// Run frames until the preview worker is done and its faces are installed.
    fn settle(ctx: &egui::Context, list: &FontList) {
        for _ in 0..200 {
            pass(ctx, |ctx| list.load_previews(ctx));
            let queued =
                ctx.data(|data| data.get_temp::<Vec<String>>(queue_id()).unwrap_or_default());
            let waiting = list.ready.lock().map(|r| r.len()).unwrap_or(0);
            if queued.is_empty() && waiting == 0 && !list.busy.load(Ordering::Acquire) {
                pass(ctx, |_| {});
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn symbol_faces_without_their_name_glyphs_are_refused() {
        let defs = egui::FontDefinitions::default();
        let bytes = defs
            .font_data
            .values()
            .next()
            .map(|data| data.font.to_vec())
            .unwrap_or_default();
        assert!(draws_name(&bytes, 0, "Sample 12"));
        assert!(
            !draws_name(&bytes, 0, "\u{4E2D}\u{6587}"),
            "no CJK glyphs in egui's font"
        );
        assert!(!draws_name(b"junk", 0, "Sample"));
    }

    #[test]
    fn previews_load_a_requested_face_and_fall_back_until_then() {
        let dir = std::env::temp_dir().join(format!("notecraft-preview-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        // egui's own UI font is a real font file to load.
        let defs = egui::FontDefinitions::default();
        let bytes = defs
            .font_data
            .values()
            .next()
            .map(|data| data.font.to_vec())
            .unwrap_or_default();
        let path = dir.join("sample.ttf");
        let _ = std::fs::write(&path, &bytes);
        let mut faces = BTreeMap::new();
        faces.insert("Sample".to_owned(), Face { path, index: 0 });
        faces.insert(
            "Missing".to_owned(),
            Face {
                path: dir.join("missing.ttf"),
                index: 0,
            },
        );
        let list = FontList::from_found(Found {
            names: vec!["Missing".to_owned(), "Sample".to_owned()],
            faces,
        });

        let ctx = egui::Context::default();
        let mut font = None;
        pass(&ctx, |ctx| font = Some(preview_font(ctx, "Sample", 13.0)));
        assert_eq!(
            font.clone().map(|f| f.family),
            Some(FontFamily::Proportional)
        );
        settle(&ctx, &list);
        pass(&ctx, |ctx| font = Some(preview_font(ctx, "Sample", 13.0)));
        assert_eq!(
            font.clone().map(|f| f.family),
            Some(FontFamily::Name("preview:Sample".into()))
        );

        // A face that cannot be read stays in the UI font and is not asked for again.
        pass(&ctx, |ctx| font = Some(preview_font(ctx, "Missing", 13.0)));
        settle(&ctx, &list);
        pass(&ctx, |ctx| font = Some(preview_font(ctx, "Missing", 13.0)));
        assert_eq!(
            font.clone().map(|f| f.family),
            Some(FontFamily::Proportional)
        );
        let queued = ctx.data(|data| data.get_temp::<Vec<String>>(queue_id()));
        assert_eq!(queued, Some(Vec::new()));
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
