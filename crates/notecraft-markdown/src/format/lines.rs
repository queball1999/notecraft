//! The block prefix of one line: indent, quote markers, list marker, task box, heading marker.

use std::ops::Range;

/// A list marker and what follows it on the line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListMark {
    /// The marker including its trailing space (`- `, `1. `), as a range of the line.
    pub marker: Range<usize>,
    /// `Some(number)` for a numbered list.
    pub number: Option<u64>,
    /// The task box including its trailing space (`[ ] `), and whether it is checked.
    pub task: Option<(Range<usize>, bool)>,
}

impl ListMark {
    /// The marker and the task box together.
    pub fn whole(&self) -> Range<usize> {
        let end = self
            .task
            .as_ref()
            .map_or(self.marker.end, |(range, _)| range.end);
        self.marker.start..end
    }
}

/// What a line starts with. All offsets are relative to the start of the line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefix {
    /// Spaces and tabs at the start.
    pub indent: usize,
    /// How many `>` markers follow the indent.
    pub quote_depth: usize,
    /// Where the quote markers end (the indent when there are none).
    pub quote_end: usize,
    pub list: Option<ListMark>,
    /// Heading level and where the heading marker (`## `) ends.
    pub heading: Option<(u8, usize)>,
    /// Where the line's text starts.
    pub content: usize,
}

impl Prefix {
    pub fn parse(line: &str) -> Self {
        let bytes = line.as_bytes();
        let indent = bytes
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();
        let mut i = indent;
        let mut depth = 0;
        while bytes.get(i) == Some(&b'>') {
            depth += 1;
            i += 1;
            if bytes.get(i) == Some(&b' ') {
                i += 1;
            }
        }
        let quote_end = i;
        let mut prefix = Self {
            indent,
            quote_depth: depth,
            quote_end,
            list: None,
            heading: None,
            content: quote_end,
        };
        if let Some(list) = list_at(line, quote_end) {
            prefix.content = list.whole().end;
            prefix.list = Some(list);
        } else if let Some((level, end)) = heading_at(line, quote_end) {
            prefix.heading = Some((level, end));
            prefix.content = end;
        }
        prefix
    }
}

fn heading_at(line: &str, at: usize) -> Option<(u8, usize)> {
    let rest = line.get(at..)?;
    let hashes = rest.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let after = rest.get(hashes..)?;
    if after.is_empty() {
        return Some((u8::try_from(hashes).ok()?, at + hashes));
    }
    after
        .starts_with(' ')
        .then(|| (u8::try_from(hashes).unwrap_or(6), at + hashes + 1))
}

fn list_at(line: &str, at: usize) -> Option<ListMark> {
    let rest = line.get(at..)?;
    let (marker_len, number) =
        if let Some(m) = ["- ", "* ", "+ "].iter().find(|m| rest.starts_with(**m)) {
            (m.len(), None)
        } else {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            let sep = rest.as_bytes().get(digits).copied();
            let ok = (1..=9).contains(&digits)
                && matches!(sep, Some(b'.' | b')'))
                && rest.as_bytes().get(digits + 1) == Some(&b' ');
            if !ok {
                return None;
            }
            (digits + 2, rest.get(..digits)?.parse::<u64>().ok())
        };
    let marker = at..at + marker_len;
    let after = line.get(marker.end..)?;
    let task = ["[ ] ", "[x] ", "[X] "]
        .iter()
        .find(|t| after.starts_with(**t))
        .map(|t| (marker.end..marker.end + t.len(), *t != "[ ] "));
    Some(ListMark {
        marker,
        number,
        task,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prefixes() {
        let p = Prefix::parse("  - [x] done");
        assert_eq!(p.indent, 2);
        assert_eq!(p.content, 8);
        assert!(matches!(
            p.list.as_ref().and_then(|l| l.task.as_ref()),
            Some((_, true))
        ));
        let p = Prefix::parse("> ## Title");
        assert_eq!(p.quote_depth, 1);
        assert_eq!(p.heading, Some((2, 5)));
        assert_eq!(p.content, 5);
        let p = Prefix::parse("12. item");
        assert_eq!(p.list.as_ref().and_then(|l| l.number), Some(12));
        assert_eq!(p.content, 4);
        let p = Prefix::parse("plain *text*");
        assert_eq!(p.content, 0);
        assert!(p.list.is_none() && p.heading.is_none());
        assert!(Prefix::parse("#tag").heading.is_none());
        assert!(Prefix::parse("**bold** x").list.is_none());
    }
}
