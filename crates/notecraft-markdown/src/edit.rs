//! Edits as source span replacements (PR 1.4).
//!
//! A page is never re-serialized from its blocks. An [`Editor`] collects span replacements
//! against the original text and [`Editor::finish`] splices them in, so every byte outside
//! the edited spans (spacing, line endings, list markers, trailing whitespace, a byte order
//! mark) comes out exactly as it went in. Edits are checked before anything is built: spans
//! must lie inside the text, sit on character boundaries, and not overlap. A bad edit gives an
//! [`EditError`], never a panic.

use std::fmt;

use crate::block::{Block, Cell, ListItem, Span};
use crate::frontmatter::{self, Entry, Frontmatter, Value, MAX_ITEMS};
use crate::style::{LineEnding, Style};

/// Why an edit was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditError {
    /// The span reaches past the end of the text (or its start is after its end).
    OutOfBounds { span: Span, len: usize },
    /// A span edge falls inside a multi-byte character.
    NotCharBoundary { offset: usize },
    /// Two edits touch the same bytes.
    Overlap { first: Span, second: Span },
    /// The block, item, or cell index does not exist.
    NoSuchIndex { index: usize, len: usize },
    /// A frontmatter key that cannot be written as a plain key.
    InvalidKey(String),
    /// A frontmatter value that cannot be written (for example, a raw value that is empty).
    InvalidValue(String),
    /// The frontmatter is over the size cap and is kept as text; its keys cannot be edited.
    FrontmatterOversize,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds { span, len } => {
                write!(
                    f,
                    "edit span {}..{} is outside the text ({len} bytes)",
                    span.start, span.end
                )
            }
            Self::NotCharBoundary { offset } => {
                write!(f, "edit offset {offset} is inside a character")
            }
            Self::Overlap { first, second } => write!(
                f,
                "edits {}..{} and {}..{} overlap",
                first.start, first.end, second.start, second.end
            ),
            Self::NoSuchIndex { index, len } => {
                write!(f, "index {index} is out of range (0..{len})")
            }
            Self::InvalidKey(key) => write!(f, "{key:?} is not a valid frontmatter key"),
            Self::InvalidValue(why) => write!(f, "frontmatter value not writable: {why}"),
            Self::FrontmatterOversize => {
                write!(
                    f,
                    "the frontmatter is over the size cap and cannot be edited"
                )
            }
        }
    }
}

impl std::error::Error for EditError {}

/// One replacement: the bytes in `span` become `text`. An empty span is an insertion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub span: Span,
    pub text: String,
}

impl Edit {
    pub fn new(span: Span, text: impl Into<String>) -> Self {
        Self {
            span,
            text: text.into(),
        }
    }
}

/// Check one span against `source`.
fn check_span(source: &str, span: Span) -> Result<(), EditError> {
    if span.start > span.end || span.end > source.len() {
        return Err(EditError::OutOfBounds {
            span,
            len: source.len(),
        });
    }
    for offset in [span.start, span.end] {
        if !source.is_char_boundary(offset) {
            return Err(EditError::NotCharBoundary { offset });
        }
    }
    Ok(())
}

/// Apply `edits` to `source`: validate every span (in bounds, on character boundaries, none
/// overlapping), then splice. Edits at the same offset keep their given order. Bytes outside
/// the edited spans are copied unchanged.
pub fn apply_edits(source: &str, edits: &[Edit]) -> Result<String, EditError> {
    for edit in edits {
        check_span(source, edit.span)?;
    }
    let mut order: Vec<&Edit> = edits.iter().collect();
    // Stable, so insertions at one offset stay in the order they were given.
    order.sort_by_key(|e| (e.span.start, e.span.end));
    for pair in order.windows(2) {
        if let [a, b] = pair {
            if b.span.start < a.span.end {
                return Err(EditError::Overlap {
                    first: a.span,
                    second: b.span,
                });
            }
        }
    }
    let added: usize = order.iter().map(|e| e.text.len()).sum();
    let mut out = String::with_capacity(source.len() + added);
    let mut cursor = 0;
    for edit in order {
        let keep = source
            .get(cursor..edit.span.start)
            .ok_or(EditError::NotCharBoundary {
                offset: edit.span.start,
            })?;
        out.push_str(keep);
        out.push_str(&edit.text);
        cursor = edit.span.end;
    }
    out.push_str(
        source
            .get(cursor..)
            .ok_or(EditError::NotCharBoundary { offset: cursor })?,
    );
    Ok(out)
}

/// Collects edits to one page and applies them together.
///
/// Spans come from the parsed [`Document`](crate::Document) of the same text. Text handed to
/// the editor is markdown source; its line endings are rewritten to the file's own.
#[derive(Debug)]
pub struct Editor<'a> {
    source: &'a str,
    style: Style,
    edits: Vec<Edit>,
    /// The index in `edits` of a frontmatter block this editor is creating, so further keys
    /// join it instead of making a second block.
    created_frontmatter: Option<usize>,
}

impl<'a> Editor<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            style: Style::detect(source),
            edits: Vec::new(),
            created_frontmatter: None,
        }
    }

    /// The style (line ending) new text is written in: the file's own.
    pub fn style(&self) -> Style {
        self.style
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// The edits collected so far.
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }

    /// Replace the bytes in `span` with `markdown`.
    pub fn replace(&mut self, span: Span, markdown: &str) -> Result<(), EditError> {
        check_span(self.source, span)?;
        let text = self.style.eol.normalize(markdown);
        self.edits.push(Edit { span, text });
        Ok(())
    }

    /// Insert `markdown` at byte offset `at`.
    pub fn insert(&mut self, at: usize, markdown: &str) -> Result<(), EditError> {
        self.replace(Span::new(at, at), markdown)
    }

    /// Remove the bytes in `span`.
    pub fn delete(&mut self, span: Span) -> Result<(), EditError> {
        self.replace(span, "")
    }

    /// Replace a whole block with new markdown. For a block inside a list item or a quote, the
    /// replacement must carry that container's own line prefixes after its first line.
    pub fn replace_block(&mut self, block: &Block, markdown: &str) -> Result<(), EditError> {
        self.replace(block.span, markdown)
    }

    /// Replace a whole list item, from its marker to the end of its last line.
    pub fn replace_item(&mut self, item: &ListItem, markdown: &str) -> Result<(), EditError> {
        self.replace(item.span, markdown)
    }

    /// Replace the content of a table cell. The padding spaces around the content stay, and
    /// `|` is escaped and line breaks become spaces, so the row keeps its shape.
    pub fn replace_cell(&mut self, cell: &Cell, markdown: &str) -> Result<(), EditError> {
        check_span(self.source, cell.span)?;
        let raw = cell.span.slice(self.source);
        let content = escape_cell(markdown);
        let trimmed_start = raw.trim_start();
        let lead = raw.len() - trimmed_start.len();
        let trail = trimmed_start.len() - trimmed_start.trim_end().len();
        if trimmed_start.trim_end().is_empty() {
            // An empty cell: give the new content one space of padding on each side.
            return self.replace(cell.span, &format!(" {content} "));
        }
        let inner = Span::new(cell.span.start + lead, cell.span.end - trail);
        self.replace(inner, &content)
    }

    /// Insert a block after `block`, separated by one blank line.
    pub fn insert_after(&mut self, block: &Block, markdown: &str) -> Result<(), EditError> {
        let eol = self.style.eol.as_str();
        self.insert(block.span.end, &format!("{eol}{eol}{markdown}"))
    }

    /// Insert a block before `block`, separated by one blank line.
    pub fn insert_before(&mut self, block: &Block, markdown: &str) -> Result<(), EditError> {
        let eol = self.style.eol.as_str();
        self.insert(block.span.start, &format!("{markdown}{eol}{eol}"))
    }

    /// Delete `siblings[index]` together with the blank lines that separated it from its
    /// neighbor, so no gap is left behind. Works for the blocks of a page, a quote, or a list
    /// item.
    pub fn delete_block(&mut self, siblings: &[Block], index: usize) -> Result<(), EditError> {
        let spans: Vec<Span> = siblings.iter().map(|b| b.span).collect();
        self.delete_sibling(&spans, index)
    }

    /// Delete `items[index]` together with the line break that separated it from its
    /// neighbor.
    pub fn delete_item(&mut self, items: &[ListItem], index: usize) -> Result<(), EditError> {
        let spans: Vec<Span> = items.iter().map(|i| i.span).collect();
        self.delete_sibling(&spans, index)
    }

    fn delete_sibling(&mut self, spans: &[Span], index: usize) -> Result<(), EditError> {
        let this = *spans.get(index).ok_or(EditError::NoSuchIndex {
            index,
            len: spans.len(),
        })?;
        let span = match (
            spans.get(index + 1),
            index.checked_sub(1).and_then(|i| spans.get(i)),
        ) {
            // Take everything up to the next sibling: its own leading prefix stays.
            (Some(next), _) => Span::new(this.start, next.start),
            // The last one: take the gap before it instead.
            (None, Some(prev)) => Span::new(prev.end, this.end),
            (None, None) => this,
        };
        self.delete(span)
    }

    /// Set a top-level frontmatter key. An existing key is rewritten in place (its position
    /// and the bytes of every other entry stay); a new key goes last; a page with no
    /// frontmatter gets a new block at the top.
    pub fn set_frontmatter(&mut self, key: &str, value: &Value) -> Result<(), EditError> {
        check_key(key)?;
        let frontmatter = frontmatter::parse(self.source);
        let Some(fm) = frontmatter else {
            return self.create_frontmatter(key, value);
        };
        if fm.oversize {
            return Err(EditError::FrontmatterOversize);
        }
        let eol = region_eol(self.source, &fm, self.style.eol);
        let entry_text = format_entry(key, value, eol)?;
        match fm.get(key) {
            Some(entry) => self.replace(entry_span(entry), &entry_text),
            None => {
                let at = closing_line_start(self.source, &fm);
                self.insert(at, &entry_text)
            }
        }
    }

    /// Remove every top-level frontmatter entry with this key. Returns whether one existed.
    /// The other entries are untouched. An emptied block stays as an empty `---` pair.
    pub fn remove_frontmatter(&mut self, key: &str) -> Result<bool, EditError> {
        let Some(fm) = frontmatter::parse(self.source) else {
            return Ok(false);
        };
        if fm.oversize {
            return Err(EditError::FrontmatterOversize);
        }
        let mut removed = false;
        for entry in fm.entries.iter().filter(|e| e.key == key) {
            self.delete(entry_span(entry))?;
            removed = true;
        }
        Ok(removed)
    }

    fn create_frontmatter(&mut self, key: &str, value: &Value) -> Result<(), EditError> {
        let eol = self.style.eol;
        let line = format_entry(key, value, eol)?;
        let close = format!("---{}", eol.as_str());
        if let Some(index) = self.created_frontmatter {
            if let Some(edit) = self.edits.get_mut(index) {
                if edit.text.ends_with(&close) {
                    edit.text.truncate(edit.text.len() - close.len());
                    edit.text.push_str(&line);
                    edit.text.push_str(&close);
                    return Ok(());
                }
            }
        }
        // After a byte order mark, if the page has one.
        let at = if self.source.starts_with('\u{feff}') {
            '\u{feff}'.len_utf8()
        } else {
            0
        };
        check_span(self.source, Span::new(at, at))?;
        self.edits.push(Edit::new(
            Span::new(at, at),
            format!("---{}{line}{close}", eol.as_str()),
        ));
        self.created_frontmatter = Some(self.edits.len() - 1);
        Ok(())
    }

    /// Apply every edit and return the new text. With no edits this is the source, byte for
    /// byte.
    pub fn finish(self) -> Result<String, EditError> {
        apply_edits(self.source, &self.edits)
    }
}

/// The lines of an entry, to its last line ending.
fn entry_span(entry: &Entry) -> Span {
    entry.span
}

/// The line ending the frontmatter block already uses (the page's, if it has no line break).
fn region_eol(source: &str, fm: &Frontmatter, fallback: LineEnding) -> LineEnding {
    let region = fm.span.slice(source);
    if region.contains('\n') {
        LineEnding::detect(region)
    } else {
        fallback
    }
}

/// The offset where the closing `---` line starts.
fn closing_line_start(source: &str, fm: &Frontmatter) -> usize {
    let region = fm.span.slice(source);
    let without_eol = region.strip_suffix('\n').unwrap_or(region);
    let without_eol = without_eol.strip_suffix('\r').unwrap_or(without_eol);
    let line = without_eol.rfind('\n').map_or(0, |i| i + 1);
    fm.span.start + line
}

/// Keys NoteCraft will write: letters, digits, `_`, `-`, `.`; not empty; not starting with a
/// character YAML gives a meaning to.
fn check_key(key: &str) -> Result<(), EditError> {
    let ok = !key.is_empty()
        && key.len() <= 256
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && key
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(EditError::InvalidKey(key.to_owned()))
    }
}

/// `key: value` as frontmatter lines, each ending with `eol`.
pub(crate) fn format_entry(key: &str, value: &Value, eol: LineEnding) -> Result<String, EditError> {
    let eol = eol.as_str();
    let mut out = String::new();
    out.push_str(key);
    out.push(':');
    match value {
        Value::Scalar(s) => {
            out.push(' ');
            out.push_str(&yaml_scalar(s));
            out.push_str(eol);
        }
        Value::List(items) => {
            out.push_str(" [");
            for (i, item) in items.iter().take(MAX_ITEMS).enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&yaml_scalar(item));
            }
            out.push(']');
            out.push_str(eol);
        }
        Value::Map(pairs) => {
            out.push_str(eol);
            for (k, v) in pairs.iter().take(MAX_ITEMS) {
                check_key(k)?;
                out.push_str("  ");
                out.push_str(k);
                out.push_str(": ");
                out.push_str(&yaml_scalar(v));
                out.push_str(eol);
            }
        }
        Value::Raw(raw) => {
            let raw = raw.trim_end_matches(['\r', '\n']);
            if raw.trim().is_empty() {
                return Err(EditError::InvalidValue("empty raw value".to_owned()));
            }
            // `Raw` holds the text after the colon, as written.
            if !raw.starts_with([' ', '\t', '\n', '\r']) {
                out.push(' ');
            }
            out.push_str(&LineEnding::normalize(
                if eol == "\r\n" {
                    LineEnding::Crlf
                } else {
                    LineEnding::Lf
                },
                raw,
            ));
            out.push_str(eol);
        }
    }
    Ok(out)
}

/// A scalar as YAML text: plain when that reads back as the same string, else double-quoted.
fn yaml_scalar(text: &str) -> String {
    if is_plain_safe(text) {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn is_plain_safe(text: &str) -> bool {
    if text.is_empty() || text.trim() != text {
        return false;
    }
    if text.chars().any(char::is_control) {
        return false;
    }
    if text.starts_with(|c: char| {
        matches!(
            c,
            '[' | ']'
                | '{'
                | '}'
                | ','
                | '#'
                | '&'
                | '*'
                | '!'
                | '|'
                | '>'
                | '\''
                | '"'
                | '%'
                | '@'
                | '`'
                | '?'
                | ':'
                | '-'
        )
    }) {
        // `-` alone is a list marker; `-5` or `-foo` read fine but quoting is always safe.
        return false;
    }
    if text.contains(": ") || text.contains(" #") || text.ends_with(':') {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "true" | "false" | "yes" | "no" | "on" | "off" | "null" | "~" | "y" | "n"
    ) {
        return false;
    }
    // Numbers would be read as numbers by other tools.
    text.parse::<f64>().is_err()
}

/// Make `markdown` safe to place inside one table cell.
fn escape_cell(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut backslashes = 0usize;
    for c in markdown.chars() {
        match c {
            '|' => {
                if backslashes.is_multiple_of(2) {
                    out.push('\\');
                }
                out.push('|');
                backslashes = 0;
            }
            '\r' | '\n' => {
                if !out.ends_with(' ') {
                    out.push(' ');
                }
                backslashes = 0;
            }
            '\\' => {
                out.push('\\');
                backslashes += 1;
            }
            other => {
                out.push(other);
                backslashes = 0;
            }
        }
    }
    out
}
