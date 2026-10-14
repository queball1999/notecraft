//! Every command id, grouped by where the command lives in the UI.
//!
//! Ids are stable once released (docs/architecture.md section 4). Each one matches
//! `^[a-z]+(\.[a-z_]+)+$`. Labels, descriptions, and default shortcuts are in the
//! built-in command table (`cmd/builtin.rs`).

use super::CommandId;

/// Declares the id constants and [`ALL`], the list of every one of them.
macro_rules! command_ids {
    ($($(#[doc = $doc:literal])* $name:ident = $id:literal;)*) => {
        $($(#[doc = $doc])* pub const $name: CommandId = CommandId::new($id);)*

        /// Every id declared in this module, in declaration order.
        pub const ALL: &[CommandId] = &[$($name),*];
    };
}

command_ids! {
    // Home tab: Clipboard group
    /// Move the selection to the clipboard.
    EDIT_CUT = "edit.cut";
    /// Copy the selection to the clipboard.
    EDIT_COPY = "edit.copy";
    /// Paste from the clipboard.
    EDIT_PASTE = "edit.paste";
    /// Paste and keep the formatting of the copied content.
    EDIT_PASTE_KEEP_FORMATTING = "edit.paste_keep_formatting";
    /// Paste and match the formatting around the cursor.
    EDIT_PASTE_MERGE_FORMATTING = "edit.paste_merge_formatting";
    /// Paste plain text without formatting.
    EDIT_PASTE_TEXT_ONLY = "edit.paste_text_only";
    /// Paste the clipboard content as a picture.
    EDIT_PASTE_PICTURE = "edit.paste_picture";
    /// Copy formatting from the selection and apply it to the next text you select.
    EDIT_FORMAT_PAINTER = "edit.format_painter";
}
