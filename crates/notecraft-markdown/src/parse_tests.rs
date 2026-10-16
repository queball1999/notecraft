use crate::{parse_blocks, Align, BlockKind, ListKind, MAX_DEPTH};

#[test]
fn blocks_keep_exact_source_spans() {
    let text = "# Title\n\nSome *text* here.\n\n---\n\n```rust\nlet x = 1;\n```\n";
    let blocks = parse_blocks(text, 0);
    let sources: Vec<&str> = blocks.iter().map(|b| b.source(text)).collect();
    assert_eq!(
        sources,
        vec![
            "# Title",
            "Some *text* here.",
            "---",
            "```rust\nlet x = 1;\n```"
        ]
    );
    assert!(matches!(
        blocks.first().map(|b| &b.kind),
        Some(BlockKind::Heading { level: 1, .. })
    ));
    assert!(matches!(
        blocks.get(2).map(|b| &b.kind),
        Some(BlockKind::Rule)
    ));
}

#[test]
fn spans_are_offset_by_base_and_survive_multibyte_text() {
    let body = "caf\u{e9} **na\u{ef}ve**\n\n> quote\n";
    let blocks = parse_blocks(body, 10);
    let first = blocks.first().map(|b| b.span);
    assert_eq!(first.map(|s| s.start), Some(10));
    let whole = format!("0123456789{body}");
    assert_eq!(
        blocks.first().map(|b| b.source(&whole)),
        Some("caf\u{e9} **na\u{ef}ve**")
    );
    assert_eq!(blocks.get(1).map(|b| b.source(&whole)), Some("> quote"));
}

#[test]
fn last_list_item_and_indented_code_have_tight_spans() {
    let text = "- one\n- two\n\ntext\n\n    code\n";
    let blocks = parse_blocks(text, 0);
    let Some(BlockKind::List { items, .. }) = blocks.first().map(|b| &b.kind) else {
        panic!("expected a list");
    };
    assert_eq!(items.last().map(|i| i.span.slice(text)), Some("- two"));
    // An indented code block owns its whole first line, indent included.
    assert_eq!(blocks.get(2).map(|b| b.source(text)), Some("    code"));
}

#[test]
fn crlf_text_keeps_spans_on_line_starts() {
    let text = "first\r\n\r\nsecond\r\n";
    let blocks = parse_blocks(text, 0);
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks.get(1).map(|b| b.source(text)), Some("second"));
}

#[test]
fn inline_runs_carry_styles_and_links() {
    let blocks = parse_blocks(
        "plain **bold *both*** ~~gone~~ `code` [site](https://example.org) end",
        0,
    );
    let Some(BlockKind::Paragraph { runs }) = blocks.first().map(|b| &b.kind) else {
        panic!("expected a paragraph");
    };
    let find = |text: &str| runs.iter().find(|r| r.text == text);
    assert!(find("plain ").is_some_and(|r| r.is_plain()));
    assert!(find("bold ").is_some_and(|r| r.bold && !r.italic));
    assert!(find("both").is_some_and(|r| r.bold && r.italic));
    assert!(find("gone").is_some_and(|r| r.strike));
    assert!(find("code").is_some_and(|r| r.code));
    assert!(find("site").is_some_and(|r| r.link.as_deref() == Some("https://example.org")));
}

#[test]
fn lists_and_task_lists() {
    let text = "- a\n- b\n  - nested\n\n3. x\n4. y\n\n- [ ] todo\n- [x] done\n";
    let blocks = parse_blocks(text, 0);
    let kinds: Vec<_> = blocks
        .iter()
        .filter_map(|b| match &b.kind {
            BlockKind::List { kind, items, .. } => Some((*kind, items.len())),
            _ => None,
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            (ListKind::Bullet, 2),
            (ListKind::Ordered(3), 2),
            (ListKind::Task, 2)
        ]
    );
    let Some(BlockKind::List { items, .. }) = blocks.get(2).map(|b| &b.kind) else {
        panic!("expected the task list");
    };
    assert_eq!(
        items.iter().map(|i| i.task).collect::<Vec<_>>(),
        [Some(false), Some(true)]
    );
    assert_eq!(items.get(1).map(|i| i.span.slice(text)), Some("- [x] done"));
}

#[test]
fn tables_have_alignment_header_and_rows() {
    let text = "| a | b |\n|:--|--:|\n| 1 | **2** |\n| 3 | 4 |\n";
    let blocks = parse_blocks(text, 0);
    let Some(BlockKind::Table(table)) = blocks.first().map(|b| &b.kind) else {
        panic!("expected a table");
    };
    assert_eq!(table.align, vec![Align::Left, Align::Right]);
    assert_eq!(table.header.len(), 2);
    assert_eq!(table.rows.len(), 2);
    assert!(table
        .rows
        .first()
        .and_then(|r| r.get(1))
        .and_then(|c| c.runs.first())
        .is_some_and(|r| r.bold));
}

#[test]
fn quotes_html_and_indented_code() {
    let text = "> quoted\n> text\n\n<div>raw</div>\n\n    indented\n";
    let blocks = parse_blocks(text, 0);
    assert!(matches!(
        blocks.first().map(|b| &b.kind),
        Some(BlockKind::Quote { blocks }) if blocks.len() == 1
    ));
    assert!(matches!(
        blocks.get(1).map(|b| &b.kind),
        Some(BlockKind::Html { literal }) if literal.contains("raw")
    ));
    assert!(matches!(
        blocks.get(2).map(|b| &b.kind),
        Some(BlockKind::Code { fenced: false, .. })
    ));
}

#[test]
fn hostile_nesting_is_bounded() {
    let quotes = format!("{}deep\n", "> ".repeat(5000));
    let blocks = parse_blocks(&quotes, 0);
    let mut depth = 0;
    let mut level = &blocks;
    while let Some(BlockKind::Quote { blocks }) = level.first().map(|b| &b.kind) {
        depth += 1;
        level = blocks;
    }
    assert!(depth <= MAX_DEPTH);

    let lists = (0..3000)
        .map(|i| format!("{}- x\n", "  ".repeat(i.min(500))))
        .collect::<String>();
    assert!(!parse_blocks(&lists, 0).is_empty());

    let emphasis = "*a ".repeat(20000);
    assert!(!parse_blocks(&emphasis, 0).is_empty());
}

#[test]
fn garbage_input_never_panics() {
    for text in [
        "",
        "\0\0\0",
        "|\n|-|\n|",
        "```",
        "[[[[]]]]((((",
        "\u{feff}# bom\n",
        "- [ ]\n- [x",
        "\r\r\n\n\r",
    ] {
        let blocks = parse_blocks(text, 0);
        for block in &blocks {
            assert!(block.span.end <= text.len());
            assert!(text.is_char_boundary(block.span.start));
            assert!(text.is_char_boundary(block.span.end));
        }
    }
}
