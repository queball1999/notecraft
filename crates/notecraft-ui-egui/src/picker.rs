//! File pickers. The shell asks a [`FilePicker`] when a file command arrives without a path;
//! the desktop app supplies one backed by the OS dialogs, and tests supply a fake.

/// Asks the user for a folder or a page file. `None` means the user canceled.
pub trait FilePicker: std::fmt::Debug {
    /// A folder, for a new or existing notebook.
    fn pick_folder(&mut self, title: &str) -> Option<String>;
    /// A `.md` or `.notecraft` page file.
    fn pick_file(&mut self, title: &str) -> Option<String>;
    /// A PNG, JPEG, or WebP picture to insert.
    fn pick_picture(&mut self, _title: &str) -> Option<String> {
        None
    }
    /// Any file to attach.
    fn pick_attachment(&mut self, _title: &str) -> Option<String> {
        None
    }
    /// The picture on the clipboard, saved as a temporary PNG file. The shell deletes the
    /// file after the command ran. `None` when the clipboard holds no picture.
    fn clipboard_picture(&mut self) -> Option<String> {
        None
    }
    /// Open `path` with the operating system's default app. Returns false if it could not
    /// be started.
    fn open_in_default_app(&mut self, _path: &str) -> bool {
        false
    }
}

/// The picture types the picture picker offers.
pub const PICTURE_FILTERS: [(&str, &[&str]); 1] = [("Pictures", &["png", "jpg", "jpeg", "webp"])];

/// A picker that always cancels, for when no dialogs are available (tests, headless runs).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPicker;

impl FilePicker for NoPicker {
    fn pick_folder(&mut self, _title: &str) -> Option<String> {
        None
    }

    fn pick_file(&mut self, _title: &str) -> Option<String> {
        None
    }
}

/// The file types the page file picker offers: a name and its extensions.
pub const PAGE_FILTERS: [(&str, &[&str]); 3] = [
    ("NoteCraft pages", &["md", "notecraft"]),
    ("Markdown", &["md"]),
    ("NoteCraft page", &["notecraft"]),
];
