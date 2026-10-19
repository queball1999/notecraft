//! Reading and writing the text formatting vocabulary (docs/markdown.md section 3.2).

use crate::{
    block_to_markdown, parse, parse_blocks, Block, BlockKind, LineEnding, Run, Script, Span,
};

fn runs_of(text: &str) -> Vec<Run> {
    let blocks = parse_blocks(text, 0);
    match blocks.into_iter().next().map(|b| b.kind) {
        Some(BlockKind::Paragraph { runs }) => runs,
        other => panic!("expected a paragraph, got {other:?}"),
    }
}

#[test]
fn html_and_highlight_become_run_styles() {
    let runs = runs_of(
        "<u>under</u> ==hi== <mark style=\"background:#00FF00\">green</mark> \
         <span style=\"color:#FF0000\">red</span> <span style=\"font-size:14pt\">big</span> \
         <span style=\"font-family:Courier New\">mono</span> H<sub>2</sub>O x<sup>2</sup> <b>raw</b>",
    );
    let find = |text: &str| runs.iter().find(|r| r.text == text);
    assert!(find("under").is_some_and(|r| r.underline));
    assert!(find("hi").is_some_and(|r| r.highlight == Some(0x00FF_F176)));
    assert!(find("green").is_some_and(|r| r.highlight == Some(0x0000_FF00)));
    assert!(find("red").is_some_and(|r| r.color == Some(0x00FF_0000)));
    assert!(find("big").is_some_and(|r| r.size_pt == Some(14)));
    assert!(find("mono").is_some_and(|r| r.family.as_deref() == Some("Courier New")));
    assert!(runs
        .iter()
        .any(|r| r.text == "2" && r.script == Script::Sub));
    assert!(runs
        .iter()
        .any(|r| r.text == "2" && r.script == Script::Sup));
    // The tags are not shown; unknown HTML stays as text.
    assert!(!runs
        .iter()
        .any(|r| r.text.contains("<u>") || r.text.contains("<span")));
    assert!(runs.iter().any(|r| r.text.contains("<b>")));
}

#[test]
fn styles_nest_inside_emphasis_and_do_not_leak() {
    let runs = runs_of("**a <u>b</u> c** d");
    assert!(runs.iter().any(|r| r.text == "b" && r.bold && r.underline));
    assert!(runs.iter().all(|r| r.text != " c" || !r.underline));
    assert!(runs
        .iter()
        .any(|r| r.text == " d" && !r.bold && !r.underline));
}

#[test]
fn an_unclosed_tag_ends_with_its_paragraph() {
    let blocks = parse_blocks("<u>open\n\nnext", 0);
    assert_eq!(blocks.len(), 2);
    let Some(BlockKind::Paragraph { runs }) = blocks.get(1).map(|b| &b.kind) else {
        panic!("expected a paragraph");
    };
    assert!(runs.iter().all(|r| !r.underline));
}

#[test]
fn styles_are_written_as_html_and_equals_and_read_back() {
    let runs = vec![
        Run {
            text: "u".into(),
            underline: true,
            script: Script::Sub,
            color: Some(0x00FF_0000),
            ..Run::default()
        },
        Run {
            text: "h".into(),
            highlight: Some(0x00FF_F176),
            ..Run::default()
        },
    ];
    let block = Block {
        span: Span::default(),
        kind: BlockKind::Paragraph { runs },
    };
    let written = block_to_markdown(&block, LineEnding::Lf);
    assert_eq!(
        written,
        "<span style=\"color:#FF0000\"><sub><u>u</u></sub></span>==h=="
    );
    let back = parse(&written);
    let Some(BlockKind::Paragraph { runs }) = back.blocks.first().map(|b| &b.kind) else {
        panic!("expected a paragraph");
    };
    assert!(runs
        .iter()
        .any(|r| r.text == "u" && r.underline && r.color == Some(0x00FF_0000)));
}
