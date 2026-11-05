//! How note tags look on the canvas (PR 2.5): the `#name` text of a known tag is taken out of
//! the paragraph and drawn as an icon at the start of it; the highlight-style tags (Remember
//! for later, Definition, Highlight) and tags with colors paint the paragraph text instead.
//! A paragraph the caret is in shows its raw markdown, tag names included.

use std::borrow::Cow;

use notecraft_markdown::tags::{strip_tags, TaggedItem};
use notecraft_markdown::{Run, Span, TagIcon, TagSet, TagStyle};

/// The tags of the page being laid out: the gallery and the tagged paragraphs, sorted by
/// where they start.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TagInput<'a> {
    pub set: &'a TagSet,
    pub items: &'a [TaggedItem],
}

/// An icon to draw at the start of a paragraph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Mark {
    pub icon: TagIcon,
    /// 0xRRGGBB.
    pub color: u32,
}

/// The runs of the paragraph or heading at `holder` as they are shown, and its icons. With no
/// tags in it the runs come back as they are.
pub(crate) fn apply<'r>(
    input: Option<TagInput<'_>>,
    holder: Span,
    runs: &'r [Run],
) -> (Cow<'r, [Run]>, Vec<Mark>) {
    let Some(input) = input else {
        return (Cow::Borrowed(runs), Vec::new());
    };
    let Some(item) = input
        .items
        .iter()
        .find(|item| item.holder == holder && !item.tags.is_empty())
    else {
        return (Cow::Borrowed(runs), Vec::new());
    };
    let known = |name: &str| input.set.get(name).is_some();
    let Some((mut shown, removed)) = strip_tags(runs, &item.tags, &known) else {
        return (Cow::Borrowed(runs), Vec::new());
    };
    let mut marks: Vec<Mark> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    let (mut highlight, mut font) = (None, None);
    for name in &removed {
        if seen.contains(&name.as_str()) {
            continue;
        }
        seen.push(name);
        let Some(def) = input.set.get(name) else {
            continue;
        };
        if def.style != TagStyle::Highlight {
            marks.push(Mark {
                icon: def.icon,
                color: def.icon_color,
            });
        }
        highlight = highlight.or(def.highlight);
        font = font.or(def.font_color);
    }
    if shown.iter().all(|run| run.text.is_empty()) {
        // A tag alone on its line: keep a space so the line has a height.
        shown = vec![Run::plain(" ")];
    }
    for run in &mut shown {
        if run.image {
            continue;
        }
        if run.highlight.is_none() {
            run.highlight = highlight;
        }
        if run.color.is_none() && run.link.is_none() {
            run.color = font;
        }
    }
    (Cow::Owned(shown), marks)
}

#[cfg(test)]
mod tests {
    use notecraft_markdown::tags::tagged_items;

    use super::*;

    fn shown(text: &str, set: &TagSet) -> (Vec<String>, Vec<TagIcon>, Vec<Option<u32>>) {
        let doc = notecraft_markdown::parse(text);
        let items = tagged_items(text, &doc);
        let notecraft_markdown::BlockKind::Paragraph { runs } = &doc.blocks[0].kind else {
            panic!("paragraph");
        };
        let input = TagInput { set, items: &items };
        let (runs, marks) = apply(Some(input), doc.blocks[0].span, runs);
        (
            runs.iter().map(|r| r.text.clone()).collect(),
            marks.iter().map(|m| m.icon).collect(),
            runs.iter().map(|r| r.highlight).collect(),
        )
    }

    #[test]
    fn known_tags_become_icons_and_leave_the_text() {
        let set = TagSet::default();
        let (text, icons, _) = shown("#important #question Call Ada\n", &set);
        assert_eq!(text, ["Call Ada"]);
        assert_eq!(icons, [TagIcon::Star, TagIcon::Question]);
    }

    #[test]
    fn unknown_tags_stay_as_text() {
        let set = TagSet::default();
        let (text, icons, _) = shown("#important #whatever Call\n", &set);
        assert_eq!(text, ["#whatever Call"]);
        assert_eq!(icons, [TagIcon::Star]);
    }

    #[test]
    fn highlight_tags_paint_the_text_and_draw_no_icon() {
        let set = TagSet::default();
        let (text, icons, highlight) = shown("#highlight Look here\n", &set);
        assert_eq!(text, ["Look here"]);
        assert!(icons.is_empty());
        assert_eq!(highlight, [Some(0xFF_F1_76)]);
    }

    #[test]
    fn a_tag_alone_keeps_a_line() {
        let set = TagSet::default();
        let (text, icons, _) = shown("#idea\n", &set);
        assert_eq!(text, [" "]);
        assert_eq!(icons, [TagIcon::Bulb]);
    }

    #[test]
    fn custom_colors_apply_to_the_text() {
        let mut set = TagSet::default();
        let slug = set
            .add_custom("Errand", TagIcon::Pin, Some(0x112233), Some(0x445566))
            .unwrap();
        let text = format!("#{slug} Pick up\n");
        let doc = notecraft_markdown::parse(&text);
        let items = tagged_items(&text, &doc);
        let notecraft_markdown::BlockKind::Paragraph { runs } = &doc.blocks[0].kind else {
            panic!("paragraph");
        };
        let (runs, marks) = apply(
            Some(TagInput {
                set: &set,
                items: &items,
            }),
            doc.blocks[0].span,
            runs,
        );
        assert_eq!(marks.len(), 1);
        assert_eq!(runs[0].color, Some(0x112233));
        assert_eq!(runs[0].highlight, Some(0x445566));
    }
}
