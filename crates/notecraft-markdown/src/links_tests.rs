//! Page link forms, escapes, and code exclusion (docs/markdown.md section 5.1).

use crate::links::{find_page_links, parse_inner, MAX_LINK_BYTES};
use crate::{blocks_to_markdown, parse_blocks, BlockKind, LineEnding, Run};

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
    assert!(parse_inner(&"a".repeat(MAX_LINK_BYTES + 1)).is_none());
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
fn page_links_round_trip_through_the_writer() {
    for src in ["See [[Plan]] and [[Work/Plan|the plan]].", "[[A]][[B]]"] {
        let blocks = parse_blocks(src, 0);
        let written = blocks_to_markdown(&blocks, LineEnding::Lf);
        assert_eq!(written.trim_end(), src);
        assert_eq!(runs_of(&written), runs_of(src));
    }
}
