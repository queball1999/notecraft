//! The OS UI font (docs/themes.md section 2). NoteCraft bundles no UI font: it reads the
//! font the OS uses for its own UI and falls back to egui's built-in font when that font is
//! missing or unreadable.
//!
//! | OS | UI font | Microsoft 365 preset |
//! |---|---|---|
//! | Windows | Segoe UI | Segoe UI Variable when present |
//! | macOS | the system font (San Francisco) | same |
//! | Linux | fontconfig `sans-serif` | same |

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::epaint::text::VariationCoords;
use egui::{FontData, FontDefinitions, FontFamily, FontTweak};
use skrifa::{attribute::Style, MetadataProvider};

use super::Preset;

/// Largest font file read, in bytes. System UI fonts are a few MiB at most.
pub const MAX_FONT_BYTES: u64 = 40 * 1024 * 1024;

/// The `FontDefinitions` key of the OS font in use.
const OS_FONT: &str = "os-ui";

/// The `FontDefinitions` key of the bold face.
const BOLD_FONT: &str = "os-ui-bold";

/// Name of the egui font family that holds the bold face (the bold face first, then the
/// regular families as fallback). Text on the page asks for it with [`bold_family`].
pub const BOLD_FAMILY: &str = "notecraft:bold";

/// Faces looked at inside one font file (a collection holds several).
const MAX_FACES: u32 = 32;

/// Weight of the bold face of a variable font.
const BOLD_WEIGHT: f32 = 700.0;

/// The egui family for bold text.
pub fn bold_family() -> FontFamily {
    FontFamily::Name(BOLD_FAMILY.into())
}

/// The OS UI fonts that were found and can be parsed. Either may be missing.
#[derive(Clone, Default)]
pub struct OsFonts {
    ui: Option<Arc<FontData>>,
    /// A variable version of the UI font, preferred by the Microsoft 365 preset.
    variable: Option<Arc<FontData>>,
    /// A static bold face of the UI font (a variable UI font makes its own bold).
    bold: Option<Arc<FontData>>,
}

impl std::fmt::Debug for OsFonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OsFonts")
            .field("ui", &self.ui.is_some())
            .field("variable", &self.variable.is_some())
            .field("bold", &self.bold.is_some())
            .finish()
    }
}

impl OsFonts {
    /// Look for the OS UI fonts. Never fails: anything missing or unreadable is skipped.
    pub fn load() -> Self {
        let (ui, variable) = candidates();
        let mut fonts = Self::from_bytes(first_usable(&ui), first_usable(&variable));
        fonts.bold = bold_candidates().iter().find_map(|path| {
            let face = bold_face(read_font_bytes(path)?)?;
            tracing::info!(font = %path.display(), "using the OS bold face");
            Some(Arc::new(face))
        });
        fonts
    }

    /// Add a bold face from raw file bytes. The font must be a bold face (OS/2 weight 600 or
    /// more, not italic) or a variable font with a weight axis; anything else is dropped.
    pub fn with_bold_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.bold = bold_face(bytes).map(Arc::new);
        self
    }

    /// Whether `definitions` will register a bold family for `preset`.
    pub fn has_bold(&self, preset: Preset) -> bool {
        self.bold_for(self.regular(preset).as_ref()).is_some()
    }

    /// Fonts from raw file bytes; bytes that do not parse as a font are dropped.
    pub fn from_bytes(ui: Option<Vec<u8>>, variable: Option<Vec<u8>>) -> Self {
        let keep = |bytes: Option<Vec<u8>>| {
            bytes
                .filter(|b| is_usable(b))
                .map(|b| Arc::new(FontData::from_owned(b)))
        };
        Self {
            ui: keep(ui),
            variable: keep(variable),
            bold: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ui.is_none() && self.variable.is_none()
    }

    /// Which font `preset` uses: `Some(true)` for the variable font, `Some(false)` for the
    /// regular one, `None` for egui's built-in font.
    pub fn choice(&self, preset: Preset) -> Option<bool> {
        let prefer_variable = preset == Preset::M365;
        match (&self.ui, &self.variable) {
            (_, Some(_)) if prefer_variable => Some(true),
            (Some(_), _) => Some(false),
            (None, Some(_)) => Some(true),
            (None, None) => None,
        }
    }

    /// The regular font `preset` uses, if any.
    fn regular(&self, preset: Preset) -> Option<Arc<FontData>> {
        match self.choice(preset) {
            Some(true) => self.variable.clone(),
            Some(false) => self.ui.clone(),
            None => None,
        }
    }

    /// The bold face that goes with `regular`: the variable font at weight 700, else the
    /// static bold file. `None` when neither exists.
    fn bold_for(&self, regular: Option<&Arc<FontData>>) -> Option<FontData> {
        regular
            .and_then(|font| variable_bold(font))
            .or_else(|| self.bold.as_ref().map(|font| (**font).clone()))
    }

    /// egui's default fonts with the OS UI font for `preset` first in the proportional
    /// family. egui's own fonts stay behind it for any character the OS font lacks. When a
    /// bold face is known, [`bold_family`] holds it followed by the proportional fonts.
    pub fn definitions(&self, preset: Preset) -> FontDefinitions {
        let mut definitions = FontDefinitions::default();
        let font = self.regular(preset);
        let bold = self.bold_for(font.as_ref());
        if let Some(font) = font {
            definitions.font_data.insert(OS_FONT.to_owned(), font);
            definitions
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, OS_FONT.to_owned());
        }
        if let Some(bold) = bold {
            definitions
                .font_data
                .insert(BOLD_FONT.to_owned(), Arc::new(bold));
            let mut names = vec![BOLD_FONT.to_owned()];
            if let Some(regular) = definitions.families.get(&FontFamily::Proportional) {
                names.extend(regular.iter().cloned());
            }
            definitions.families.insert(bold_family(), names);
        }
        definitions
    }
}

/// `font` at weight 700 when it is a variable font with a weight axis.
fn variable_bold(font: &FontData) -> Option<FontData> {
    let axis = font
        .variation_axes()
        .into_iter()
        .find(|axis| axis.tag.to_be_bytes() == *b"wght")?;
    let weight = BOLD_WEIGHT.clamp(axis.range.min, axis.range.max);
    // A font whose axis tops out below semi-bold has no real bold.
    if weight < 600.0 {
        return None;
    }
    Some(font.clone().tweak(FontTweak {
        coords: VariationCoords::new([(b"wght", weight)]),
        ..FontTweak::default()
    }))
}

/// The bold face in the font file `bytes`: a variable font at weight 700, or the first face
/// that is bold (OS/2 weight 600 or more) and upright. Looks at the faces of a collection.
fn bold_face(bytes: Vec<u8>) -> Option<FontData> {
    let mut found = None;
    for index in 0..MAX_FACES {
        let Ok(font) = skrifa::FontRef::from_index(&bytes, index) else {
            break;
        };
        if font.charmap().map('a').is_none() {
            continue;
        }
        let attributes = font.attributes();
        let variable = font
            .axes()
            .iter()
            .any(|a| a.tag().to_be_bytes() == *b"wght");
        if variable || (attributes.weight.value() >= 600.0 && attributes.style == Style::Normal) {
            found = Some(index);
            break;
        }
    }
    let index = found?;
    let data = FontData {
        index,
        ..FontData::from_owned(bytes)
    };
    if data
        .variation_axes()
        .iter()
        .any(|a| a.tag.to_be_bytes() == *b"wght")
    {
        variable_bold(&data)
    } else {
        Some(data)
    }
}

/// Whether egui can load `bytes`. epaint panics on font data it cannot parse, so every
/// OS font is checked with the same parser (skrifa) before egui sees it.
pub fn is_usable(bytes: &[u8]) -> bool {
    skrifa::FontRef::from_index(bytes, 0).is_ok_and(|font| font.charmap().map('a').is_some())
}

fn first_usable(paths: &[PathBuf]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| read_font(path))
}

/// A font file's bytes, or `None` when it is missing, empty, or over [`MAX_FONT_BYTES`].
fn read_font_bytes(path: &Path) -> Option<Vec<u8>> {
    let len = std::fs::metadata(path).ok()?.len();
    if len == 0 || len > MAX_FONT_BYTES {
        return None;
    }
    std::fs::read(path).ok()
}

fn read_font(path: &Path) -> Option<Vec<u8>> {
    let bytes = read_font_bytes(path)?;
    if is_usable(&bytes) {
        tracing::info!(font = %path.display(), "using the OS UI font");
        Some(bytes)
    } else {
        tracing::warn!(font = %path.display(), "OS UI font could not be parsed; skipped");
        None
    }
}

/// Paths to try for the regular and the variable UI font, best first.
#[cfg(windows)]
fn candidates() -> (Vec<PathBuf>, Vec<PathBuf>) {
    let fonts = std::env::var_os("WINDIR")
        .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
        .join("Fonts");
    (
        vec![fonts.join("segoeui.ttf")],
        vec![fonts.join("SegUIVar.ttf")],
    )
}

/// Paths to try for the bold face of the UI font, best first. Each file is checked for a
/// bold face (or a weight axis) before it is used.
#[cfg(windows)]
fn bold_candidates() -> Vec<PathBuf> {
    let fonts = std::env::var_os("WINDIR")
        .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
        .join("Fonts");
    vec![fonts.join("segoeuib.ttf")]
}

/// The system font is variable (it makes its own bold from the weight axis); Helvetica.ttc
/// holds a bold face for the case where it is the regular font instead.
#[cfg(target_os = "macos")]
fn bold_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/System/Library/Fonts/SFNS.ttf"),
        PathBuf::from("/System/Library/Fonts/Helvetica.ttc"),
    ]
}

/// fontconfig names the file behind the desktop's bold sans-serif font (DejaVu Sans Bold,
/// Noto Sans Bold, Cantarell Bold, or a variable font with a weight axis).
#[cfg(all(unix, not(target_os = "macos")))]
fn bold_candidates() -> Vec<PathBuf> {
    fc_match_file("sans-serif:bold").into_iter().collect()
}

#[cfg(not(any(windows, unix)))]
fn bold_candidates() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn fc_match_file(pattern: &str) -> Option<PathBuf> {
    std::process::Command::new("fc-match")
        .args(["--format=%{file}", pattern])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|file| file.trim().to_owned())
        .filter(|file| !file.is_empty() && file.len() < 4096)
        .map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn candidates() -> (Vec<PathBuf>, Vec<PathBuf>) {
    (
        vec![
            PathBuf::from("/System/Library/Fonts/SFNS.ttf"),
            PathBuf::from("/System/Library/Fonts/Helvetica.ttc"),
        ],
        Vec::new(),
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn candidates() -> (Vec<PathBuf>, Vec<PathBuf>) {
    // fontconfig names the file behind the desktop's sans-serif font.
    (
        fc_match_file("sans-serif").into_iter().collect(),
        Vec::new(),
    )
}

#[cfg(not(any(windows, unix)))]
fn candidates() -> (Vec<PathBuf>, Vec<PathBuf>) {
    (Vec::new(), Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real font: the first of egui's built-in fonts.
    fn sample_font() -> Vec<u8> {
        let definitions = FontDefinitions::default();
        let font = definitions.font_data.values().next().unwrap();
        font.font.to_vec()
    }

    #[test]
    fn rejects_bytes_that_are_not_a_font() {
        assert!(is_usable(&sample_font()));
        assert!(!is_usable(&[]));
        assert!(!is_usable(b"not a font at all"));
        let mut truncated = sample_font();
        truncated.truncate(64);
        assert!(!is_usable(&truncated));
        let fonts = OsFonts::from_bytes(Some(b"junk".to_vec()), Some(truncated));
        assert!(fonts.is_empty());
        assert_eq!(fonts.choice(Preset::M365), None);
    }

    #[test]
    fn microsoft_365_prefers_the_variable_font() {
        let both = OsFonts::from_bytes(Some(sample_font()), Some(sample_font()));
        assert_eq!(both.choice(Preset::M365), Some(true));
        assert_eq!(both.choice(Preset::OneNote2016), Some(false));
        let only_variable = OsFonts::from_bytes(None, Some(sample_font()));
        assert_eq!(only_variable.choice(Preset::OneNote2019), Some(true));
        let only_regular = OsFonts::from_bytes(Some(sample_font()), None);
        assert_eq!(only_regular.choice(Preset::M365), Some(false));
    }

    #[test]
    fn os_font_goes_first_and_egui_fonts_stay_as_fallback() {
        let defaults = FontDefinitions::default();
        let fonts = OsFonts::from_bytes(Some(sample_font()), None);
        let definitions = fonts.definitions(Preset::OneNote2021);
        let family = definitions.families.get(&FontFamily::Proportional).unwrap();
        assert_eq!(family.first().map(String::as_str), Some(OS_FONT));
        assert_eq!(
            family.len(),
            defaults.families[&FontFamily::Proportional].len() + 1
        );
        assert_eq!(
            definitions.families.get(&FontFamily::Monospace),
            defaults.families.get(&FontFamily::Monospace)
        );
        assert_eq!(
            OsFonts::default().definitions(Preset::M365).font_data.len(),
            defaults.font_data.len()
        );
    }

    #[test]
    fn loading_from_this_machine_never_fails() {
        // Whatever the OS has (or lacks), loading returns fonts egui can parse.
        let fonts = OsFonts::load();
        let ctx = egui::Context::default();
        ctx.set_fonts(fonts.definitions(Preset::M365));
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label("NoteCraft");
        });
        // No renderer here to upload the font atlas to.
        output.textures_delta.clear();
    }

    #[test]
    fn a_font_that_is_not_bold_is_not_taken_as_the_bold_face() {
        // egui's built-in font is a regular, static face.
        let fonts = OsFonts::from_bytes(Some(sample_font()), None).with_bold_bytes(sample_font());
        assert!(!fonts.has_bold(Preset::OneNote2021));
        let definitions = fonts.definitions(Preset::OneNote2021);
        assert!(!definitions.families.contains_key(&bold_family()));
        let fonts = fonts.with_bold_bytes(b"junk".to_vec());
        assert!(!fonts.has_bold(Preset::M365));
    }

    #[test]
    fn a_bold_family_lists_the_bold_face_then_the_regular_fonts() {
        let mut fonts = OsFonts::from_bytes(Some(sample_font()), None);
        // Stand in a bold face without going through the weight check.
        fonts.bold = Some(Arc::new(FontData::from_owned(sample_font())));
        assert!(fonts.has_bold(Preset::M365));
        let definitions = fonts.definitions(Preset::M365);
        let bold = definitions.families.get(&bold_family()).unwrap();
        let regular = definitions.families.get(&FontFamily::Proportional).unwrap();
        assert_eq!(bold.first().map(String::as_str), Some(BOLD_FONT));
        assert_eq!(&bold[1..], regular.as_slice());
        assert!(definitions.font_data.contains_key(BOLD_FONT));
        // The definitions load into egui without trouble.
        let ctx = egui::Context::default();
        ctx.set_fonts(definitions);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label(egui::RichText::new("NoteCraft").family(bold_family()));
        });
        output.textures_delta.clear();
    }

    #[test]
    fn no_fonts_means_no_bold_family() {
        let definitions = OsFonts::default().definitions(Preset::M365);
        assert!(!definitions.families.contains_key(&bold_family()));
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn segoe_ui_bold_is_found_when_installed() {
        let installed = bold_candidates().iter().any(|path| path.is_file());
        let fonts = OsFonts::load();
        assert_eq!(fonts.has_bold(Preset::OneNote2021), installed);
    }
}
