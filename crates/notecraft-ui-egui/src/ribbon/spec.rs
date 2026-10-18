//! The ribbon as data: tabs, groups, and buttons, each bound to a command id. The ribbon
//! widget only renders this table. Tab and group names and order follow OneNote 2016 to 2021;
//! there are no tabs OneNote does not have (build_plan/02 PR 0.5).

use notecraft_engine::cmd::{ids, CommandId, Map, Value};

use crate::icons::Icon;

/// A fixed argument value for a button or menu item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArgValue {
    Str(&'static str),
    Int(i64),
    Num(f64),
}

/// One named argument passed when a button or menu item runs its command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arg {
    pub name: &'static str,
    pub value: ArgValue,
}

pub const fn str_arg(name: &'static str, value: &'static str) -> Arg {
    Arg {
        name,
        value: ArgValue::Str(value),
    }
}

pub const fn int_arg(name: &'static str, value: i64) -> Arg {
    Arg {
        name,
        value: ArgValue::Int(value),
    }
}

pub const fn num_arg(name: &'static str, value: f64) -> Arg {
    Arg {
        name,
        value: ArgValue::Num(value),
    }
}

/// The JSON arguments for a command: an object, or null when there are none.
pub fn args_value(args: &[Arg]) -> Value {
    if args.is_empty() {
        return Value::Null;
    }
    let mut map = Map::new();
    for arg in args {
        let value = match arg.value {
            ArgValue::Str(s) => Value::from(s),
            ArgValue::Int(i) => Value::from(i),
            ArgValue::Num(n) => Value::from(n),
        };
        map.insert(arg.name.to_owned(), value);
    }
    Value::Object(map)
}

/// An entry in a dropdown menu or the File backstage list.
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub label: &'static str,
    pub command: CommandId,
    pub args: &'static [Arg],
    pub preview: Preview,
}

/// What a menu item shows in front of its label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Preview {
    None,
    /// A color swatch, 0xRRGGBB.
    Color(u32),
    /// A stroke of this width in points.
    Width(f32),
}

impl Item {
    pub const fn new(label: &'static str, command: CommandId) -> Self {
        Self {
            label,
            command,
            args: &[],
            preview: Preview::None,
        }
    }

    pub const fn args(mut self, args: &'static [Arg]) -> Self {
        self.args = args;
        self
    }

    pub const fn color(mut self, rgb: u32) -> Self {
        self.preview = Preview::Color(rgb);
        self
    }

    pub const fn width(mut self, points: f32) -> Self {
        self.preview = Preview::Width(points);
        self
    }
}

/// How a color menu lays out its swatches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Palette {
    /// Office's layout: Theme Colors (the first row of hues apart from their tints and
    /// shades) and a row of Standard Colors.
    Theme,
    /// One plain grid.
    Grid,
}

/// A color picker menu: swatches, Recent Colors, an optional "no color" entry, and an
/// optional More Colors dialog. The chosen color goes to the button's command as `arg` =
/// `#RRGGBB`.
#[derive(Clone, Copy, Debug)]
pub struct ColorMenu {
    /// The button applies the last color used (shown as the bar under its icon) and its
    /// arrow opens the picker; otherwise the whole button opens the picker.
    pub split: bool,
    pub arg: &'static str,
    pub colors: &'static [u32],
    pub columns: usize,
    pub palette: Palette,
    /// The bar's color before anything is picked.
    pub default: u32,
    /// Label and argument value of the "no color" entry, if the command has one.
    pub none: Option<(&'static str, &'static str)>,
    /// The "no color" entry comes last instead of first (Text Highlight Color).
    pub none_last: bool,
    /// Offer More Colors, which opens the Colors dialog.
    pub custom: bool,
}

/// An editable box with a list, as OneNote's Font and Font Size boxes. What is typed or
/// picked goes to the button's command as `arg`.
#[derive(Clone, Copy, Debug)]
pub struct Combo {
    /// The list when nothing better is known; the font box lists the installed families.
    pub items: &'static [Item],
    /// The value shown until the selection says otherwise.
    pub current: &'static str,
    pub width: f32,
    pub arg: &'static str,
    /// Typing filters the list (the font box).
    pub search: bool,
    /// The value is a number from `min` to `max` (the size box); other text is ignored.
    pub number: Option<(f64, f64)>,
    /// Use the installed font families as the list once they are known.
    pub fonts: bool,
}

/// How a button is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    /// 32 px icon with the label under it.
    Large,
    /// 16 px icon with the label beside it.
    Small,
    /// 16 px icon only; the label is the tooltip and the accessible name.
    IconOnly,
}

/// What a button's menu does.
#[derive(Clone, Copy, Debug)]
pub enum Menu {
    None,
    /// The button runs its command; its arrow opens the menu.
    Split(&'static [Item]),
    /// The whole button opens the menu.
    Dropdown(&'static [Item]),
    /// A color picker.
    Colors(ColorMenu),
    /// A grid to pick a table size; runs the button's command with `rows` and `columns`.
    TableGrid,
    /// The button is drawn as an editable box with a list.
    Combo(Combo),
    /// The whole button opens a gallery of paper thumbnails, one card per item (Rule Lines).
    /// Each item's `style` argument names the rule lines style its card draws.
    Papers(&'static [Item]),
    /// Format Background: the Rule Lines and Grid Lines cards, then submenus for the rule line
    /// color and the page color.
    Background(&'static BackgroundMenu),
}

/// What the Format Background dropdown holds.
#[derive(Clone, Copy, Debug)]
pub struct BackgroundMenu {
    /// The cards, as the Rule Lines menu's items.
    pub papers: &'static [Item],
    /// The Rule Line Color submenu (`view.rule_line_color`).
    pub rule_color: ColorMenu,
    /// The Page Color submenu (`view.page_color`).
    pub page_color: ColorMenu,
}

impl Menu {
    /// The fixed menu items; color pickers and the table grid build theirs at run time.
    pub fn items(&self) -> &'static [Item] {
        match self {
            Self::Split(items) | Self::Dropdown(items) | Self::Papers(items) => items,
            Self::Background(menu) => menu.papers,
            Self::Combo(combo) => combo.items,
            Self::None | Self::Colors(_) | Self::TableGrid => &[],
        }
    }

    /// Whether clicking the button's main part runs its command (rather than opening the
    /// menu).
    pub fn is_split(&self) -> bool {
        match self {
            Self::Split(_) => true,
            Self::Colors(colors) => colors.split,
            Self::None
            | Self::Dropdown(_)
            | Self::Papers(_)
            | Self::Background(_)
            | Self::TableGrid
            | Self::Combo(_) => false,
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Button {
    pub label: &'static str,
    pub icon: Icon,
    /// The command the button runs, or for a dropdown the command its items share.
    pub command: CommandId,
    pub args: &'static [Arg],
    pub size: Size,
    pub menu: Menu,
}

impl Button {
    pub const fn large(label: &'static str, icon: Icon, command: CommandId) -> Self {
        Self::new(label, icon, command, Size::Large)
    }

    pub const fn small(label: &'static str, icon: Icon, command: CommandId) -> Self {
        Self::new(label, icon, command, Size::Small)
    }

    pub const fn icon(label: &'static str, icon: Icon, command: CommandId) -> Self {
        Self::new(label, icon, command, Size::IconOnly)
    }

    const fn new(label: &'static str, icon: Icon, command: CommandId, size: Size) -> Self {
        Self {
            label,
            icon,
            command,
            args: &[],
            size,
            menu: Menu::None,
        }
    }

    pub const fn args(mut self, args: &'static [Arg]) -> Self {
        self.args = args;
        self
    }

    pub const fn split(mut self, items: &'static [Item]) -> Self {
        self.menu = Menu::Split(items);
        self
    }

    pub const fn dropdown(mut self, items: &'static [Item]) -> Self {
        self.menu = Menu::Dropdown(items);
        self
    }

    pub const fn papers(mut self, items: &'static [Item]) -> Self {
        self.menu = Menu::Papers(items);
        self
    }

    pub const fn background(mut self, menu: &'static BackgroundMenu) -> Self {
        self.menu = Menu::Background(menu);
        self
    }

    pub const fn colors(mut self, menu: ColorMenu) -> Self {
        self.menu = Menu::Colors(menu);
        self
    }

    pub const fn table_grid(mut self) -> Self {
        self.menu = Menu::TableGrid;
        self
    }

    pub const fn combo(mut self, combo: Combo) -> Self {
        self.menu = Menu::Combo(combo);
        self
    }
}

/// The main color grid: ten hues in five rows (the hue, three lighter tints, and a darker
/// shade). Original values.
pub const COLOR_GRID: &[u32] = &[
    0x1F1F1F, 0x6B6B6B, 0xC62828, 0xE65100, 0xF9A825, 0x2E7D32, 0x00838F, 0x1565C0, 0x4527A0,
    0xAD1457, 0xD2D2D2, 0xE1E1E1, 0xF4D4D4, 0xFADCCC, 0xFEEED3, 0xD5E5D6, 0xCCE6E9, 0xD0E0F2,
    0xDAD4EC, 0xEFD0DD, 0xA5A5A5, 0xC4C4C4, 0xE8A9A9, 0xF5B999, 0xFDDCA8, 0xABCBAD, 0x99CDD2,
    0xA1C1E6, 0xB5A9D9, 0xDEA1BC, 0x6D6D6D, 0x9F9F9F, 0xDA7373, 0xEF8E59, 0xFBC671, 0x77AA7A,
    0x59AEB6, 0x679BD6, 0x8673C1, 0xCA6692, 0x141414, 0x464646, 0x811A1A, 0x963500, 0xA26D18,
    0x1E5120, 0x00555D, 0x0E427D, 0x2D1968, 0x700D39,
];

/// The Standard Colors row under Theme Colors: ten plain hues. Original values.
pub const STANDARD_COLORS: &[u32] = &[
    0xB71C1C, 0xE53935, 0xFB8C00, 0xFDD835, 0x9CCC65, 0x43A047, 0x29B6F6, 0x1E88E5, 0x1A237E,
    0x7B1FA2,
];

/// Highlighter colors, fifteen as in OneNote: the light ones read text through, the dark ones
/// suit light text. Original values.
pub const HIGHLIGHT_GRID: &[u32] = &[
    0xFFF176, 0xA5D6A7, 0x80DEEA, 0xF48FB1, 0x90CAF9, 0xFFCC80, 0xCE93D8, 0xC5E1A5, 0xBCAAA4,
    0xE0E0E0, 0xEF5350, 0x26A69A, 0x5C6BC0, 0x9E9E9E, 0x616161,
];

/// Page background colors: soft enough for any ink color on top.
pub const PAGE_COLOR_GRID: &[u32] = &[
    0xDCEBF7, 0xE2F0D9, 0xFFF8DC, 0xFBE3EC, 0xECE3F5, 0xFDE9D9, 0xE0F7FA, 0xF1F8E9, 0xFFF3E0,
    0xEEEEEE,
];

/// How a gallery draws its entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GalleryKind {
    /// Each entry is drawn in its own style (size and weight), as the Styles gallery.
    Styles,
    /// Each entry has its tag icon and, for the first nine, its Ctrl+number shortcut.
    Tags,
}

/// An in-ribbon gallery, as OneNote's Styles and Tags: a bordered box showing `visible`
/// entries, arrows to scroll through them, and a More arrow that opens the full list.
///
/// The gallery is backed by a dropdown button (`button`) whose items are the full list. That
/// button is what the simplified ribbon, the More Options menu, key tips, and the command
/// tests see, so a gallery is one dropdown there. In the classic ribbon the first
/// `scroll_len` items are the entries the box scrolls through; the rest of the list (such as
/// Customize Tags) is only in the More menu.
#[derive(Clone, Copy, Debug)]
pub struct Gallery {
    pub button: Button,
    pub kind: GalleryKind,
    /// How many entries show at once, and how far an arrow scrolls.
    pub visible: usize,
    /// Width of the entry box in points.
    pub width: f32,
    /// How many leading items of the list are entries of the box.
    pub scroll_len: usize,
}

impl Gallery {
    pub const fn new(
        kind: GalleryKind,
        button: Button,
        visible: usize,
        width: f32,
        scroll_len: usize,
    ) -> Self {
        Self {
            button,
            kind,
            visible,
            width,
            scroll_len,
        }
    }

    /// The full list, which the More arrow opens.
    pub fn items(&self) -> &'static [Item] {
        self.button.menu.items()
    }

    /// The items the box scrolls through.
    pub fn entries(&self) -> &'static [Item] {
        let items = self.items();
        items.get(..self.scroll_len).unwrap_or(items)
    }
}

/// One column of a group.
#[derive(Clone, Copy, Debug)]
pub enum Column {
    /// A gallery of entries.
    Gallery(Gallery),
    /// One large button.
    Large(Button),
    /// Up to three small buttons stacked.
    Stack(&'static [Button]),
    /// Up to three rows of small or icon-only buttons.
    Rows(&'static [&'static [Button]]),
    /// The Draw tab's pens and highlighters: one tall button per tool of the list the user
    /// keeps (see `ribbon::tools`). The list is data, so these are not in [`Group::buttons`].
    Tools,
}

#[derive(Clone, Copy, Debug)]
pub struct Group {
    pub name: &'static str,
    pub columns: &'static [Column],
}

#[derive(Clone, Copy, Debug)]
pub enum TabKind {
    /// The File tab: a full-window list of entries instead of groups.
    Backstage(&'static [Item]),
    Groups(&'static [Group]),
}

#[derive(Clone, Copy, Debug)]
pub struct Tab {
    pub name: &'static str,
    /// The key tip letter (Alt, then this letter).
    pub key: &'static str,
    pub kind: TabKind,
}

impl Group {
    /// Every button in the group, in drawing order.
    pub fn buttons(&self) -> impl Iterator<Item = &'static Button> + '_ {
        self.columns
            .iter()
            .flat_map(|column| -> Vec<&'static Button> {
                match column {
                    Column::Gallery(gallery) => vec![&gallery.button],
                    Column::Large(button) => vec![button],
                    Column::Stack(buttons) => buttons.iter().collect(),
                    Column::Rows(rows) => rows.iter().flat_map(|row| row.iter()).collect(),
                    Column::Tools => Vec::new(),
                }
            })
    }
}

/// Every command the ribbon can run, with the label it shows: buttons, their menu items,
/// and the File backstage entries.
pub fn all_commands() -> Vec<(&'static str, CommandId, &'static [Arg])> {
    let mut out = Vec::new();
    for tab in TABS {
        match tab.kind {
            TabKind::Backstage(items) => {
                out.extend(items.iter().map(|i| (i.label, i.command, i.args)));
            }
            TabKind::Groups(groups) => {
                for button in groups.iter().flat_map(|g| g.buttons().collect::<Vec<_>>()) {
                    out.push((button.label, button.command, button.args));
                    let items = button.menu.items();
                    out.extend(items.iter().map(|i| (i.label, i.command, i.args)));
                }
            }
        }
    }
    out
}

// File tab

const BACKSTAGE: &[Item] = &[
    Item::new("Info", ids::FILE_INFO),
    Item::new("New", ids::FILE_NEW_NOTEBOOK),
    Item::new("Open", ids::FILE_OPEN),
    Item::new("Print", ids::FILE_PRINT),
    Item::new("Export", ids::FILE_EXPORT),
    Item::new("Close", ids::FILE_CLOSE_NOTEBOOK),
    Item::new("Options", ids::FILE_OPTIONS),
    Item::new("About", ids::FILE_ABOUT),
];

// Home tab

const PASTE_OPTIONS: &[Item] = &[
    Item::new("Keep Source Formatting", ids::EDIT_PASTE_KEEP_FORMATTING),
    Item::new("Merge Formatting", ids::EDIT_PASTE_MERGE_FORMATTING),
    Item::new("Keep Text Only", ids::EDIT_PASTE_TEXT_ONLY),
    Item::new("Paste as Picture", ids::EDIT_PASTE_PICTURE),
];

const CLIPBOARD: Group = Group {
    name: "Clipboard",
    columns: &[
        Column::Large(Button::large("Paste", Icon::Paste, ids::EDIT_PASTE).split(PASTE_OPTIONS)),
        Column::Stack(&[
            Button::small("Cut", Icon::Cut, ids::EDIT_CUT),
            Button::small("Copy", Icon::Copy, ids::EDIT_COPY),
            Button::small(
                "Format Painter",
                Icon::FormatPainter,
                ids::EDIT_FORMAT_PAINTER,
            ),
        ]),
    ],
};

/// Font families offered in the Font list. Only the names are stored; nothing is bundled.
const FONTS: &[Item] = &[
    Item::new("Calibri", ids::FORMAT_FONT_FAMILY).args(&[str_arg("name", "Calibri")]),
    Item::new("Arial", ids::FORMAT_FONT_FAMILY).args(&[str_arg("name", "Arial")]),
    Item::new("Segoe UI", ids::FORMAT_FONT_FAMILY).args(&[str_arg("name", "Segoe UI")]),
    Item::new("Georgia", ids::FORMAT_FONT_FAMILY).args(&[str_arg("name", "Georgia")]),
    Item::new("Times New Roman", ids::FORMAT_FONT_FAMILY)
        .args(&[str_arg("name", "Times New Roman")]),
    Item::new("Consolas", ids::FORMAT_FONT_FAMILY).args(&[str_arg("name", "Consolas")]),
];

const FONT_SIZES: &[Item] = &[
    Item::new("8", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 8.0)]),
    Item::new("9", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 9.0)]),
    Item::new("10", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 10.0)]),
    Item::new("11", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 11.0)]),
    Item::new("12", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 12.0)]),
    Item::new("14", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 14.0)]),
    Item::new("16", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 16.0)]),
    Item::new("18", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 18.0)]),
    Item::new("20", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 20.0)]),
    Item::new("24", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 24.0)]),
    Item::new("28", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 28.0)]),
    Item::new("36", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 36.0)]),
    Item::new("48", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 48.0)]),
    Item::new("72", ids::FORMAT_FONT_SIZE).args(&[num_arg("points", 72.0)]),
];

const BASIC_TEXT: Group = Group {
    name: "Basic Text",
    columns: &[Column::Rows(&[
        &[
            Button::small("Font", Icon::Font, ids::FORMAT_FONT_FAMILY).combo(Combo {
                items: FONTS,
                current: "Calibri",
                width: 128.0,
                arg: "name",
                search: true,
                number: None,
                fonts: true,
            }),
            Button::small("Font Size", Icon::FontSize, ids::FORMAT_FONT_SIZE).combo(Combo {
                items: FONT_SIZES,
                current: "11",
                width: 44.0,
                arg: "points",
                search: false,
                number: Some((1.0, 1638.0)),
                fonts: false,
            }),
            Button::icon("Bullets", Icon::Bullets, ids::FORMAT_BULLETS),
            Button::icon("Numbering", Icon::Numbering, ids::FORMAT_NUMBERING),
            Button::icon("Decrease Indent", Icon::Outdent, ids::FORMAT_OUTDENT),
            Button::icon("Increase Indent", Icon::Indent, ids::FORMAT_INDENT),
            Button::icon(
                "Clear All Formatting",
                Icon::ClearFormatting,
                ids::FORMAT_CLEAR,
            ),
        ],
        &[
            Button::icon("Bold", Icon::Bold, ids::FORMAT_BOLD),
            Button::icon("Italic", Icon::Italic, ids::FORMAT_ITALIC),
            Button::icon("Underline", Icon::Underline, ids::FORMAT_UNDERLINE),
            Button::icon(
                "Strikethrough",
                Icon::Strikethrough,
                ids::FORMAT_STRIKETHROUGH,
            ),
            Button::icon("Subscript", Icon::Subscript, ids::FORMAT_SUBSCRIPT),
            Button::icon("Superscript", Icon::Superscript, ids::FORMAT_SUPERSCRIPT),
            Button::icon(
                "Text Highlight Color",
                Icon::Highlight,
                ids::FORMAT_HIGHLIGHT,
            )
            .colors(ColorMenu {
                split: true,
                arg: "color",
                colors: HIGHLIGHT_GRID,
                columns: 5,
                palette: Palette::Grid,
                default: 0xFFF176,
                none: Some(("No Color", "none")),
                none_last: true,
                custom: true,
            }),
            Button::icon("Font Color", Icon::FontColor, ids::FORMAT_FONT_COLOR).colors(ColorMenu {
                split: true,
                arg: "color",
                colors: COLOR_GRID,
                columns: 10,
                palette: Palette::Theme,
                default: 0xE53935,
                none: Some(("Automatic", "none")),
                none_last: false,
                custom: true,
            }),
        ],
        &[
            Button::icon("Align Left", Icon::AlignLeft, ids::FORMAT_ALIGN_LEFT),
            Button::icon("Center", Icon::AlignCenter, ids::FORMAT_ALIGN_CENTER),
            Button::icon("Align Right", Icon::AlignRight, ids::FORMAT_ALIGN_RIGHT),
        ],
    ])],
};

const ALL_STYLES: &[Item] = &[
    Item::new("Heading 1", ids::STYLE_APPLY).args(&[str_arg("style", "heading1")]),
    Item::new("Heading 2", ids::STYLE_APPLY).args(&[str_arg("style", "heading2")]),
    Item::new("Heading 3", ids::STYLE_APPLY).args(&[str_arg("style", "heading3")]),
    Item::new("Heading 4", ids::STYLE_APPLY).args(&[str_arg("style", "heading4")]),
    Item::new("Heading 5", ids::STYLE_APPLY).args(&[str_arg("style", "heading5")]),
    Item::new("Heading 6", ids::STYLE_APPLY).args(&[str_arg("style", "heading6")]),
    Item::new("Page Title", ids::STYLE_APPLY).args(&[str_arg("style", "title")]),
    Item::new("Citation", ids::STYLE_APPLY).args(&[str_arg("style", "citation")]),
    Item::new("Quote", ids::STYLE_APPLY).args(&[str_arg("style", "quote")]),
    Item::new("Code", ids::STYLE_APPLY).args(&[str_arg("style", "code")]),
    Item::new("Normal", ids::STYLE_APPLY).args(&[str_arg("style", "normal")]),
];

const STYLES: Group = Group {
    name: "Styles",
    columns: &[Column::Gallery(Gallery::new(
        GalleryKind::Styles,
        Button::large("Styles", Icon::Style, ids::STYLE_APPLY).dropdown(ALL_STYLES),
        2,
        132.0,
        11,
    ))],
};

const MORE_TAGS: &[Item] = &[
    Item::new("To Do", ids::TAG_APPLY).args(&[str_arg("tag", "todo")]),
    Item::new("Important", ids::TAG_APPLY).args(&[str_arg("tag", "important")]),
    Item::new("Question", ids::TAG_APPLY).args(&[str_arg("tag", "question")]),
    Item::new("Remember for Later", ids::TAG_APPLY).args(&[str_arg("tag", "remember")]),
    Item::new("Definition", ids::TAG_APPLY).args(&[str_arg("tag", "definition")]),
    Item::new("Highlight", ids::TAG_APPLY).args(&[str_arg("tag", "highlight")]),
    Item::new("Contact", ids::TAG_APPLY).args(&[str_arg("tag", "contact")]),
    Item::new("Address", ids::TAG_APPLY).args(&[str_arg("tag", "address")]),
    Item::new("Phone Number", ids::TAG_APPLY).args(&[str_arg("tag", "phone")]),
    Item::new("Idea", ids::TAG_APPLY).args(&[str_arg("tag", "idea")]),
    Item::new("Customize Tags", ids::TAG_CUSTOMIZE),
    Item::new("Remove Tag", ids::TAG_REMOVE),
];

const TAGS: Group = Group {
    name: "Tags",
    columns: &[
        Column::Gallery(Gallery::new(
            GalleryKind::Tags,
            Button::large("Tags", Icon::Tag, ids::TAG_APPLY).dropdown(MORE_TAGS),
            2,
            160.0,
            10,
        )),
        Column::Large(Button::large("To Do Tag", Icon::TodoTag, ids::TAG_TODO)),
        Column::Large(Button::large("Find Tags", Icon::FindTags, ids::TAG_FIND)),
    ],
};

const EMAIL: Group = Group {
    name: "Email",
    columns: &[Column::Large(Button::large(
        "Email Page",
        Icon::Email,
        ids::EMAIL_PAGE,
    ))],
};

const MEETINGS: Group = Group {
    name: "Meetings",
    columns: &[Column::Large(Button::large(
        "Meeting Details",
        Icon::Meeting,
        ids::MEETING_DETAILS,
    ))],
};

// Insert tab

/// Page templates. These are NoteCraft's own templates, named by purpose.
const TEMPLATES: &[Item] = &[
    Item::new("Blank", ids::INSERT_PAGE_TEMPLATE).args(&[str_arg("template", "blank")]),
    Item::new("Meeting Notes", ids::INSERT_PAGE_TEMPLATE).args(&[str_arg("template", "meeting")]),
    Item::new("To Do List", ids::INSERT_PAGE_TEMPLATE).args(&[str_arg("template", "todo")]),
    Item::new("Lecture Notes", ids::INSERT_PAGE_TEMPLATE).args(&[str_arg("template", "lecture")]),
    Item::new("Project Overview", ids::INSERT_PAGE_TEMPLATE)
        .args(&[str_arg("template", "project")]),
];

/// Common symbols. The values are escapes so the source stays ASCII.
const SYMBOLS: &[Item] = &[
    Item::new("Degree", ids::INSERT_SYMBOL).args(&[str_arg("symbol", "\u{00B0}")]),
    Item::new("Plus-Minus", ids::INSERT_SYMBOL).args(&[str_arg("symbol", "\u{00B1}")]),
    Item::new("Multiplication", ids::INSERT_SYMBOL).args(&[str_arg("symbol", "\u{00D7}")]),
    Item::new("Division", ids::INSERT_SYMBOL).args(&[str_arg("symbol", "\u{00F7}")]),
    Item::new("Pi", ids::INSERT_SYMBOL).args(&[str_arg("symbol", "\u{03C0}")]),
    Item::new("Euro", ids::INSERT_SYMBOL).args(&[str_arg("symbol", "\u{20AC}")]),
    Item::new("Copyright", ids::INSERT_SYMBOL).args(&[str_arg("symbol", "\u{00A9}")]),
    Item::new("Check Mark", ids::INSERT_SYMBOL).args(&[str_arg("symbol", "\u{2713}")]),
];

const INSERT_GROUPS: &[Group] = &[
    Group {
        name: "Insert",
        columns: &[Column::Large(Button::large(
            "Insert Space",
            Icon::InsertSpace,
            ids::INSERT_SPACE,
        ))],
    },
    Group {
        name: "Tables",
        columns: &[Column::Large(
            Button::large("Table", Icon::Table, ids::INSERT_TABLE).table_grid(),
        )],
    },
    Group {
        name: "Files",
        columns: &[
            Column::Large(Button::large(
                "File Printout",
                Icon::FilePrintout,
                ids::INSERT_FILE_PRINTOUT,
            )),
            Column::Large(Button::large(
                "File Attachment",
                Icon::FileAttachment,
                ids::INSERT_FILE_ATTACHMENT,
            )),
        ],
    },
    Group {
        name: "Images",
        columns: &[
            Column::Large(Button::large(
                "Pictures",
                Icon::Picture,
                ids::INSERT_PICTURE,
            )),
            Column::Large(Button::large(
                "Screen Clipping",
                Icon::ScreenClipping,
                ids::INSERT_SCREEN_CLIPPING,
            )),
        ],
    },
    Group {
        name: "Links",
        columns: &[Column::Large(Button::large(
            "Link",
            Icon::Link,
            ids::INSERT_LINK,
        ))],
    },
    Group {
        name: "Recording",
        columns: &[
            Column::Large(Button::large(
                "Record Audio",
                Icon::Audio,
                ids::INSERT_AUDIO,
            )),
            Column::Large(Button::large(
                "Record Video",
                Icon::Video,
                ids::INSERT_VIDEO,
            )),
        ],
    },
    Group {
        name: "Time Stamp",
        columns: &[Column::Stack(&[
            Button::small("Date", Icon::Date, ids::INSERT_DATE),
            Button::small("Time", Icon::Time, ids::INSERT_TIME),
            Button::small("Date & Time", Icon::DateTime, ids::INSERT_DATE_TIME),
        ])],
    },
    Group {
        name: "Pages",
        columns: &[Column::Large(
            Button::large(
                "Page Templates",
                Icon::PageTemplate,
                ids::INSERT_PAGE_TEMPLATE,
            )
            .split(TEMPLATES),
        )],
    },
    Group {
        name: "Symbols",
        columns: &[
            Column::Large(Button::large(
                "Equation",
                Icon::Equation,
                ids::INSERT_EQUATION,
            )),
            Column::Large(Button::large("Symbol", Icon::Symbol, ids::INSERT_SYMBOL).split(SYMBOLS)),
        ],
    },
];

// Draw tab

const ERASERS: &[Item] = &[
    Item::new("Stroke Eraser", ids::DRAW_ERASER).args(&[str_arg("mode", "stroke")]),
    Item::new("Small Eraser", ids::DRAW_ERASER)
        .args(&[str_arg("mode", "point"), str_arg("size", "small")]),
    Item::new("Medium Eraser", ids::DRAW_ERASER)
        .args(&[str_arg("mode", "point"), str_arg("size", "medium")]),
    Item::new("Large Eraser", ids::DRAW_ERASER)
        .args(&[str_arg("mode", "point"), str_arg("size", "large")]),
];

const SHAPE_KINDS: &[Item] = &[
    Item::new("Line", ids::DRAW_SHAPE).args(&[str_arg("kind", "line")]),
    Item::new("Arrow", ids::DRAW_SHAPE).args(&[str_arg("kind", "arrow")]),
    Item::new("Double Arrow", ids::DRAW_SHAPE).args(&[str_arg("kind", "double_arrow")]),
    Item::new("Rectangle", ids::DRAW_SHAPE).args(&[str_arg("kind", "rectangle")]),
    Item::new("Ellipse", ids::DRAW_SHAPE).args(&[str_arg("kind", "ellipse")]),
    Item::new("Triangle", ids::DRAW_SHAPE).args(&[str_arg("kind", "triangle")]),
    Item::new("Diamond", ids::DRAW_SHAPE).args(&[str_arg("kind", "diamond")]),
    Item::new("Parallelogram", ids::DRAW_SHAPE).args(&[str_arg("kind", "parallelogram")]),
    Item::new("Graph Axes", ids::DRAW_SHAPE).args(&[str_arg("kind", "graph_axes")]),
];

const ADD_TOOLS: &[Item] = &[
    Item::new("Pen", ids::DRAW_TOOL_ADD).args(&[str_arg("kind", "pen")]),
    Item::new("Highlighter", ids::DRAW_TOOL_ADD).args(&[str_arg("kind", "highlighter")]),
    Item::new("Pencil", ids::DRAW_TOOL_ADD)
        .args(&[str_arg("kind", "pen"), str_arg("texture", "pencil")]),
    Item::new("Rainbow Pen", ids::DRAW_TOOL_ADD)
        .args(&[str_arg("kind", "pen"), str_arg("texture", "rainbow")]),
    Item::new("Galaxy Pen", ids::DRAW_TOOL_ADD)
        .args(&[str_arg("kind", "pen"), str_arg("texture", "galaxy")]),
    Item::new("Lava Pen", ids::DRAW_TOOL_ADD)
        .args(&[str_arg("kind", "pen"), str_arg("texture", "lava")]),
    Item::new("Ocean Pen", ids::DRAW_TOOL_ADD)
        .args(&[str_arg("kind", "pen"), str_arg("texture", "ocean")]),
    Item::new("Gold Pen", ids::DRAW_TOOL_ADD)
        .args(&[str_arg("kind", "pen"), str_arg("texture", "gold")]),
    Item::new("Silver Pen", ids::DRAW_TOOL_ADD)
        .args(&[str_arg("kind", "pen"), str_arg("texture", "silver")]),
    Item::new("Bronze Pen", ids::DRAW_TOOL_ADD)
        .args(&[str_arg("kind", "pen"), str_arg("texture", "bronze")]),
    Item::new("Rose Gold Pen", ids::DRAW_TOOL_ADD)
        .args(&[str_arg("kind", "pen"), str_arg("texture", "rose_gold")]),
];

/// The colors of the rule lines and grid: soft grays and hues that read as paper lines.
pub const RULE_LINE_COLORS: &[u32] = &[
    0xB8C4D0, 0x9FB6CD, 0x7BA7D6, 0x9CCBA2, 0xE3B778, 0xE59A9A, 0xB9A3D9, 0xA0A0A0, 0x6F6F6F,
    0x2F2F2F,
];

/// Format Background (Draw tab): the rule lines and grid cards, the rule line color, and the
/// page color.
const FORMAT_BACKGROUND: BackgroundMenu = BackgroundMenu {
    papers: RULE_LINES,
    rule_color: ColorMenu {
        split: false,
        arg: "color",
        colors: RULE_LINE_COLORS,
        columns: 5,
        palette: Palette::Grid,
        default: 0xB8C4D0,
        none: Some(("Automatic", "none")),
        none_last: false,
        custom: true,
    },
    page_color: ColorMenu {
        split: false,
        arg: "color",
        colors: PAGE_COLOR_GRID,
        columns: 5,
        palette: Palette::Grid,
        default: 0xDCEBF7,
        none: Some(("No Color", "none")),
        none_last: false,
        custom: true,
    },
};

const DRAW_GROUPS: &[Group] = &[
    Group {
        name: "Undo",
        columns: &[Column::Stack(&[
            Button::icon("Undo", Icon::Undo, ids::EDIT_UNDO),
            Button::icon("Redo", Icon::Redo, ids::EDIT_REDO),
        ])],
    },
    Group {
        name: "Selection",
        columns: &[Column::Stack(&[
            Button::small("Select", Icon::Select, ids::DRAW_TYPE),
            Button::small("Lasso Select", Icon::Lasso, ids::DRAW_LASSO),
        ])],
    },
    Group {
        name: "Drawing Tools",
        columns: &[
            Column::Large(Button::large("Eraser", Icon::Eraser, ids::DRAW_ERASER).split(ERASERS)),
            Column::Tools,
            Column::Large(
                Button::large("Add", Icon::AddTool, ids::DRAW_TOOL_ADD).dropdown(ADD_TOOLS),
            ),
        ],
    },
    Group {
        name: "Input Mode",
        columns: &[Column::Large(Button::large(
            "Panning Hand",
            Icon::PanHand,
            ids::DRAW_PAN,
        ))],
    },
    Group {
        name: "Stencils",
        columns: &[Column::Large(Button::large(
            "Ruler",
            Icon::Ruler,
            ids::DRAW_RULER,
        ))],
    },
    Group {
        name: "Edit",
        columns: &[
            Column::Large(Button::large(
                "Insert Space",
                Icon::InsertSpace,
                ids::INSERT_SPACE,
            )),
            Column::Large(
                Button::large(
                    "Format Background",
                    Icon::FormatBackground,
                    ids::VIEW_RULE_LINES,
                )
                .background(&FORMAT_BACKGROUND),
            ),
        ],
    },
    Group {
        name: "Shapes",
        columns: &[
            Column::Large(
                Button::large("Shapes", Icon::Shapes, ids::DRAW_SHAPE).dropdown(SHAPE_KINDS),
            ),
            Column::Large(Button::large(
                "Automatic Shapes",
                Icon::InkToShape,
                ids::DRAW_INK_TO_SHAPE,
            )),
        ],
    },
    Group {
        name: "Text",
        columns: &[Column::Large(Button::large(
            "Ink to Text",
            Icon::InkToText,
            ids::DRAW_INK_TO_TEXT,
        ))],
    },
    Group {
        name: "Math",
        columns: &[Column::Large(Button::large(
            "Ink to Math",
            Icon::InkToMath,
            ids::DRAW_INK_TO_MATH,
        ))],
    },
    Group {
        name: "Replay",
        columns: &[Column::Large(Button::large(
            "Ink Replay",
            Icon::InkReplay,
            ids::DRAW_INK_REPLAY,
        ))],
    },
    Group {
        name: "Mode",
        columns: &[Column::Large(Button::large(
            "Full Page View",
            Icon::FullPage,
            ids::VIEW_FULL_PAGE,
        ))],
    },
    Group {
        name: "Help",
        columns: &[Column::Large(Button::large(
            "Ink Help",
            Icon::Help,
            ids::HELP_HELP,
        ))],
    },
];

// History tab

const HISTORY_GROUPS: &[Group] = &[
    Group {
        name: "Unread",
        columns: &[
            Column::Large(Button::large(
                "Next Unread",
                Icon::NextUnread,
                ids::HISTORY_NEXT_UNREAD,
            )),
            Column::Stack(&[
                Button::small("Mark as Read", Icon::MarkRead, ids::HISTORY_MARK_READ),
                Button::small(
                    "Show Unread Changes",
                    Icon::ShowUnread,
                    ids::HISTORY_SHOW_UNREAD,
                ),
            ]),
        ],
    },
    Group {
        name: "Authors",
        columns: &[
            Column::Large(Button::large(
                "Find by Author",
                Icon::FindByAuthor,
                ids::HISTORY_FIND_BY_AUTHOR,
            )),
            Column::Large(Button::large(
                "Hide Authors",
                Icon::HideAuthors,
                ids::HISTORY_HIDE_AUTHORS,
            )),
        ],
    },
    Group {
        name: "History",
        columns: &[
            Column::Large(Button::large(
                "Page Versions",
                Icon::PageVersions,
                ids::HISTORY_PAGE_VERSIONS,
            )),
            Column::Large(Button::large(
                "Notebook Recycle Bin",
                Icon::RecycleBin,
                ids::HISTORY_RECYCLE_BIN,
            )),
            Column::Large(Button::large(
                "Recent Edits",
                Icon::RecentEdits,
                ids::HISTORY_RECENT_EDITS,
            )),
        ],
    },
];

// Review tab

const REVIEW_GROUPS: &[Group] = &[
    Group {
        name: "Spelling",
        columns: &[
            Column::Large(Button::large(
                "Spelling",
                Icon::Spelling,
                ids::REVIEW_SPELLING,
            )),
            Column::Stack(&[Button::small(
                "Thesaurus",
                Icon::Thesaurus,
                ids::REVIEW_THESAURUS,
            )]),
        ],
    },
    Group {
        name: "Language",
        columns: &[Column::Large(Button::large(
            "Language",
            Icon::Language,
            ids::REVIEW_LANGUAGE,
        ))],
    },
    Group {
        name: "Section",
        columns: &[
            Column::Large(Button::large(
                "Password",
                Icon::Password,
                ids::REVIEW_PASSWORD,
            )),
            Column::Stack(&[Button::small(
                "Lock All",
                Icon::LockAll,
                ids::REVIEW_LOCK_ALL,
            )]),
        ],
    },
    Group {
        name: "Notes",
        columns: &[Column::Large(Button::large(
            "Linked Notes",
            Icon::LinkedNotes,
            ids::REVIEW_LINKED_NOTES,
        ))],
    },
];

// View tab

pub(crate) const RULE_LINES: &[Item] = &[
    Item::new("None", ids::VIEW_RULE_LINES).args(&[str_arg("style", "none")]),
    Item::new("Narrow Ruled", ids::VIEW_RULE_LINES).args(&[str_arg("style", "narrow")]),
    Item::new("College Ruled", ids::VIEW_RULE_LINES).args(&[str_arg("style", "college")]),
    Item::new("Standard Ruled", ids::VIEW_RULE_LINES).args(&[str_arg("style", "standard")]),
    Item::new("Wide Ruled", ids::VIEW_RULE_LINES).args(&[str_arg("style", "wide")]),
    Item::new("Small Grid", ids::VIEW_RULE_LINES).args(&[str_arg("style", "grid_small")]),
    Item::new("Medium Grid", ids::VIEW_RULE_LINES).args(&[str_arg("style", "grid_medium")]),
    Item::new("Large Grid", ids::VIEW_RULE_LINES).args(&[str_arg("style", "grid_large")]),
    Item::new("Very Large Grid", ids::VIEW_RULE_LINES).args(&[str_arg("style", "grid_xlarge")]),
    Item::new("Dotted", ids::VIEW_RULE_LINES).args(&[str_arg("style", "dotted")]),
];

pub(crate) const PAPER_SIZES: &[Item] = &[
    Item::new("Auto", ids::VIEW_PAPER_SIZE).args(&[str_arg("size", "auto")]),
    Item::new("Letter", ids::VIEW_PAPER_SIZE).args(&[str_arg("size", "letter")]),
    Item::new("Legal", ids::VIEW_PAPER_SIZE).args(&[str_arg("size", "legal")]),
    Item::new("A4", ids::VIEW_PAPER_SIZE).args(&[str_arg("size", "a4")]),
    Item::new("A5", ids::VIEW_PAPER_SIZE).args(&[str_arg("size", "a5")]),
];

pub(crate) const ZOOM_LEVELS: &[Item] = &[
    Item::new("50%", ids::VIEW_ZOOM).args(&[int_arg("percent", 50)]),
    Item::new("75%", ids::VIEW_ZOOM).args(&[int_arg("percent", 75)]),
    Item::new("100%", ids::VIEW_ZOOM).args(&[int_arg("percent", 100)]),
    Item::new("125%", ids::VIEW_ZOOM).args(&[int_arg("percent", 125)]),
    Item::new("150%", ids::VIEW_ZOOM).args(&[int_arg("percent", 150)]),
    Item::new("200%", ids::VIEW_ZOOM).args(&[int_arg("percent", 200)]),
];

const THEMES: &[Item] = &[
    Item::new("OneNote 2016", ids::VIEW_THEME_SET).args(&[str_arg("preset", "onenote2016")]),
    Item::new("OneNote 2019", ids::VIEW_THEME_SET).args(&[str_arg("preset", "onenote2019")]),
    Item::new("OneNote 2021", ids::VIEW_THEME_SET).args(&[str_arg("preset", "onenote2021")]),
    Item::new("Microsoft 365", ids::VIEW_THEME_SET).args(&[str_arg("preset", "m365")]),
];

const MODES: &[Item] = &[
    Item::new("Light", ids::VIEW_MODE_SET).args(&[str_arg("mode", "light")]),
    Item::new("Dark", ids::VIEW_MODE_SET).args(&[str_arg("mode", "dark")]),
    Item::new("Use System Setting", ids::VIEW_MODE_SET).args(&[str_arg("mode", "system")]),
];

const VIEW_GROUPS: &[Group] = &[
    Group {
        name: "Views",
        columns: &[
            Column::Large(Button::large(
                "Normal View",
                Icon::NormalView,
                ids::VIEW_NORMAL,
            )),
            Column::Large(Button::large(
                "Full Page View",
                Icon::FullPage,
                ids::VIEW_FULL_PAGE,
            )),
            Column::Large(Button::large("Dock to Desktop", Icon::Dock, ids::VIEW_DOCK)),
            Column::Large(Button::large(
                "Navigation Panes",
                Icon::Navigation,
                ids::VIEW_NAVIGATION_TOGGLE,
            )),
        ],
    },
    Group {
        name: "Page Setup",
        columns: &[
            Column::Large(
                Button::large("Page Color", Icon::PageColor, ids::VIEW_PAGE_COLOR).colors(
                    ColorMenu {
                        split: false,
                        arg: "color",
                        colors: PAGE_COLOR_GRID,
                        columns: 5,
                        palette: Palette::Grid,
                        default: 0xDCEBF7,
                        none: Some(("No Color", "none")),
                        none_last: false,
                        custom: true,
                    },
                ),
            ),
            Column::Large(
                Button::large("Rule Lines", Icon::RuleLines, ids::VIEW_RULE_LINES)
                    .papers(RULE_LINES),
            ),
            Column::Stack(&[
                Button::small("Hide Page Title", Icon::HideTitle, ids::VIEW_HIDE_TITLE),
                Button::small("Paper Size", Icon::PaperSize, ids::VIEW_PAPER_SIZE)
                    .dropdown(PAPER_SIZES),
            ]),
        ],
    },
    Group {
        name: "Zoom",
        columns: &[
            Column::Stack(&[
                Button::small("Zoom Out", Icon::ZoomOut, ids::VIEW_ZOOM_OUT),
                Button::small("Zoom In", Icon::ZoomIn, ids::VIEW_ZOOM_IN),
            ]),
            Column::Large(Button::large("Zoom", Icon::Zoom, ids::VIEW_ZOOM).dropdown(ZOOM_LEVELS)),
            Column::Stack(&[
                Button::small("100%", Icon::ZoomActual, ids::VIEW_ZOOM_ACTUAL),
                Button::small("Page Width", Icon::PageWidth, ids::VIEW_ZOOM_PAGE_WIDTH),
            ]),
        ],
    },
    Group {
        name: "Window",
        columns: &[
            Column::Large(Button::large(
                "New Window",
                Icon::NewWindow,
                ids::VIEW_NEW_WINDOW,
            )),
            Column::Large(Button::large(
                "New Docked Window",
                Icon::NewDockedWindow,
                ids::VIEW_NEW_DOCKED_WINDOW,
            )),
        ],
    },
    Group {
        name: "Theme",
        columns: &[
            Column::Large(
                Button::large("Theme", Icon::Theme, ids::VIEW_THEME_SET).dropdown(THEMES),
            ),
            Column::Large(
                Button::large("Dark Mode", Icon::DarkMode, ids::VIEW_MODE_TOGGLE).split(MODES),
            ),
        ],
    },
];

// Help tab

const HELP_GROUPS: &[Group] = &[Group {
    name: "Help",
    columns: &[
        Column::Large(Button::large("Help", Icon::Help, ids::HELP_HELP)),
        Column::Large(Button::large("Support", Icon::Support, ids::HELP_SUPPORT)),
        Column::Large(Button::large(
            "Feedback",
            Icon::Feedback,
            ids::HELP_FEEDBACK,
        )),
        Column::Large(Button::large(
            "Show Training",
            Icon::Training,
            ids::HELP_TRAINING,
        )),
    ],
}];

/// Every ribbon tab, in OneNote order.
pub static TABS: &[Tab] = &[
    Tab {
        name: "File",
        key: "F",
        kind: TabKind::Backstage(BACKSTAGE),
    },
    Tab {
        name: "Home",
        key: "H",
        kind: TabKind::Groups(&[CLIPBOARD, BASIC_TEXT, STYLES, TAGS, EMAIL, MEETINGS]),
    },
    Tab {
        name: "Insert",
        key: "N",
        kind: TabKind::Groups(INSERT_GROUPS),
    },
    Tab {
        name: "Draw",
        key: "D",
        kind: TabKind::Groups(DRAW_GROUPS),
    },
    Tab {
        name: "History",
        key: "S",
        kind: TabKind::Groups(HISTORY_GROUPS),
    },
    Tab {
        name: "Review",
        key: "R",
        kind: TabKind::Groups(REVIEW_GROUPS),
    },
    Tab {
        name: "View",
        key: "W",
        kind: TabKind::Groups(VIEW_GROUPS),
    },
    Tab {
        name: "Help",
        key: "Y",
        kind: TabKind::Groups(HELP_GROUPS),
    },
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use notecraft_engine::cmd::{ArgKind, CommandSpec, Registry};

    use super::*;

    /// What a click sends: the commands of buttons that run their own command, and of
    /// every menu item and backstage entry. A dropdown button only opens its menu.
    fn dispatched() -> Vec<(&'static str, CommandId, Vec<Arg>)> {
        let mut out = Vec::new();
        for tab in TABS {
            match tab.kind {
                TabKind::Backstage(items) => {
                    out.extend(items.iter().map(|i| (i.label, i.command, i.args.to_vec())));
                }
                TabKind::Groups(groups) => {
                    for group in groups {
                        for button in group.buttons() {
                            if button.menu.is_none() || button.menu.is_split() {
                                out.push((button.label, button.command, button.args.to_vec()));
                            }
                            let items = button.menu.items();
                            out.extend(items.iter().map(|i| (i.label, i.command, i.args.to_vec())));
                            match button.menu {
                                Menu::Colors(colors) => {
                                    let standard = match colors.palette {
                                        Palette::Theme => STANDARD_COLORS,
                                        Palette::Grid => &[],
                                    };
                                    for &rgb in colors.colors.iter().chain(standard) {
                                        let value: &'static str =
                                            Box::leak(format!("#{rgb:06X}").into_boxed_str());
                                        out.push((
                                            button.label,
                                            button.command,
                                            vec![str_arg(colors.arg, value)],
                                        ));
                                    }
                                    if let Some((_, none)) = colors.none {
                                        out.push((
                                            button.label,
                                            button.command,
                                            vec![str_arg(colors.arg, none)],
                                        ));
                                    }
                                }
                                Menu::TableGrid => {
                                    out.push((
                                        button.label,
                                        button.command,
                                        vec![int_arg("rows", 3), int_arg("columns", 4)],
                                    ));
                                    out.push((button.label, button.command, Vec::new()));
                                }
                                Menu::Background(menu) => {
                                    for (colors, command) in [
                                        (menu.rule_color, ids::VIEW_RULE_LINE_COLOR),
                                        (menu.page_color, ids::VIEW_PAGE_COLOR),
                                    ] {
                                        for &rgb in colors.colors {
                                            let value: &'static str =
                                                Box::leak(format!("#{rgb:06X}").into_boxed_str());
                                            out.push((
                                                button.label,
                                                command,
                                                vec![str_arg(colors.arg, value)],
                                            ));
                                        }
                                    }
                                }
                                Menu::None
                                | Menu::Split(_)
                                | Menu::Dropdown(_)
                                | Menu::Papers(_)
                                | Menu::Combo(_) => {}
                            }
                        }
                    }
                }
            }
        }
        out
    }

    fn accepts(kind: ArgKind, value: ArgValue) -> bool {
        matches!(
            (kind, value),
            (ArgKind::String, ArgValue::Str(_))
                | (ArgKind::Integer, ArgValue::Int(_))
                | (ArgKind::Number, ArgValue::Int(_) | ArgValue::Num(_))
        )
    }

    fn check_args(label: &str, spec: &CommandSpec, args: &[Arg]) {
        for arg in args {
            let declared = spec.args_schema.iter().find(|a| a.name == arg.name);
            let declared = declared
                .unwrap_or_else(|| panic!("{label}: {} has no argument {}", spec.id, arg.name));
            assert!(
                accepts(declared.kind, arg.value),
                "{label}: {} type",
                arg.name
            );
        }
        for required in spec.args_schema.iter().filter(|a| a.required) {
            assert!(
                args.iter().any(|a| a.name == required.name),
                "{label}: {} needs {}",
                spec.id,
                required.name
            );
        }
    }

    #[test]
    fn every_ribbon_button_maps_to_a_registered_command() {
        let registry = Registry::builtin();
        for (label, command, _) in all_commands() {
            assert!(!label.trim().is_empty(), "{command} has an empty label");
            assert!(
                registry.get(command.as_str()).is_some(),
                "{label}: {command} is not registered"
            );
        }
    }

    #[test]
    fn every_click_sends_arguments_the_command_accepts() {
        let registry = Registry::builtin();
        for (label, command, args) in dispatched() {
            let spec = registry.get(command.as_str()).unwrap();
            check_args(label, spec, &args);
        }
    }

    #[test]
    fn tabs_follow_onenote_and_labels_are_unique_per_group() {
        let names: Vec<&str> = TABS.iter().map(|tab| tab.name).collect();
        let onenote = [
            "File", "Home", "Insert", "Draw", "History", "Review", "View", "Help",
        ];
        assert!(names.iter().all(|name| onenote.contains(name)));
        assert_eq!(names.first(), Some(&"File"));
        for tab in TABS {
            if let TabKind::Groups(groups) = tab.kind {
                for group in groups {
                    let mut seen = BTreeSet::new();
                    for button in group.buttons() {
                        assert!(
                            seen.insert(button.label),
                            "{} twice in {}",
                            button.label,
                            group.name
                        );
                    }
                }
            }
        }
    }

    /// Every gallery column of every tab.
    fn galleries() -> Vec<&'static Gallery> {
        let mut out = Vec::new();
        for tab in TABS {
            if let TabKind::Groups(groups) = tab.kind {
                for group in groups {
                    for column in group.columns {
                        if let Column::Gallery(gallery) = column {
                            out.push(gallery);
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn galleries_are_backed_by_a_dropdown_whose_items_are_all_dispatched() {
        let found = galleries();
        assert_eq!(found.len(), 2);
        let dispatched = dispatched();
        for gallery in found {
            assert!(matches!(gallery.button.menu, Menu::Dropdown(_)));
            assert!(gallery.visible >= 1);
            assert!(gallery.entries().len() >= gallery.visible);
            assert!(gallery.entries().len() <= gallery.items().len());
            for item in gallery.items() {
                assert!(
                    dispatched
                        .iter()
                        .any(|(label, command, _)| *label == item.label
                            && *command == item.command),
                    "{} is not covered",
                    item.label
                );
            }
            assert!(gallery
                .button
                .menu
                .items()
                .iter()
                .all(|i| !i.label.is_empty()));
        }
    }

    #[test]
    fn color_grids_are_full_and_distinct() {
        assert_eq!(COLOR_GRID.len(), 50);
        for grid in [
            COLOR_GRID,
            HIGHLIGHT_GRID,
            PAGE_COLOR_GRID,
            RULE_LINE_COLORS,
        ] {
            let unique: BTreeSet<u32> = grid.iter().copied().collect();
            assert_eq!(unique.len(), grid.len());
            assert!(grid.iter().all(|&rgb| rgb <= 0xFF_FF_FF));
        }
    }

    #[test]
    fn args_become_a_json_object() {
        assert_eq!(args_value(&[]), Value::Null);
        let value = args_value(&[str_arg("style", "heading1"), num_arg("points", 11.0)]);
        assert_eq!(
            value,
            serde_json::json!({ "style": "heading1", "points": 11.0 })
        );
    }
}
