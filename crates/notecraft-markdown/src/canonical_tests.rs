#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::frontmatter::Value;
use crate::{
    block_to_markdown, blocks_to_markdown, page_to_markdown, parse, Align, Block, BlockKind, Cell,
    LineEnding, ListItem, ListKind, Run, Span, Table,
};

fn block(kind: BlockKind) -> Block {
    Block {
        span: Span::default(),
        kind,
    }
}

fn para(text: &str) -> Block {
    block(BlockKind::Paragraph {
        runs: vec![Run::plain(text)],
    })
}

#[test]
fn headings_use_atx() {
    let h = block(BlockKind::Heading {
        level: 2,
        runs: vec![Run::plain("Title")],
    });
    assert_eq!(block_to_markdown(&h, LineEnding::Lf), "## Title");
}

#[test]
fn bullets_use_dash_and_numbers_count_up() {
    let item = |t: &str, task| ListItem {
        span: Span::default(),
        task,
        blocks: vec![para(t)],
    };
    let bullets = block(BlockKind::List {
        kind: ListKind::Bullet,
        tight: true,
        items: vec![item("a", None), item("b", None)],
    });
    assert_eq!(block_to_markdown(&bullets, LineEnding::Lf), "- a\n- b");
    let numbers = block(BlockKind::List {
        kind: ListKind::Ordered(3),
        tight: true,
        items: vec![item("a", None), item("b", None)],
    });
    assert_eq!(block_to_markdown(&numbers, LineEnding::Lf), "3. a\n4. b");
    let tasks = block(BlockKind::List {
        kind: ListKind::Task,
        tight: true,
        items: vec![item("a", Some(false)), item("b", Some(true))],
    });
    assert_eq!(
        block_to_markdown(&tasks, LineEnding::Lf),
        "- [ ] a\n- [x] b"
    );
}

#[test]
fn inline_styles_and_escapes() {
    let runs = vec![
        Run {
            text: "bold".into(),
            bold: true,
            ..Run::default()
        },
        Run::plain(" a*b "),
        Run {
            text: "go".into(),
            link: Some("http://x.y/a b".into()),
            ..Run::default()
        },
        Run {
            text: "c`d".into(),
            code: true,
            ..Run::default()
        },
    ];
    let p = block(BlockKind::Paragraph { runs });
    assert_eq!(
        block_to_markdown(&p, LineEnding::Lf),
        "**bold** a\\*b [go](http://x.y/a%20b)``c`d``"
    );
}

#[test]
fn text_that_looks_like_a_block_is_escaped() {
    for text in [
        "# not a heading",
        "- not a list",
        "1. not a list",
        "> not a quote",
    ] {
        let out = block_to_markdown(&para(text), LineEnding::Lf);
        let doc = parse(&out);
        assert_eq!(doc.blocks.len(), 1, "{out:?}");
        assert!(
            matches!(doc.blocks[0].kind, BlockKind::Paragraph { .. }),
            "{out:?}"
        );
        assert_eq!(doc.plain_text().trim_end(), text);
    }
}

#[test]
fn code_fence_grows_past_backticks_in_the_code() {
    let c = block(BlockKind::Code {
        info: "rust".into(),
        fenced: true,
        literal: "let a = \"```\";\n".into(),
    });
    let out = block_to_markdown(&c, LineEnding::Lf);
    assert_eq!(out, "````rust\nlet a = \"```\";\n````");
}

#[test]
fn table_pads_ragged_rows_and_escapes_pipes() {
    let cell = |t: &str| Cell {
        span: Span::default(),
        runs: vec![Run::plain(t)],
    };
    let t = block(BlockKind::Table(Table {
        align: vec![Align::Left, Align::Right],
        header: vec![cell("a|b"), cell("c")],
        rows: vec![vec![cell("1")]],
    }));
    assert_eq!(
        block_to_markdown(&t, LineEnding::Lf),
        "| a\\|b | c |\n| :-- | --: |\n| 1 |  |"
    );
}

#[test]
fn blocks_are_separated_by_one_blank_line_with_the_files_eol() {
    let blocks = [para("one"), block(BlockKind::Rule), para("two")];
    assert_eq!(
        blocks_to_markdown(&blocks, LineEnding::Lf),
        "one\n\n---\n\ntwo\n"
    );
    assert_eq!(
        blocks_to_markdown(&blocks, LineEnding::Crlf),
        "one\r\n\r\n---\r\n\r\ntwo\r\n"
    );
    assert_eq!(blocks_to_markdown(&[], LineEnding::Lf), "");
}

#[test]
fn fresh_page_has_frontmatter_then_body() {
    let out = page_to_markdown(
        &[("title".into(), Value::Scalar("Hi".into()))],
        &[para("body")],
        LineEnding::Lf,
    );
    assert_eq!(out, "---\ntitle: Hi\n---\nbody\n");
    assert_eq!(page_to_markdown(&[], &[], LineEnding::Lf), "");
}

#[test]
fn serializing_a_parsed_page_is_stable_and_keeps_the_text() {
    let sample = "# Head\n\nSome **bold** and *it* and `code` [l](http://a.b).\n\n- a\n- b\n  - c\n\n1. x\n2. y\n\n- [ ] t\n- [x] u\n\n> quote\n> more\n\n```py\nprint(1)\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n---\n";
    let first = parse(sample);
    let once = blocks_to_markdown(&first.blocks, LineEnding::Lf);
    let second = parse(&once);
    assert_eq!(second.plain_text(), first.plain_text());
    let twice = blocks_to_markdown(&second.blocks, LineEnding::Lf);
    assert_eq!(once, twice);
}
