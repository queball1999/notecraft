#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;
use crate::apply_edits;

const PAGE: &str = "Intro\n\n<!-- notecraft:container id=c1 -->\n## Agenda\n\n- Budget\n\n<!-- notecraft:container id=c2 -->\nSend the deck.\n";

#[test]
fn text_before_the_first_marker_is_c0() {
    let cs = read_containers(PAGE, PAGE.len());
    let ids: Vec<&str> = cs.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["c0", "c1", "c2"]);
    assert_eq!(cs[0].content(PAGE), "Intro\n\n");
    assert_eq!(cs[1].content(PAGE), "## Agenda\n\n- Budget\n\n");
    assert_eq!(cs[2].content(PAGE), "Send the deck.\n");
    assert_eq!(
        cs[2].marker.unwrap().slice(PAGE),
        "<!-- notecraft:container id=c2 -->"
    );
    assert_eq!(cs[2].id_span.unwrap().slice(PAGE), "c2");
}

#[test]
fn a_page_with_no_markers_is_one_implicit_container() {
    let cs = read_containers("Just text\n", 10);
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].id, IMPLICIT_ID);
    assert_eq!(read_containers("", 0).len(), 1);
}

#[test]
fn a_leading_marker_has_no_implicit_container() {
    let text = "<!-- notecraft:container id=a -->\nx\n";
    let cs = read_containers(text, text.len());
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].id, "a");
}

#[test]
fn markers_in_code_fences_and_frontmatter_do_not_count() {
    let text = "---\ntitle: T\n---\n```\n<!-- notecraft:container id=x -->\n```\ntext\n<!-- notecraft:container id=y -->\nmore\n";
    let cs = read_containers(text, text.len());
    let ids: Vec<&str> = cs.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["c0", "y"]);
}

#[test]
fn the_limit_stops_the_last_container() {
    let text = "<!-- notecraft:container id=a -->\nx\n```notecraft-canvas\n{}\n```\n";
    let limit = text.find("```notecraft").unwrap();
    let cs = read_containers(text, limit);
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0].span.end, limit);
}

#[test]
fn quoted_ids_and_odd_spacing_are_read() {
    let text = "<!--notecraft:container   id=\"q1\"   -->\nx\n";
    let cs = read_containers(text, text.len());
    assert_eq!(cs[0].id, "q1");
    assert_eq!(cs[0].id_span.unwrap().slice(text), "q1");
    for not_markers in [
        "<!-- notecraft:containers id=a -->",
        "<!-- other id=a -->",
        "<!-- notecraft:container -->",
        "    <!-- notecraft:container id=a -->",
        "<!-- notecraft:container id=a",
    ] {
        let cs = read_containers(not_markers, not_markers.len());
        assert!(cs.iter().all(|c| c.marker.is_none()), "{not_markers:?}");
    }
}

#[test]
fn bad_and_duplicate_ids_get_fresh_ids_and_nothing_else_changes() {
    let text = "<!-- notecraft:container id=c1 -->\na\n<!-- notecraft:container id=c1 -->\nb\n<!-- notecraft:container id=c0 -->\nc\n<!-- notecraft:container id=we!rd -->\nd\n";
    let out = apply_edits(text, &fix_container_ids(text, text.len())).unwrap();
    let cs = read_containers(&out, out.len());
    let ids: Vec<&str> = cs.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["c1", "c2", "c3", "c4"]);
    // Bodies are untouched.
    assert_eq!(
        cs.iter().map(|c| c.content(&out)).collect::<Vec<_>>(),
        ["a\n", "b\n", "c\n", "d\n"]
    );
    // Already fine: no edits.
    assert!(fix_container_ids(&out, out.len()).is_empty());
    assert!(fix_container_ids(PAGE, PAGE.len()).is_empty());
}

#[test]
fn crlf_pages_read_the_same() {
    let text = PAGE.replace('\n', "\r\n");
    let cs = read_containers(&text, text.len());
    assert_eq!(cs.len(), 3);
    assert_eq!(cs[2].content(&text), "Send the deck.\r\n");
    assert_eq!(cs[1].id_span.unwrap().slice(&text), "c1");
}

#[test]
fn marker_count_is_capped() {
    let text = "<!-- notecraft:container id=a -->\n".repeat(MAX_CONTAINERS + 50);
    let cs = read_containers(&text, text.len());
    assert!(cs.len() <= MAX_CONTAINERS + 1);
}

#[test]
fn hostile_input_never_panics() {
    for text in [
        "",
        "<!--",
        "<!-- notecraft:container id= -->",
        "<!-- notecraft:container id=\" -->",
        "<!-- notecraft:container id=\"\" -->",
        "\u{feff}<!-- notecraft:container id=\u{e9} -->",
        "```\n<!-- notecraft:container id=a -->",
        "~~~~\n~~~\n<!-- notecraft:container id=a -->\n~~~~\n",
    ] {
        let cs = read_containers(text, text.len());
        let edits = fix_container_ids(text, text.len());
        let _ = apply_edits(text, &edits).unwrap();
        for c in cs {
            assert!(text.get(c.span.start..c.span.end).is_some());
        }
    }
    let cs = read_containers("abc", 9999);
    assert_eq!(cs.len(), 1);
}

#[test]
fn markers_can_be_removed_alone() {
    let out = apply_edits(PAGE, &remove_container_markers(PAGE, PAGE.len())).unwrap();
    assert_eq!(out, "Intro\n\n## Agenda\n\n- Budget\n\nSend the deck.\n");
    let crlf = PAGE.replace('\n', "\r\n");
    let out = apply_edits(&crlf, &remove_container_markers(&crlf, crlf.len())).unwrap();
    assert!(!out.contains("notecraft:container") && out.contains("Send the deck.\r\n"));
}

#[test]
fn new_markers_are_built_from_a_fresh_id() {
    let cs = read_containers(PAGE, PAGE.len());
    assert_eq!(fresh_container_id(&cs), "c3");
    assert_eq!(marker_line("c3"), "<!-- notecraft:container id=c3 -->");
    assert!(is_valid_id("c3") && !is_valid_id("") && !is_valid_id("a b"));
}
