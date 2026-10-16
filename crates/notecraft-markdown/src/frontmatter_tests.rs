use crate::frontmatter::{parse, Value, MAX_FRONTMATTER_BYTES};
use crate::{parse as parse_page, BlockKind};

const FULL: &str = "---\ntitle: Meeting notes\ncreated: 2026-10-14T09:30:00-05:00\ntags: [work, \"q3, plan\"]\naliases:\n  - Q3 sync\n  - 'Dana''s sync'\nnotecraft:\n  id: pg_01J9Z4B2M7XR    # stable page id\n  paper: lined\n  color: \"#FFF8E1\"\nmood: \"quoted: value\"\n---\n# Body\n";

#[test]
fn reads_scalars_lists_and_one_level_maps() {
    let fm = parse(FULL).expect("frontmatter");
    assert_eq!(fm.title(), Some("Meeting notes"));
    assert_eq!(fm.scalar("created"), Some("2026-10-14T09:30:00-05:00"));
    assert_eq!(fm.tags(), vec!["work", "q3, plan"]);
    assert_eq!(fm.aliases(), vec!["Q3 sync", "Dana's sync"]);
    assert_eq!(fm.notecraft("id"), Some("pg_01J9Z4B2M7XR"));
    assert_eq!(fm.notecraft("paper"), Some("lined"));
    assert_eq!(fm.notecraft("color"), Some("#FFF8E1"));
    assert_eq!(fm.scalar("mood"), Some("quoted: value"));
    assert!(!fm.oversize);
}

#[test]
fn entry_spans_cover_exact_source_lines() {
    let fm = parse(FULL).expect("frontmatter");
    let aliases = fm.get("aliases").expect("aliases");
    assert_eq!(
        &FULL[aliases.span.start..aliases.span.end],
        "aliases:\n  - Q3 sync\n  - 'Dana''s sync'\n"
    );
    assert_eq!(
        &FULL[fm.span.start..fm.span.end],
        &FULL[..FULL.len() - "# Body\n".len()]
    );
}

#[test]
fn unknown_and_unsupported_values_are_kept_as_raw_text() {
    let src = "---\nsummary: |\n  line one\n  line two\nnested:\n  a:\n    b: 1\nflow: {x: 1}\nplain: ok\n---\n";
    let fm = parse(src).expect("frontmatter");
    assert!(
        matches!(&fm.get("summary").expect("summary").value, Value::Raw(r) if r.contains("line two"))
    );
    assert!(
        matches!(&fm.get("nested").expect("nested").value, Value::Raw(r) if r.contains("b: 1"))
    );
    assert!(matches!(&fm.get("flow").expect("flow").value, Value::Raw(r) if r.contains("{x: 1}")));
    assert_eq!(fm.scalar("plain"), Some("ok"));
    assert_eq!(fm.entries.len(), 4);
}

#[test]
fn page_without_closing_line_has_no_frontmatter() {
    assert!(parse("---\ntitle: x\nbody text\n").is_none());
    assert!(parse("# Heading\n---\n").is_none());
    assert!(parse("").is_none());
}

#[test]
fn crlf_and_bom_are_accepted() {
    let src = "\u{feff}---\r\ntitle: Hi\r\ntags: [a]\r\n---\r\nbody\r\n";
    let fm = parse(src).expect("frontmatter");
    assert_eq!(fm.title(), Some("Hi"));
    assert_eq!(fm.tags(), vec!["a"]);
    let doc = parse_page(src);
    assert_eq!(doc.body_start, src.len() - "body\r\n".len());
}

#[test]
fn oversize_frontmatter_is_kept_as_text_with_no_keys() {
    let big = format!(
        "---\nnotes: {}\n---\nbody\n",
        "x".repeat(MAX_FRONTMATTER_BYTES)
    );
    let fm = parse(&big).expect("frontmatter");
    assert!(fm.oversize);
    assert!(fm.entries.is_empty());
    let doc = parse_page(&big);
    assert_eq!(doc.body_start, big.len() - "body\n".len());
}

#[test]
fn page_parse_offsets_body_spans_past_the_frontmatter() {
    let doc = parse_page(FULL);
    let Some(block) = doc.blocks.first() else {
        panic!("expected a heading");
    };
    assert!(matches!(block.kind, BlockKind::Heading { level: 1, .. }));
    assert_eq!(block.source(FULL), "# Body");
}

#[test]
fn hostile_frontmatter_never_panics() {
    let many_keys: String = (0..5000).map(|i| format!("k{i}: v\n")).collect();
    let many_keys = format!("---\n{many_keys}---\n");
    if let Some(fm) = parse(&many_keys) {
        assert!(fm.entries.len() <= crate::frontmatter::MAX_ENTRIES);
    }
    for src in [
        "---\n---\n",
        "---\n: x\n---\n",
        "---\n\"unclosed: x\n---\n",
        "---\nk: [a, \"b\n---\n",
        "---\nk: [[[[\n---\n",
        "---\n- a\n- b\n---\n",
        "---\nk:\n\t- a\n  - b\n---\n",
        "---\nk: 'it''s\n---\n",
        "---\n\0\0\n---\n",
        "---\nk: \u{e9}\u{e9}: \u{1f600}\n---\n",
    ] {
        let _ = parse(src);
        let _ = parse_page(src);
    }
}
