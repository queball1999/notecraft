//! Note tags in the page text (docs/markdown.md section 4): inline `#name` tags with their
//! source spans, and the paragraphs and list items that hold them.
//!
//! A tag is a `#` that starts a word, followed by a name. The exact grammar:
//!
//! - The `#` is the first character of the text or comes right after whitespace. So
//!   `a#b`, `(#b)`, `[#b](x)`, `*#b*`, `\#b`, and `http://x/#b` are not tags.
//! - The name starts with an ASCII letter and goes on with ASCII letters, digits, `-`, and
//!   `_`, at most [`MAX_NAME_BYTES`] bytes. Trailing `-` and `_` are not part of the name.
//!   Names compare ignoring case; [`InlineTag::name`] is lowercase.
//! - The name ends at the end of the text, whitespace, or one of `. , ; : ! ? ) ] } " ' * ~ <`.
//!   Any other next character (a non-ASCII letter, `/`, `#`, `@`) means the word is not a tag.
//! - Nothing in a code span, a code block, raw HTML, an autolink or HTML tag (`<...>`), a link
//!   or image (`[text](target)`), or a page link (`[[...]]`) is a tag. A heading's leading
//!   `#` marks are not tags, because a name must start with a letter and the marks are
//!   followed by a space.
//!
//! A tag belongs to the paragraph or heading that holds it ([`TaggedItem::holder`]); OneNote
//! tags a paragraph. A task list item (`- [ ]`) is a To Do tag on its paragraph
//! ([`TaggedItem::task`]).

use crate::{Block, BlockKind, Document, ListItem, Run, Span};

/// Longest tag name, in bytes.
pub const MAX_NAME_BYTES: usize = 64;
/// Most tagged items [`tagged_items`] returns for one page.
pub const MAX_TAGGED_ITEMS: usize = 20_000;
/// Most inline tags kept for one paragraph.
const MAX_TAGS_PER_ITEM: usize = 64;
/// Most bytes of paragraph text kept with a tagged item.
const MAX_ITEM_TEXT: usize = 300;
/// How far the scanner looks ahead for the end of a code span, tag, or link, in bytes.
const MAX_LOOKAHEAD: usize = 2048;

/// One inline `#name` tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineTag {
    /// The name without the `#`, lowercase.
    pub name: String,
    /// The source bytes of `#name` as written.
    pub span: Span,
}

/// The check box of a task list item, which is a To Do tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskBox {
    /// The source bytes of `[ ]`, `[x]`, or `[X]`.
    pub span: Span,
    pub checked: bool,
}

/// A paragraph or heading that has a tag or a check box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaggedItem {
    /// The source bytes of the paragraph or heading block.
    pub holder: Span,
    /// The inline tags in the holder, in source order.
    pub tags: Vec<InlineTag>,
    /// The check box, when the holder is the first paragraph of a task list item.
    pub task: Option<TaskBox>,
    /// The holder's text without markdown syntax or tag names, at most 300 bytes.
    pub text: String,
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic()
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// Whether a tag name may be followed by `c`.
fn ends_name(c: Option<char>) -> bool {
    match c {
        None => true,
        Some(c) => c.is_whitespace() || ".,;:!?)]}\"'*~<".contains(c),
    }
}

/// The tag name at the start of `rest` (the text after a `#`), and its length in bytes.
/// `None` when the word is not a tag.
pub(crate) fn name_at(rest: &str) -> Option<(String, usize)> {
    let bytes = rest.as_bytes();
    if !bytes.first().copied().is_some_and(is_name_start) {
        return None;
    }
    let mut len = bytes
        .iter()
        .take(MAX_NAME_BYTES + 1)
        .take_while(|&&b| is_name_byte(b))
        .count();
    if len > MAX_NAME_BYTES {
        return None;
    }
    // The name must end at a word boundary, before trailing hyphens and underscores are cut.
    if !ends_name(rest.get(len..).and_then(|t| t.chars().next())) {
        return None;
    }
    while len > 1 && matches!(bytes.get(len - 1), Some(b'-' | b'_')) {
        len -= 1;
    }
    let name = rest.get(..len)?.to_ascii_lowercase();
    Some((name, len))
}

/// Whether `name` is a usable tag name by the grammar above.
pub fn is_valid_name(name: &str) -> bool {
    name_at(name).is_some_and(|(_, len)| len == name.len())
}

/// Find the inline tags in `text`, which is paragraph source starting at page offset `base`.
/// Spans are page offsets. Bounded: at most 64 tags are returned.
pub fn scan_tags(text: &str, base: usize) -> Vec<InlineTag> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut prev: Option<char> = None;
    while i < bytes.len() && out.len() < MAX_TAGS_PER_ITEM {
        let b = bytes.get(i).copied().unwrap_or(0);
        let skip_to = match b {
            b'`' => code_span_end(bytes, i),
            b'\\' => escape_end(text, i),
            b'<' => angle_end(bytes, i),
            b'[' => link_end(bytes, i),
            b'#' => {
                let word_start = prev.is_none_or(char::is_whitespace);
                let rest = text.get(i + 1..).unwrap_or("");
                match name_at(rest).filter(|_| word_start) {
                    Some((name, len)) => {
                        out.push(InlineTag {
                            name,
                            span: Span::new(base + i, base + i + 1 + len),
                        });
                        Some(i + 1 + len)
                    }
                    None => None,
                }
            }
            _ => None,
        };
        let next = skip_to.filter(|&n| n > i).unwrap_or_else(|| {
            // One character forward.
            i + text
                .get(i..)
                .and_then(|t| t.chars().next())
                .map_or(1, char::len_utf8)
        });
        prev = text.get(..next).and_then(|t| t.chars().next_back());
        i = next;
    }
    out
}

/// The end of the code span that opens at `i`, if it closes.
fn code_span_end(bytes: &[u8], i: usize) -> Option<usize> {
    let open = bytes.iter().skip(i).take_while(|&&b| b == b'`').count();
    let mut j = i + open;
    let limit = bytes.len().min(i + MAX_LOOKAHEAD);
    while j < limit {
        if bytes.get(j) == Some(&b'`') {
            let run = bytes.iter().skip(j).take_while(|&&b| b == b'`').count();
            if run == open {
                return Some(j + run);
            }
            j += run;
        } else {
            j += 1;
        }
    }
    // No closing run: the backticks are plain text.
    Some(i + open)
}

/// A backslash escapes the next character.
fn escape_end(text: &str, i: usize) -> Option<usize> {
    let next = text.get(i + 1..)?.chars().next()?;
    Some(i + 1 + next.len_utf8())
}

/// An HTML tag, comment, or autolink (`<...>`) is skipped whole.
fn angle_end(bytes: &[u8], i: usize) -> Option<usize> {
    let first = bytes.get(i + 1).copied()?;
    if !(first.is_ascii_alphabetic() || matches!(first, b'/' | b'!' | b'?')) {
        return None;
    }
    let limit = bytes.len().min(i + MAX_LOOKAHEAD);
    let close = bytes.get(i..limit)?.iter().position(|&b| b == b'>')?;
    Some(i + close + 1)
}

/// A page link `[[...]]` or a link or image `[text](target)` is skipped whole.
fn link_end(bytes: &[u8], i: usize) -> Option<usize> {
    let limit = bytes.len().min(i + MAX_LOOKAHEAD);
    let window = bytes.get(i..limit)?;
    if window.get(1) == Some(&b'[') {
        let close = window.windows(2).position(|w| w == b"]]")?;
        return Some(i + close + 2);
    }
    let close = window.iter().position(|&b| b == b']')?;
    if window.get(close + 1) != Some(&b'(') {
        return None;
    }
    let end = window.get(close + 2..)?.iter().position(|&b| b == b')')?;
    Some(i + close + 2 + end + 1)
}

/// The check box of the task list item that starts at `item`, as page offsets.
fn task_box(source: &str, item: &ListItem) -> Option<TaskBox> {
    let text = item.span.slice(source);
    let bytes = text.as_bytes();
    let mut i = bytes
        .iter()
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    match bytes.get(i)? {
        b'-' | b'*' | b'+' => i += 1,
        b'0'..=b'9' => {
            i += bytes
                .iter()
                .skip(i)
                .take_while(|b| b.is_ascii_digit())
                .count();
            if !matches!(bytes.get(i), Some(b'.' | b')')) {
                return None;
            }
            i += 1;
        }
        _ => return None,
    }
    i += bytes
        .iter()
        .skip(i)
        .take_while(|&&b| b == b' ' || b == b'\t')
        .count();
    if bytes.get(i) != Some(&b'[') || bytes.get(i + 2) != Some(&b']') {
        return None;
    }
    let checked = match bytes.get(i + 1)? {
        b' ' => false,
        b'x' | b'X' => true,
        _ => return None,
    };
    let start = item.span.start + i;
    Some(TaskBox {
        span: Span::new(start, start + 3),
        checked,
    })
}

/// Every paragraph and heading of the page that has an inline tag or is the first paragraph
/// of a task list item, in source order. Table cells, code, and raw HTML are not tagged.
/// Bounded to [`MAX_TAGGED_ITEMS`] items.
pub fn tagged_items(source: &str, doc: &Document) -> Vec<TaggedItem> {
    let mut out = Vec::new();
    collect(source, &doc.blocks, None, &mut out);
    out
}

fn collect(source: &str, blocks: &[Block], mut task: Option<TaskBox>, out: &mut Vec<TaggedItem>) {
    for block in blocks {
        if out.len() >= MAX_TAGGED_ITEMS {
            return;
        }
        // The check box belongs to the first block of the item only.
        let task = task.take();
        match &block.kind {
            BlockKind::Paragraph { runs } | BlockKind::Heading { runs, .. } => {
                let tags = scan_tags(block.span.slice(source), block.span.start);
                if tags.is_empty() && task.is_none() {
                    continue;
                }
                let text = display_text(runs, &tags);
                out.push(TaggedItem {
                    holder: block.span,
                    tags,
                    task,
                    text,
                });
            }
            BlockKind::List { items, .. } => {
                for item in items {
                    let task = item.task.and_then(|_| task_box(source, item));
                    collect(source, &item.blocks, task, out);
                }
            }
            BlockKind::Quote { blocks } => collect(source, blocks, None, out),
            _ => {}
        }
    }
}

/// The text of `runs` with the tag names taken out, trimmed and capped.
fn display_text(runs: &[Run], tags: &[InlineTag]) -> String {
    let stripped = strip_tags(runs, tags, &|_| true).map(|(runs, _)| runs);
    let runs = stripped.as_deref().unwrap_or(runs);
    let mut text = String::new();
    for run in runs {
        if run.image {
            continue;
        }
        text.push_str(&run.text);
        if text.len() > MAX_ITEM_TEXT * 2 {
            break;
        }
    }
    let text = text.trim();
    let mut end = text.len().min(MAX_ITEM_TEXT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.get(..end).unwrap_or("").to_owned()
}

/// A tag token found in a run list.
struct Token {
    run: usize,
    start: usize,
    end: usize,
    name: String,
}

/// The `#name` tokens in the text of `runs`, by the same grammar as [`scan_tags`] on text.
/// Code, link, and image runs are skipped.
fn run_tokens(runs: &[Run]) -> Vec<Token> {
    let mut out = Vec::new();
    let mut prev: Option<char> = None;
    for (index, run) in runs.iter().enumerate() {
        if run.code || run.image || run.link.is_some() || run.math != crate::Math::None {
            prev = run.text.chars().next_back().or(prev);
            continue;
        }
        let text = run.text.as_str();
        let mut i = 0;
        while let Some(c) = text.get(i..).and_then(|t| t.chars().next()) {
            if c == '#' && prev.is_none_or(char::is_whitespace) {
                if let Some((name, len)) = name_at(text.get(i + 1..).unwrap_or("")) {
                    out.push(Token {
                        run: index,
                        start: i,
                        end: i + 1 + len,
                        name,
                    });
                    i += 1 + len;
                    prev = text.get(..i).and_then(|t| t.chars().next_back());
                    continue;
                }
            }
            prev = Some(c);
            i += c.len_utf8();
        }
    }
    out
}

/// Take the tag tokens `remove` accepts out of `runs` (and one space after each), for showing
/// a paragraph without its tag names. Returns the new runs and the names removed, in order.
///
/// `expected` is what [`scan_tags`] found in the paragraph's source. When the runs hold a
/// different list of tokens (an escape or other markup that the runs no longer show), nothing
/// is removed and the result is `None`.
pub fn strip_tags(
    runs: &[Run],
    expected: &[InlineTag],
    remove: &dyn Fn(&str) -> bool,
) -> Option<(Vec<Run>, Vec<String>)> {
    let tokens = run_tokens(runs);
    let same = tokens.len() == expected.len()
        && tokens.iter().zip(expected).all(|(t, e)| t.name == e.name);
    if !same {
        return None;
    }
    let mut out: Vec<Run> = runs.to_vec();
    let mut removed = Vec::new();
    // Back to front, so earlier offsets stay valid.
    for token in tokens.iter().rev() {
        if !remove(&token.name) {
            continue;
        }
        let Some(run) = out.get_mut(token.run) else {
            continue;
        };
        let mut end = token.end;
        if run.text.get(end..).is_some_and(|t| t.starts_with(' ')) {
            end += 1;
        }
        if run.text.get(token.start..end).is_some() {
            run.text.replace_range(token.start..end, "");
            removed.push(token.name.clone());
        }
    }
    removed.reverse();
    out.retain(|r| !r.text.is_empty() || r.image);
    Some((out, removed))
}
