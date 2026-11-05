//! Page link forms, escapes, and code exclusion (docs/markdown.md section 5.1).

use crate::links::{find_page_links, MAX_LINK_BYTES};
use crate::{blocks_to_markdown, parse_blocks, Block, BlockKind, LineEnding, Run, Span};

fn runs_of(text: &str) -> Vec<Run> {
    match parse_blocks(text, 0).into_iter().next().map(|b| b.kind) {
        Some(BlockKind::Paragraph { runs }) => runs,
        other => panic!("expected a paragraph, got {other:?}"),
    }
}

#[test]
fn the_three_forms_have_spans() {
    let src = "See [[Plan]], [[Plan|the plan]] and [[Work/Plan]].";
    let links = find_page_links(src, 0);
    assert_eq!(links.len(), 3);
    assert_eq!(links[0].target, "Plan");
    assert_eq!(links[0].display, None);
    assert_eq!(links[0].span.slice(src), "[[Plan]]");
    assert_eq!(links[1].target, "Plan");
    assert_eq!(links[1].display.as_deref(), Some("the plan"));
    assert_eq!(links[1].span.slice(src), "[[Plan|the plan]]");
    assert_eq!(links[2].target, "Work/Plan");
    assert_eq!(links[2].span.slice(src), "[[Work/Plan]]");
}

#[test]
fn spans_are_offset_by_the_base() {
    let links = find_page_links("x [[A]]", 100);
    assert_eq!(links[0].span.start, 102);
    assert_eq!(links[0].span.end, 107);
}

#[test]
fn code_spans_and_blocks_are_skipped() {
    let src = "`[[a]]` and ``x [[b]] y`` and [[c]]\n\n```\n[[d]]\n```\n\n    [[e]]\n\n> ```\n> [[f]]\n> ```\n\n[[g]]\n";
    let targets: Vec<_> = find_page_links(src, 0)
        .into_iter()
        .map(|l| l.target)
        .collect();
    assert_eq!(targets, ["c", "g"]);
}

#[test]
fn escaped_and_malformed_links_are_text() {
    for src in [
        "\\[[a]]", "[[]]", "[[ ]]", "[[|x]]", "[[a", "[[a\nb]]", "[[a[b]]", "[ [a]]",
    ] {
        assert!(find_page_links(src, 0).is_empty(), "{src:?}");
    }
}

#[test]
fn the_length_cap_is_1024_bytes() {
    let ok = format!("[[{}]]", "a".repeat(MAX_LINK_BYTES));
    let long = format!("[[{}]]", "a".repeat(MAX_LINK_BYTES + 1));
    assert_eq!(find_page_links(&ok, 0).len(), 1);
    assert!(find_page_links(&long, 0).is_empty());
    // Multi-byte text at the cap edge never panics.
    let wide = format!("[[{}]]", "\u{e9}".repeat(600));
    assert!(find_page_links(&wide, 0).is_empty());
}

#[test]
fn table_cells_use_an_escaped_pipe() {
    let src = "| a | b |\n| --- | --- |\n| [[Page\\|shown]] | [[Other]] |\n";
    let links = find_page_links(src, 0);
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].target, "Page");
    assert_eq!(links[0].display.as_deref(), Some("shown"));
    assert_eq!(links[0].span.slice(src), "[[Page\\|shown]]");

    let blocks = parse_blocks(src, 0);
    let BlockKind::Table(table) = &blocks[0].kind else {
        panic!("expected a table");
    };
    let cell = &table.rows[0][0].runs;
    assert_eq!(cell.len(), 1);
    assert_eq!(cell[0].page_link.as_deref(), Some("Page"));
    assert_eq!(cell[0].text, "shown");
    // The writer escapes the pipe again.
    let written = blocks_to_markdown(&blocks, LineEnding::Lf);
    assert!(written.contains("[[Page\\|shown]]"), "{written}");
}

#[test]
fn runs_carry_the_link_target_and_shown_text() {
    let runs = runs_of("go to [[Plan|the plan]] now");
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].text, "go to ");
    assert_eq!(runs[0].page_link, None);
    assert_eq!(runs[1].text, "the plan");
    assert_eq!(runs[1].page_link.as_deref(), Some("Plan"));
    assert_eq!(runs[2].text, " now");
}

#[test]
fn styled_links_keep_their_style_and_code_stays_text() {
    let runs = runs_of("**[[Bold]]** `[[code]]` [x]([[y]])");
    assert!(runs
        .iter()
        .any(|r| r.bold && r.page_link.as_deref() == Some("Bold")));
    assert!(runs.iter().any(|r| r.code && r.text == "[[code]]"));
    assert!(runs.iter().all(|r| r.page_link.as_deref() != Some("code")));
}

#[test]
fn escaped_brackets_stay_plain_text_through_a_round_trip() {
    let plain = vec![Run::plain("[[x]] and a | bar")];
    let block = Block {
        span: Span::default(),
        kind: BlockKind::Paragraph {
            runs: plain.clone(),
        },
    };
    let written = blocks_to_markdown(&[block], LineEnding::Lf);
    assert_eq!(runs_of(&written), plain, "{written}");

    // Next to a real link, and escaped by hand.
    let runs = runs_of("\\[\\[x\\]\\] and [[y]] and \\[[z]]");
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].text, "[[x]] and ");
    assert_eq!(runs[0].page_link, None);
    assert_eq!(runs[1].page_link.as_deref(), Some("y"));
    assert_eq!(runs[2].text, " and [[z]]");
    assert_eq!(runs[2].page_link, None);
    let src = "\\[\\[x\\]\\] and [[y]]";
    let again = blocks_to_markdown(&parse_blocks(src, 0), LineEnding::Lf);
    assert_eq!(again.trim_end(), src);
}

#[test]
fn escaped_brackets_stay_plain_inside_a_table_cell() {
    let src = "| a | b |\n| --- | --- |\n| \\[\\[x\\]\\] | [[y]] |\n";
    let blocks = parse_blocks(src, 0);
    let BlockKind::Table(table) = &blocks[0].kind else {
        panic!("expected a table");
    };
    assert_eq!(table.rows[0][0].runs, vec![Run::plain("[[x]]")]);
    assert_eq!(table.rows[0][1].runs[0].page_link.as_deref(), Some("y"));
    let written = blocks_to_markdown(&blocks, LineEnding::Lf);
    let again = parse_blocks(&written, 0);
    let BlockKind::Table(t2) = &again[0].kind else {
        panic!("expected a table");
    };
    assert_eq!(t2.rows[0][0].runs, table.rows[0][0].runs);
    assert_eq!(t2.rows[0][1].runs, table.rows[0][1].runs);
}

#[test]
fn an_escaped_pipe_only_separates_inside_a_table() {
    // Outside a table the escaped pipe is a literal pipe in the target.
    let src = "[[a\\|b]] and [[a|b]]";
    let links = find_page_links(src, 0);
    assert_eq!(links[0].target, "a|b");
    assert_eq!(links[0].display, None);
    assert_eq!(links[1].target, "a");
    assert_eq!(links[1].display.as_deref(), Some("b"));
    let runs = runs_of(src);
    assert_eq!(runs[0].page_link.as_deref(), Some("a|b"));
    assert_eq!(runs[0].text, "a|b");
    assert_eq!(runs[2].page_link.as_deref(), Some("a"));
    assert_eq!(runs[2].text, "b");
    let written = blocks_to_markdown(&parse_blocks(src, 0), LineEnding::Lf);
    assert_eq!(written.trim_end(), src);
}

#[test]
fn untouched_blocks_keep_their_link_text_byte_for_byte() {
    let src = "[[T|T]] and [[ spaced ]]\n\nsecond\n";
    let blocks = parse_blocks(src, 0);
    let mut editor = crate::Editor::new(src);
    assert!(editor.replace_block(&blocks[1], "changed").is_ok());
    let out = editor.finish().unwrap_or_default();
    assert!(out.starts_with("[[T|T]] and [[ spaced ]]\n"), "{out}");
}

#[test]
fn page_links_round_trip_through_the_writer() {
    for src in ["See [[Plan]] and [[Work/Plan|the plan]].", "[[A]][[B]]"] {
        let blocks = parse_blocks(src, 0);
        let written = blocks_to_markdown(&blocks, LineEnding::Lf);
        assert_eq!(written.trim_end(), src);
        assert_eq!(runs_of(&written), runs_of(src));
    }
}

#[test]
fn a_link_split_by_emphasis_keeps_both_brackets() {
    // The emphasis cuts the marked link across two runs, so neither run holds it whole and
    // both brackets go back as plain text.
    let src = "*a [[b* c]]";
    let text: String = runs_of(src).iter().map(|r| r.text.as_str()).collect();
    assert!(text.contains("]]"), "{text:?}");
    assert_eq!(text.matches("[[").count(), 1, "{text:?}");
}

#[test]
fn double_brackets_inside_math_stay_math() {
    let runs = runs_of("see $[[0,1]]$ here");
    let math: Vec<_> = runs
        .iter()
        .filter(|r| r.math != crate::block::Math::None)
        .collect();
    assert_eq!(math.len(), 1, "{runs:?}");
    assert_eq!(math[0].text, "[[0,1]]");
    assert!(runs.iter().all(|r| r.page_link.is_none()));
}
