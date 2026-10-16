//! The formatting at the caret or selection, for the ribbon buttons.

use super::blocks::{align_marker, paragraph_start};
use super::inline::segments;
use super::lines::Prefix;
use super::scan::{scan, SpanInfo};
use super::{fences, in_fence, is_marker, Ctx, Line};
use crate::markup::{self, Mark};
use crate::Align;

/// What kind of list a line is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListState {
    #[default]
    None,
    Bullet,
    Numbered,
    Task,
}

/// The formatting of the text at the caret, or of all the selected text: a mark is on only
/// when every selected character has it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormatState {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: bool,
    pub subscript: bool,
    pub superscript: bool,
    /// Highlight color as `#RRGGBB`.
    pub highlight: Option<String>,
    /// Text color as `#RRGGBB`.
    pub color: Option<String>,
    /// Font size in points.
    pub size: Option<String>,
    pub family: Option<String>,
    /// Heading level 1 to 6, or 0.
    pub heading: u8,
    pub list: ListState,
    pub quote: bool,
    pub code: bool,
    pub align: Align,
}

impl FormatState {
    /// True when `mark` is on (a value mark with any value counts).
    pub fn has(&self, mark: Mark) -> bool {
        match mark {
            Mark::Bold => self.bold,
            Mark::Italic => self.italic,
            Mark::Strike => self.strike,
            Mark::Underline => self.underline,
            Mark::Subscript => self.subscript,
            Mark::Superscript => self.superscript,
            Mark::Highlight => self.highlight.is_some(),
            Mark::Color => self.color.is_some(),
            Mark::Size => self.size.is_some(),
            Mark::Family => self.family.is_some(),
        }
    }
}

/// The innermost span of `mark` that holds the caret at `p`.
fn at_caret(spans: &[SpanInfo], mark: Mark, p: usize) -> Option<&SpanInfo> {
    spans
        .iter()
        .filter(|s| s.mark == mark && s.content().start <= p && p <= s.content().end)
        .max_by_key(|s| s.open.start)
}

/// The innermost span of `mark` that holds `a..b`.
fn around(spans: &[SpanInfo], mark: Mark, a: usize, b: usize) -> Option<&SpanInfo> {
    spans
        .iter()
        .filter(|s| s.mark == mark && s.open.start <= a && b <= s.close.end)
        .max_by_key(|s| s.open.start)
}

/// The formatting at the selection in `ctx`.
pub fn state_at(ctx: &Ctx<'_>) -> FormatState {
    let text = ctx.text;
    let (lo, hi) = ctx.range();
    let line = Line::at(text, lo);
    let s = line.str(text);
    let mut state = FormatState::default();
    if line.start < ctx.floor || is_marker(s) {
        return state;
    }
    let ranges = fences(text, ctx.floor);
    if in_fence(&ranges, line.start) {
        state.code = true;
        return state;
    }
    let prefix = Prefix::parse(s);
    state.heading = prefix.heading.map_or(0, |(level, _)| level);
    state.quote = prefix.quote_depth > 0;
    state.list = match &prefix.list {
        None => ListState::None,
        Some(l) if l.task.is_some() => ListState::Task,
        Some(l) if l.number.is_some() => ListState::Numbered,
        Some(_) => ListState::Bullet,
    };
    let first = paragraph_start(text, line, ctx.floor);
    state.align = align_marker(text, first, ctx.floor).map_or(Align::Left, |(_, a)| a);

    let content = line.start + prefix.content;
    let marks: Vec<(Mark, Option<String>)> = if lo == hi {
        let spans = scan(text, content, line.end);
        Mark::ALL
            .iter()
            .filter_map(|&m| at_caret(&spans, m, lo).map(|s| (m, value_of(m, s))))
            .collect()
    } else {
        let segs = segments(ctx);
        if segs.is_empty() {
            Vec::new()
        } else {
            let spans: Vec<Vec<SpanInfo>> = segs
                .iter()
                .map(|g| scan(text, g.content, g.line.end))
                .collect();
            Mark::ALL
                .iter()
                .filter_map(|&m| {
                    let mut value: Option<Option<String>> = None;
                    for (g, sp) in segs.iter().zip(&spans) {
                        let span = around(sp, m, g.a, g.b)?;
                        let v = value_of(m, span);
                        match &value {
                            None => value = Some(v),
                            Some(prev) if *prev != v => value = Some(None),
                            Some(_) => {}
                        }
                    }
                    value.map(|v| (m, v))
                })
                .collect()
        }
    };
    for (mark, value) in marks {
        match mark {
            Mark::Bold => state.bold = true,
            Mark::Italic => state.italic = true,
            Mark::Strike => state.strike = true,
            Mark::Underline => state.underline = true,
            Mark::Subscript => state.subscript = true,
            Mark::Superscript => state.superscript = true,
            Mark::Highlight => state.highlight = value.or_else(|| Some(default_highlight())),
            Mark::Color => state.color = value,
            Mark::Size => state.size = value,
            Mark::Family => state.family = value,
        }
    }
    state
}

fn default_highlight() -> String {
    markup::color_text(markup::DEFAULT_HIGHLIGHT)
}

fn value_of(mark: Mark, span: &SpanInfo) -> Option<String> {
    if mark == Mark::Highlight {
        return Some(span.value.clone().unwrap_or_else(default_highlight));
    }
    span.value.clone()
}
