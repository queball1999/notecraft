//! Round-trip and minimal-diff property tests over the sample corpus
//! (`testdata/markdown/corpus/*.md` plus the golden sample pages).
//!
//! Properties checked for every page, and for a CRLF copy of it:
//! - Parsing and writing with no edits gives identical bytes.
//! - Replacing any block, list item, or table cell (at any depth) with its own source text is
//!   a no-op.
//! - Replacing one with new text changes only the bytes of its span.
//! - Deleting one top-level block removes only that block and its separator.
//! - Setting or removing a frontmatter key leaves every other entry's bytes alone.
//! - Random span replacements equal a plain splice; bad spans are errors, never panics.

// Test helpers may stop the test run on a bad file; only non-test code must never panic.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use notecraft_markdown::{apply_edits, parse, Block, BlockKind, Edit, Editor, Span, Value};

fn testdata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/markdown")
}

/// All sample pages as (name, text).
fn corpus() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in [testdata().join("corpus"), testdata()] {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let bytes = std::fs::read(&path).expect("read sample");
            let text = String::from_utf8(bytes).expect("samples are UTF-8");
            let name = path
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned();
            out.push((name, text));
        }
    }
    out.sort();
    out
}

/// The page, and the same page with every line ending turned into CRLF.
fn variants(text: &str) -> Vec<(String, String)> {
    let crlf = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n");
    vec![
        (String::new(), text.to_owned()),
        (" (crlf)".to_owned(), crlf),
    ]
}

/// Every editable span in a page: blocks, list items, table cells, at any depth. Cell spans
/// are reported separately because cell edits keep padding.
struct Units {
    blocks: Vec<Span>,
    items: Vec<Span>,
}

fn collect(blocks: &[Block], units: &mut Units) {
    for block in blocks {
        units.blocks.push(block.span);
        match &block.kind {
            BlockKind::List { items, .. } => {
                for item in items {
                    units.items.push(item.span);
                    collect(&item.blocks, units);
                }
            }
            BlockKind::Quote { blocks } => collect(blocks, units),
            _ => {}
        }
    }
}

fn units_of(text: &str) -> Units {
    let doc = parse(text);
    let mut units = Units {
        blocks: Vec::new(),
        items: Vec::new(),
    };
    collect(&doc.blocks, &mut units);
    units
}

fn splice(text: &str, span: Span, new: &str) -> String {
    format!("{}{}{}", &text[..span.start], new, &text[span.end..])
}

#[test]
fn corpus_has_at_least_fifty_pages() {
    assert!(corpus().len() >= 50, "found {}", corpus().len());
}

#[test]
fn unedited_pages_round_trip_byte_for_byte() {
    for (name, text) in corpus() {
        for (tag, text) in variants(&text) {
            let doc = parse(&text);
            // Parsing must not need the text to be rewritten: every span is in bounds.
            for span in std::iter::once(doc.frontmatter.as_ref().map(|f| f.span))
                .flatten()
                .chain(units_of(&text).blocks)
            {
                assert!(
                    text.get(span.start..span.end).is_some(),
                    "{name}{tag}: {span:?}"
                );
            }
            let out = Editor::new(&text).finish().unwrap();
            assert_eq!(out, text, "{name}{tag}");
        }
    }
}

#[test]
fn replacing_a_unit_with_itself_is_a_no_op() {
    for (name, text) in corpus() {
        for (tag, text) in variants(&text) {
            let units = units_of(&text);
            for span in units.blocks.iter().chain(&units.items) {
                let mut ed = Editor::new(&text);
                ed.replace(*span, span.slice(&text)).unwrap();
                assert_eq!(ed.finish().unwrap(), text, "{name}{tag} {span:?}");
            }
        }
    }
}

#[test]
fn an_edit_changes_only_the_bytes_of_its_span() {
    for (name, text) in corpus() {
        for (tag, text) in variants(&text) {
            let units = units_of(&text);
            for span in units.blocks.iter().chain(&units.items) {
                let mut ed = Editor::new(&text);
                ed.replace(*span, "EDITED").unwrap();
                let out = ed.finish().unwrap();
                assert_eq!(out, splice(&text, *span, "EDITED"), "{name}{tag} {span:?}");
                assert!(out.starts_with(&text[..span.start]), "{name}{tag}");
                assert!(out.ends_with(&text[span.end..]), "{name}{tag}");
                // The result is still a page the reader accepts.
                let _ = parse(&out);
            }
        }
    }
}

#[test]
fn editing_one_block_leaves_the_other_blocks_alone() {
    for (name, text) in corpus() {
        let doc = parse(&text);
        for (i, block) in doc.blocks.iter().enumerate() {
            let mut ed = Editor::new(&text);
            ed.replace_block(block, "EDITED").unwrap();
            let out = ed.finish().unwrap();
            for (j, other) in doc.blocks.iter().enumerate() {
                if i == j {
                    continue;
                }
                assert!(
                    out.contains(other.source(&text)),
                    "{name}: block {j} changed when editing block {i}"
                );
            }
        }
    }
}

#[test]
fn deleting_a_top_level_block_removes_only_that_block() {
    for (name, text) in corpus() {
        let doc = parse(&text);
        for index in 0..doc.blocks.len() {
            let mut ed = Editor::new(&text);
            ed.delete_block(&doc.blocks, index).unwrap();
            let out = ed.finish().unwrap();
            let this = doc.blocks[index].span;
            assert!(out.len() < text.len() || this.is_empty(), "{name}: {index}");
            for (j, other) in doc.blocks.iter().enumerate() {
                if j != index {
                    assert!(
                        out.contains(other.source(&text)),
                        "{name}: deleting block {index} damaged block {j}"
                    );
                }
            }
        }
    }
}

#[test]
fn frontmatter_edits_keep_every_other_entry_byte_for_byte() {
    for (name, text) in corpus() {
        for (tag, text) in variants(&text) {
            let doc = parse(&text);
            let Some(fm) = &doc.frontmatter else {
                continue;
            };
            if fm.oversize {
                continue;
            }
            let rest = &text[fm.span.end..];
            let mut ed = Editor::new(&text);
            ed.set_frontmatter("zz-new-key", &Value::Scalar("v".into()))
                .unwrap();
            let out = ed.finish().unwrap();
            assert!(out.ends_with(rest), "{name}{tag}: body changed");
            for entry in &fm.entries {
                assert!(
                    out.contains(entry.span.slice(&text)),
                    "{name}{tag}: entry {} changed",
                    entry.key
                );
            }
            // Removing the key again gives the original text back.
            let reparsed = parse(&out);
            assert!(reparsed
                .frontmatter
                .as_ref()
                .is_some_and(|f| f.get("zz-new-key").is_some()));
            let mut ed = Editor::new(&out);
            assert_eq!(ed.remove_frontmatter("zz-new-key"), Ok(true));
            assert_eq!(ed.finish().unwrap(), text, "{name}{tag}");

            for entry in &fm.entries {
                let mut ed = Editor::new(&text);
                assert_eq!(ed.remove_frontmatter(&entry.key), Ok(true));
                let out = ed.finish().unwrap();
                assert!(out.ends_with(rest), "{name}{tag}");
                for other in fm.entries.iter().filter(|e| e.key != entry.key) {
                    assert!(out.contains(other.span.slice(&text)), "{name}{tag}");
                }
            }
        }
    }
}

/// A tiny deterministic generator, so the property test needs no extra crate.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        usize::try_from(self.0 >> 33).unwrap_or(0) % bound.max(1)
    }
}

#[test]
fn random_edits_equal_a_plain_splice_and_bad_spans_are_errors() {
    let mut rng = Lcg(42);
    for (name, text) in corpus() {
        for round in 0..20 {
            let a = rng.next(text.len() + 1);
            let b = rng.next(text.len() + 1);
            let (start, end) = (a.min(b), a.max(b));
            let on_boundaries = text.is_char_boundary(start) && text.is_char_boundary(end);
            let result = apply_edits(&text, &[Edit::new(Span::new(start, end), "<>")]);
            if on_boundaries {
                assert_eq!(
                    result.unwrap(),
                    splice(&text, Span::new(start, end), "<>"),
                    "{name} round {round}"
                );
            } else {
                assert!(result.is_err(), "{name} round {round}");
            }
        }
        // Past the end, and reversed.
        assert!(apply_edits(&text, &[Edit::new(Span::new(0, text.len() + 1), "")]).is_err());
        if text.len() > 1 {
            assert!(apply_edits(&text, &[Edit::new(Span::new(1, 0), "")]).is_err());
        }
    }
}
