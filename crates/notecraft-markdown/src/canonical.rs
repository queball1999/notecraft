//! Canonical serialization of new blocks (PR 1.4).
//!
//! Only text NoteCraft creates goes through here: ATX headings, `-` bullets, `1.` numbering,
//! `*` and `**` emphasis, fenced code, LF line endings unless the file already uses CRLF.
//! Text that is already on the page is never re-serialized; it is edited as a span
//! replacement (see `edit`).

use crate::block::{Align, Block, BlockKind, Cell, ListItem, ListKind, Run, Script, Table};
use crate::edit::format_entry;
use crate::frontmatter::Value;
use crate::markup::{self, Mark};
use crate::style::LineEnding;

/// Deepest nesting written; deeper blocks are dropped instead of recursing without bound.
const MAX_WRITE_DEPTH: usize = 64;

/// Serialize `blocks` as a page body: blocks separated by one blank line, a final line
/// ending. Line endings are `eol`.
pub fn blocks_to_markdown(blocks: &[Block], eol: LineEnding) -> String {
    eol.normalize(&blocks_lf(blocks, 0))
}

/// Serialize one block (no trailing line ending).
pub fn block_to_markdown(block: &Block, eol: LineEnding) -> String {
    eol.normalize(&block_lf(block, 0))
}

/// Serialize runs as inline markdown.
pub fn runs_to_markdown(runs: &[Run]) -> String {
    inline(runs, false)
}

/// A whole fresh page: optional frontmatter entries, then the body. Entries whose key cannot
/// be written are skipped.
pub fn page_to_markdown(entries: &[(String, Value)], blocks: &[Block], eol: LineEnding) -> String {
    let mut front = String::new();
    for (key, value) in entries {
        if let Ok(text) = format_entry(key, value, LineEnding::Lf) {
            front.push_str(&text);
        }
    }
    let mut out = String::new();
    if !front.is_empty() {
        out.push_str("---\n");
        out.push_str(&front);
        out.push_str("---\n");
    }
    out.push_str(&blocks_lf(blocks, 0));
    eol.normalize(&out)
}

fn blocks_lf(blocks: &[Block], depth: usize) -> String {
    let parts: Vec<String> = blocks
        .iter()
        .map(|b| block_lf(b, depth))
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return String::new();
    }
    let mut out = parts.join("\n\n");
    out.push('\n');
    out
}

fn block_lf(block: &Block, depth: usize) -> String {
    if depth > MAX_WRITE_DEPTH {
        return String::new();
    }
    match &block.kind {
        BlockKind::Heading { level, runs } => {
            let level = usize::from((*level).clamp(1, 6));
            let text = inline(runs, false).replace('\n', " ");
            format!("{} {}", "#".repeat(level), text)
                .trim_end()
                .to_owned()
        }
        BlockKind::Paragraph { runs } => paragraph(&inline(runs, false)),
        BlockKind::List { kind, tight, items } => list(*kind, *tight, items, depth),
        BlockKind::Table(table) => write_table(table),
        BlockKind::Code { info, literal, .. } => code(info, literal),
        BlockKind::Quote { blocks } => {
            let inner = blocks_lf(blocks, depth + 1);
            let inner = inner.trim_end_matches('\n');
            if inner.is_empty() {
                return ">".to_owned();
            }
            inner
                .split('\n')
                .map(|l| {
                    if l.is_empty() {
                        ">".to_owned()
                    } else {
                        format!("> {l}")
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        BlockKind::Rule => "---".to_owned(),
        BlockKind::Html { literal } => literal.trim_end_matches('\n').to_owned(),
    }
}

/// Escape a paragraph's first line if it would read as another kind of block.
fn paragraph(text: &str) -> String {
    let t = text.trim_start();
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    let numbered = digits > 0
        && digits <= 9
        && t.get(digits..).is_some_and(|r| r.starts_with(['.', ')']))
        && t.get(digits + 1..)
            .is_some_and(|r| r.starts_with([' ', '\t']));
    let marker = t.starts_with(['#', '>', '-', '+', '=', '|'])
        || t.starts_with("***")
        || t.starts_with("___")
        || t.starts_with("```")
        || t.starts_with("~~~");
    if numbered {
        if let (Some(head), Some(tail)) = (t.get(..digits), t.get(digits..)) {
            return format!("{head}\\{tail}");
        }
    }
    if marker {
        return format!("\\{t}");
    }
    t.to_owned()
}

fn longest_run(text: &str, target: char) -> usize {
    let (mut longest, mut run) = (0usize, 0usize);
    for c in text.chars() {
        if c == target {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    longest
}

fn code(info: &str, literal: &str) -> String {
    let body = literal.trim_end_matches('\n');
    let fence = "`".repeat((longest_run(body, '`') + 1).max(3));
    let info = info.replace(['\n', '\r', '`'], " ");
    format!("{fence}{}\n{body}\n{fence}", info.trim())
}

fn list(kind: ListKind, tight: bool, items: &[ListItem], depth: usize) -> String {
    let mut out: Vec<String> = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let marker = match kind {
            ListKind::Ordered(first) => format!("{}.", u64::from(first) + i as u64),
            ListKind::Bullet | ListKind::Task => "-".to_owned(),
        };
        let task = match (kind, item.task) {
            (_, Some(true)) => "[x] ",
            (_, Some(false)) | (ListKind::Task, None) => "[ ] ",
            _ => "",
        };
        let body = if tight {
            item.blocks
                .iter()
                .map(|b| block_lf(b, depth + 1))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            blocks_lf(&item.blocks, depth + 1)
                .trim_end_matches('\n')
                .to_owned()
        };
        let indent = " ".repeat(marker.len() + 1);
        let mut text = String::new();
        for (n, line) in format!("{task}{body}").split('\n').enumerate() {
            if n == 0 {
                text.push_str(&format!("{marker} {line}"));
            } else {
                text.push('\n');
                if !line.is_empty() {
                    text.push_str(&indent);
                    text.push_str(line);
                }
            }
        }
        out.push(text.trim_end().to_owned());
    }
    out.join(if tight { "\n" } else { "\n\n" })
}

fn write_table(table: &Table) -> String {
    let cols = table
        .header
        .len()
        .max(table.rows.iter().map(Vec::len).max().unwrap_or(0))
        .max(1);
    let row = |cells: &[Cell]| {
        let mut line = String::from("|");
        for col in 0..cols {
            let text = cells
                .get(col)
                .map(|c| inline(&c.runs, true).replace('\n', " "))
                .unwrap_or_default();
            line.push(' ');
            line.push_str(&text);
            line.push_str(" |");
        }
        line
    };
    let mut lines = vec![row(&table.header)];
    let mut rule = String::from("|");
    for col in 0..cols {
        rule.push_str(match table.align.get(col).copied().unwrap_or(Align::None) {
            Align::None => " --- |",
            Align::Left => " :-- |",
            Align::Center => " :-: |",
            Align::Right => " --: |",
        });
    }
    lines.push(rule);
    for r in &table.rows {
        lines.push(row(r));
    }
    lines.join("\n")
}

fn inline(runs: &[Run], in_cell: bool) -> String {
    let mut out = String::new();
    for run in runs {
        if run.text == "\n" && run.is_plain() {
            out.push_str("\\\n");
            continue;
        }
        let mut text = if run.code {
            code_span(&run.text)
        } else {
            escape(&run.text, in_cell)
        };
        if run.strike {
            text = format!("~~{text}~~");
        }
        text = wrap_html(text, run);
        if run.italic {
            text = format!("*{text}*");
        }
        if run.bold {
            text = format!("**{text}**");
        }
        if let Some(url) = &run.link {
            text = format!("[{text}]({})", escape_url(url));
        }
        out.push_str(&text);
    }
    out
}

/// Wrap `text` in the HTML tags and `==` that `run` needs beyond markdown emphasis.
fn wrap_html(mut text: String, run: &Run) -> String {
    let mut wrap = |mark: Mark, value: Option<&str>| {
        let (open, close) = markup::delimiters(mark, value);
        text = format!("{open}{text}{close}");
    };
    if run.underline {
        wrap(Mark::Underline, None);
    }
    match run.script {
        Script::Sub => wrap(Mark::Subscript, None),
        Script::Sup => wrap(Mark::Superscript, None),
        Script::None => {}
    }
    if let Some(family) = &run.family {
        wrap(Mark::Family, Some(family));
    }
    if let Some(size) = run.size_pt {
        wrap(Mark::Size, Some(&size.to_string()));
    }
    if let Some(color) = run.color {
        wrap(Mark::Color, Some(&markup::color_text(color)));
    }
    if let Some(color) = run.highlight {
        wrap(Mark::Highlight, Some(&markup::color_text(color)));
    }
    text
}

fn code_span(text: &str) -> String {
    let ticks = "`".repeat(longest_run(text, '`') + 1);
    let pad = if text.starts_with('`') || text.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{ticks}{pad}{text}{pad}{ticks}")
}

fn escape(text: &str, in_cell: bool) -> String {
    let mut out = String::with_capacity(text.len());
    // `==` is highlight, so a run of equals signs in text is escaped.
    let has_pair = text.contains("==");
    for c in text.chars() {
        match c {
            '=' if has_pair => out.push_str("\\="),
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '~' => {
                out.push('\\');
                out.push(c);
            }
            '|' if in_cell => out.push_str("\\|"),
            other => out.push(other),
        }
    }
    out
}

fn escape_url(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for c in url.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '(' | ')' | '<' | '>' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' | '\r' => {}
            other => out.push(other),
        }
    }
    out
}
