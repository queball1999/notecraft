//! Find on page (TX-11): the matching logic and the find bar's state.
//!
//! The page text that can be found is the text the canvas shows, one "unit" per laid-out text
//! block (a paragraph, heading, table cell, or code block), in container order and then block
//! order. Markdown syntax is never matched, only what the reader sees. The front end lays out
//! the same units, so a match here is a match on screen, in the same order. Matching is
//! case-insensitive and bounded: a query is cut to [`MAX_QUERY_CHARS`], and at most
//! [`MAX_MATCHES`] matches (and [`MAX_UNITS`] units) are looked at.

use std::ops::Range;

use notecraft_markdown::objects::is_file_paragraph;
use notecraft_markdown::{markup, Block, BlockKind, ContainerRun, Document, Run};

/// Longest query used, in characters.
pub const MAX_QUERY_CHARS: usize = 256;
/// Most matches found on one page.
pub const MAX_MATCHES: usize = 10_000;
/// Most text units looked at on one page.
pub const MAX_UNITS: usize = 100_000;
/// Most characters of one unit searched.
const MAX_UNIT_CHARS: usize = 1 << 20;
/// Most table columns the canvas lays out (the same cap as the canvas).
pub const MAX_COLUMNS: usize = 64;

/// The state of the find bar. It belongs to the session, so the bar keeps its query when the
/// page changes, as OneNote's does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FindState {
    /// The find bar is showing.
    pub open: bool,
    /// What to find (empty finds nothing).
    pub query: String,
    /// Index of the match the user is on. It may be past the last match after the page
    /// changed; use [`FindState::current_of`].
    pub current: usize,
    /// Goes up whenever the page should scroll to the current match.
    pub scroll: u64,
    /// Goes up whenever the find bar should take keyboard focus.
    pub focus: u64,
}

impl FindState {
    /// The current match index for `count` matches, or `None` when there are none.
    pub fn current_of(&self, count: usize) -> Option<usize> {
        (count > 0).then(|| self.current.min(count - 1))
    }
}

/// One match: the text unit it is in and its byte range there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub unit: usize,
    pub range: Range<usize>,
}

/// The query as it is searched: cut to [`MAX_QUERY_CHARS`].
pub fn clean_query(query: &str) -> String {
    query.chars().take(MAX_QUERY_CHARS).collect()
}

/// One character folded for a case-insensitive comparison.
fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Every non-overlapping, case-insensitive match of `needle` in `haystack`, left to right, as
/// byte ranges on character boundaries. At most `limit` are returned. An empty needle finds
/// nothing.
pub fn find_ranges(haystack: &str, needle: &str, limit: usize) -> Vec<Range<usize>> {
    let needle: Vec<char> = needle.chars().take(MAX_QUERY_CHARS).map(fold).collect();
    if needle.is_empty() || limit == 0 {
        return Vec::new();
    }
    let hay: Vec<(usize, char)> = haystack
        .char_indices()
        .take(MAX_UNIT_CHARS)
        .map(|(at, c)| (at, fold(c)))
        .collect();
    let end_of = |index: usize| hay.get(index).map_or(haystack.len(), |(at, _)| *at);
    let mut found = Vec::new();
    let mut i = 0;
    while found.len() < limit {
        let Some(window) = hay.get(i..i + needle.len()) else {
            break;
        };
        let hit = window.iter().zip(&needle).all(|((_, h), n)| h == n);
        match window.first() {
            Some((start, _)) if hit => {
                found.push(*start..end_of(i + needle.len()));
                i += needle.len();
            }
            _ => i += 1,
        }
    }
    found
}

/// Every match of `query` in `units`, in order, at most [`MAX_MATCHES`].
pub fn find_in_units<S: AsRef<str>>(units: &[S], query: &str) -> Vec<Match> {
    let query = clean_query(query);
    let mut out = Vec::new();
    for (unit, text) in units.iter().enumerate().take(MAX_UNITS) {
        let room = MAX_MATCHES - out.len();
        if room == 0 {
            break;
        }
        for range in find_ranges(text.as_ref(), &query, room) {
            out.push(Match { unit, range });
        }
    }
    out
}

/// The text units of a page, in the order the canvas lays them out.
pub fn page_units(doc: &Document, runs: &[ContainerRun]) -> Vec<String> {
    let mut units = Vec::new();
    for run in runs {
        let blocks = doc.blocks.get(run.blocks.clone()).unwrap_or(&[]);
        block_units(blocks, &mut units);
    }
    units
}

fn joined(runs: &[Run]) -> String {
    runs.iter().map(|r| r.text.as_str()).collect()
}

fn block_units(blocks: &[Block], units: &mut Vec<String>) {
    for block in blocks {
        if units.len() >= MAX_UNITS {
            return;
        }
        match &block.kind {
            BlockKind::Heading { runs, .. } => {
                if !runs.is_empty() {
                    units.push(joined(runs));
                }
            }
            BlockKind::Paragraph { runs } => paragraph_units(runs, units),
            BlockKind::List { items, .. } => {
                for item in items {
                    block_units(&item.blocks, units);
                }
            }
            BlockKind::Table(table) => {
                let columns = std::iter::once(&table.header)
                    .chain(&table.rows)
                    .map(Vec::len)
                    .max()
                    .unwrap_or(0)
                    .min(MAX_COLUMNS);
                for row in std::iter::once(&table.header).chain(&table.rows) {
                    for cell in row.iter().take(columns) {
                        units.push(joined(&cell.runs));
                    }
                }
            }
            BlockKind::Code { literal, .. } => {
                units.push(literal.trim_end_matches('\n').to_owned());
            }
            BlockKind::Html { literal } => {
                // A paragraph alignment comment draws nothing.
                if markup::parse_align_comment(literal).is_none() {
                    units.push(literal.trim_end_matches('\n').to_owned());
                }
            }
            BlockKind::Quote { blocks } => block_units(blocks, units),
            BlockKind::Rule => {}
        }
    }
}

fn paragraph_units(runs: &[Run], units: &mut Vec<String>) {
    if is_file_paragraph(runs) {
        return;
    }
    if !runs.iter().any(|r| r.image) {
        if !runs.is_empty() {
            units.push(joined(runs));
        }
        return;
    }
    // Text on either side of a picture is laid out on its own; the picture's alt text is not
    // page text.
    let mut pending: Vec<Run> = Vec::new();
    let flush = |pending: &mut Vec<Run>, units: &mut Vec<String>| {
        if !pending.iter().all(|r| r.text.trim().is_empty()) {
            units.push(joined(pending));
        }
        pending.clear();
    };
    for run in runs {
        if run.image {
            flush(&mut pending, units);
        } else {
            pending.push(run.clone());
        }
    }
    flush(&mut pending, units);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranges(hay: &str, needle: &str) -> Vec<Range<usize>> {
        find_ranges(hay, needle, MAX_MATCHES)
    }

    #[test]
    fn matching_ignores_case_and_does_not_overlap() {
        assert_eq!(
            ranges("Hello hello HELLO", "hello"),
            vec![0..5, 6..11, 12..17]
        );
        assert_eq!(ranges("aaaa", "aa"), vec![0..2, 2..4]);
        assert_eq!(ranges("abc", "d"), Vec::<Range<usize>>::new());
    }

    #[test]
    fn an_empty_query_finds_nothing() {
        assert!(ranges("abc", "").is_empty());
        assert!(find_ranges("abc", "a", 0).is_empty());
    }

    #[test]
    fn ranges_are_on_character_boundaries() {
        let text = "caf\u{e9} \u{c9}t\u{e9} \u{1F44D}x";
        let found = ranges(text, "\u{e9}");
        assert_eq!(found.len(), 3);
        for r in found {
            assert!(text.is_char_boundary(r.start) && text.is_char_boundary(r.end));
            assert_eq!(text[r].to_lowercase(), "\u{e9}");
        }
        assert_eq!(ranges("a \u{1F44D} b", "\u{1F44D}"), vec![2..6]);
    }

    #[test]
    fn the_match_count_and_query_are_bounded() {
        let hay = "a".repeat(50_000);
        assert_eq!(find_ranges(&hay, "a", 100).len(), 100);
        assert_eq!(find_ranges(&hay, "a", MAX_MATCHES).len(), MAX_MATCHES);
        let long = "b".repeat(MAX_QUERY_CHARS * 2);
        assert_eq!(clean_query(&long).chars().count(), MAX_QUERY_CHARS);
        assert_eq!(
            find_in_units(&["b".repeat(MAX_QUERY_CHARS)], &long).len(),
            1
        );
        let units = vec!["a".repeat(MAX_MATCHES); 3];
        assert_eq!(find_in_units(&units, "a").len(), MAX_MATCHES);
    }

    #[test]
    fn units_are_found_in_order() {
        let units = ["one two", "Two", "none"];
        let hits = find_in_units(&units, "TWO");
        assert_eq!(
            hits,
            vec![
                Match {
                    unit: 0,
                    range: 4..7
                },
                Match {
                    unit: 1,
                    range: 0..3
                }
            ]
        );
    }

    fn page(text: &str) -> Vec<String> {
        let doc = notecraft_markdown::parse(text);
        let runs = notecraft_markdown::split_containers(text, &doc);
        page_units(&doc, &runs)
    }

    #[test]
    fn markdown_syntax_is_not_searched_but_the_text_is() {
        let units = page("# Big **bold** title\n\n- one\n- two *two*\n\n> quoted\n\n`code`\n");
        assert_eq!(units[0], "Big bold title");
        assert!(units.iter().any(|u| u == "two two"));
        assert!(units.iter().any(|u| u == "quoted"));
        assert!(find_in_units(&units, "**").is_empty());
        assert_eq!(find_in_units(&units, "bold").len(), 1);
    }

    #[test]
    fn every_container_and_table_cell_is_searched() {
        let marker = notecraft_markdown::marker_line("c1");
        let text =
            format!("first\n\n{marker}\nsecond first\n\n| a | first |\n|---|---|\n| first | b |\n");
        let units = page(&text);
        assert_eq!(find_in_units(&units, "first").len(), 4, "{units:?}");
    }

    #[test]
    fn picture_alt_text_is_not_page_text() {
        let units = page("before ![secret alt](a.png) after\n");
        assert!(find_in_units(&units, "secret").is_empty());
        assert_eq!(find_in_units(&units, "before").len(), 1);
        assert_eq!(find_in_units(&units, "after").len(), 1);
    }

    #[test]
    fn hostile_pages_do_not_panic() {
        let text = "[".repeat(5000) + &"\u{0}".repeat(100) + "| a |\n|\n";
        let units = page(&text);
        let _ = find_in_units(&units, "[");
    }

    #[test]
    fn the_current_match_stays_in_range() {
        let state = FindState {
            current: 9,
            ..FindState::default()
        };
        assert_eq!(state.current_of(0), None);
        assert_eq!(state.current_of(3), Some(2));
    }
}
