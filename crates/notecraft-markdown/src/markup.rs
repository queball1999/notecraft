//! The inline markup NoteCraft writes for text formatting (docs/markdown.md section 3.2), in
//! one place so reading, writing, and the formatting commands agree.
//!
//! Markdown has syntax for bold, italic, and strikethrough. For the rest the format spec
//! chooses: highlight is `==text==` (yellow) or `<mark style="background:#RRGGBB">`; underline,
//! subscript, and superscript are `<u>`, `<sub>`, and `<sup>`; text color, font size, and font
//! family are `<span style="color:#RRGGBB">`, `<span style="font-size:12pt">`, and
//! `<span style="font-family:Name">`. Paragraph alignment is a comment line above the
//! paragraph (`<!-- notecraft:align=center -->`).

use crate::Align;

/// The highlight color `==text==` stands for, as 0xRRGGBB.
pub const DEFAULT_HIGHLIGHT: u32 = 0x00FF_F176;

/// Longest HTML tag the reader looks at, in bytes.
const MAX_TAG: usize = 240;
/// Longest font family name written, in bytes.
pub const MAX_FAMILY: usize = 64;

/// One kind of inline formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Bold,
    Italic,
    Strike,
    Underline,
    Subscript,
    Superscript,
    Highlight,
    Color,
    Size,
    Family,
}

impl Mark {
    /// Every mark, in the order commands list them.
    pub const ALL: [Mark; 10] = [
        Mark::Bold,
        Mark::Italic,
        Mark::Strike,
        Mark::Underline,
        Mark::Subscript,
        Mark::Superscript,
        Mark::Highlight,
        Mark::Color,
        Mark::Size,
        Mark::Family,
    ];

    /// True for marks that carry a value (a color, size, or family).
    pub const fn has_value(self) -> bool {
        matches!(
            self,
            Mark::Highlight | Mark::Color | Mark::Size | Mark::Family
        )
    }
}

/// An HTML tag the formatting vocabulary knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tag {
    /// An opening tag and its value (a normalized color, a size in points, or a family name).
    Open(Mark, Option<String>),
    /// A closing tag of a fixed name (`</u>`, `</sub>`, `</sup>`, `</mark>`).
    Close(Mark),
    /// `</span>`: closes the innermost open span (color, size, or family).
    CloseSpan,
}

/// `#RRGGBB` (also `#RGB` and `#RRGGBBAA`, whose alpha is dropped) as an uppercase `#RRGGBB`.
pub fn normalize_color(text: &str) -> Option<String> {
    let hex = text.trim().strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let full = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => hex.to_owned(),
        8 => hex.get(..6)?.to_owned(),
        _ => return None,
    };
    Some(format!("#{}", full.to_ascii_uppercase()))
}

/// `#RRGGBB` as 0xRRGGBB.
pub fn color_value(text: &str) -> Option<u32> {
    let norm = normalize_color(text)?;
    u32::from_str_radix(norm.get(1..)?, 16).ok()
}

/// 0xRRGGBB as `#RRGGBB`.
pub fn color_text(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0x00FF_FFFF)
}

/// A font size in points, as digits (whole points, 1 to 1638).
pub fn normalize_size(text: &str) -> Option<String> {
    let text = text.trim().trim_end_matches("pt").trim();
    let value: f64 = text.parse().ok()?;
    if !value.is_finite() || !(1.0..=1638.0).contains(&value) {
        return None;
    }
    Some(format!("{}", value.round() as u32))
}

/// A font family name that is safe to write inside a style attribute, or `None`.
pub fn normalize_family(text: &str) -> Option<String> {
    let name = text.trim().trim_matches(['"', '\'']).trim();
    let ok = !name.is_empty()
        && name.len() <= MAX_FAMILY
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'));
    ok.then(|| name.to_owned())
}

/// Normalize `value` for `mark`, or `None` when it is not a value the mark can carry.
pub fn normalize_value(mark: Mark, value: &str) -> Option<String> {
    match mark {
        Mark::Highlight | Mark::Color => normalize_color(value),
        Mark::Size => normalize_size(value),
        Mark::Family => normalize_family(value),
        _ => None,
    }
}

/// The text that opens and closes `mark` around text. `value` is the color, size, or family
/// (already normalized). A highlight with no value, or the default yellow, is `==`.
pub fn delimiters(mark: Mark, value: Option<&str>) -> (String, String) {
    let pair = |open: &str, close: &str| (open.to_owned(), close.to_owned());
    match mark {
        Mark::Bold => pair("**", "**"),
        Mark::Italic => pair("*", "*"),
        Mark::Strike => pair("~~", "~~"),
        Mark::Underline => pair("<u>", "</u>"),
        Mark::Subscript => pair("<sub>", "</sub>"),
        Mark::Superscript => pair("<sup>", "</sup>"),
        Mark::Highlight => match value {
            Some(color) if color_value(color) != Some(DEFAULT_HIGHLIGHT) => (
                format!("<mark style=\"background:{color}\">"),
                "</mark>".to_owned(),
            ),
            _ => pair("==", "=="),
        },
        Mark::Color => (
            format!("<span style=\"color:{}\">", value.unwrap_or("#000000")),
            "</span>".to_owned(),
        ),
        Mark::Size => (
            format!("<span style=\"font-size:{}pt\">", value.unwrap_or("11")),
            "</span>".to_owned(),
        ),
        Mark::Family => (
            format!(
                "<span style=\"font-family:{}\">",
                value.unwrap_or("Calibri")
            ),
            "</span>".to_owned(),
        ),
    }
}

/// The value of the CSS property `name` in a `style="..."` attribute text.
fn style_property<'a>(style: &'a str, names: &[&str]) -> Option<&'a str> {
    style.split(';').find_map(|part| {
        let (key, value) = part.split_once(':')?;
        let key = key.trim();
        names
            .iter()
            .any(|n| key.eq_ignore_ascii_case(n))
            .then_some(value.trim())
    })
}

/// The `style` attribute of a tag's attribute text.
fn style_attribute(attrs: &str) -> Option<&str> {
    let lower = attrs.to_ascii_lowercase();
    let at = lower.find("style")?;
    let rest = attrs
        .get(at + 5..)?
        .trim_start()
        .strip_prefix('=')?
        .trim_start();
    let quote = rest.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let body = rest.get(1..)?;
    let end = body.find(quote)?;
    body.get(..end)
}

/// Read one formatting tag at the start of `text` (which starts with `<`), returning it and
/// its length in bytes. Anything else, including tags with attributes NoteCraft does not
/// write, is `None`, so unknown HTML stays plain text.
pub fn parse_tag(text: &str) -> Option<(Tag, usize)> {
    let head = text.get(..text.len().min(MAX_TAG))?;
    let head = match head.char_indices().find(|&(_, c)| c == '>') {
        Some((i, _)) => head.get(..=i)?,
        None => return None,
    };
    let inner = head.strip_prefix('<')?.strip_suffix('>')?;
    let len = head.len();
    if let Some(name) = inner.strip_prefix('/') {
        let tag = match name.trim().to_ascii_lowercase().as_str() {
            "u" => Tag::Close(Mark::Underline),
            "sub" => Tag::Close(Mark::Subscript),
            "sup" => Tag::Close(Mark::Superscript),
            "mark" => Tag::Close(Mark::Highlight),
            "span" => Tag::CloseSpan,
            _ => return None,
        };
        return Some((tag, len));
    }
    let (name, attrs) = match inner.split_once(char::is_whitespace) {
        Some((name, attrs)) => (name, attrs),
        None => (inner, ""),
    };
    let attrs = attrs.trim();
    let tag = match name.to_ascii_lowercase().as_str() {
        "u" if attrs.is_empty() => Tag::Open(Mark::Underline, None),
        "sub" if attrs.is_empty() => Tag::Open(Mark::Subscript, None),
        "sup" if attrs.is_empty() => Tag::Open(Mark::Superscript, None),
        "mark" if attrs.is_empty() => Tag::Open(Mark::Highlight, None),
        "mark" => {
            let style = style_attribute(attrs)?;
            let color = style_property(style, &["background", "background-color"])?;
            Tag::Open(Mark::Highlight, Some(normalize_color(color)?))
        }
        "span" => {
            let style = style_attribute(attrs)?;
            if let Some(color) = style_property(style, &["color"]) {
                Tag::Open(Mark::Color, Some(normalize_color(color)?))
            } else if let Some(size) = style_property(style, &["font-size"]) {
                Tag::Open(Mark::Size, Some(normalize_size(size)?))
            } else {
                let family = style_property(style, &["font-family"])?;
                Tag::Open(Mark::Family, Some(normalize_family(family)?))
            }
        }
        _ => return None,
    };
    Some((tag, len))
}

/// The prefix and suffix of a paragraph alignment comment.
const ALIGN_OPEN: &str = "<!-- notecraft:align=";
const ALIGN_CLOSE: &str = " -->";

/// The comment line (without a line ending) that aligns the paragraph below it. Left is the
/// default and has no comment.
pub fn align_comment(align: Align) -> Option<String> {
    let name = match align {
        Align::Center => "center",
        Align::Right => "right",
        Align::Left | Align::None => return None,
    };
    Some(format!("{ALIGN_OPEN}{name}{ALIGN_CLOSE}"))
}

/// The alignment a comment line sets, if `line` is an alignment comment.
pub fn parse_align_comment(line: &str) -> Option<Align> {
    let inner = line
        .trim()
        .strip_prefix("<!--")?
        .strip_suffix("-->")?
        .trim()
        .strip_prefix("notecraft:align=")?;
    match inner.trim() {
        "left" => Some(Align::Left),
        "center" => Some(Align::Center),
        "right" => Some(Align::Right),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_normalize() {
        assert_eq!(normalize_color("#abc").as_deref(), Some("#AABBCC"));
        assert_eq!(normalize_color(" #1f3a93ff ").as_deref(), Some("#1F3A93"));
        assert_eq!(normalize_color("red"), None);
        assert_eq!(normalize_color("#12345"), None);
        assert_eq!(color_value("#FFF176"), Some(DEFAULT_HIGHLIGHT));
    }

    #[test]
    fn tags_round_trip_through_delimiters() {
        for (mark, value) in [
            (Mark::Underline, None),
            (Mark::Subscript, None),
            (Mark::Superscript, None),
            (Mark::Highlight, Some("#112233")),
            (Mark::Color, Some("#E53935")),
            (Mark::Size, Some("14")),
            (Mark::Family, Some("Courier New")),
        ] {
            let (open, close) = delimiters(mark, value);
            let (tag, len) = parse_tag(&open).expect("open tag");
            assert_eq!(len, open.len());
            assert_eq!(tag, Tag::Open(mark, value.map(str::to_owned)), "{open}");
            let (closing, len) = parse_tag(&close).expect("close tag");
            assert_eq!(len, close.len());
            assert!(matches!(closing, Tag::Close(_) | Tag::CloseSpan));
        }
    }

    #[test]
    fn unknown_html_is_not_a_tag() {
        assert_eq!(parse_tag("<b>"), None);
        assert_eq!(parse_tag("<span class=\"x\">"), None);
        assert_eq!(parse_tag("<span style=\"color:red\">"), None);
        assert_eq!(parse_tag("<u onclick=\"x\">"), None);
        assert_eq!(parse_tag("<u"), None);
    }

    #[test]
    fn the_default_highlight_is_the_equals_form() {
        assert_eq!(
            delimiters(Mark::Highlight, Some("#FFF176")),
            ("==".to_owned(), "==".to_owned())
        );
        assert_eq!(delimiters(Mark::Highlight, None).0, "==");
    }

    #[test]
    fn align_comments_parse() {
        let line = align_comment(Align::Center).expect("comment");
        assert_eq!(parse_align_comment(&line), Some(Align::Center));
        assert_eq!(align_comment(Align::Left), None);
        assert_eq!(parse_align_comment("<!-- other -->"), None);
    }
}
