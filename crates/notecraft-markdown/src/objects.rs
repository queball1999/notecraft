//! The pictures and file attachments a page's text holds, in reading order.
//!
//! A placed object (docs/markdown.md section 3.4) names one of these by its link and its
//! number among the occurrences of that link. The canvas lays the page out in the same order,
//! so both sides agree on which occurrence is which.

use notecraft_model::ObjectKind;

use crate::{Block, BlockKind, Document, Run};

/// Most nested levels walked when looking for objects.
const MAX_DEPTH: usize = 32;
/// Most objects listed.
const MAX_SLOTS: usize = 100_000;

/// Whether a link points at a file in the notebook (not the web, an anchor, or mail).
pub fn is_file_link(link: &str) -> bool {
    let link = link.trim();
    if link.is_empty() || link.starts_with('#') || link.starts_with("//") {
        return false;
    }
    match link.split_once(':') {
        Some((scheme, _)) => scheme.len() == 1,
        None => true,
    }
}

/// A paragraph that is one link to a file in the notebook: a file attachment object.
pub fn is_file_paragraph(runs: &[Run]) -> bool {
    matches!(runs, [run] if !run.image && !run.code
        && run.link.as_deref().is_some_and(is_file_link)
        && !run.text.trim().is_empty())
}

/// Every picture and file attachment in `doc`, in reading order, as its kind and link.
pub fn object_slots(doc: &Document) -> Vec<(ObjectKind, String)> {
    let mut out = Vec::new();
    walk(&doc.blocks, 0, &mut out);
    out
}

fn walk(blocks: &[Block], depth: usize, out: &mut Vec<(ObjectKind, String)>) {
    if depth > MAX_DEPTH {
        return;
    }
    for block in blocks {
        if out.len() >= MAX_SLOTS {
            return;
        }
        match &block.kind {
            BlockKind::Paragraph { runs } if is_file_paragraph(runs) => {
                if let Some(link) = runs.first().and_then(|r| r.link.clone()) {
                    out.push((ObjectKind::File, link));
                }
            }
            BlockKind::Paragraph { runs } => {
                for run in runs.iter().filter(|r| r.image) {
                    out.push((ObjectKind::Picture, run.link.clone().unwrap_or_default()));
                }
            }
            BlockKind::Quote { blocks } => walk(blocks, depth + 1, out),
            BlockKind::List { items, .. } => {
                for item in items {
                    walk(&item.blocks, depth + 1, out);
                }
            }
            _ => {}
        }
    }
}

/// How many times `link` occurs as an object of `kind` in `slots`.
pub fn count_of(slots: &[(ObjectKind, String)], kind: ObjectKind, link: &str) -> usize {
    slots
        .iter()
        .filter(|(k, l)| *k == kind && l == link)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn pictures_and_files_are_listed_in_reading_order() {
        let text = "![a](x.png) and ![b](y.png)\n\n[Doc](attachments/d.pdf)\n\n> ![c](x.png)\n\n- [Sheet](s.xlsx)\n- item\n\n[web](https://example.org)\n";
        let slots = object_slots(&parse(text));
        assert_eq!(
            slots,
            vec![
                (ObjectKind::Picture, "x.png".to_owned()),
                (ObjectKind::Picture, "y.png".to_owned()),
                (ObjectKind::File, "attachments/d.pdf".to_owned()),
                (ObjectKind::Picture, "x.png".to_owned()),
                (ObjectKind::File, "s.xlsx".to_owned()),
            ]
        );
        assert_eq!(count_of(&slots, ObjectKind::Picture, "x.png"), 2);
        assert_eq!(count_of(&slots, ObjectKind::File, "x.png"), 0);
    }

    #[test]
    fn hostile_nesting_is_bounded() {
        let text = format!("{}![a](x.png)\n", "> ".repeat(500));
        let _ = object_slots(&parse(&text));
        let text = "![a](x.png) ".repeat(50_000);
        assert!(object_slots(&parse(&text)).len() <= MAX_SLOTS);
    }

    #[test]
    fn link_kinds() {
        assert!(is_file_link("attachments/a.pdf") && is_file_link("C:/a.pdf"));
        assert!(!is_file_link("https://a.org") && !is_file_link("#x") && !is_file_link("//s/x"));
    }
}
