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
    // File tab (backstage view)
    /// Create a notebook in a folder you choose.
    FILE_NEW_NOTEBOOK = "file.new_notebook";
    /// Open a notebook folder.
    FILE_OPEN_NOTEBOOK = "file.open_notebook";
    /// Open a .md or .notecraft page file.
    FILE_OPEN = "file.open";
    /// Remove every notebook and page from the recent list.
    FILE_RECENT_CLEAR = "file.recent.clear";
    /// Show the open notebooks and where they are stored.
    FILE_INFO = "file.info";
    /// Rename the notebook, change its color, or show its folder.
    FILE_NOTEBOOK_PROPERTIES = "file.notebook_properties";
    /// Close the current notebook.
    FILE_CLOSE_NOTEBOOK = "file.close_notebook";
    /// Print the current page.
    FILE_PRINT = "file.print";
    /// Preview pages before printing them.
    FILE_PRINT_PREVIEW = "file.print_preview";
    /// Export a page, section, or notebook to PDF, HTML, or a zip file.
    FILE_EXPORT = "file.export";
    /// Open the NoteCraft settings.
    FILE_OPTIONS = "file.options";
    /// Show the version, build, and licenses.
    FILE_ABOUT = "file.about";
    /// Copy the version, build, and system details to the clipboard for a bug report.
    FILE_COPY_ABOUT_INFO = "file.copy_about_info";
    /// Put back the unsaved text found from the last session.
    FILE_RESTORE_UNSAVED = "file.restore_unsaved";
    /// Throw away the unsaved text found from the last session.
    FILE_DISCARD_UNSAVED = "file.discard_unsaved";
    /// Keep the text on the page when the file also changed outside NoteCraft.
    FILE_KEEP_MY_VERSION = "file.keep_my_version";
    /// Replace the text on the page with the version changed outside NoteCraft.
    FILE_USE_OUTSIDE_VERSION = "file.use_outside_version";

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

    // Editing commands shared by the Quick Access Toolbar, the Draw tab, and shortcuts
    /// Undo the last change.
    EDIT_UNDO = "edit.undo";
    /// Redo the last undone change.
    EDIT_REDO = "edit.redo";
    /// Select everything in the current container, then on the page.
    EDIT_SELECT_ALL = "edit.select_all";
    /// Delete the selection.
    EDIT_DELETE = "edit.delete";
    /// Find text on the current page.
    EDIT_FIND = "edit.find";

    // Home tab: Basic Text group
    /// Set the font family of the selection.
    FORMAT_FONT_FAMILY = "format.font_family";
    /// Set the font size of the selection.
    FORMAT_FONT_SIZE = "format.font_size";
    /// Start or remove a bulleted list.
    FORMAT_BULLETS = "format.bullets";
    /// Start or remove a numbered list.
    FORMAT_NUMBERING = "format.numbering";
    /// Move the paragraph one level left.
    FORMAT_OUTDENT = "format.outdent";
    /// Move the paragraph one level right.
    FORMAT_INDENT = "format.indent";
    /// Remove all formatting from the selection.
    FORMAT_CLEAR = "format.clear";
    /// Make the selection bold.
    FORMAT_BOLD = "format.bold";
    /// Make the selection italic.
    FORMAT_ITALIC = "format.italic";
    /// Underline the selection.
    FORMAT_UNDERLINE = "format.underline";
    /// Draw a line through the selection.
    FORMAT_STRIKETHROUGH = "format.strikethrough";
    /// Make the selection subscript.
    FORMAT_SUBSCRIPT = "format.subscript";
    /// Make the selection superscript.
    FORMAT_SUPERSCRIPT = "format.superscript";
    /// Highlight the selection with a color.
    FORMAT_HIGHLIGHT = "format.highlight";
    /// Set the text color of the selection.
    FORMAT_FONT_COLOR = "format.font_color";
    /// Align the paragraph to the left.
    FORMAT_ALIGN_LEFT = "format.align_left";
    /// Center the paragraph.
    FORMAT_ALIGN_CENTER = "format.align_center";
    /// Align the paragraph to the right.
    FORMAT_ALIGN_RIGHT = "format.align_right";

    // Home tab: Styles group
    /// Apply a heading or paragraph style to the selection.
    STYLE_APPLY = "style.apply";

    // Home tab: Tags group
    /// Apply a note tag to the selected paragraph.
    TAG_APPLY = "tag.apply";
    /// Add or check a To Do checkbox on the selected paragraph.
    TAG_TODO = "tag.todo";
    /// Remove all note tags from the selected paragraph.
    TAG_REMOVE = "tag.remove";
    /// List every tagged note in the Tags Summary pane.
    TAG_FIND = "tag.find";
    /// Add, change, or reorder the note tags in the gallery.
    TAG_CUSTOMIZE = "tag.customize";

    // Home tab: Email and Meetings groups
    /// Send the current page as an email.
    EMAIL_PAGE = "email.page";
    /// Insert the details of a meeting on the page.
    MEETING_DETAILS = "meeting.details";

    // Insert tab
    /// Add or remove space on the page by dragging.
    INSERT_SPACE = "insert.space";
    /// Insert a table.
    INSERT_TABLE = "insert.table";
    /// Attach a file to the page as an icon.
    INSERT_FILE_ATTACHMENT = "insert.file_attachment";
    /// Insert the pages of a file as pictures.
    INSERT_FILE_PRINTOUT = "insert.file_printout";
    /// Insert a picture from a file.
    INSERT_PICTURE = "insert.picture";
    /// Open an attached file in its default app.
    ATTACHMENT_OPEN = "attachment.open";
    /// Capture part of the screen and insert it as a picture.
    INSERT_SCREEN_CLIPPING = "insert.screen_clipping";
    /// Insert a link to a web page, file, or another page.
    INSERT_LINK = "insert.link";
    /// Record audio and insert it on the page.
    INSERT_AUDIO = "insert.audio";
    /// Record video and insert it on the page.
    INSERT_VIDEO = "insert.video";
    /// Insert the current date.
    INSERT_DATE = "insert.date";
    /// Insert the current time.
    INSERT_TIME = "insert.time";
    /// Insert the current date and time.
    INSERT_DATE_TIME = "insert.date_time";
    /// Create a page from a template.
    INSERT_PAGE_TEMPLATE = "insert.page_template";
    /// Insert an equation.
    INSERT_EQUATION = "insert.equation";
    /// Insert a symbol.
    INSERT_SYMBOL = "insert.symbol";

    // Draw tab
    /// Switch to typing and selecting text.
    DRAW_TYPE = "draw.type";
    /// Select ink and objects by drawing around them.
    DRAW_LASSO = "draw.lasso";
    /// Move around the page by dragging.
    DRAW_PAN = "draw.pan";
    /// Draw with a pen.
    DRAW_PEN = "draw.pen";
    /// Draw with a highlighter.
    DRAW_HIGHLIGHTER = "draw.highlighter";
    /// Erase ink strokes or parts of strokes.
    DRAW_ERASER = "draw.eraser";
    /// Pick up a pen or highlighter from the Draw tab's list.
    DRAW_TOOL_SELECT = "draw.tool_select";
    /// Add a pen or highlighter to the Draw tab's list.
    DRAW_TOOL_ADD = "draw.tool_add";
    /// Remove a pen or highlighter from the Draw tab's list.
    DRAW_TOOL_DELETE = "draw.tool_delete";
    /// Pick the color of a stroke on the page for the current pen.
    DRAW_EYEDROPPER = "draw.eyedropper";
    /// Play back how the ink on the page was drawn.
    DRAW_INK_REPLAY = "draw.ink_replay";
    /// Set the color of the current pen.
    DRAW_COLOR = "draw.color";
    /// Set the width of the current pen.
    DRAW_THICKNESS = "draw.thickness";
    /// Add a stroke drawn with the current pen to the page.
    INK_ADD_STROKE = "ink.add_stroke";

    // Text editing in containers (not on the ribbon; keys and clicks send them)
    /// Put the text caret at a position, or extend the selection to it.
    TEXT_SET_CARET = "text.set_caret";
    /// Move the text caret by a character, word, or line, or extend the selection.
    TEXT_MOVE = "text.move";
    /// Type text at the caret, replacing the selection.
    TEXT_INSERT = "text.insert";
    /// Delete backward (Backspace) or forward (Delete) by a character or a word.
    TEXT_DELETE = "text.delete";
    /// Start a new paragraph or list item, or a line break.
    TEXT_ENTER = "text.enter";
    /// Move list items in or out a level.
    TEXT_TAB = "text.tab";
    /// Start a new note container at a point on the page.
    CONTAINER_NEW = "container.new";
    /// Move a note container to a point on the page.
    CONTAINER_MOVE = "container.move";
    /// Change the width of a note container.
    CONTAINER_RESIZE = "container.resize";
    /// Erase ink along a path.
    INK_ERASE = "ink.erase";
    /// Show or hide the ruler for drawing straight lines.
    DRAW_RULER = "draw.ruler";
    /// Draw a shape.
    DRAW_SHAPE = "draw.shape";
    /// Move the selection in front of all other objects.
    DRAW_BRING_TO_FRONT = "draw.bring_to_front";
    /// Move the selection one step forward.
    DRAW_BRING_FORWARD = "draw.bring_forward";
    /// Move the selection one step back.
    DRAW_SEND_BACKWARD = "draw.send_backward";
    /// Move the selection behind all other objects.
    DRAW_SEND_TO_BACK = "draw.send_to_back";
    /// Rotate the selection 90 degrees clockwise.
    DRAW_ROTATE_RIGHT = "draw.rotate_right";
    /// Rotate the selection 90 degrees counterclockwise.
    DRAW_ROTATE_LEFT = "draw.rotate_left";
    /// Flip the selection left to right.
    DRAW_FLIP_HORIZONTAL = "draw.flip_horizontal";
    /// Flip the selection top to bottom.
    DRAW_FLIP_VERTICAL = "draw.flip_vertical";
    /// Turn new drawings into clean shapes.
    DRAW_INK_TO_SHAPE = "draw.ink_to_shape";
    /// Convert the selected handwriting to typed text.
    DRAW_INK_TO_TEXT = "draw.ink_to_text";
    /// Convert a handwritten equation to a typed equation.
    DRAW_INK_TO_MATH = "draw.ink_to_math";

    // History tab
    /// Go to the next page with unread changes.
    HISTORY_NEXT_UNREAD = "history.next_unread";
    /// Mark the current page as read.
    HISTORY_MARK_READ = "history.mark_read";
    /// Highlight unread changes in this notebook.
    HISTORY_SHOW_UNREAD = "history.show_unread";
    /// List pages changed recently.
    HISTORY_RECENT_EDITS = "history.recent_edits";
    /// List changes grouped by author.
    HISTORY_FIND_BY_AUTHOR = "history.find_by_author";
    /// Show or hide author initials next to notes.
    HISTORY_HIDE_AUTHORS = "history.hide_authors";
    /// Show earlier versions of the current page.
    HISTORY_PAGE_VERSIONS = "history.page_versions";
    /// Show deleted pages and sections and restore them.
    HISTORY_RECYCLE_BIN = "history.recycle_bin";

    // Review tab
    /// Check spelling on the current page.
    REVIEW_SPELLING = "review.spelling";
    /// Find other words with a similar meaning.
    REVIEW_THESAURUS = "review.thesaurus";
    /// Set the proofing language of the selection.
    REVIEW_LANGUAGE = "review.language";
    /// Protect the current section with a password.
    REVIEW_PASSWORD = "review.password";
    /// Lock every password-protected section.
    REVIEW_LOCK_ALL = "review.lock_all";
    /// Take notes linked to what you are viewing.
    REVIEW_LINKED_NOTES = "review.linked_notes";

    // View tab
    /// Show the ribbon and the navigation panes.
    VIEW_NORMAL = "view.normal";
    /// Show only the page.
    VIEW_FULL_PAGE = "view.full_page";
    /// Dock the window to the side of the screen.
    VIEW_DOCK = "view.dock";
    /// Show or hide the notebook, section, and page lists.
    VIEW_NAVIGATION_TOGGLE = "view.navigation.toggle";
    /// Show only the ribbon tab names.
    VIEW_RIBBON_TOGGLE = "view.ribbon.toggle";
    /// Use the classic or the simplified ribbon.
    VIEW_RIBBON_LAYOUT = "view.ribbon.layout";
    /// Always show the ribbon, show only its tabs, or hide it (full-screen mode).
    VIEW_RIBBON_SHOW = "view.ribbon.show";
    /// Set the background color of the page.
    VIEW_PAGE_COLOR = "view.page_color";
    /// Show rule lines or a grid on the page.
    VIEW_RULE_LINES = "view.rule_lines";
    /// Set the color of the rule lines and grid on the current page.
    VIEW_RULE_LINE_COLOR = "view.rule_line_color";
    /// Hide the title, date, and time of the page.
    VIEW_HIDE_TITLE = "view.hide_title";
    /// Set the paper size of the page.
    VIEW_PAPER_SIZE = "view.paper_size";
    /// Zoom in on the page.
    VIEW_ZOOM_IN = "view.zoom_in";
    /// Zoom out of the page.
    VIEW_ZOOM_OUT = "view.zoom_out";
    /// Set the zoom level.
    VIEW_ZOOM = "view.zoom";
    /// Zoom to 100 percent.
    VIEW_ZOOM_ACTUAL = "view.zoom_actual";
    /// Zoom so the page fills the window width.
    VIEW_ZOOM_PAGE_WIDTH = "view.zoom_page_width";
    /// Open another window on the same notebook.
    VIEW_NEW_WINDOW = "view.new_window";
    /// Open another window docked to the side of the screen.
    VIEW_NEW_DOCKED_WINDOW = "view.new_docked_window";
    /// Switch to another theme preset.
    VIEW_THEME_SET = "view.theme.set";
    /// Switch between light and dark mode.
    VIEW_MODE_TOGGLE = "view.mode.toggle";
    /// Use light, dark, or the system color mode.
    VIEW_MODE_SET = "view.mode.set";

    // Help tab
    /// Open the help for NoteCraft.
    HELP_HELP = "help.help";
    /// Find ways to get support.
    HELP_SUPPORT = "help.support";
    /// Send feedback about NoteCraft.
    HELP_FEEDBACK = "help.feedback";
    /// Show the training for NoteCraft.
    HELP_TRAINING = "help.training";

    // Navigation: the notebook list, the section tabs, and the page list
    /// Show a notebook, at its first section's first page.
    NAV_NOTEBOOK_SELECT = "nav.notebook.select";
    /// Show a section, at its first page.
    NAV_SECTION_SELECT = "nav.section.select";
    /// Show a page.
    NAV_PAGE_SELECT = "nav.page.select";
    /// Add a section to the current notebook.
    SECTION_NEW = "section.new";
    /// Add a page to the current section.
    PAGE_NEW = "page.new";
    /// Rename a page.
    PAGE_RENAME = "page.rename";
    /// Delete a page (it goes to the trash folder of its notebook).
    PAGE_DELETE = "page.delete";
    /// Move a page to another place in its section or to another section.
    PAGE_MOVE = "page.move";
    /// Set how far a page is indented as a subpage.
    PAGE_SET_LEVEL = "page.set_level";
    /// Make a page a subpage of the page above it.
    PAGE_MAKE_SUBPAGE = "page.make_subpage";
    /// Move a subpage up one level.
    PAGE_PROMOTE_SUBPAGE = "page.promote_subpage";
    /// Add a section group to the current notebook.
    SECTION_GROUP_NEW = "section.group_new";
    /// Rename a section or section group.
    SECTION_RENAME = "section.rename";
    /// Delete a section or section group (it goes to the trash folder of its notebook).
    SECTION_DELETE = "section.delete";
    /// Move a section or section group into a group or to the top level.
    SECTION_MOVE = "section.move";
    /// Set the color of a section or section group.
    SECTION_SET_COLOR = "section.set_color";
    /// Rename a notebook.
    NOTEBOOK_RENAME = "notebook.rename";
    /// Set the color of a notebook.
    NOTEBOOK_SET_COLOR = "notebook.set_color";
    /// Turn the page shown into a plain markdown page, dropping its ink and positions.
    PAGE_CONVERT_TO_MARKDOWN = "page.convert_to_markdown";

    // Search (Phase 2)
    /// Search every open notebook.
    SEARCH_ALL = "search.all";
}
