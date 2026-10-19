//! Page style in the page file: rule lines, rule line color, page color, and paper size are
//! kept in the `notecraft:` map of the page frontmatter (docs/markdown.md section 3.1).
//!
//! Reading is lenient: a missing, unknown, or malformed value is ignored and the default
//! stays. Writing changes only the keys whose value differs from what the file's own text
//! already reads as, through the minimal-diff frontmatter key rewrite of the markdown edit
//! layer, so a page whose style did not change keeps every byte, and a key goes away when its
//! value returns to the default.

use notecraft_canvas::{PaperSize, RuleLines};
use notecraft_markdown::{parse_frontmatter, Editor};

use super::Session;
use crate::view::PageStyle;

/// `notecraft.paper`: `blank`, `lined`, `grid`, or `dotted`.
const KEY_PAPER: &str = "paper";
/// `notecraft.paper_spacing`: the line spacing within `lined` or `grid` paper.
const KEY_SPACING: &str = "paper_spacing";
/// `notecraft.paper_size`: `letter`, `legal`, `a4`, or `a5` (absent means auto).
const KEY_SIZE: &str = "paper_size";
/// `notecraft.rule_color`: `#RRGGBB` or `#RRGGBBAA`.
const KEY_RULE_COLOR: &str = "rule_color";
/// `notecraft.color`: the page background, `#RRGGBB` or `#RRGGBBAA`.
const KEY_COLOR: &str = "color";

/// The five values of a style as they appear in the file; `None` is the default (no key).
#[derive(Debug, Default, PartialEq, Eq)]
struct Keys {
    paper: Option<&'static str>,
    spacing: Option<&'static str>,
    size: Option<&'static str>,
    rule_color: Option<String>,
    color: Option<String>,
}

/// The spacing a `lined` page has when `paper_spacing` is absent.
const LINED_DEFAULT: RuleLines = RuleLines::College;
/// The spacing a `grid` page has when `paper_spacing` is absent.
const GRID_DEFAULT: RuleLines = RuleLines::GridMedium;

/// The file keys for a rule line style: the paper kind and, when not the default for that
/// kind, the spacing name.
fn rule_keys(rules: RuleLines) -> (Option<&'static str>, Option<&'static str>) {
    let (paper, spacing) = match rules {
        RuleLines::None => return (None, None),
        RuleLines::Narrow => ("lined", "narrow"),
        RuleLines::College => ("lined", "college"),
        RuleLines::Standard => ("lined", "standard"),
        RuleLines::Wide => ("lined", "wide"),
        RuleLines::GridSmall => ("grid", "small"),
        RuleLines::GridMedium => ("grid", "medium"),
        RuleLines::GridLarge => ("grid", "large"),
        RuleLines::GridXLarge => ("grid", "xlarge"),
        RuleLines::Dotted => return (Some("dotted"), None),
    };
    let default = if paper == "lined" {
        LINED_DEFAULT
    } else {
        GRID_DEFAULT
    };
    (Some(paper), (rules != default).then_some(spacing))
}

/// The rule line style a paper kind and spacing name read as; unknown names fall back.
fn rules_from_keys(paper: Option<&str>, spacing: Option<&str>) -> RuleLines {
    let spacing = spacing.map(str::trim);
    match paper.map(str::trim) {
        Some("lined") => match spacing {
            Some("narrow") => RuleLines::Narrow,
            Some("college") => RuleLines::College,
            Some("standard") => RuleLines::Standard,
            Some("wide") => RuleLines::Wide,
            _ => LINED_DEFAULT,
        },
        Some("grid") => match spacing {
            Some("small") => RuleLines::GridSmall,
            Some("medium") => RuleLines::GridMedium,
            Some("large") => RuleLines::GridLarge,
            Some("xlarge") => RuleLines::GridXLarge,
            _ => GRID_DEFAULT,
        },
        Some("dotted") => RuleLines::Dotted,
        _ => RuleLines::None,
    }
}

/// `#RRGGBB` or `#RRGGBBAA`, case-insensitive; anything else is `None`.
fn parse_hex(text: &str) -> Option<[u8; 4]> {
    let digits = text.trim().strip_prefix('#')?;
    if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| {
        digits
            .get(i..i + 2)
            .and_then(|d| u8::from_str_radix(d, 16).ok())
    };
    let alpha = if digits.len() == 8 { byte(6)? } else { u8::MAX };
    Some([byte(0)?, byte(2)?, byte(4)?, alpha])
}

/// `#RRGGBB` when opaque, `#RRGGBBAA` otherwise; uppercase.
fn format_hex([r, g, b, a]: [u8; 4]) -> String {
    if a == u8::MAX {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

fn keys_of(style: &PageStyle) -> Keys {
    let (paper, spacing) = rule_keys(style.rule_lines);
    Keys {
        paper,
        spacing,
        size: (style.paper_size != PaperSize::Auto).then(|| style.paper_size.id()),
        rule_color: style.rule_color.map(format_hex),
        color: style.color.map(format_hex),
    }
}

/// The style the frontmatter of `text` describes. Missing, unknown, and malformed values leave
/// the default.
pub(super) fn style_from_text(text: &str) -> PageStyle {
    let Some(fm) = parse_frontmatter(text) else {
        return PageStyle::default();
    };
    let rule_lines = rules_from_keys(fm.notecraft(KEY_PAPER), fm.notecraft(KEY_SPACING));
    let paper_size = fm
        .notecraft(KEY_SIZE)
        .and_then(|id| PaperSize::from_id(id.trim()))
        .unwrap_or_default();
    PageStyle {
        rule_lines,
        color: fm.notecraft(KEY_COLOR).and_then(parse_hex),
        rule_color: fm.notecraft(KEY_RULE_COLOR).and_then(parse_hex),
        paper_size,
    }
}

/// `text` with the style keys of `style` written: only keys whose value differs from what
/// `text` already reads as are touched. When nothing differs, or the frontmatter cannot be
/// edited (too large, or `notecraft:` is not a one-level map), `text` comes back unchanged.
pub(super) fn text_with_style(text: &str, style: &PageStyle) -> String {
    let have = keys_of(&style_from_text(text));
    let want = keys_of(style);
    if have == want {
        return text.to_owned();
    }
    let mut changes: Vec<(&str, Option<&str>)> = Vec::new();
    if have.paper != want.paper {
        changes.push((KEY_PAPER, want.paper));
    }
    if have.spacing != want.spacing {
        changes.push((KEY_SPACING, want.spacing));
    }
    if have.size != want.size {
        changes.push((KEY_SIZE, want.size));
    }
    if have.rule_color != want.rule_color {
        changes.push((KEY_RULE_COLOR, want.rule_color.as_deref()));
    }
    if have.color != want.color {
        changes.push((KEY_COLOR, want.color.as_deref()));
    }
    let mut editor = Editor::new(text);
    if editor.set_frontmatter_map("notecraft", &changes).is_err() {
        return text.to_owned();
    }
    editor.finish().unwrap_or_else(|_| text.to_owned())
}

impl Session {
    /// Read the style of file page `page_id` from the frontmatter of `text` (the page was just
    /// loaded). Pages with no file, and reads cut short, keep what they have.
    pub(super) fn load_page_style(&mut self, page_id: &str, text: &str, truncated: bool) {
        if truncated || self.page_location(page_id).is_none() {
            return;
        }
        self.set_style_of(page_id, style_from_text(text));
    }
}

#[cfg(test)]
#[path = "page_style_tests.rs"]
mod tests;
