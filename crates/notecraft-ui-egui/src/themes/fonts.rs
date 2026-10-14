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

use egui::{FontData, FontDefinitions, FontFamily};
use skrifa::MetadataProvider;

use super::Preset;

/// Largest font file read, in bytes. System UI fonts are a few MiB at most.
pub const MAX_FONT_BYTES: u64 = 40 * 1024 * 1024;

/// The `FontDefinitions` key of the OS font in use.
const OS_FONT: &str = "os-ui";

/// The OS UI fonts that were found and can be parsed. Either may be missing.
#[derive(Clone, Default)]
pub struct OsFonts {
    ui: Option<Arc<FontData>>,
    /// A variable version of the UI font, preferred by the Microsoft 365 preset.
    variable: Option<Arc<FontData>>,
}

impl std::fmt::Debug for OsFonts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OsFonts")
            .field("ui", &self.ui.is_some())
            .field("variable", &self.variable.is_some())
            .finish()
    }
}

impl OsFonts {
    /// Look for the OS UI fonts. Never fails: anything missing or unreadable is skipped.
    pub fn load() -> Self {
        let (ui, variable) = candidates();
        Self::from_bytes(first_usable(&ui), first_usable(&variable))
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

    /// egui's default fonts with the OS UI font for `preset` first in the proportional
    /// family. egui's own fonts stay behind it for any character the OS font lacks.
    pub fn definitions(&self, preset: Preset) -> FontDefinitions {
        let mut definitions = FontDefinitions::default();
        let font = match self.choice(preset) {
            Some(true) => self.variable.clone(),
            Some(false) => self.ui.clone(),
            None => None,
        };
        if let Some(font) = font {
            definitions.font_data.insert(OS_FONT.to_owned(), font);
            definitions
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, OS_FONT.to_owned());
        }
        definitions
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

fn read_font(path: &Path) -> Option<Vec<u8>> {
    let len = std::fs::metadata(path).ok()?.len();
    if len == 0 || len > MAX_FONT_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
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
    let matched = std::process::Command::new("fc-match")
        .args(["--format=%{file}", "sans-serif"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|file| file.trim().to_owned())
        .filter(|file| !file.is_empty() && file.len() < 4096);
    (matched.into_iter().map(PathBuf::from).collect(), Vec::new())
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
}
