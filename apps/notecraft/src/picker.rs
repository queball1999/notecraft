//! The OS open dialogs, through `rfd`, for the shell's [`FilePicker`]. Paths that are not
//! valid Unicode are converted lossily; the engine checks every path it is given.

use notecraft_ui_egui::picker::PAGE_FILTERS;
use notecraft_ui_egui::FilePicker;

#[derive(Clone, Copy, Debug, Default)]
pub struct OsPicker;

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
}
