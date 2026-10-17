//! The built-in command table: label, description, arguments, and default shortcut for
//! every id in [`ids`]. Commands whose engine work is not done yet are Coming Soon; a
//! feature PR flips its command to Ready and adds its handler to [`HANDLERS`].

use super::page;
use super::{
    canvas, draw, edit, file, ids, layout, nav, structure, theme, ArgKind, ArgSpec, CommandId,
    CommandSpec, Handler,
};

const NOTEBOOK_PATH_ARGS: &[ArgSpec] = &[ArgSpec::optional(
    "path",
    ArgKind::String,
    "Notebook folder; the app asks with a folder picker when omitted.",
)];
const FILE_PATH_ARGS: &[ArgSpec] = &[ArgSpec::optional(
    "path",
    ArgKind::String,
    "A .md or .notecraft file; the app asks with a file picker when omitted.",
)];
const NAV_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "id",
    ArgKind::String,
    "The notebook, section, or page id.",
)];
const OPTIONAL_ID_ARGS: &[ArgSpec] = &[ArgSpec::optional(
    "id",
    ArgKind::String,
    "The notebook, section, or page id; the one shown when omitted.",
)];
const RENAME_ARGS: &[ArgSpec] = &[
    ArgSpec::optional(
        "id",
        ArgKind::String,
        "The notebook, section, or page id; the one shown when omitted.",
    ),
    ArgSpec::optional(
        "name",
        ArgKind::String,
        "The new name; the app asks for it when omitted.",
    ),
];
const SET_COLOR_ARGS: &[ArgSpec] = &[
    ArgSpec::optional(
        "id",
        ArgKind::String,
        "The notebook or section id; the one shown when omitted.",
    ),
    ArgSpec::required("color", ArgKind::String, "Color as #RRGGBB."),
];
const NEW_PAGE_ARGS: &[ArgSpec] = &[
    ArgSpec::optional(
        "title",
        ArgKind::String,
        "The page title; Untitled Page when omitted.",
    ),
    ArgSpec::optional(
        "section",
        ArgKind::String,
        "The section id; the section shown when omitted.",
    ),
];
const NEW_SECTION_ARGS: &[ArgSpec] = &[
    ArgSpec::optional(
        "name",
        ArgKind::String,
        "The name; New Section when omitted.",
    ),
    ArgSpec::optional(
        "parent",
        ArgKind::String,
        "The section group to add it to; the top level when omitted.",
    ),
    ArgSpec::optional(
        "notebook",
        ArgKind::String,
        "The notebook id; the notebook shown when omitted.",
    ),
];
const PAGE_MOVE_ARGS: &[ArgSpec] = &[
    ArgSpec::optional(
        "id",
        ArgKind::String,
        "The page id; the page shown when omitted.",
    ),
    ArgSpec::optional(
        "section",
        ArgKind::String,
        "The section to move to; the page's own section when omitted.",
    ),
    ArgSpec::optional(
        "index",
        ArgKind::Integer,
        "Position among the section's pages; last when omitted.",
    ),
];
const PAGE_LEVEL_ARGS: &[ArgSpec] = &[
    ArgSpec::optional(
        "id",
        ArgKind::String,
        "The page id; the page shown when omitted.",
    ),
    ArgSpec::required(
        "level",
        ArgKind::Integer,
        "0 for a page, 1 or 2 for a subpage.",
    ),
];
const SECTION_MOVE_ARGS: &[ArgSpec] = &[
    ArgSpec::optional(
        "id",
        ArgKind::String,
        "The section or group id; the section shown when omitted.",
    ),
    ArgSpec::optional(
        "parent",
        ArgKind::String,
        "The section group to move into; the top level when omitted.",
    ),
    ArgSpec::optional(
        "index",
        ArgKind::Integer,
        "Position among the children of the parent; last when omitted.",
    ),
];
const FONT_FAMILY_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "name",
    ArgKind::String,
    "Font family name.",
)];
const FONT_SIZE_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "points",
    ArgKind::Number,
    "Font size in points.",
)];
const COLOR_ARGS: &[ArgSpec] = &[ArgSpec::optional(
    "color",
    ArgKind::String,
    "Color as #RRGGBB or #RRGGBBAA; the last used color when omitted.",
)];
const STYLE_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "style",
    ArgKind::String,
    "Style name: heading1 to heading6, title, citation, quote, code, or normal.",
)];
const TAG_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "tag",
    ArgKind::String,
    "Tag id, for example todo, important, or question.",
)];
const TABLE_ARGS: &[ArgSpec] = &[
    ArgSpec::optional("rows", ArgKind::Integer, "Number of rows."),
    ArgSpec::optional("columns", ArgKind::Integer, "Number of columns."),
];
const TEMPLATE_ARGS: &[ArgSpec] = &[ArgSpec::optional(
    "template",
    ArgKind::String,
    "Template id; opens the template pane when omitted.",
)];
const SYMBOL_ARGS: &[ArgSpec] = &[ArgSpec::optional(
    "symbol",
    ArgKind::String,
    "The symbol to insert; opens the symbol picker when omitted.",
)];
const PEN_ARGS: &[ArgSpec] = &[
    ArgSpec::optional("color", ArgKind::String, "Color as #RRGGBB or #RRGGBBAA."),
    ArgSpec::optional("width", ArgKind::Number, "Stroke width in points."),
];
const TOOL_INDEX_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "index",
    ArgKind::Integer,
    "Position in the Draw tab's list of pens, counting from 0.",
)];
const TOOL_DELETE_ARGS: &[ArgSpec] = &[ArgSpec::optional(
    "index",
    ArgKind::Integer,
    "Position in the list, counting from 0; the tool in use when omitted.",
)];
const TOOL_ADD_ARGS: &[ArgSpec] = &[
    ArgSpec::optional(
        "kind",
        ArgKind::String,
        "pen or highlighter; pen when omitted.",
    ),
    ArgSpec::optional("color", ArgKind::String, "Color as #RRGGBB or #RRGGBBAA."),
    ArgSpec::optional("width", ArgKind::Number, "Stroke width in points."),
];
const EYEDROPPER_ARGS: &[ArgSpec] = &[
    ArgSpec::required("x", ArgKind::Number, "Page x in page units."),
    ArgSpec::required("y", ArgKind::Number, "Page y in page units."),
    ArgSpec::optional(
        "radius",
        ArgKind::Number,
        "How far from a stroke's edge still counts, in page units.",
    ),
];
const ERASER_ARGS: &[ArgSpec] = &[
    ArgSpec::optional("mode", ArgKind::String, "stroke or point."),
    ArgSpec::optional(
        "size",
        ArgKind::String,
        "small, medium, or large; the size of the point eraser.",
    ),
];
const ERASE_ARGS: &[ArgSpec] = &[
    ArgSpec::required(
        "points",
        ArgKind::Array,
        "The path of the eraser's center in page units, each [x, y].",
    ),
    ArgSpec::required(
        "radius",
        ArgKind::Number,
        "The eraser's reach in page units.",
    ),
    ArgSpec::optional(
        "mode",
        ArgKind::String,
        "stroke or point; the eraser in use when omitted.",
    ),
    ArgSpec::optional(
        "join",
        ArgKind::Bool,
        "True to undo together with the erase before it, for the rest of one drag.",
    ),
];
const REQUIRED_COLOR_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "color",
    ArgKind::String,
    "Color as #RRGGBB or #RRGGBBAA, or none.",
)];
const PEN_WIDTH_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "width",
    ArgKind::Number,
    "Stroke width in points.",
)];
const STROKE_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "points",
    ArgKind::Array,
    "The stroke's points in page units, each [x, y] or [x, y, pressure] with pressure 0 to 1.",
)];
const SHAPE_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "kind",
    ArgKind::String,
    "Shape kind, for example line, arrow, rectangle, or ellipse.",
)];
const CONVERT_ARGS: &[ArgSpec] = &[ArgSpec::optional(
    "confirm",
    ArgKind::Bool,
    "True to convert; without it the command only returns the warning to show first.",
)];
const RULE_LINE_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "style",
    ArgKind::String,
    "none, narrow, college, standard, wide, grid_small, grid_medium, grid_large, or grid_xlarge.",
)];
const PAPER_SIZE_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "size",
    ArgKind::String,
    "auto, letter, legal, a4, or a5.",
)];
const ZOOM_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "percent",
    ArgKind::Integer,
    "Zoom level, 10 to 400.",
)];
const THEME_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "preset",
    ArgKind::String,
    "onenote2016, onenote2019, onenote2021, or m365.",
)];
const MODE_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "mode",
    ArgKind::String,
    "light, dark, or system.",
)];
const RIBBON_LAYOUT_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "layout",
    ArgKind::String,
    "classic or simplified.",
)];
const RIBBON_SHOW_ARGS: &[ArgSpec] = &[ArgSpec::required(
    "mode",
    ArgKind::String,
    "always, tabs, or full_screen.",
)];

/// Every built-in command, in ribbon order.
pub(super) static SPECS: &[CommandSpec] = &[
    // File tab (backstage view)
    CommandSpec::new(
        ids::FILE_NEW_NOTEBOOK,
        "New Notebook",
        "Create a notebook in a folder you choose.",
    )
    .args(NOTEBOOK_PATH_ARGS)
    .ready(),
    CommandSpec::new(
        ids::FILE_OPEN_NOTEBOOK,
        "Open Notebook",
        "Open a notebook folder.",
    )
    .args(NOTEBOOK_PATH_ARGS)
    .shortcut("Ctrl+O")
    .ready(),
    CommandSpec::new(
        ids::FILE_OPEN,
        "Open File",
        "Open a .md or .notecraft page file.",
    )
    .args(FILE_PATH_ARGS)
    .ready(),
    CommandSpec::new(
        ids::FILE_RECENT_CLEAR,
        "Clear Recent",
        "Remove every notebook and page from the recent list.",
    )
    .ready(),
    CommandSpec::new(
        ids::FILE_INFO,
        "Info",
        "Show the open notebooks and where they are stored.",
    ),
    CommandSpec::new(
        ids::FILE_NOTEBOOK_PROPERTIES,
        "Notebook Properties",
        "Rename the notebook, change its color, or show its folder.",
    ),
    CommandSpec::new(
        ids::FILE_CLOSE_NOTEBOOK,
        "Close Notebook",
        "Close the current notebook. Its folder and files stay where they are.",
    )
    .args(OPTIONAL_ID_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(ids::FILE_PRINT, "Print", "Print the current page.").shortcut("Ctrl+P"),
    CommandSpec::new(
        ids::FILE_PRINT_PREVIEW,
        "Print Preview",
        "Preview pages before printing them.",
    ),
    CommandSpec::new(
        ids::FILE_EXPORT,
        "Export",
        "Export a page, section, or notebook to PDF, HTML, or a zip file.",
    ),
    CommandSpec::new(ids::FILE_OPTIONS, "Options", "Open the NoteCraft settings."),
    CommandSpec::new(
        ids::FILE_ABOUT,
        "About NoteCraft",
        "Show the version, build, and licenses.",
    )
    .ready(),
    CommandSpec::new(
        ids::FILE_COPY_ABOUT_INFO,
        "Copy About Info",
        "Copy the version, build, and system details to the clipboard for a bug report.",
    )
    .ready(),
    CommandSpec::new(
        ids::FILE_RESTORE_UNSAVED,
        "Restore Unsaved Changes",
        "Put back the unsaved text found from the last session.",
    )
    .ready(),
    CommandSpec::new(
        ids::FILE_DISCARD_UNSAVED,
        "Discard Unsaved Changes",
        "Throw away the unsaved text found from the last session.",
    )
    .destructive()
    .ready(),
    CommandSpec::new(
        ids::FILE_KEEP_MY_VERSION,
        "Keep My Version",
        "Keep the text on the page when the file also changed outside NoteCraft.",
    )
    .ready(),
    CommandSpec::new(
        ids::FILE_USE_OUTSIDE_VERSION,
        "Use Outside Version",
        "Replace the text on the page with the version changed outside NoteCraft.",
    )
    .destructive()
    .ready(),
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
    // Editing commands shared by the Quick Access Toolbar, the Draw tab, and shortcuts
    CommandSpec::new(ids::EDIT_UNDO, "Undo", "Undo the last change.")
        .shortcut("Ctrl+Z")
        .ready(),
    CommandSpec::new(ids::EDIT_REDO, "Redo", "Redo the last undone change.")
        .shortcut("Ctrl+Y")
        .ready(),
    CommandSpec::new(
        ids::EDIT_SELECT_ALL,
        "Select All",
        "Select everything in the current container, then on the page.",
    )
    .shortcut("Ctrl+A"),
    CommandSpec::new(ids::EDIT_DELETE, "Delete", "Delete the selection.")
        .undoable()
        .destructive()
        .shortcut("Delete"),
    CommandSpec::new(
        ids::EDIT_FIND,
        "Find on Page",
        "Find text on the current page.",
    )
    .shortcut("Ctrl+F"),
    // Home tab: Basic Text group
    CommandSpec::new(
        ids::FORMAT_FONT_FAMILY,
        "Font",
        "Set the font family of the selection.",
    )
    .undoable()
    .args(FONT_FAMILY_ARGS),
    CommandSpec::new(
        ids::FORMAT_FONT_SIZE,
        "Font Size",
        "Set the font size of the selection.",
    )
    .undoable()
    .args(FONT_SIZE_ARGS),
    CommandSpec::new(
        ids::FORMAT_BULLETS,
        "Bullets",
        "Start or remove a bulleted list.",
    )
    .undoable()
    .shortcut("Ctrl+."),
    CommandSpec::new(
        ids::FORMAT_NUMBERING,
        "Numbering",
        "Start or remove a numbered list.",
    )
    .undoable()
    .shortcut("Ctrl+/"),
    CommandSpec::new(
        ids::FORMAT_OUTDENT,
        "Decrease Indent",
        "Move the paragraph one level left.",
    )
    .undoable()
    .shortcut("Alt+Shift+Left"),
    CommandSpec::new(
        ids::FORMAT_INDENT,
        "Increase Indent",
        "Move the paragraph one level right.",
    )
    .undoable()
    .shortcut("Alt+Shift+Right"),
    CommandSpec::new(
        ids::FORMAT_CLEAR,
        "Clear All Formatting",
        "Remove all formatting from the selection.",
    )
    .undoable()
    .shortcut("Ctrl+Shift+N"),
    CommandSpec::new(ids::FORMAT_BOLD, "Bold", "Make the selection bold.")
        .undoable()
        .shortcut("Ctrl+B"),
    CommandSpec::new(ids::FORMAT_ITALIC, "Italic", "Make the selection italic.")
        .undoable()
        .shortcut("Ctrl+I"),
    CommandSpec::new(
        ids::FORMAT_UNDERLINE,
        "Underline",
        "Underline the selection.",
    )
    .undoable()
    .shortcut("Ctrl+U"),
    CommandSpec::new(
        ids::FORMAT_STRIKETHROUGH,
        "Strikethrough",
        "Draw a line through the selection.",
    )
    .undoable()
    .shortcut("Ctrl+-"),
    CommandSpec::new(
        ids::FORMAT_SUBSCRIPT,
        "Subscript",
        "Make the selection subscript.",
    )
    .undoable()
    .shortcut("Ctrl+="),
    CommandSpec::new(
        ids::FORMAT_SUPERSCRIPT,
        "Superscript",
        "Make the selection superscript.",
    )
    .undoable()
    .shortcut("Ctrl+Shift+="),
    CommandSpec::new(
        ids::FORMAT_HIGHLIGHT,
        "Text Highlight Color",
        "Highlight the selection with a color.",
    )
    .undoable()
    .shortcut("Ctrl+Alt+H")
    .args(COLOR_ARGS),
    CommandSpec::new(
        ids::FORMAT_FONT_COLOR,
        "Font Color",
        "Set the text color of the selection.",
    )
    .undoable()
    .args(COLOR_ARGS),
    CommandSpec::new(
        ids::FORMAT_ALIGN_LEFT,
        "Align Left",
        "Align the paragraph to the left.",
    )
    .undoable()
    .shortcut("Ctrl+L"),
    CommandSpec::new(ids::FORMAT_ALIGN_CENTER, "Center", "Center the paragraph.").undoable(),
    CommandSpec::new(
        ids::FORMAT_ALIGN_RIGHT,
        "Align Right",
        "Align the paragraph to the right.",
    )
    .undoable()
    .shortcut("Ctrl+R"),
    // Home tab: Styles group
    CommandSpec::new(
        ids::STYLE_APPLY,
        "Styles",
        "Apply a heading or paragraph style to the selection.",
    )
    .undoable()
    .args(STYLE_ARGS),
    // Home tab: Tags group
    CommandSpec::new(
        ids::TAG_APPLY,
        "Tags",
        "Apply a note tag to the selected paragraph.",
    )
    .undoable()
    .args(TAG_ARGS),
    CommandSpec::new(
        ids::TAG_TODO,
        "To Do Tag",
        "Add or check a To Do checkbox on the selected paragraph.",
    )
    .undoable()
    .shortcut("Ctrl+1"),
    CommandSpec::new(
        ids::TAG_REMOVE,
        "Remove Tag",
        "Remove all note tags from the selected paragraph.",
    )
    .undoable()
    .shortcut("Ctrl+0"),
    CommandSpec::new(
        ids::TAG_FIND,
        "Find Tags",
        "List every tagged note in the Tags Summary pane.",
    ),
    CommandSpec::new(
        ids::TAG_CUSTOMIZE,
        "Customize Tags",
        "Add, change, or reorder the note tags in the gallery.",
    ),
    // Home tab: Email and Meetings groups
    CommandSpec::new(
        ids::EMAIL_PAGE,
        "Email Page",
        "Send the current page as an email.",
    )
    .shortcut("Ctrl+Shift+E"),
    CommandSpec::new(
        ids::MEETING_DETAILS,
        "Meeting Details",
        "Insert the details of a meeting on the page.",
    )
    .undoable(),
    // Insert tab
    CommandSpec::new(
        ids::INSERT_SPACE,
        "Insert Space",
        "Add or remove space on the page by dragging.",
    )
    .undoable(),
    CommandSpec::new(ids::INSERT_TABLE, "Table", "Insert a table.")
        .undoable()
        .args(TABLE_ARGS),
    CommandSpec::new(
        ids::INSERT_FILE_ATTACHMENT,
        "File Attachment",
        "Attach a file to the page as an icon.",
    )
    .undoable(),
    CommandSpec::new(
        ids::INSERT_FILE_PRINTOUT,
        "File Printout",
        "Insert the pages of a file as pictures.",
    )
    .undoable(),
    CommandSpec::new(
        ids::INSERT_PICTURE,
        "Pictures",
        "Insert a picture from a file.",
    )
    .undoable(),
    CommandSpec::new(
        ids::INSERT_SCREEN_CLIPPING,
        "Screen Clipping",
        "Capture part of the screen and insert it as a picture.",
    )
    .undoable(),
    CommandSpec::new(
        ids::INSERT_LINK,
        "Link",
        "Insert a link to a web page, file, or another page.",
    )
    .undoable()
    .shortcut("Ctrl+K"),
    CommandSpec::new(
        ids::INSERT_AUDIO,
        "Record Audio",
        "Record audio and insert it on the page.",
    )
    .undoable(),
    CommandSpec::new(
        ids::INSERT_VIDEO,
        "Record Video",
        "Record video and insert it on the page.",
    )
    .undoable(),
    CommandSpec::new(ids::INSERT_DATE, "Date", "Insert the current date.")
        .undoable()
        .shortcut("Alt+Shift+D"),
    CommandSpec::new(ids::INSERT_TIME, "Time", "Insert the current time.")
        .undoable()
        .shortcut("Alt+Shift+T"),
    CommandSpec::new(
        ids::INSERT_DATE_TIME,
        "Date & Time",
        "Insert the current date and time.",
    )
    .undoable()
    .shortcut("Alt+Shift+F"),
    CommandSpec::new(
        ids::INSERT_PAGE_TEMPLATE,
        "Page Templates",
        "Create a page from a template.",
    )
    .undoable()
    .args(TEMPLATE_ARGS),
    CommandSpec::new(ids::INSERT_EQUATION, "Equation", "Insert an equation.")
        .undoable()
        .shortcut("Alt+="),
    CommandSpec::new(ids::INSERT_SYMBOL, "Symbol", "Insert a symbol.")
        .undoable()
        .args(SYMBOL_ARGS),
    // Draw tab
    CommandSpec::new(
        ids::DRAW_TYPE,
        "Type",
        "Switch to typing and selecting text.",
    )
    .ready(),
    CommandSpec::new(
        ids::DRAW_LASSO,
        "Lasso Select",
        "Select ink and objects by drawing around them.",
    ),
    CommandSpec::new(
        ids::DRAW_PAN,
        "Panning Hand",
        "Move around the page by dragging.",
    ),
    CommandSpec::new(ids::DRAW_PEN, "Pen", "Draw with a pen.")
        .args(PEN_ARGS)
        .ready(),
    CommandSpec::new(
        ids::DRAW_HIGHLIGHTER,
        "Highlighter",
        "Draw with a highlighter.",
    )
    .args(PEN_ARGS)
    .ready(),
    CommandSpec::new(
        ids::DRAW_ERASER,
        "Eraser",
        "Erase ink strokes or parts of strokes.",
    )
    .args(ERASER_ARGS)
    .ready(),
    CommandSpec::new(
        ids::INK_ERASE,
        "Erase Ink",
        "Erase ink along a path: whole strokes or only the part under the eraser.",
    )
    .args(ERASE_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::DRAW_TOOL_SELECT,
        "Pick Up Tool",
        "Pick up a pen or highlighter from the Draw tab's list.",
    )
    .args(TOOL_INDEX_ARGS)
    .ready(),
    CommandSpec::new(
        ids::DRAW_TOOL_ADD,
        "Add Pen",
        "Add a pen or highlighter to the Draw tab's list and pick it up.",
    )
    .args(TOOL_ADD_ARGS)
    .ready(),
    CommandSpec::new(
        ids::DRAW_TOOL_DELETE,
        "Delete Tool",
        "Remove a pen or highlighter from the Draw tab's list.",
    )
    .args(TOOL_DELETE_ARGS)
    .destructive()
    .ready(),
    CommandSpec::new(
        ids::DRAW_EYEDROPPER,
        "Eyedropper",
        "Take the color of the ink at a point on the page for the current pen.",
    )
    .args(EYEDROPPER_ARGS)
    .ready(),
    CommandSpec::new(
        ids::DRAW_INK_REPLAY,
        "Ink Replay",
        "Play back how the ink on the page was drawn.",
    ),
    CommandSpec::new(
        ids::DRAW_COLOR,
        "Color",
        "Set the color of the current pen or highlighter.",
    )
    .args(REQUIRED_COLOR_ARGS)
    .ready(),
    CommandSpec::new(
        ids::DRAW_THICKNESS,
        "Thickness",
        "Set the width of the current pen.",
    )
    .args(PEN_WIDTH_ARGS)
    .ready(),
    CommandSpec::new(
        ids::INK_ADD_STROKE,
        "Add Stroke",
        "Add a stroke drawn with the current pen to the page.",
    )
    .args(STROKE_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::DRAW_RULER,
        "Ruler",
        "Show or hide the ruler for drawing straight lines.",
    ),
    CommandSpec::new(ids::DRAW_SHAPE, "Shapes", "Draw a shape.").args(SHAPE_ARGS),
    CommandSpec::new(
        ids::DRAW_BRING_TO_FRONT,
        "Bring to Front",
        "Move the selection in front of all other objects.",
    )
    .undoable(),
    CommandSpec::new(
        ids::DRAW_BRING_FORWARD,
        "Bring Forward",
        "Move the selection one step forward.",
    )
    .undoable(),
    CommandSpec::new(
        ids::DRAW_SEND_BACKWARD,
        "Send Backward",
        "Move the selection one step back.",
    )
    .undoable(),
    CommandSpec::new(
        ids::DRAW_SEND_TO_BACK,
        "Send to Back",
        "Move the selection behind all other objects.",
    )
    .undoable(),
    CommandSpec::new(
        ids::DRAW_ROTATE_RIGHT,
        "Rotate Right 90",
        "Rotate the selection 90 degrees clockwise.",
    )
    .undoable(),
    CommandSpec::new(
        ids::DRAW_ROTATE_LEFT,
        "Rotate Left 90",
        "Rotate the selection 90 degrees counterclockwise.",
    )
    .undoable(),
    CommandSpec::new(
        ids::DRAW_FLIP_HORIZONTAL,
        "Flip Horizontal",
        "Flip the selection left to right.",
    )
    .undoable(),
    CommandSpec::new(
        ids::DRAW_FLIP_VERTICAL,
        "Flip Vertical",
        "Flip the selection top to bottom.",
    )
    .undoable(),
    CommandSpec::new(
        ids::DRAW_INK_TO_SHAPE,
        "Ink to Shape",
        "Turn new drawings into clean shapes.",
    ),
    CommandSpec::new(
        ids::DRAW_INK_TO_TEXT,
        "Ink to Text",
        "Convert the selected handwriting to typed text.",
    )
    .undoable(),
    CommandSpec::new(
        ids::DRAW_INK_TO_MATH,
        "Ink to Math",
        "Convert a handwritten equation to a typed equation.",
    )
    .undoable(),
    // History tab
    CommandSpec::new(
        ids::HISTORY_NEXT_UNREAD,
        "Next Unread",
        "Go to the next page with unread changes.",
    ),
    CommandSpec::new(
        ids::HISTORY_MARK_READ,
        "Mark as Read",
        "Mark the current page as read.",
    )
    .shortcut("Ctrl+Q"),
    CommandSpec::new(
        ids::HISTORY_SHOW_UNREAD,
        "Show Unread Changes",
        "Highlight unread changes in this notebook.",
    ),
    CommandSpec::new(
        ids::HISTORY_RECENT_EDITS,
        "Recent Edits",
        "List pages changed recently.",
    ),
    CommandSpec::new(
        ids::HISTORY_FIND_BY_AUTHOR,
        "Find by Author",
        "List changes grouped by author.",
    ),
    CommandSpec::new(
        ids::HISTORY_HIDE_AUTHORS,
        "Hide Authors",
        "Show or hide author initials next to notes.",
    ),
    CommandSpec::new(
        ids::HISTORY_PAGE_VERSIONS,
        "Page Versions",
        "Show earlier versions of the current page.",
    ),
    CommandSpec::new(
        ids::HISTORY_RECYCLE_BIN,
        "Notebook Recycle Bin",
        "Show deleted pages and sections and restore them.",
    ),
    // Review tab
    CommandSpec::new(
        ids::REVIEW_SPELLING,
        "Spelling",
        "Check spelling on the current page.",
    )
    .shortcut("F7"),
    CommandSpec::new(
        ids::REVIEW_THESAURUS,
        "Thesaurus",
        "Find other words with a similar meaning.",
    )
    .shortcut("Shift+F7"),
    CommandSpec::new(
        ids::REVIEW_LANGUAGE,
        "Language",
        "Set the proofing language of the selection.",
    ),
    CommandSpec::new(
        ids::REVIEW_PASSWORD,
        "Password",
        "Protect the current section with a password.",
    ),
    CommandSpec::new(
        ids::REVIEW_LOCK_ALL,
        "Lock All",
        "Lock every password-protected section.",
    )
    .shortcut("Ctrl+Alt+L"),
    CommandSpec::new(
        ids::REVIEW_LINKED_NOTES,
        "Linked Notes",
        "Take notes linked to what you are viewing.",
    ),
    // View tab
    CommandSpec::new(
        ids::VIEW_NORMAL,
        "Normal View",
        "Show the ribbon and the navigation panes.",
    )
    .ready(),
    CommandSpec::new(ids::VIEW_FULL_PAGE, "Full Page View", "Show only the page.")
        .shortcut("F11")
        .ready(),
    CommandSpec::new(
        ids::VIEW_DOCK,
        "Dock to Desktop",
        "Dock the window to the side of the screen.",
    )
    .shortcut("Ctrl+Alt+D"),
    CommandSpec::new(
        ids::VIEW_NAVIGATION_TOGGLE,
        "Navigation Panes",
        "Show or hide the notebook, section, and page lists.",
    ),
    CommandSpec::new(
        ids::VIEW_RIBBON_TOGGLE,
        "Collapse the Ribbon",
        "Show only the ribbon tab names.",
    )
    .shortcut("Ctrl+F1")
    .ready(),
    CommandSpec::new(
        ids::VIEW_RIBBON_LAYOUT,
        "Ribbon Layout",
        "Use the classic or the simplified ribbon.",
    )
    .args(RIBBON_LAYOUT_ARGS)
    .ready(),
    CommandSpec::new(
        ids::VIEW_RIBBON_SHOW,
        "Show Ribbon",
        "Always show the ribbon, show only its tabs, or hide it until the pointer reaches the top.",
    )
    .args(RIBBON_SHOW_ARGS)
    .ready(),
    CommandSpec::new(
        ids::VIEW_PAGE_COLOR,
        "Page Color",
        "Set the background color of the page.",
    )
    .undoable()
    .args(REQUIRED_COLOR_ARGS)
    .ready(),
    CommandSpec::new(
        ids::VIEW_RULE_LINES,
        "Rule Lines",
        "Show rule lines or a grid on the page.",
    )
    .undoable()
    .args(RULE_LINE_ARGS)
    .ready(),
    CommandSpec::new(
        ids::VIEW_RULE_LINE_COLOR,
        "Rule Line Color",
        "Set the color of the rule lines and grid on the page.",
    )
    .undoable()
    .args(REQUIRED_COLOR_ARGS)
    .ready(),
    CommandSpec::new(
        ids::VIEW_HIDE_TITLE,
        "Hide Page Title",
        "Hide the title, date, and time of the page.",
    )
    .undoable(),
    CommandSpec::new(
        ids::VIEW_PAPER_SIZE,
        "Paper Size",
        "Set the paper size of the page.",
    )
    .undoable()
    .args(PAPER_SIZE_ARGS)
    .ready(),
    CommandSpec::new(ids::VIEW_ZOOM_IN, "Zoom In", "Zoom in on the page.")
        .shortcut("Ctrl+Alt+=")
        .ready(),
    CommandSpec::new(ids::VIEW_ZOOM_OUT, "Zoom Out", "Zoom out of the page.")
        .shortcut("Ctrl+Alt+-")
        .ready(),
    CommandSpec::new(ids::VIEW_ZOOM, "Zoom", "Set the zoom level.")
        .args(ZOOM_ARGS)
        .ready(),
    CommandSpec::new(ids::VIEW_ZOOM_ACTUAL, "100%", "Zoom to 100 percent.").ready(),
    CommandSpec::new(
        ids::VIEW_ZOOM_PAGE_WIDTH,
        "Page Width",
        "Zoom so the page fills the window width.",
    )
    .ready(),
    CommandSpec::new(
        ids::VIEW_NEW_WINDOW,
        "New Window",
        "Open another window on the same notebook.",
    )
    .shortcut("Ctrl+M"),
    CommandSpec::new(
        ids::VIEW_NEW_DOCKED_WINDOW,
        "New Docked Window",
        "Open another window docked to the side of the screen.",
    ),
    CommandSpec::new(
        ids::VIEW_THEME_SET,
        "Theme",
        "Switch to another theme preset.",
    )
    .args(THEME_ARGS)
    .ready(),
    CommandSpec::new(
        ids::VIEW_MODE_TOGGLE,
        "Dark Mode",
        "Switch between light and dark mode.",
    )
    .ready(),
    CommandSpec::new(
        ids::VIEW_MODE_SET,
        "Color Mode",
        "Use light, dark, or the system color mode.",
    )
    .args(MODE_ARGS)
    .ready(),
    // Help tab
    CommandSpec::new(ids::HELP_HELP, "Help", "Open the help for NoteCraft."),
    CommandSpec::new(
        ids::HELP_SUPPORT,
        "Support",
        "Find ways to get support for NoteCraft.",
    ),
    CommandSpec::new(
        ids::HELP_FEEDBACK,
        "Feedback",
        "Send feedback about NoteCraft.",
    ),
    CommandSpec::new(
        ids::HELP_TRAINING,
        "Show Training",
        "Show the training for NoteCraft.",
    ),
    // Navigation: the notebook list, the section tabs, and the page list
    CommandSpec::new(
        ids::NAV_NOTEBOOK_SELECT,
        "Select Notebook",
        "Show a notebook, at its first section's first page.",
    )
    .args(NAV_ARGS)
    .ready(),
    CommandSpec::new(
        ids::NAV_SECTION_SELECT,
        "Select Section",
        "Show a section, at its first page.",
    )
    .args(NAV_ARGS)
    .ready(),
    CommandSpec::new(ids::NAV_PAGE_SELECT, "Select Page", "Show a page.")
        .args(NAV_ARGS)
        .ready(),
    CommandSpec::new(
        ids::SECTION_NEW,
        "New Section",
        "Add a section to the current notebook.",
    )
    .args(NEW_SECTION_ARGS)
    .undoable()
    .shortcut("Ctrl+T")
    .ready(),
    CommandSpec::new(
        ids::SECTION_GROUP_NEW,
        "New Section Group",
        "Add a section group to the current notebook.",
    )
    .args(NEW_SECTION_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::SECTION_RENAME,
        "Rename Section",
        "Rename a section or section group.",
    )
    .args(RENAME_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::SECTION_DELETE,
        "Delete Section",
        "Delete a section or section group. It goes to the trash folder of its notebook.",
    )
    .args(OPTIONAL_ID_ARGS)
    .undoable()
    .destructive()
    .ready(),
    CommandSpec::new(
        ids::SECTION_MOVE,
        "Move Section",
        "Move a section or section group into a group or to the top level.",
    )
    .args(SECTION_MOVE_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::SECTION_SET_COLOR,
        "Section Color",
        "Set the color of a section or section group.",
    )
    .args(SET_COLOR_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::PAGE_NEW,
        "Add Page",
        "Add a page to the current section.",
    )
    .args(NEW_PAGE_ARGS)
    .undoable()
    .shortcut("Ctrl+N")
    .ready(),
    CommandSpec::new(ids::PAGE_RENAME, "Rename Page", "Rename a page.")
        .args(RENAME_ARGS)
        .undoable()
        .ready(),
    CommandSpec::new(
        ids::PAGE_DELETE,
        "Delete Page",
        "Delete a page. It goes to the trash folder of its notebook.",
    )
    .args(OPTIONAL_ID_ARGS)
    .undoable()
    .destructive()
    .ready(),
    CommandSpec::new(
        ids::PAGE_MOVE,
        "Move Page",
        "Move a page to another place in its section or to another section.",
    )
    .args(PAGE_MOVE_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::PAGE_SET_LEVEL,
        "Set Page Level",
        "Set how far a page is indented as a subpage.",
    )
    .args(PAGE_LEVEL_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::PAGE_MAKE_SUBPAGE,
        "Make Subpage",
        "Make a page a subpage of the page above it.",
    )
    .args(OPTIONAL_ID_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::PAGE_PROMOTE_SUBPAGE,
        "Promote Subpage",
        "Move a subpage up one level.",
    )
    .args(OPTIONAL_ID_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::NOTEBOOK_RENAME,
        "Rename Notebook",
        "Rename a notebook. Its folder keeps its name.",
    )
    .args(RENAME_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::NOTEBOOK_SET_COLOR,
        "Notebook Color",
        "Set the color of a notebook.",
    )
    .args(SET_COLOR_ARGS)
    .undoable()
    .ready(),
    CommandSpec::new(
        ids::PAGE_CONVERT_TO_MARKDOWN,
        "Convert to Markdown",
        "Turn the page into a plain markdown page. Ink and note positions are removed.",
    )
    .args(CONVERT_ARGS)
    .destructive()
    .ready(),
    CommandSpec::new(ids::SEARCH_ALL, "Search", "Search every open notebook.").shortcut("Ctrl+E"),
];

/// The handler of every Ready command.
static HANDLERS: &[(CommandId, Handler)] = &[
    (ids::FILE_NEW_NOTEBOOK, file::new_notebook),
    (ids::FILE_OPEN_NOTEBOOK, file::open_notebook),
    (ids::FILE_OPEN, file::open_file),
    (ids::FILE_RECENT_CLEAR, file::recent_clear),
    (ids::FILE_ABOUT, file::about),
    (ids::FILE_COPY_ABOUT_INFO, file::copy_about_info),
    (ids::FILE_RESTORE_UNSAVED, file::restore_unsaved),
    (ids::FILE_DISCARD_UNSAVED, file::discard_unsaved),
    (ids::FILE_KEEP_MY_VERSION, file::keep_my_version),
    (ids::FILE_USE_OUTSIDE_VERSION, file::use_outside_version),
    (ids::VIEW_THEME_SET, theme::theme_set),
    (ids::VIEW_MODE_TOGGLE, theme::mode_toggle),
    (ids::VIEW_MODE_SET, theme::mode_set),
    (ids::VIEW_RIBBON_TOGGLE, layout::ribbon_toggle),
    (ids::VIEW_RIBBON_LAYOUT, layout::ribbon_layout),
    (ids::VIEW_RIBBON_SHOW, layout::ribbon_show),
    (ids::VIEW_FULL_PAGE, layout::full_page),
    (ids::VIEW_NORMAL, layout::normal),
    (ids::VIEW_PAGE_COLOR, canvas::page_color),
    (ids::VIEW_RULE_LINES, canvas::rule_lines),
    (ids::VIEW_RULE_LINE_COLOR, canvas::rule_line_color),
    (ids::VIEW_PAPER_SIZE, canvas::paper_size),
    (ids::VIEW_ZOOM_IN, canvas::zoom_in),
    (ids::VIEW_ZOOM_OUT, canvas::zoom_out),
    (ids::VIEW_ZOOM, canvas::zoom),
    (ids::VIEW_ZOOM_ACTUAL, canvas::zoom_actual),
    (ids::VIEW_ZOOM_PAGE_WIDTH, canvas::zoom_page_width),
    (ids::DRAW_TYPE, draw::draw_type),
    (ids::DRAW_PEN, draw::draw_pen),
    (ids::DRAW_HIGHLIGHTER, draw::draw_highlighter),
    (ids::DRAW_ERASER, draw::draw_eraser),
    (ids::INK_ERASE, draw::erase),
    (ids::DRAW_TOOL_SELECT, draw::tool_select),
    (ids::DRAW_TOOL_ADD, draw::tool_add),
    (ids::DRAW_TOOL_DELETE, draw::tool_delete),
    (ids::DRAW_EYEDROPPER, draw::eyedropper),
    (ids::DRAW_COLOR, draw::draw_color),
    (ids::DRAW_THICKNESS, draw::draw_thickness),
    (ids::INK_ADD_STROKE, draw::add_stroke),
    (ids::EDIT_UNDO, edit::undo),
    (ids::EDIT_REDO, edit::redo),
    (ids::NAV_NOTEBOOK_SELECT, nav::notebook_select),
    (ids::NAV_SECTION_SELECT, nav::section_select),
    (ids::NAV_PAGE_SELECT, nav::page_select),
    (ids::FILE_CLOSE_NOTEBOOK, structure::close_notebook),
    (ids::SECTION_NEW, structure::section_new),
    (ids::SECTION_GROUP_NEW, structure::section_group_new),
    (ids::SECTION_RENAME, structure::section_rename),
    (ids::SECTION_DELETE, structure::section_delete),
    (ids::SECTION_MOVE, structure::section_move),
    (ids::SECTION_SET_COLOR, structure::section_set_color),
    (ids::PAGE_NEW, structure::page_new),
    (ids::PAGE_RENAME, structure::page_rename),
    (ids::PAGE_DELETE, structure::page_delete),
    (ids::PAGE_MOVE, structure::page_move),
    (ids::PAGE_SET_LEVEL, structure::page_set_level),
    (ids::PAGE_MAKE_SUBPAGE, structure::page_make_subpage),
    (ids::PAGE_PROMOTE_SUBPAGE, structure::page_promote_subpage),
    (ids::NOTEBOOK_RENAME, structure::notebook_rename),
    (ids::NOTEBOOK_SET_COLOR, structure::notebook_set_color),
    (ids::PAGE_CONVERT_TO_MARKDOWN, page::convert_to_markdown),
];

/// The handler for `id`, if it has one.
pub(super) fn handler(id: CommandId) -> Option<Handler> {
    HANDLERS
        .iter()
        .find(|(handled, _)| *handled == id)
        .map(|(_, handler)| *handler)
}

/// Every id with a handler, for tests.
#[cfg(test)]
pub(super) fn handled_ids() -> impl Iterator<Item = CommandId> {
    HANDLERS.iter().map(|(id, _)| *id)
}
