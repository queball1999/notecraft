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
}

impl Item {
    pub const fn new(label: &'static str, command: CommandId) -> Self {
        Self {
            label,
            command,
            args: &[],
        }
    }

    pub const fn args(mut self, args: &'static [Arg]) -> Self {
        self.args = args;
        self
    }
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
}

impl Menu {
    pub fn items(&self) -> &'static [Item] {
        match self {
            Self::None => &[],
            Self::Split(items) | Self::Dropdown(items) => items,
        }
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
}

/// One column of a group.
#[derive(Clone, Copy, Debug)]
pub enum Column {
    /// One large button.
    Large(Button),
    /// Up to three small buttons stacked.
    Stack(&'static [Button]),
    /// Up to three rows of small or icon-only buttons.
    Rows(&'static [&'static [Button]]),
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
    pub kind: TabKind,
}

impl Group {
    /// Every button in the group, in drawing order.
    pub fn buttons(&self) -> impl Iterator<Item = &'static Button> + '_ {
        self.columns
            .iter()
            .flat_map(|column| -> Vec<&'static Button> {
                match column {
                    Column::Large(button) => vec![button],
                    Column::Stack(buttons) => buttons.iter().collect(),
                    Column::Rows(rows) => rows.iter().flat_map(|row| row.iter()).collect(),
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

const HIGHLIGHT_COLORS: &[Item] = &[
    Item::new("Yellow", ids::FORMAT_HIGHLIGHT).args(&[str_arg("color", "#FFF176")]),
    Item::new("Green", ids::FORMAT_HIGHLIGHT).args(&[str_arg("color", "#A5D6A7")]),
    Item::new("Turquoise", ids::FORMAT_HIGHLIGHT).args(&[str_arg("color", "#80DEEA")]),
    Item::new("Pink", ids::FORMAT_HIGHLIGHT).args(&[str_arg("color", "#F48FB1")]),
    Item::new("No Color", ids::FORMAT_HIGHLIGHT).args(&[str_arg("color", "none")]),
];

const FONT_COLORS: &[Item] = &[
    Item::new("Automatic", ids::FORMAT_FONT_COLOR).args(&[str_arg("color", "none")]),
    Item::new("Red", ids::FORMAT_FONT_COLOR).args(&[str_arg("color", "#C62828")]),
    Item::new("Orange", ids::FORMAT_FONT_COLOR).args(&[str_arg("color", "#EF6C00")]),
    Item::new("Green", ids::FORMAT_FONT_COLOR).args(&[str_arg("color", "#2E7D32")]),
    Item::new("Blue", ids::FORMAT_FONT_COLOR).args(&[str_arg("color", "#1565C0")]),
    Item::new("Purple", ids::FORMAT_FONT_COLOR).args(&[str_arg("color", "#6A1B9A")]),
];

const BASIC_TEXT: Group = Group {
    name: "Basic Text",
    columns: &[Column::Rows(&[
        &[
            Button::small("Font", Icon::Font, ids::FORMAT_FONT_FAMILY).dropdown(FONTS),
            Button::small("Size", Icon::FontSize, ids::FORMAT_FONT_SIZE).dropdown(FONT_SIZES),
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
            .split(HIGHLIGHT_COLORS),
            Button::icon("Font Color", Icon::FontColor, ids::FORMAT_FONT_COLOR).split(FONT_COLORS),
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
    columns: &[
        Column::Stack(&[
            Button::small("Heading 1", Icon::Style, ids::STYLE_APPLY)
                .args(&[str_arg("style", "heading1")]),
            Button::small("Heading 2", Icon::Style, ids::STYLE_APPLY)
                .args(&[str_arg("style", "heading2")]),
            Button::small("Normal", Icon::Style, ids::STYLE_APPLY)
                .args(&[str_arg("style", "normal")]),
        ]),
        Column::Large(Button::large("Styles", Icon::Style, ids::STYLE_APPLY).dropdown(ALL_STYLES)),
    ],
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
        Column::Stack(&[
            Button::small("To Do", Icon::TodoTag, ids::TAG_APPLY).args(&[str_arg("tag", "todo")]),
            Button::small("Important", Icon::Important, ids::TAG_APPLY)
                .args(&[str_arg("tag", "important")]),
            Button::small("Question", Icon::Question, ids::TAG_APPLY)
                .args(&[str_arg("tag", "question")]),
        ]),
        Column::Large(Button::large("Tags", Icon::Tag, ids::TAG_APPLY).dropdown(MORE_TAGS)),
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

/// Every ribbon tab, in OneNote order. Insert, Draw, History, Review, and View follow in
/// PR 0.5b.
pub static TABS: &[Tab] = &[
    Tab {
        name: "File",
        kind: TabKind::Backstage(BACKSTAGE),
    },
    Tab {
        name: "Home",
        kind: TabKind::Groups(&[CLIPBOARD, BASIC_TEXT, STYLES, TAGS, EMAIL, MEETINGS]),
    },
];
