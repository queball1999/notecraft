#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::edit::{apply_edits, Edit, EditError, Editor};
use crate::frontmatter::Value;
use crate::{parse, BlockKind, Span};

const PAGE: &str = "---\ntitle: T\ntags: [a, b]\n---\n# Head\n\nOne  paragraph.   \n\n* [ ] task\n* [x] done\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nLast.\n";

#[test]
fn no_edits_is_identity() {
    for text in [PAGE, "", "\u{feff}# bom\r\n", "no newline", "a\r\nb\nc\r"] {
        assert_eq!(Editor::new(text).finish().as_deref(), Ok(text));
    }
}

#[test]
fn replace_block_touches_only_its_bytes() {
    let doc = parse(PAGE);
    let block = &doc.blocks[1];
    assert!(matches!(block.kind, BlockKind::Paragraph { .. }));
    let mut ed = Editor::new(PAGE);
    ed.replace_block(block, "Changed.").unwrap();
    let out = ed.finish().unwrap();
    assert_eq!(out, PAGE.replacen("One  paragraph.   ", "Changed.", 1));
}

#[test]
fn crlf_pages_get_crlf_text() {
    let page = "# A\r\n\r\npara\r\n";
    let doc = parse(page);
    let mut ed = Editor::new(page);
    ed.replace_block(&doc.blocks[1], "x\ny").unwrap();
    assert_eq!(ed.finish().unwrap(), "# A\r\n\r\nx\r\ny\r\n");
}

#[test]
fn insert_and_delete_blocks() {
    let page = "# A\n\none\n\ntwo\n";
    let doc = parse(page);
    let mut ed = Editor::new(page);
    ed.insert_after(&doc.blocks[1], "new").unwrap();
    assert_eq!(ed.finish().unwrap(), "# A\n\none\n\nnew\n\ntwo\n");

    let mut ed = Editor::new(page);
    ed.insert_before(&doc.blocks[0], "top").unwrap();
    assert_eq!(ed.finish().unwrap(), "top\n\n# A\n\none\n\ntwo\n");

    let mut ed = Editor::new(page);
    ed.delete_block(&doc.blocks, 1).unwrap();
    assert_eq!(ed.finish().unwrap(), "# A\n\ntwo\n");

    let mut ed = Editor::new(page);
    ed.delete_block(&doc.blocks, 2).unwrap();
    assert_eq!(ed.finish().unwrap(), "# A\n\none\n");

    let mut ed = Editor::new(page);
    assert_eq!(
        ed.delete_block(&doc.blocks, 9),
        Err(EditError::NoSuchIndex { index: 9, len: 3 })
    );
}

#[test]
fn replace_item_and_cell() {
    let doc = parse(PAGE);
    let BlockKind::List { items, .. } = &doc.blocks[2].kind else {
        panic!("list expected");
    };
    let mut ed = Editor::new(PAGE);
    ed.replace_item(&items[0], "* [x] task").unwrap();
    assert_eq!(
        ed.finish().unwrap(),
        PAGE.replacen("* [ ] task", "* [x] task", 1)
    );

    let BlockKind::Table(table) = &doc.blocks[3].kind else {
        panic!("table expected");
    };
    let mut ed = Editor::new(PAGE);
    ed.replace_cell(&table.rows[0][1], "x|y\nz").unwrap();
    assert_eq!(
        ed.finish().unwrap(),
        PAGE.replacen("| 1 | 2 |", "| 1 | x\\|y z |", 1)
    );
}

#[test]
fn frontmatter_edits_keep_other_bytes() {
    let mut ed = Editor::new(PAGE);
    ed.set_frontmatter("title", &Value::Scalar("New: title".into()))
        .unwrap();
    ed.set_frontmatter("pinned", &Value::Scalar("true".into()))
        .unwrap();
    let out = ed.finish().unwrap();
    assert!(
        out.starts_with("---\ntitle: \"New: title\"\ntags: [a, b]\npinned: \"true\"\n---\n# Head")
    );
    assert!(out.ends_with(&PAGE[PAGE.find("# Head").unwrap()..]));

    let mut ed = Editor::new(PAGE);
    assert_eq!(ed.remove_frontmatter("tags"), Ok(true));
    assert_eq!(ed.remove_frontmatter("nope"), Ok(false));
    assert_eq!(ed.finish().unwrap(), PAGE.replacen("tags: [a, b]\n", "", 1));
}

#[test]
fn frontmatter_is_created_once() {
    let page = "# A\n";
    let mut ed = Editor::new(page);
    ed.set_frontmatter("title", &Value::Scalar("T".into()))
        .unwrap();
    ed.set_frontmatter("tags", &Value::List(vec!["a".into(), "b c".into()]))
        .unwrap();
    assert_eq!(
        ed.finish().unwrap(),
        "---\ntitle: T\ntags: [a, b c]\n---\n# A\n"
    );

    let mut ed = Editor::new("\u{feff}# A\r\n");
    ed.set_frontmatter("title", &Value::Scalar("T".into()))
        .unwrap();
    assert_eq!(
        ed.finish().unwrap(),
        "\u{feff}---\r\ntitle: T\r\n---\r\n# A\r\n"
    );
}

#[test]
fn bad_keys_and_values_are_refused() {
    let mut ed = Editor::new("x");
    assert!(matches!(
        ed.set_frontmatter("a: b", &Value::Scalar("1".into())),
        Err(EditError::InvalidKey(_))
    ));
    assert!(matches!(
        ed.set_frontmatter("k", &Value::Raw("  ".into())),
        Err(EditError::InvalidValue(_))
    ));
}

#[test]
fn bad_spans_are_errors_not_panics() {
    let text = "h\u{e9}llo";
    assert!(matches!(
        apply_edits(text, &[Edit::new(Span::new(0, 99), "")]),
        Err(EditError::OutOfBounds { .. })
    ));
    assert!(matches!(
        apply_edits(text, &[Edit::new(Span::new(2, 3), "")]),
        Err(EditError::NotCharBoundary { .. })
    ));
    assert!(matches!(
        apply_edits(
            "hello",
            &[
                Edit::new(Span::new(0, 3), ""),
                Edit::new(Span::new(2, 4), "")
            ]
        ),
        Err(EditError::Overlap { .. })
    ));
    assert_eq!(
        apply_edits(
            "abc",
            &[
                Edit::new(Span::new(1, 1), "x"),
                Edit::new(Span::new(1, 1), "y")
            ]
        )
        .as_deref(),
        Ok("axybc")
    );
}
