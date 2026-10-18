//! The display width of a picture (docs/markdown.md section 2.4).
//!
//! A picture shows at its natural size unless the user resized it. The width, in page units
//! (one unit is one pixel of the picture at 100% zoom), is kept in an HTML comment right after
//! the image: `![alt](attachments/a1b2c3-photo.png)<!-- notecraft:width=320 -->`. Other
//! markdown readers ignore the comment and show the picture at its natural size. The height
//! always follows from the picture's own proportions.
//!
//! Reading is lenient: a comment that is not exactly this form, or whose number is out of
//! range, is left as plain inline HTML. Writing is one small [`Edit`] that inserts, replaces,
//! or removes just the comment.

use crate::block::Span;
use crate::container::{closes, fence_of};
use crate::edit::Edit;

/// Narrowest display width, in page units.
pub const MIN_WIDTH: u16 = 16;
/// Widest display width, in page units.
pub const MAX_WIDTH: u16 = 4096;
/// Most images looked at in one page when searching for the one to resize.
const MAX_IMAGES: usize = 20_000;

const PREFIX: &str = "notecraft:width=";

/// The comment that records a display width of `width` units (clamped to the allowed range).
pub fn width_comment(width: u16) -> String {
    format!("<!-- {PREFIX}{} -->", width.clamp(MIN_WIDTH, MAX_WIDTH))
}

/// The width in `html` if it is exactly one width comment.
pub fn parse_width_comment(html: &str) -> Option<u16> {
    let (width, len) = width_at(html)?;
    (len == html.len()).then_some(width)
}

/// The width comment at the start of `text` and its length in bytes.
fn width_at(text: &str) -> Option<(u16, usize)> {
    let rest = text.strip_prefix("<!--")?;
    let end = rest.get(..64.min(rest.len()))?.find("-->")?;
    let inner = rest.get(..end)?.trim();
    let digits = inner.strip_prefix(PREFIX)?;
    if digits.is_empty() || digits.len() > 5 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let width: u16 = digits.parse().ok()?;
    if !(MIN_WIDTH..=MAX_WIDTH).contains(&width) {
        return None;
    }
    Some((width, "<!--".len() + end + "-->".len()))
}

/// Where the image `![..](dest)` that starts at `from` in `line` ends: the byte after its `)`,
/// and the destination text.
fn image_end(line: &str, from: usize) -> Option<(usize, &str)> {
    let open = line.get(from..)?.find("](")? + from + 2;
    let rest = line.get(open..)?;
    let rest_trim = rest.trim_start();
    let skipped = rest.len() - rest_trim.len();
    let (dest, after) = if let Some(inner) = rest_trim.strip_prefix('<') {
        let close = inner.find('>')?;
        (inner.get(..close)?, open + skipped + 1 + close + 1)
    } else {
        let stop = rest_trim.find([')', ' ', '\t'])?;
        (rest_trim.get(..stop)?, open + skipped + stop)
    };
    // A title may follow the destination; the image ends at the next `)`.
    let close = line.get(after..)?.find(')')? + after + 1;
    Some((close, dest))
}

/// The edit that records `width` for the `nth` image (counting from 0, in page order) whose
/// destination is `link`, or removes the recorded width when `width` is `None`. `None` if the
/// page has no such image. The edit may change nothing (the width was already that); compare
/// the text of its span with its replacement to tell. Images in fenced code are not counted.
pub fn set_image_width(text: &str, link: &str, nth: usize, width: Option<u16>) -> Option<Edit> {
    let mut seen = 0;
    let mut inspected = 0;
    let mut offset = 0;
    let mut open: Option<(u8, usize)> = None;
    for line in text.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();
        let body = line.trim_end_matches(['\n', '\r']);
        match open {
            Some(fence) => {
                if closes(fence, body) {
                    open = None;
                }
                continue;
            }
            None => {
                if let Some(fence) = fence_of(body) {
                    open = Some(fence);
                    continue;
                }
            }
        }
        let mut from = 0;
        while let Some(at) = body.get(from..).and_then(|s| s.find("![")) {
            let start = from + at;
            from = start + 2;
            let Some((end, dest)) = image_end(body, from) else {
                break;
            };
            inspected += 1;
            if inspected > MAX_IMAGES {
                return None;
            }
            if dest != link {
                continue;
            }
            if seen != nth {
                seen += 1;
                continue;
            }
            let after = line_start + end;
            let existing = body
                .get(end..)
                .and_then(width_at)
                .map(|(_, len)| Span::new(after, after + len));
            return Some(match (existing, width) {
                (Some(span), Some(w)) => Edit::new(span, width_comment(w)),
                (Some(span), None) => Edit::new(span, ""),
                (None, Some(w)) => Edit::new(Span::new(after, after), width_comment(w)),
                (None, None) => Edit::new(Span::new(after, after), ""),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{apply_edits, parse_blocks, BlockKind};

    fn apply(text: &str, edit: Edit) -> String {
        apply_edits(text, std::slice::from_ref(&edit)).expect("apply")
    }

    #[test]
    fn the_comment_round_trips_and_is_strict() {
        assert_eq!(width_comment(320), "<!-- notecraft:width=320 -->");
        assert_eq!(
            parse_width_comment("<!-- notecraft:width=320 -->"),
            Some(320)
        );
        assert_eq!(parse_width_comment("<!--notecraft:width=16-->"), Some(16));
        for bad in [
            "<!-- notecraft:width=15 -->",
            "<!-- notecraft:width=4097 -->",
            "<!-- notecraft:width=99999999 -->",
            "<!-- notecraft:width= -->",
            "<!-- notecraft:width=3x -->",
            "<!-- other -->",
            "<!-- notecraft:width=320 --> tail",
            "<!-- notecraft:width=320",
        ] {
            assert_eq!(parse_width_comment(bad), None, "{bad}");
        }
        assert_eq!(width_comment(1), "<!-- notecraft:width=16 -->");
        assert_eq!(width_comment(60_000), "<!-- notecraft:width=4096 -->");
    }

    #[test]
    fn a_width_is_added_replaced_and_removed_with_a_minimal_edit() {
        let text = "# A\n\n![pic](attachments/a.png)\n\nend\n";
        let edit = set_image_width(text, "attachments/a.png", 0, Some(300)).expect("found");
        assert_eq!(edit.span.start, edit.span.end);
        let with = apply(text, edit);
        assert_eq!(
            with,
            "# A\n\n![pic](attachments/a.png)<!-- notecraft:width=300 -->\n\nend\n"
        );
        let edit = set_image_width(&with, "attachments/a.png", 0, Some(450)).expect("found");
        let again = apply(&with, edit);
        assert!(again.contains("width=450 -->") && !again.contains("width=300"));
        let edit = set_image_width(&again, "attachments/a.png", 0, None).expect("found");
        assert_eq!(apply(&again, edit), text);
    }

    #[test]
    fn the_nth_image_with_the_same_link_is_chosen() {
        let text = "![a](x.png) and ![b](x.png)<!-- notecraft:width=100 -->\n\n![c](x.png)\n";
        let edit = set_image_width(text, "x.png", 2, Some(50)).expect("third");
        assert!(apply(text, edit).ends_with("![c](x.png)<!-- notecraft:width=50 -->\n"));
        let edit = set_image_width(text, "x.png", 1, Some(200)).expect("second");
        assert!(apply(text, edit).contains("![b](x.png)<!-- notecraft:width=200 -->\n"));
        assert!(set_image_width(text, "x.png", 3, Some(50)).is_none());
        assert!(set_image_width(text, "y.png", 0, Some(50)).is_none());
    }

    #[test]
    fn crlf_titles_angle_links_and_fences() {
        let text = "```\n![f](x.png)\n```\r\n![t](x.png \"title\")\r\nnext\r\n";
        let edit = set_image_width(text, "x.png", 0, Some(80)).expect("found");
        assert_eq!(
            apply(text, edit),
            "```\n![f](x.png)\n```\r\n![t](x.png \"title\")<!-- notecraft:width=80 -->\r\nnext\r\n"
        );
        let text = "![s](<my pic.png>)\n";
        let edit = set_image_width(text, "my pic.png", 0, Some(80)).expect("angle");
        assert_eq!(
            apply(text, edit),
            "![s](<my pic.png>)<!-- notecraft:width=80 -->\n"
        );
    }

    #[test]
    fn hostile_text_is_bounded() {
        let text = "![".repeat(100_000);
        assert!(set_image_width(&text, "x.png", 0, Some(80)).is_none());
        let text = "![a](x.png) ".repeat(30_000);
        let _ = set_image_width(&text, "x.png", 29_999, Some(80));
    }

    #[test]
    fn the_parser_reads_the_width_after_an_image() {
        let blocks = parse_blocks("![a](x.png)<!-- notecraft:width=300 --> tail\n", 0);
        let Some(BlockKind::Paragraph { runs }) = blocks.first().map(|b| &b.kind) else {
            panic!("paragraph");
        };
        assert_eq!(runs.first().and_then(|r| r.image_width), Some(300));
        assert_eq!(runs.len(), 2, "{runs:?}");
        assert_eq!(runs.get(1).map(|r| r.text.as_str()), Some(" tail"));
        // A comment that is not ours stays as text.
        let blocks = parse_blocks("![a](x.png)<!-- hi -->\n", 0);
        let Some(BlockKind::Paragraph { runs }) = blocks.first().map(|b| &b.kind) else {
            panic!("paragraph");
        };
        assert_eq!(runs.first().and_then(|r| r.image_width), None);
    }
}
