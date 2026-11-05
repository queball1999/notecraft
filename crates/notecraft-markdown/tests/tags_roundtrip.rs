//! Note tags survive parse and write byte for byte (docs/markdown.md section 4), both by the
//! span-replacement writer and by the canonical serializer.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use notecraft_markdown::tags::{scan_tags, tagged_items};
use notecraft_markdown::{
    block_to_markdown, page_to_markdown, parse, BlockKind, Editor, LineEnding, Span,
};

const PAGE: &str = include_str!("../../../testdata/markdown/corpus/note-tags.md");

#[test]
fn the_writer_keeps_a_tagged_page_exactly() {
    let doc = parse(PAGE);
    assert_eq!(
        Editor::new(PAGE).finish().expect("no edits"),
        PAGE,
        "an untouched page is written back as it was"
    );
    // Replacing each tagged paragraph with its own source is a no-op.
    let mut editor = Editor::new(PAGE);
    for item in tagged_items(PAGE, &doc) {
        editor
            .replace(
                Span::new(item.holder.start, item.holder.end),
                item.holder.slice(PAGE),
            )
            .expect("replace");
    }
    assert_eq!(editor.finish().expect("finish"), PAGE);
}

#[test]
fn the_page_has_the_tags_it_should() {
    let doc = parse(PAGE);
    let items = tagged_items(PAGE, &doc);
    let found: Vec<(Vec<String>, Option<bool>)> = items
        .iter()
        .map(|i| {
            (
                i.tags.iter().map(|t| t.name.clone()).collect(),
                i.task.map(|t| t.checked),
            )
        })
        .collect();
    let names = |n: &[&str]| n.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        found,
        vec![
            (names(&["important"]), None),
            (names(&["question", "idea"]), None),
            (names(&["project_x", "a-b"]), None),
            (names(&["important"]), Some(false)),
            (names(&[]), Some(true)),
            (names(&[]), Some(false)),
            (names(&["phone"]), Some(false)),
            (names(&["contact"]), None),
            (names(&["definition"]), None),
            (names(&["remember"]), None),
        ]
    );
}

#[test]
fn the_canonical_writer_keeps_tagged_paragraphs() {
    let doc = parse(PAGE);
    for block in &doc.blocks {
        let source = block.source(PAGE);
        if let BlockKind::Paragraph { .. } = &block.kind {
            if scan_tags(source, 0).is_empty() {
                continue;
            }
            let written = block_to_markdown(block, LineEnding::Lf);
            assert_eq!(written, source, "paragraph changed when written");
        }
    }
}

#[test]
fn a_rewritten_page_reads_the_same_tags() {
    let doc = parse(PAGE);
    let before: Vec<_> = tagged_items(PAGE, &doc)
        .into_iter()
        .map(|i| {
            (
                i.tags.into_iter().map(|t| t.name).collect::<Vec<_>>(),
                i.task.map(|t| t.checked),
                i.text,
            )
        })
        .collect();
    let written = page_to_markdown(&[], &doc.blocks, LineEnding::Lf);
    let again = parse(&written);
    let after: Vec<_> = tagged_items(&written, &again)
        .into_iter()
        .map(|i| {
            (
                i.tags.into_iter().map(|t| t.name).collect::<Vec<_>>(),
                i.task.map(|t| t.checked),
                i.text,
            )
        })
        .collect();
    assert_eq!(before, after);
}
