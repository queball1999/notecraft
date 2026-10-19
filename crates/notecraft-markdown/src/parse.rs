//! Markdown to blocks. Wraps `comrak` (GFM tables, task lists, strikethrough, autolinks) so
//! the parser can be swapped without changing the block model.

use comrak::nodes::{AstNode, ListType, NodeValue, Sourcepos, TableAlignment};
use comrak::{parse_document, Arena, Options};

use crate::block::{Align, Block, BlockKind, Cell, ListItem, ListKind, Run, Script, Span, Table};
use crate::markup::{self, Mark, Tag};

/// Deepest block nesting parsed into structure (docs/markdown.md section 9). Deeper content
/// is shown as plain text.
pub const MAX_DEPTH: usize = 64;

/// Largest text parsed into structure, 64 MiB (docs/markdown.md section 9). Larger text is
/// one plain paragraph.
pub const MAX_PARSE_BYTES: usize = 64 * 1024 * 1024;

fn options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.autolink = true;
    options.extension.highlight = true;
    options
}

/// Byte offset of each line start, to turn comrak's (line, column) positions into offsets.
struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
    /// Added to every offset, when `text` is a tail of a larger source.
    base: usize,
}

impl<'a> Lines<'a> {
    fn new(text: &'a str, base: usize) -> Self {
        let mut starts = vec![0];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self { text, starts, base }
    }

    fn start_of(&self, line: usize) -> usize {
        let index = line.saturating_sub(1);
        self.starts.get(index).copied().unwrap_or(self.text.len())
    }

    /// The page offset of the start of the line that holds page offset `offset`.
    fn line_start_of(&self, offset: usize) -> usize {
        let local = offset.saturating_sub(self.base).min(self.text.len());
        let before = self.text.get(..local).unwrap_or("");
        let start = before.rfind('\n').map_or(0, |i| i + 1);
        start + self.base
    }

    /// Move `offset` forward to a character boundary and clamp it to the text.
    fn snap(&self, offset: usize) -> usize {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset += 1;
        }
        offset
    }

    fn span(&self, pos: Sourcepos) -> Span {
        let start = self.snap(self.start_of(pos.start.line) + pos.start.column.saturating_sub(1));
        // The end position is the last byte, inclusive. Column 0 means "the line before ended
        // the node" (comrak does this for the last item of a list), so step back over the
        // line ending.
        let mut end = self.snap(self.start_of(pos.end.line) + pos.end.column);
        if pos.end.column == 0 {
            let bytes = self.text.as_bytes();
            while end > start && matches!(bytes.get(end - 1), Some(b'\n' | b'\r')) {
                end -= 1;
            }
        }
        let end = end.max(start);
        Span::new(start + self.base, end + self.base)
    }

    /// The raw text of a span made by `span`.
    fn raw(&self, span: Span) -> &'a str {
        self.text
            .get(span.start.saturating_sub(self.base)..span.end.saturating_sub(self.base))
            .unwrap_or("")
    }
}

/// Parse `text` into blocks. `base` is the offset of `text` within the whole page source, so
/// the spans are page offsets (the frontmatter is cut off before the body is parsed).
pub fn parse_blocks(text: &str, base: usize) -> Vec<Block> {
    if text.len() > MAX_PARSE_BYTES {
        return vec![Block {
            span: Span::new(base, base + text.len()),
            kind: BlockKind::Paragraph {
                runs: vec![Run::plain(text)],
            },
        }];
    }
    let lines = Lines::new(text, base);
    let arena = Arena::new();
    let root = parse_document(&arena, text, &options());
    blocks_of(root, &lines, 0)
}

fn blocks_of<'a>(parent: &'a AstNode<'a>, lines: &Lines<'_>, depth: usize) -> Vec<Block> {
    parent
        .children()
        .filter_map(|node| block_of(node, lines, depth))
        .collect()
}

fn block_of<'a>(node: &'a AstNode<'a>, lines: &Lines<'_>, depth: usize) -> Option<Block> {
    let (mut span, value) = {
        let data = node.data.borrow();
        (lines.span(data.sourcepos), data.value.clone())
    };
    // comrak starts an indented code block after its indent; the block owns the whole line.
    if matches!(&value, NodeValue::CodeBlock(code) if !code.fenced) {
        span.start = lines.line_start_of(span.start);
    }
    if depth >= MAX_DEPTH {
        return Some(Block {
            span,
            kind: BlockKind::Paragraph {
                runs: vec![Run::plain(lines.raw(span))],
            },
        });
    }
    let kind = match value {
        NodeValue::Heading(h) => BlockKind::Heading {
            level: h.level.clamp(1, 6),
            runs: runs_of(node),
        },
        NodeValue::Paragraph => BlockKind::Paragraph {
            runs: runs_of(node),
        },
        NodeValue::BlockQuote => BlockKind::Quote {
            blocks: blocks_of(node, lines, depth + 1),
        },
        NodeValue::ThematicBreak => BlockKind::Rule,
        NodeValue::CodeBlock(code) => BlockKind::Code {
            info: code.info,
            fenced: code.fenced,
            literal: code.literal,
        },
        NodeValue::HtmlBlock(html) => BlockKind::Html {
            literal: html.literal,
        },
        NodeValue::List(list) => list_of(node, &list, lines, depth),
        NodeValue::Table(table) => table_of(node, &table.alignments, lines),
        NodeValue::Item(_) | NodeValue::TaskItem(_) | NodeValue::TableRow(_) => return None,
        // Anything else (not enabled today) is kept as its source text.
        _ => BlockKind::Paragraph {
            runs: vec![Run::plain(lines.raw(span))],
        },
    };
    Some(Block { span, kind })
}

fn list_of<'a>(
    node: &'a AstNode<'a>,
    list: &comrak::nodes::NodeList,
    lines: &Lines<'_>,
    depth: usize,
) -> BlockKind {
    let mut items = Vec::new();
    for item in node.children() {
        let (span, task) = {
            let data = item.data.borrow();
            let task = match &data.value {
                NodeValue::TaskItem(t) => Some(t.symbol.is_some()),
                _ => None,
            };
            (lines.span(data.sourcepos), task)
        };
        items.push(ListItem {
            span,
            task,
            blocks: blocks_of(item, lines, depth + 1),
        });
    }
    let kind = if items.iter().any(|i| i.task.is_some()) {
        ListKind::Task
    } else if list.list_type == ListType::Ordered {
        ListKind::Ordered(u32::try_from(list.start).unwrap_or(u32::MAX))
    } else {
        ListKind::Bullet
    };
    BlockKind::List {
        kind,
        tight: list.tight,
        items,
    }
}

fn table_of<'a>(
    node: &'a AstNode<'a>,
    alignments: &[TableAlignment],
    lines: &Lines<'_>,
) -> BlockKind {
    let mut header = Vec::new();
    let mut rows = Vec::new();
    for row in node.children() {
        let is_header = matches!(row.data.borrow().value, NodeValue::TableRow(true));
        let cells: Vec<Cell> = row
            .children()
            .map(|cell| Cell {
                span: lines.span(cell.data.borrow().sourcepos),
                runs: runs_of(cell),
            })
            .collect();
        if is_header {
            header = cells;
        } else {
            rows.push(cells);
        }
    }
    let align = alignments
        .iter()
        .map(|a| match a {
            TableAlignment::None => Align::None,
            TableAlignment::Left => Align::Left,
            TableAlignment::Center => Align::Center,
            TableAlignment::Right => Align::Right,
        })
        .collect();
    BlockKind::Table(Table {
        align,
        header,
        rows,
    })
}

/// The style being applied while walking inline nodes.
#[derive(Clone, Default)]
struct Style {
    bold: bool,
    italic: bool,
    strike: bool,
    underline: bool,
    script: Script,
    highlight: Option<u32>,
    color: Option<u32>,
    size_pt: Option<u16>,
    family: Option<String>,
    link: Option<String>,
}

fn runs_of<'a>(node: &'a AstNode<'a>) -> Vec<Run> {
    let mut runs = Vec::new();
    inline_children(node, &Style::default(), 0, &mut runs);
    merge(runs)
}

fn push(runs: &mut Vec<Run>, text: &str, style: &Style, code: bool) {
    if text.is_empty() {
        return;
    }
    runs.push(Run {
        text: text.to_owned(),
        bold: style.bold,
        italic: style.italic,
        strike: style.strike,
        code,
        underline: style.underline,
        script: style.script,
        highlight: style.highlight,
        color: style.color,
        size_pt: style.size_pt,
        family: style.family.clone(),
        link: style.link.clone(),
        image: false,
        image_width: None,
    });
}

/// The alt text of an image node: the text and code under it, up to a small cap.
fn alt_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut alt = String::new();
    for d in node.descendants().skip(1) {
        let value = &d.data.borrow().value;
        let piece: &str = match value {
            NodeValue::Text(t) => t,
            NodeValue::Code(c) => &c.literal,
            NodeValue::SoftBreak | NodeValue::LineBreak => " ",
            _ => continue,
        };
        if alt.len() >= MAX_ALT_BYTES {
            break;
        }
        alt.push_str(piece);
    }
    alt
}

/// If `h` is a width comment and the run before it is an image, record the width on the image
/// and return true (the comment is then not shown as text).
fn image_width_for(last: Option<&mut Run>, h: &str) -> bool {
    match (last, crate::image::parse_width_comment(h)) {
        (Some(run), Some(width)) if run.image && run.image_width.is_none() => {
            run.image_width = Some(width);
            true
        }
        _ => false,
    }
}

/// Alt text kept per image.
const MAX_ALT_BYTES: usize = 1024;

fn inline_children<'a>(node: &'a AstNode<'a>, outer: &Style, depth: usize, runs: &mut Vec<Run>) {
    // HTML tags (`<u>`, `<mark>`, `<span style=...>`) are sibling nodes, so the style they set
    // lasts for the rest of this parent.
    let mut current = outer.clone();
    let mut open_spans: Vec<Mark> = Vec::new();
    for child in node.children() {
        let style = &current;
        if depth >= MAX_DEPTH {
            // Too deep to keep the structure: keep only the text.
            for d in child.descendants() {
                if let NodeValue::Text(t) = &d.data.borrow().value {
                    push(runs, t, style, false);
                }
            }
            continue;
        }
        let value = child.data.borrow().value.clone();
        if let NodeValue::HtmlInline(h) = &value {
            if image_width_for(runs.last_mut(), h) {
                continue;
            }
        }
        let mut nested = |style: Style| inline_children(child, &style, depth + 1, runs);
        match value {
            NodeValue::Text(t) => push(runs, &t, style, false),
            NodeValue::Code(c) => push(runs, &c.literal, style, true),
            NodeValue::HtmlInline(h) => match markup::parse_tag(&h) {
                Some((tag, len)) if len == h.len() => apply_tag(&mut current, &mut open_spans, tag),
                _ => push(runs, &h, style, false),
            },
            NodeValue::Highlight => nested(Style {
                highlight: Some(markup::DEFAULT_HIGHLIGHT),
                ..style.clone()
            }),
            NodeValue::SoftBreak => push(runs, " ", style, false),
            NodeValue::LineBreak => push(runs, "\n", style, false),
            NodeValue::Emph => nested(Style {
                italic: true,
                ..style.clone()
            }),
            NodeValue::Strong => nested(Style {
                bold: true,
                ..style.clone()
            }),
            NodeValue::Strikethrough => nested(Style {
                strike: true,
                ..style.clone()
            }),
            NodeValue::Link(link) => nested(Style {
                link: Some(link.url),
                ..style.clone()
            }),
            NodeValue::Image(link) => runs.push(Run {
                text: alt_text(child),
                bold: style.bold,
                italic: style.italic,
                strike: style.strike,
                code: false,
                underline: style.underline,
                script: style.script,
                highlight: style.highlight,
                color: style.color,
                size_pt: style.size_pt,
                family: style.family.clone(),
                link: Some(link.url),
                image: true,
                image_width: None,
            }),
            _ => nested(style.clone()),
        }
    }
}

/// Join neighboring runs that have the same style.
fn merge(runs: Vec<Run>) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs {
        match out.last_mut() {
            Some(last)
                if !last.image
                    && !run.image
                    && last.bold == run.bold
                    && last.italic == run.italic
                    && last.strike == run.strike
                    && last.code == run.code
                    && last.underline == run.underline
                    && last.script == run.script
                    && last.highlight == run.highlight
                    && last.color == run.color
                    && last.size_pt == run.size_pt
                    && last.family == run.family
                    && last.link == run.link =>
            {
                last.text.push_str(&run.text);
            }
            _ => out.push(run),
        }
    }
    out
}

/// Apply a formatting tag to the style that holds for the following siblings.
fn apply_tag(style: &mut Style, open_spans: &mut Vec<Mark>, tag: Tag) {
    match tag {
        Tag::Open(mark, value) => {
            set_mark(style, mark, value.as_deref(), true);
            if matches!(mark, Mark::Color | Mark::Size | Mark::Family) {
                // Bounded: a hostile page cannot grow the stack past its own tag count, but
                // keep it small anyway.
                if open_spans.len() < 64 {
                    open_spans.push(mark);
                }
            }
        }
        Tag::Close(mark) => set_mark(style, mark, None, false),
        Tag::CloseSpan => {
            if let Some(mark) = open_spans.pop() {
                set_mark(style, mark, None, false);
            }
        }
    }
}

fn set_mark(style: &mut Style, mark: Mark, value: Option<&str>, on: bool) {
    match mark {
        Mark::Underline => style.underline = on,
        Mark::Subscript => {
            style.script = if on {
                Script::Sub
            } else if style.script == Script::Sub {
                Script::None
            } else {
                style.script
            }
        }
        Mark::Superscript => {
            style.script = if on {
                Script::Sup
            } else if style.script == Script::Sup {
                Script::None
            } else {
                style.script
            }
        }
        Mark::Highlight => {
            style.highlight = on.then(|| {
                value
                    .and_then(markup::color_value)
                    .unwrap_or(markup::DEFAULT_HIGHLIGHT)
            });
        }
        Mark::Color => style.color = value.and_then(markup::color_value).filter(|_| on),
        Mark::Size => {
            style.size_pt = value.and_then(|v| v.parse::<u16>().ok()).filter(|_| on);
        }
        Mark::Family => style.family = value.map(str::to_owned).filter(|_| on),
        Mark::Bold | Mark::Italic | Mark::Strike => {}
    }
}
