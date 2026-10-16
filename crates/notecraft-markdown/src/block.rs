//! The block model: what a markdown page body is made of, with the exact source byte span of
//! every block so edits can later be applied as span replacements (PR 1.4).

/// A half-open byte range `start..end` into the page source text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// The text this span covers in `source`, or an empty string if the span does not fit
    /// (wrong source, or not on character boundaries).
    pub fn slice<'a>(&self, source: &'a str) -> &'a str {
        source.get(self.start..self.end).unwrap_or("")
    }
}

/// A piece of inline text with one style. A line break inside a paragraph is a run whose text
/// is `" "` (soft break) or `"\n"` (hard break).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    /// The link target, for text inside a link (or an image, whose alt text is shown).
    pub link: Option<String>,
    /// The run is an image (`![alt](target)`): `text` is the alt text and `link` the target.
    pub image: bool,
}

impl Run {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// True if the run has no styling at all.
    pub fn is_plain(&self) -> bool {
        !(self.bold || self.italic || self.strike || self.code || self.image) && self.link.is_none()
    }
}

/// Column alignment in a table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    None,
    Left,
    Center,
    Right,
}

/// What kind of list this is. A list is a task list when any of its items has a checkbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    /// Numbered; carries the first number.
    Ordered(u32),
    Task,
}

/// One item of a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub span: Span,
    /// `Some(checked)` for a task item (`- [ ]` or `- [x]`).
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// One table cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub span: Span,
    pub runs: Vec<Run>,
}

/// A GFM table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub align: Vec<Align>,
    pub header: Vec<Cell>,
    pub rows: Vec<Vec<Cell>>,
}

/// The kind of a block and its content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    /// ATX or setext heading; `level` is 1 to 6.
    Heading {
        level: u8,
        runs: Vec<Run>,
    },
    Paragraph {
        runs: Vec<Run>,
    },
    List {
        kind: ListKind,
        tight: bool,
        items: Vec<ListItem>,
    },
    Table(Table),
    /// Fenced or indented code. `literal` is the code without fences.
    Code {
        info: String,
        fenced: bool,
        literal: String,
    },
    Quote {
        blocks: Vec<Block>,
    },
    /// A thematic break (`---`, `***`, `___`).
    Rule,
    /// Raw HTML, kept verbatim and never executed.
    Html {
        literal: String,
    },
}

/// A block and the exact bytes it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub span: Span,
    pub kind: BlockKind,
}

impl Block {
    /// The source text of this block.
    pub fn source<'a>(&self, text: &'a str) -> &'a str {
        self.span.slice(text)
    }
}
