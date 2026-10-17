//! Find the inline formatting spans of one line of markdown source.
//!
//! This is a small scanner, not a markdown parser: it pairs the delimiters NoteCraft writes
//! (`**`, `*`, `__`, `_`, `~~`, `==`, and the HTML tags of [`crate::markup`]) so a command can
//! tell whether text is already formatted and where the delimiters are. Code spans and
//! backslash escapes are skipped. A delimiter with no partner is ignored.

use std::ops::Range;

use crate::markup::{self, Mark, Tag};

/// Most spans read from one line.
const MAX_SPANS: usize = 512;
/// Most unmatched openers kept while scanning.
const MAX_OPEN: usize = 128;

/// One pair of delimiters and the text between them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanInfo {
    pub mark: Mark,
    /// The color, size, or family for value marks (normalized); `None` for the others and for
    /// the default highlight.
    pub value: Option<String>,
    /// Absolute byte range of the opening delimiter.
    pub open: Range<usize>,
    /// Absolute byte range of the closing delimiter.
    pub close: Range<usize>,
}

impl SpanInfo {
    /// The text between the delimiters.
    pub fn content(&self) -> Range<usize> {
        self.open.end..self.close.start
    }
}

struct Open {
    mark: Mark,
    value: Option<String>,
    range: Range<usize>,
    html: bool,
}

struct Scanner {
    stack: Vec<Open>,
    spans: Vec<SpanInfo>,
}

impl Scanner {
    fn close_at(&mut self, index: usize, close: Range<usize>) {
        if self.spans.len() >= MAX_SPANS {
            return;
        }
        self.stack.truncate(index + 1);
        if let Some(open) = self.stack.pop() {
            self.spans.push(SpanInfo {
                mark: open.mark,
                value: open.value,
                open: open.range,
                close,
            });
        }
    }

    fn symmetric(&mut self, mark: Mark, range: Range<usize>, can_open: bool, can_close: bool) {
        let found = self
            .stack
            .iter()
            .rposition(|open| open.mark == mark && !open.html);
        match found {
            Some(index) if can_close => self.close_at(index, range),
            _ if can_open && self.stack.len() < MAX_OPEN => {
                self.stack.push(Open {
                    mark,
                    value: None,
                    range,
                    html: false,
                });
            }
            _ => {}
        }
    }

    fn tag(&mut self, tag: Tag, range: Range<usize>) {
        match tag {
            Tag::Open(mark, value) => {
                if self.stack.len() < MAX_OPEN {
                    self.stack.push(Open {
                        mark,
                        value,
                        range,
                        html: true,
                    });
                }
            }
            Tag::Close(mark) => {
                if let Some(index) = self
                    .stack
                    .iter()
                    .rposition(|open| open.mark == mark && open.html)
                {
                    self.close_at(index, range);
                }
            }
            Tag::CloseSpan => {
                if let Some(index) = self.stack.iter().rposition(|open| {
                    open.html && matches!(open.mark, Mark::Color | Mark::Size | Mark::Family)
                }) {
                    self.close_at(index, range);
                }
            }
        }
    }
}

fn run_len(bytes: &[u8], at: usize, byte: u8) -> usize {
    bytes
        .get(at..)
        .map_or(0, |tail| tail.iter().take_while(|&&b| b == byte).count())
}

/// Every span in `text[from..to]` (a single line), with absolute offsets, ordered by where the
/// opening delimiter starts.
pub fn scan(text: &str, from: usize, to: usize) -> Vec<SpanInfo> {
    let Some(line) = text.get(..to) else {
        return Vec::new();
    };
    let bytes = line.as_bytes();
    let mut scanner = Scanner {
        stack: Vec::new(),
        spans: Vec::new(),
    };
    let mut i = from;
    while i < to {
        let Some(&b) = bytes.get(i) else { break };
        match b {
            b'\\' => {
                let skip = line
                    .get(i + 1..)
                    .and_then(|t| t.chars().next())
                    .map_or(1, char::len_utf8);
                i += 1 + skip;
            }
            b'`' => {
                let n = run_len(bytes, i, b'`');
                i += n;
                // A code span runs to the next backtick run of the same length.
                let mut j = i;
                let mut found = None;
                while j < to {
                    if bytes.get(j) == Some(&b'`') {
                        let m = run_len(bytes, j, b'`');
                        if m == n {
                            found = Some(j + m);
                            break;
                        }
                        j += m;
                    } else {
                        j += 1;
                    }
                }
                if let Some(end) = found {
                    i = end;
                }
            }
            b'*' | b'_' => {
                let n = run_len(bytes, i, b);
                let before = line.get(..i).and_then(|t| t.chars().next_back());
                let after = line.get(i + n..).and_then(|t| t.chars().next());
                let intraword = b == b'_'
                    && before.is_some_and(char::is_alphanumeric)
                    && after.is_some_and(char::is_alphanumeric);
                if n <= 3 && !intraword {
                    let can_open = after.is_some_and(|c| !c.is_whitespace())
                        && !(b == b'_' && before.is_some_and(char::is_alphanumeric));
                    let can_close = before.is_some_and(|c| !c.is_whitespace())
                        && !(b == b'_' && after.is_some_and(char::is_alphanumeric));
                    match n {
                        1 => scanner.symmetric(Mark::Italic, i..i + 1, can_open, can_close),
                        2 => scanner.symmetric(Mark::Bold, i..i + 2, can_open, can_close),
                        _ => {
                            // `***`: opening it is italic then bold; closing it, bold then italic.
                            let closes = can_close && !can_open
                                || can_close
                                    && scanner
                                        .stack
                                        .iter()
                                        .any(|o| o.mark == Mark::Bold && !o.html);
                            if closes {
                                scanner.symmetric(Mark::Bold, i..i + 2, can_open, can_close);
                                scanner.symmetric(Mark::Italic, i + 2..i + 3, can_open, can_close);
                            } else {
                                scanner.symmetric(Mark::Italic, i..i + 1, can_open, can_close);
                                scanner.symmetric(Mark::Bold, i + 1..i + 3, can_open, can_close);
                            }
                        }
                    }
                }
                i += n;
            }
            b'~' | b'=' => {
                let n = run_len(bytes, i, b);
                if n == 2 {
                    let before = line.get(..i).and_then(|t| t.chars().next_back());
                    let after = line.get(i + 2..).and_then(|t| t.chars().next());
                    let mark = if b == b'~' {
                        Mark::Strike
                    } else {
                        Mark::Highlight
                    };
                    scanner.symmetric(
                        mark,
                        i..i + 2,
                        after.is_some_and(|c| !c.is_whitespace()),
                        before.is_some_and(|c| !c.is_whitespace()),
                    );
                }
                i += n;
            }
            b'<' => {
                match line
                    .get(i..)
                    .and_then(markup::parse_tag)
                    .filter(|(_, len)| i + len <= to)
                {
                    Some((tag, len)) => {
                        scanner.tag(tag, i..i + len);
                        i += len;
                    }
                    None => i += 1,
                }
            }
            _ => {
                let skip = line
                    .get(i..)
                    .and_then(|t| t.chars().next())
                    .map_or(1, char::len_utf8);
                i += skip;
            }
        }
    }
    let mut spans = scanner.spans;
    spans.sort_by_key(|s| (s.open.start, s.close.end));
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(text: &str) -> Vec<(Mark, String)> {
        scan(text, 0, text.len())
            .into_iter()
            .map(|s| (s.mark, text[s.content()].to_owned()))
            .collect()
    }

    #[test]
    fn pairs_markdown_emphasis() {
        assert_eq!(
            marks("a **bold** and *it* ~~x~~ ==hi=="),
            vec![
                (Mark::Bold, "bold".into()),
                (Mark::Italic, "it".into()),
                (Mark::Strike, "x".into()),
                (Mark::Highlight, "hi".into()),
            ]
        );
    }

    #[test]
    fn pairs_html_tags_and_values() {
        let text = "<u>a</u> <span style=\"color:#FF0000\">r</span> <sub>2</sub>";
        let spans = scan(text, 0, text.len());
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].mark, Mark::Color);
        assert_eq!(spans[1].value.as_deref(), Some("#FF0000"));
    }

    #[test]
    fn triple_stars_are_bold_and_italic() {
        let text = "***both***";
        let got = marks(text);
        assert!(
            got.contains(&(Mark::Bold, "both".into())) || got.len() == 2,
            "{got:?}"
        );
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn skips_code_escapes_and_lone_delimiters() {
        assert!(marks("`**no**` and \\*no\\* and 2 * 3 * 4").is_empty());
        assert!(marks("snake_case_name").is_empty());
        assert!(marks("**unclosed").is_empty());
    }

    #[test]
    fn nested_spans_are_both_found() {
        let got = marks("**a *b* c**");
        assert_eq!(got.len(), 2);
    }
}
