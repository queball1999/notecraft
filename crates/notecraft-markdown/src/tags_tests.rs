use crate::tags::{is_valid_name, scan_tags, strip_tags, tagged_items, MAX_NAME_BYTES};
use crate::{parse, BlockKind, Span};

fn names(text: &str) -> Vec<String> {
    scan_tags(text, 0).into_iter().map(|t| t.name).collect()
}

#[test]
fn finds_tags_at_word_starts() {
    assert_eq!(names("#important buy milk"), ["important"]);
    assert_eq!(names("buy #Milk and #eggs."), ["milk", "eggs"]);
    assert_eq!(names("end of line #idea"), ["idea"]);
    assert_eq!(names("#a-b_c, #d!"), ["a-b_c", "d"]);
}

#[test]
fn spans_cover_the_tag_as_written() {
    let text = "x #Question y";
    let tags = scan_tags(text, 10);
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].span, Span::new(12, 21));
    assert_eq!(
        tags[0].span.slice(&format!("{}{text}", " ".repeat(10))),
        "#Question"
    );
}

#[test]
fn a_hash_must_start_a_word() {
    assert!(names("a#b").is_empty());
    assert!(names("(#b)").is_empty());
    assert!(names("*#b*").is_empty());
    assert!(names("\\#b").is_empty());
    assert!(names("see http://x.y/#frag now").is_empty());
    assert!(names("c# and f#").is_empty());
}

#[test]
fn names_follow_the_grammar() {
    assert!(names("#5").is_empty(), "digit first");
    assert!(names("#-x").is_empty());
    assert!(names("# x").is_empty());
    assert!(names("##x").is_empty());
    assert!(
        names("#caf\u{e9}").is_empty(),
        "non-ASCII continues the word"
    );
    assert!(names("#a/b").is_empty());
    assert!(names("#a#b").is_empty());
    assert_eq!(
        names("#tag-"),
        ["tag"],
        "trailing hyphen is not part of the name"
    );
    let long = format!("#a{}", "b".repeat(MAX_NAME_BYTES));
    assert!(names(&long).is_empty(), "too long");
    let ok = format!("#a{}", "b".repeat(MAX_NAME_BYTES - 1));
    assert_eq!(names(&ok).len(), 1);
    assert!(is_valid_name("project-a"));
    assert!(!is_valid_name("1x"));
    assert!(!is_valid_name("a b"));
}

#[test]
fn code_html_links_and_urls_are_skipped() {
    assert!(names("`#code` and ``a `#b` c``").is_empty());
    assert!(names("<span style=\"color:#fff\">x</span>").is_empty());
    assert!(names("<https://x.y/ #a>").is_empty());
    assert!(names("[see #here](http://x.y/#frag)").is_empty());
    assert!(names("![alt #x](a.png)").is_empty());
    assert!(names("[[Page #x]] and [[A/B|#c]]").is_empty());
    assert_eq!(names("`code` #real"), ["real"]);
    // Unclosed constructs are plain text.
    assert_eq!(names("` #a"), ["a"]);
    assert_eq!(names("[x #a"), ["a"]);
}

#[test]
fn hostile_input_is_bounded() {
    let many = "#a ".repeat(100_000);
    assert!(scan_tags(&many, 0).len() <= 64);
    let ticks = "`".repeat(50_000);
    assert!(scan_tags(&ticks, 0).is_empty());
    let brackets = "[".repeat(50_000);
    assert!(scan_tags(&brackets, 0).is_empty());
    let angles = "<a ".repeat(50_000);
    assert!(scan_tags(&angles, 0).is_empty());
    assert!(scan_tags("\\", 0).is_empty());
    assert_eq!(names("\u{1F600} #ok \u{e9}#no"), ["ok"]);
}

fn items(source: &str) -> Vec<(String, Vec<String>, Option<bool>)> {
    let doc = parse(source);
    tagged_items(source, &doc)
        .into_iter()
        .map(|i| {
            (
                i.text,
                i.tags.into_iter().map(|t| t.name).collect(),
                i.task.map(|t| t.checked),
            )
        })
        .collect()
}

#[test]
fn tags_attach_to_the_paragraph_that_holds_them() {
    let source = "First #important paragraph\n\nSecond one\n\n#question Third\n";
    let found = items(source);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].0, "First paragraph");
    assert_eq!(found[0].1, ["important"]);
    assert_eq!(found[1].0, "Third");
    let doc = parse(source);
    let list = tagged_items(source, &doc);
    assert_eq!(list[0].holder.slice(source), "First #important paragraph");
}

#[test]
fn list_items_and_task_items_are_tagged() {
    let source = "- plain\n- #idea shiny\n- [ ] open task\n- [x] done #important\n  - [ ] nested\n";
    let found = items(source);
    assert_eq!(found.len(), 4);
    assert_eq!(
        found[0],
        ("shiny".to_owned(), vec!["idea".to_owned()], None)
    );
    assert_eq!(found[1], ("open task".to_owned(), vec![], Some(false)));
    assert_eq!(
        found[2],
        ("done".to_owned(), vec!["important".to_owned()], Some(true))
    );
    assert_eq!(found[3].2, Some(false));
    let doc = parse(source);
    let boxes: Vec<&str> = tagged_items(source, &doc)
        .iter()
        .filter_map(|i| i.task.map(|t| t.span.slice(source)))
        .collect();
    assert_eq!(boxes, ["[ ]", "[x]", "[ ]"]);
}

#[test]
fn headings_code_blocks_html_and_tables_are_handled() {
    let source = "# Title #todo\n\n## #\n\n```\n#nope\n```\n\n<div>#nope</div>\n\n| a | b |\n|---|---|\n| #nope | x |\n\n> quoted #question\n";
    let found = items(source);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].1, ["todo"]);
    assert_eq!(found[1].1, ["question"]);
    assert!(matches!(
        parse(source).blocks[0].kind,
        BlockKind::Heading { .. }
    ));
}

#[test]
fn an_escaped_hash_is_not_a_tag_and_is_not_confused_with_one() {
    let source = "\\#nope and #yes\n";
    let doc = parse(source);
    let list = tagged_items(source, &doc);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].tags.len(), 1);
    assert_eq!(list[0].tags[0].name, "yes");
}

#[test]
fn stripping_takes_the_names_out_of_the_runs() {
    let source = "#important keep **bold** #question this\n";
    let doc = parse(source);
    let BlockKind::Paragraph { runs } = &doc.blocks[0].kind else {
        panic!("paragraph");
    };
    let tags = scan_tags(source, 0);
    let (stripped, removed) = strip_tags(runs, &tags, &|_| true).expect("same tokens");
    let text: String = stripped.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(text, "keep bold this");
    assert_eq!(removed, ["important", "question"]);
    // Only the names asked for are removed.
    let (partial, removed) = strip_tags(runs, &tags, &|n| n == "question").expect("same tokens");
    let text: String = partial.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(text, "#important keep bold this");
    assert_eq!(removed, ["question"]);
    // A disagreement with the source scan removes nothing.
    assert!(strip_tags(runs, &[], &|_| true).is_none());
}

#[test]
fn a_tag_alone_leaves_an_empty_paragraph() {
    let source = "#important\n";
    let doc = parse(source);
    let list = tagged_items(source, &doc);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].text, "");
}

#[test]
fn text_is_capped() {
    let source = format!("#a {}\n", "word ".repeat(500));
    let list = tagged_items(&source, &parse(&source));
    assert!(list[0].text.len() <= 300);
}
