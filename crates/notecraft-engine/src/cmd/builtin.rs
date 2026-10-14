//! The built-in command table: label, description, arguments, and default shortcut for
//! every id in [`ids`]. Commands whose engine work is not done yet are Coming Soon; a
//! feature PR flips its command to Ready and registers a handler.

use super::{ids, CommandSpec};

/// Every built-in command, in ribbon order.
pub(super) static SPECS: &[CommandSpec] = &[
    // Home tab: Clipboard group
    CommandSpec::new(ids::EDIT_CUT, "Cut", "Move the selection to the clipboard.")
        .undoable()
        .shortcut("Ctrl+X"),
    CommandSpec::new(
        ids::EDIT_COPY,
        "Copy",
        "Copy the selection to the clipboard.",
    )
    .shortcut("Ctrl+C"),
    CommandSpec::new(ids::EDIT_PASTE, "Paste", "Paste from the clipboard.")
        .undoable()
        .shortcut("Ctrl+V"),
    CommandSpec::new(
        ids::EDIT_PASTE_KEEP_FORMATTING,
        "Keep Source Formatting",
        "Paste and keep the formatting of the copied content.",
    )
    .undoable(),
    CommandSpec::new(
        ids::EDIT_PASTE_MERGE_FORMATTING,
        "Merge Formatting",
        "Paste and match the formatting around the cursor.",
    )
    .undoable(),
    CommandSpec::new(
        ids::EDIT_PASTE_TEXT_ONLY,
        "Keep Text Only",
        "Paste plain text without formatting.",
    )
    .undoable(),
    CommandSpec::new(
        ids::EDIT_PASTE_PICTURE,
        "Paste as Picture",
        "Paste the clipboard content as a picture.",
    )
    .undoable(),
    CommandSpec::new(
        ids::EDIT_FORMAT_PAINTER,
        "Format Painter",
        "Copy formatting from the selection and apply it to the next text you select.",
    )
    .shortcut("Ctrl+Shift+C"),
];
