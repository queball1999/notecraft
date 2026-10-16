//! Golden tests: every `testdata/markdown/*.md` page is parsed and dumped as text (block kinds,
//! source spans, the source text each span covers, and inline runs), and the dump must equal
//! the `.golden` file next to it. Regenerate after an intended change with
//! `UPDATE_GOLDEN=1 cargo test -p notecraft-markdown --test golden`, then review the diff.

// Test helpers may stop the test run on a bad file; only non-test code must never panic.
#![allow(clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use notecraft_markdown::{parse, Block, BlockKind, Document, Run, Span, Value};

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/markdown")
}

fn show(text: &str) -> String {
    format!("{text:?}")
}

fn runs(runs: &[Run]) -> String {
    let parts: Vec<String> = runs
        .iter()
        .map(|r| {
            let mut tags = String::new();
            for (on, tag) in [
                (r.bold, 'b'),
                (r.italic, 'i'),
                (r.strike, 's'),
                (r.code, 'c'),
            ] {
                if on {
                    tags.push(tag);
                }
            }
            match (&r.link, tags.is_empty()) {
                (Some(url), _) => format!("{}[{tags}@{url}]", show(&r.text)),
                (None, true) => show(&r.text),
                (None, false) => format!("{}[{tags}]", show(&r.text)),
            }
        })
        .collect();
    parts.join(" + ")
}

fn dump_blocks(blocks: &[Block], source: &str, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    for block in blocks {
        let Span { start, end } = block.span;
        let head = match &block.kind {
            BlockKind::Heading { level, runs: r } => format!("heading({level}) {}", runs(r)),
            BlockKind::Paragraph { runs: r } => format!("paragraph {}", runs(r)),
            BlockKind::List { kind, tight, items } => {
                format!("list {kind:?} tight={tight} items={}", items.len())
            }
            BlockKind::Table(t) => format!(
                "table align={:?} header={} rows={}",
                t.align,
                t.header.len(),
                t.rows.len()
            ),
            BlockKind::Code {
                info,
                fenced,
                literal,
            } => format!("code info={} fenced={fenced} {}", show(info), show(literal)),
            BlockKind::Quote { .. } => "quote".to_owned(),
            BlockKind::Rule => "rule".to_owned(),
            BlockKind::Html { literal } => format!("html {}", show(literal)),
        };
        let _ = writeln!(out, "{pad}{start}..{end} {head}");
        let _ = writeln!(out, "{pad}  source {}", show(block.source(source)));
        match &block.kind {
            BlockKind::List { items, .. } => {
                for item in items {
                    let task = item.task.map_or(String::new(), |c| format!(" task={c}"));
                    let _ = writeln!(
                        out,
                        "{pad}  item {}..{}{task} {}",
                        item.span.start,
                        item.span.end,
                        show(item.span.slice(source))
                    );
                    dump_blocks(&item.blocks, source, depth + 2, out);
                }
            }
            BlockKind::Table(t) => {
                for (label, row) in
                    std::iter::once(("head", &t.header)).chain(t.rows.iter().map(|r| ("row", r)))
                {
                    let cells: Vec<String> = row.iter().map(|c| runs(&c.runs)).collect();
                    let _ = writeln!(out, "{pad}  {label} | {}", cells.join(" | "));
                }
            }
            BlockKind::Quote { blocks } => dump_blocks(blocks, source, depth + 1, out),
            _ => {}
        }
    }
}

fn dump(source: &str) -> String {
    let doc: Document = parse(source);
    let mut out = String::new();
    match &doc.frontmatter {
        None => out.push_str("frontmatter none\n"),
        Some(fm) => {
            let _ = writeln!(
                out,
                "frontmatter {}..{} oversize={}",
                fm.span.start, fm.span.end, fm.oversize
            );
            for e in &fm.entries {
                let value = match &e.value {
                    Value::Scalar(s) => format!("scalar {}", show(s)),
                    Value::List(l) => format!("list {l:?}"),
                    Value::Map(m) => format!("map {m:?}"),
                    Value::Raw(r) => format!("raw {}", show(r)),
                };
                let _ = writeln!(
                    out,
                    "  {} {}..{} {value}",
                    show(&e.key),
                    e.span.start,
                    e.span.end
                );
            }
        }
    }
    let _ = writeln!(out, "body {}", doc.body_start);
    dump_blocks(&doc.blocks, source, 0, &mut out);
    out
}

fn samples() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir())
        .expect("testdata/markdown exists")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    files
}

#[test]
fn sample_pages_match_their_golden_dumps() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let files = samples();
    assert!(
        files.len() >= 6,
        "expected the sample pages, found {files:?}"
    );
    for path in files {
        let source = std::fs::read_to_string(&path).expect("sample is readable");
        let actual = dump(&source);
        let golden = path.with_extension("golden");
        if update {
            std::fs::write(&golden, &actual).expect("golden is writable");
            continue;
        }
        let expected = std::fs::read_to_string(&golden)
            .unwrap_or_else(|_| panic!("missing {}; run with UPDATE_GOLDEN=1", golden.display()));
        assert_eq!(
            actual.replace("\r\n", "\n"),
            expected.replace("\r\n", "\n"),
            "{} changed; if intended, regenerate with UPDATE_GOLDEN=1",
            path.display()
        );
    }
}

#[test]
fn every_span_lies_on_character_boundaries_inside_the_page() {
    fn check(blocks: &[Block], source: &str) {
        for block in blocks {
            let Span { start, end } = block.span;
            assert!(start <= end && end <= source.len());
            assert!(source.is_char_boundary(start) && source.is_char_boundary(end));
            match &block.kind {
                BlockKind::Quote { blocks } => check(blocks, source),
                BlockKind::List { items, .. } => {
                    for item in items {
                        assert!(item.span.start >= start && item.span.end <= end);
                        check(&item.blocks, source);
                    }
                }
                _ => {}
            }
        }
    }
    for path in samples() {
        let source = std::fs::read_to_string(&path).expect("sample is readable");
        check(&parse(&source).blocks, &source);
        // The same page with Windows line endings parses to the same block kinds.
        let crlf = source.replace('\n', "\r\n");
        let (lf_doc, crlf_doc) = (parse(&source), parse(&crlf));
        assert_eq!(
            lf_doc.blocks.len(),
            crlf_doc.blocks.len(),
            "{}",
            path.display()
        );
        check(&crlf_doc.blocks, &crlf);
    }
}
