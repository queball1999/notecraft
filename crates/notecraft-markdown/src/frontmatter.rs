//! The frontmatter parser: a small, bounded subset of YAML (docs/markdown.md section 3.1).
//!
//! Supported: scalars (plain, single-quoted, double-quoted), lists (inline `[a, b]` and block
//! `- a`), and one level of maps. Anything else (block scalars, deeper maps, flow maps,
//! anchors) is kept as raw text, never an error. Every entry keeps its exact source text, so
//! unknown keys survive a save unchanged. No YAML crate is used.

use crate::Span;

/// Largest frontmatter body read, 64 KiB (docs/markdown.md section 9). A larger block is kept
/// as text and its keys are ignored.
pub const MAX_FRONTMATTER_BYTES: usize = 64 * 1024;

/// Most entries read from one frontmatter block.
pub const MAX_ENTRIES: usize = 1_000;

/// Most items read into one list or map.
pub const MAX_ITEMS: usize = 1_000;

/// A parsed value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Scalar(String),
    List(Vec<String>),
    /// One level of map, in file order.
    Map(Vec<(String, String)>),
    /// Anything the subset does not understand, as written (without the key).
    Raw(String),
}

/// One top-level key and its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub value: Value,
    /// The exact source lines of this entry, from the key line to its last continuation line
    /// (including line endings), as offsets into the page source.
    pub span: Span,
}

/// The frontmatter block at the top of a page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frontmatter {
    /// From the first byte of the opening `---` line to the end of the closing line
    /// (including its line ending).
    pub span: Span,
    pub entries: Vec<Entry>,
    /// The block was over the size cap: it is kept as text and `entries` is empty.
    pub oversize: bool,
}

impl Frontmatter {
    /// The first entry with this key.
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// A scalar value.
    pub fn scalar(&self, key: &str) -> Option<&str> {
        match &self.get(key)?.value {
            Value::Scalar(s) => Some(s),
            _ => None,
        }
    }

    /// A list value; a single scalar counts as a one-item list.
    pub fn list(&self, key: &str) -> Vec<&str> {
        match self.get(key).map(|e| &e.value) {
            Some(Value::List(items)) => items.iter().map(String::as_str).collect(),
            Some(Value::Scalar(s)) if !s.is_empty() => vec![s.as_str()],
            _ => Vec::new(),
        }
    }

    pub fn title(&self) -> Option<&str> {
        self.scalar("title")
    }

    pub fn tags(&self) -> Vec<&str> {
        self.list("tags")
    }

    pub fn aliases(&self) -> Vec<&str> {
        self.list("aliases")
    }

    /// A value inside the `notecraft:` map (`id`, `paper`, `color`).
    pub fn notecraft(&self, key: &str) -> Option<&str> {
        match &self.get("notecraft")?.value {
            Value::Map(pairs) => pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str()),
            _ => None,
        }
    }
}

/// One line of the source with its offsets.
struct Line<'a> {
    /// The line without its line ending.
    text: &'a str,
    start: usize,
    /// Offset just past the line ending (or the end of the text).
    end: usize,
}

fn lines(text: &str) -> impl Iterator<Item = Line<'_>> {
    let mut start = 0;
    std::iter::from_fn(move || {
        if start >= text.len() {
            return None;
        }
        let rest = text.get(start..)?;
        let (body, advance) = match rest.find('\n') {
            Some(i) => (rest.get(..i)?, i + 1),
            None => (rest, rest.len()),
        };
        let line = Line {
            text: body.strip_suffix('\r').unwrap_or(body),
            start,
            end: start + advance,
        };
        start += advance;
        Some(line)
    })
}

/// Parse the frontmatter at the top of `source`, if there is one: an opening `---` line, then
/// a closing `---` (or `...`) line. A leading byte order mark is allowed. Returns `None` when
/// the page has no frontmatter.
pub fn parse(source: &str) -> Option<Frontmatter> {
    let bom = if source.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };
    let mut all = lines(source.get(bom..)?).map(|mut l| {
        l.start += bom;
        l.end += bom;
        l
    });
    let open = all.next()?;
    if open.text.trim_end() != "---" {
        return None;
    }
    let mut body = Vec::new();
    let mut close_end = None;
    for line in all {
        let t = line.text.trim_end();
        if t == "---" || t == "..." {
            close_end = Some(line.end);
            break;
        }
        body.push(line);
    }
    let close_end = close_end?;
    let span = Span::new(0, close_end);
    let size: usize = body.iter().map(|l| l.end - l.start).sum();
    if size > MAX_FRONTMATTER_BYTES {
        return Some(Frontmatter {
            span,
            entries: Vec::new(),
            oversize: true,
        });
    }
    Some(Frontmatter {
        span,
        entries: entries(source, &body),
        oversize: false,
    })
}

/// True for a line that is blank or a comment.
fn is_idle(text: &str) -> bool {
    let t = text.trim();
    t.is_empty() || t.starts_with('#')
}

fn indent(text: &str) -> usize {
    text.len() - text.trim_start_matches([' ', '\t']).len()
}

/// Split `key: rest` at the first colon followed by a space or the end of the line.
fn split_key(text: &str) -> Option<(&str, &str)> {
    let (key, rest) = if let Some(quote) = text.chars().next().filter(|c| matches!(c, '"' | '\'')) {
        let close = text.get(1..)?.find(quote)? + 1;
        let after = text.get(close + 1..)?.strip_prefix(':')?;
        (text.get(1..close)?, after)
    } else {
        let mut at = None;
        for (i, c) in text.char_indices() {
            if c == ':'
                && text
                    .get(i + 1..)
                    .is_some_and(|r| r.is_empty() || r.starts_with(' '))
            {
                at = Some(i);
                break;
            }
        }
        let i = at?;
        (text.get(..i)?.trim_end(), text.get(i + 1..)?)
    };
    if key.is_empty() {
        return None;
    }
    if !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')) {
        return None;
    }
    Some((key, rest.trim()))
}

fn entries(source: &str, body: &[Line<'_>]) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let mut i = 0;
    while let Some(line) = body.get(i) {
        let top_level =
            indent(line.text) == 0 && !is_idle(line.text) && !line.text.starts_with("- ");
        let Some((key, rest)) = split_key(line.text).filter(|_| top_level) else {
            // Not a key line. Fold stray lines into the previous entry so its text stays whole.
            if let Some(prev) = out.last_mut() {
                prev.span.end = line.end;
            }
            i += 1;
            continue;
        };
        // Continuation lines: indented, block list items, blank, or comments.
        let mut j = i + 1;
        while let Some(next) = body.get(j) {
            let continues = is_idle(next.text)
                || indent(next.text) > 0
                || next.text.starts_with("- ")
                || next.text.trim_end() == "-";
            if !continues {
                break;
            }
            j += 1;
        }
        // Trailing blank lines belong to the gap between entries, not the entry.
        let mut last = j;
        while last > i + 1 && body.get(last - 1).is_some_and(|l| l.text.trim().is_empty()) {
            last -= 1;
        }
        let nested: Vec<&str> = body
            .get(i + 1..last)
            .unwrap_or_default()
            .iter()
            .map(|l| l.text)
            .collect();
        if out.len() < MAX_ENTRIES {
            let end = body.get(last - 1).map_or(line.end, |l| l.end);
            let raw_value = source
                .get(line.start..end)
                .and_then(|t| t.split_once(':'))
                .map_or("", |(_, v)| v);
            out.push(Entry {
                key: key.to_owned(),
                value: value_of(rest, &nested, raw_value),
                span: Span::new(line.start, end),
            });
        }
        i = j;
    }
    out
}

fn value_of(rest: &str, nested: &[&str], raw_value: &str) -> Value {
    let raw = || Value::Raw(raw_value.trim_matches(['\r', '\n']).to_owned());
    let content: Vec<&str> = nested.iter().copied().filter(|l| !is_idle(l)).collect();
    if rest.is_empty() {
        if content.is_empty() {
            return Value::Scalar(String::new());
        }
        return block_value(&content).unwrap_or_else(raw);
    }
    if !content.is_empty() {
        // A value on the key line plus indented lines is a multi-line scalar: keep as written.
        return raw();
    }
    if rest.starts_with('[') {
        return inline_list(rest).map_or_else(raw, Value::List);
    }
    if rest.starts_with(['{', '|', '>', '&', '*', '!']) {
        return raw();
    }
    scalar(rest).map_or_else(raw, Value::Scalar)
}

/// The value under a key with nothing after the colon: a block list or a one-level map.
fn block_value(content: &[&str]) -> Option<Value> {
    let first = content.first()?;
    let base = indent(first);
    if first.trim_start().starts_with("- ") || first.trim() == "-" {
        let mut items = Vec::new();
        for line in content {
            let item = line.trim_start().strip_prefix('-')?.trim();
            if indent(line) != base || item.contains(": ") && !item.starts_with(['"', '\'']) {
                return None;
            }
            if items.len() < MAX_ITEMS {
                items.push(scalar(item)?);
            }
        }
        return Some(Value::List(items));
    }
    let mut pairs = Vec::new();
    for line in content {
        if indent(line) != base {
            return None;
        }
        let (key, value) = split_key(line.trim_start())?;
        if value.is_empty() || value.starts_with(['[', '{', '|', '>']) {
            // A second level (or a flow value) is outside the subset.
            return None;
        }
        if pairs.len() < MAX_ITEMS {
            pairs.push((key.to_owned(), scalar(value)?));
        }
    }
    Some(Value::Map(pairs))
}

/// `[a, "b, c", 'd']` to its items. `None` if the brackets do not close.
fn inline_list(text: &str) -> Option<Vec<String>> {
    let inner = text.strip_prefix('[')?;
    let close = closing_bracket(inner)?;
    let after = inner.get(close + 1..)?.trim();
    if !(after.is_empty() || after.starts_with('#')) {
        return None;
    }
    let inner = inner.get(..close)?;
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                current.push(c);
                if c == '\\' && q == '"' {
                    if let Some(n) = chars.next() {
                        current.push(n);
                    }
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    current.push(c);
                }
                ',' => {
                    push_item(&mut items, &current)?;
                    current.clear();
                }
                _ => current.push(c),
            },
        }
    }
    if !current.trim().is_empty() || !items.is_empty() {
        push_item(&mut items, &current)?;
    }
    Some(items)
}

fn push_item(items: &mut Vec<String>, raw: &str) -> Option<()> {
    let raw = raw.trim();
    if raw.is_empty() {
        // `[a, ]` has a trailing comma: ignore the empty item.
        return Some(());
    }
    let value = scalar(raw)?;
    if items.len() < MAX_ITEMS {
        items.push(value);
    }
    Some(())
}

/// Index of the `]` that closes a list whose `[` was already consumed, skipping quotes.
fn closing_bracket(inner: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in inner.char_indices() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' && q == '"' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                ']' => return Some(i),
                _ => {}
            },
        }
    }
    None
}

/// A scalar: quoted (with the usual escapes) or plain (trailing ` # comment` removed).
/// `None` if a quote is left open.
fn scalar(text: &str) -> Option<String> {
    let text = text.trim();
    if let Some(inner) = text.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = inner.chars();
        loop {
            match chars.next()? {
                '"' => break,
                '\\' => match chars.next()? {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '0' => out.push('\0'),
                    other => out.push(other),
                },
                c => out.push(c),
            }
        }
        return Some(out);
    }
    if let Some(inner) = text.strip_prefix('\'') {
        let mut out = String::new();
        let mut chars = inner.chars().peekable();
        loop {
            match chars.next()? {
                '\'' if chars.peek() == Some(&'\'') => {
                    chars.next();
                    out.push('\'');
                }
                '\'' => break,
                c => out.push(c),
            }
        }
        return Some(out);
    }
    let plain = text
        .find(" #")
        .map_or(text, |i| text.get(..i).unwrap_or(text));
    Some(plain.trim().to_owned())
}
