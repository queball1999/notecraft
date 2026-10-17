//! Checks run by the markdown fuzz targets and by their stable property-test twins in
//! `crates/notecraft-markdown/tests/fuzz_props.rs`. Each takes arbitrary bytes, never panics
//! on any input, and asserts the limits in docs/markdown.md section 9.

#![allow(dead_code)]

use notecraft_markdown::frontmatter::{MAX_ENTRIES, MAX_FRONTMATTER_BYTES, MAX_ITEMS};
use notecraft_markdown::{
    apply_edits, fix_container_ids, is_valid_id, parse, parse_frontmatter, read_containers,
    remove_container_markers, Block, BlockKind, Editor, Span, Value, MAX_CONTAINERS, MAX_DEPTH,
};

fn text_of(data: &[u8]) -> String {
    String::from_utf8_lossy(data).into_owned()
}

fn check_span(span: Span, text: &str) {
    assert!(span.start <= span.end, "inverted span {span:?}");
    assert!(span.end <= text.len(), "span {span:?} past {}", text.len());
    assert!(
        text.get(span.start..span.end).is_some(),
        "span {span:?} not on char boundaries"
    );
}

fn walk(blocks: &[Block], text: &str, depth: usize, deepest: &mut usize) {
    *deepest = (*deepest).max(depth);
    for block in blocks {
        check_span(block.span, text);
        match &block.kind {
            BlockKind::Heading { level, .. } => assert!((1..=6).contains(level)),
            BlockKind::Quote { blocks } => walk(blocks, text, depth + 1, deepest),
            BlockKind::List { items, .. } => {
                for item in items {
                    check_span(item.span, text);
                    walk(&item.blocks, text, depth + 1, deepest);
                }
            }
            BlockKind::Table(table) => {
                for cell in table.header.iter().chain(table.rows.iter().flatten()) {
                    check_span(cell.span, text);
                }
            }
            _ => {}
        }
    }
}

/// Markdown blocks: parse a whole page. Spans stay inside the text, nesting stays within
/// the depth limit (blocks deeper than it are plain paragraphs), plain text stays capped.
pub fn check_markdown(data: &[u8]) {
    let text = text_of(data);
    let doc = parse(&text);
    assert!(doc.body_start <= text.len());
    let mut deepest = 0;
    walk(&doc.blocks, &text, 1, &mut deepest);
    assert!(
        deepest <= MAX_DEPTH + 1,
        "nesting {deepest} over the limit {MAX_DEPTH}"
    );
    assert!(doc.plain_text().len() <= 16 * 1024);
    // The editor with no edits writes the page back byte for byte.
    assert_eq!(apply_edits(&text, &[]).ok().as_deref(), Some(text.as_str()));
    let editor = Editor::new(&text);
    assert_eq!(editor.finish().ok().as_deref(), Some(text.as_str()));
    // Edits through the editor never panic, and what they write parses again.
    let mut editor = Editor::new(&text);
    if let Some(first) = doc.blocks.first() {
        let _ = editor.insert_after(first, "inserted\n");
    }
    let _ = editor.set_frontmatter("title", &Value::Scalar("Fuzz".into()));
    let _ = editor.remove_frontmatter("tags");
    if let Ok(out) = editor.finish() {
        let _ = parse(&out);
    }
}

fn check_value(value: &Value) {
    match value {
        Value::List(items) => assert!(items.len() <= MAX_ITEMS),
        Value::Map(items) => assert!(items.len() <= MAX_ITEMS),
        _ => {}
    }
}

/// Frontmatter: entries and spans stay inside the page, the entry count is capped, and a
/// block over 64 KiB is kept as text with its keys ignored.
pub fn check_frontmatter(data: &[u8]) {
    let text = text_of(data);
    let Some(fm) = parse_frontmatter(&text) else {
        return;
    };
    check_span(fm.span, &text);
    assert!(fm.entries.len() <= MAX_ENTRIES);
    if fm.oversize {
        assert!(fm.entries.is_empty());
    } else {
        assert!(fm.span.len() <= MAX_FRONTMATTER_BYTES + 16);
    }
    for entry in &fm.entries {
        check_span(entry.span, &text);
        assert!(entry.span.start >= fm.span.start && entry.span.end <= fm.span.end);
        check_value(&entry.value);
    }
    let _ = (
        fm.title(),
        fm.tags(),
        fm.aliases(),
        fm.scalar("x"),
        fm.list("x"),
    );
}

/// Container markers: at most 10,000 markers read, spans inside the page, and the id fix
/// leaves every marker with a valid, unique id that is not `c0`.
pub fn check_containers(data: &[u8]) {
    let text = text_of(data);
    let containers = read_containers(&text, text.len());
    assert!(containers.len() <= MAX_CONTAINERS + 1);
    for c in &containers {
        check_span(c.span, &text);
        if let Some(m) = c.marker {
            check_span(m, &text);
        }
        if let Some(i) = c.id_span {
            check_span(i, &text);
        }
        let _ = c.content(&text);
    }
    // A limit in the middle of the text, or past its end, must not panic either.
    let _ = read_containers(&text, text.len() / 2);
    let _ = read_containers(&text, usize::MAX);

    let edits = fix_container_ids(&text, text.len());
    let fixed = apply_edits(&text, &edits);
    assert!(fixed.is_ok(), "id fix edits must apply");
    let fixed = fixed.unwrap_or_default();
    let after = read_containers(&fixed, fixed.len());
    let mut seen = std::collections::HashSet::new();
    for c in after.iter().filter(|c| c.marker.is_some()) {
        assert!(is_valid_id(&c.id), "invalid id {:?} after the fix", c.id);
        assert_ne!(c.id, "c0");
        assert!(seen.insert(c.id.clone()), "duplicate id {:?}", c.id);
    }
    let removal = remove_container_markers(&text, text.len());
    assert!(
        apply_edits(&text, &removal).is_ok(),
        "marker removal edits must apply"
    );
}
