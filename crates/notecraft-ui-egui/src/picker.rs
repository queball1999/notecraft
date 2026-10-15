//! File pickers. The shell asks a [`FilePicker`] when a file command arrives without a path;
//! the desktop app supplies one backed by the OS dialogs, and tests supply a fake.

/// Asks the user for a folder or a page file. `None` means the user canceled.
pub trait FilePicker: std::fmt::Debug {
    /// A folder, for a new or existing notebook.
    fn pick_folder(&mut self, title: &str) -> Option<String>;
    /// A `.md` or `.notecraft` page file.
    fn pick_file(&mut self, title: &str) -> Option<String>;
}

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
