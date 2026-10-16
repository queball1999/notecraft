//! The OS dialogs, clipboard, and default-app launcher for the shell's [`FilePicker`]. Paths
//! that are not valid Unicode are converted lossily; the engine checks every path it is given.

use std::sync::atomic::{AtomicU32, Ordering};

use notecraft_ui_egui::picker::{PAGE_FILTERS, PICTURE_FILTERS};
use notecraft_ui_egui::FilePicker;

#[derive(Clone, Copy, Debug, Default)]
pub struct OsPicker;

static PASTE_COUNTER: AtomicU32 = AtomicU32::new(0);

/// The picture on the clipboard as a PNG in the temp folder, or `None`.
fn clipboard_png() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let data = clipboard.get_image().ok()?;
    let (width, height) = (
        u32::try_from(data.width).ok()?,
        u32::try_from(data.height).ok()?,
    );
    let pixels = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || pixels > 36_000_000 {
        return None;
    }
    let image = image::RgbaImage::from_raw(width, height, data.bytes.into_owned())?;
    let n = PASTE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!("Pasted picture {}-{n}.png", std::process::id());
    let path = std::env::temp_dir().join(name);
    image
        .save_with_format(&path, image::ImageFormat::Png)
        .ok()?;
    Some(path.to_string_lossy().into_owned())
}

impl FilePicker for OsPicker {
    fn pick_folder(&mut self, title: &str) -> Option<String> {
        let folder = rfd::FileDialog::new().set_title(title).pick_folder()?;
        Some(folder.to_string_lossy().into_owned())
    }

    fn pick_file(&mut self, title: &str) -> Option<String> {
        let mut dialog = rfd::FileDialog::new().set_title(title);
        for (name, extensions) in PAGE_FILTERS {
            dialog = dialog.add_filter(name, extensions);
        }
        let file = dialog.pick_file()?;
        Some(file.to_string_lossy().into_owned())
    }

    fn pick_picture(&mut self, title: &str) -> Option<String> {
        let mut dialog = rfd::FileDialog::new().set_title(title);
        for (name, extensions) in PICTURE_FILTERS {
            dialog = dialog.add_filter(name, extensions);
        }
        let file = dialog.pick_file()?;
        Some(file.to_string_lossy().into_owned())
    }

    fn pick_attachment(&mut self, title: &str) -> Option<String> {
        let file = rfd::FileDialog::new().set_title(title).pick_file()?;
        Some(file.to_string_lossy().into_owned())
    }

    fn clipboard_picture(&mut self) -> Option<String> {
        clipboard_png()
    }

    fn open_in_default_app(&mut self, path: &str) -> bool {
        open::that_detached(path).is_ok()
    }
}
