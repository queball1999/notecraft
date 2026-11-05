use super::{remove_tags, toggle_tag, Ctx, FormatEdit};

fn ctx(text: &str, anchor: usize, head: usize) -> Ctx<'_> {
    Ctx {
        text,
        floor: 0,
        anchor,
        head,
        eol: "\n",
    }
}

fn apply(text: &str, edit: &FormatEdit) -> String {
    let mut out = text.to_owned();
    out.replace_range(edit.at..edit.at + edit.remove, &edit.insert);
    out
}

fn toggled(text: &str, at: usize, slug: &str) -> Option<String> {
    toggle_tag(&ctx(text, at, at), slug).map(|e| apply(text, &e))
}

#[test]
fn a_tag_goes_at_the_start_of_the_paragraph_and_comes_off_again() {
    let text = "Buy milk\n";
    let tagged = toggled(text, 3, "important").unwrap();
    assert_eq!(tagged, "#important Buy milk\n");
    assert_eq!(toggled(&tagged, 3, "important").unwrap(), text);
}

#[test]
fn tags_keep_the_order_they_were_applied_in() {
    let tagged = toggled("#important Buy milk\n", 15, "question").unwrap();
    assert_eq!(tagged, "#important #question Buy milk\n");
    assert_eq!(
        toggled(&tagged, 15, "important").unwrap(),
        "#question Buy milk\n"
    );
}

#[test]
fn the_tag_goes_after_list_markers_boxes_headings_and_quotes() {
    assert_eq!(toggled("- item\n", 3, "idea").unwrap(), "- #idea item\n");
    assert_eq!(
        toggled("- [x] done\n", 8, "idea").unwrap(),
        "- [x] #idea done\n"
    );
    assert_eq!(toggled("## Head\n", 4, "idea").unwrap(), "## #idea Head\n");
    assert_eq!(toggled("> quote\n", 4, "idea").unwrap(), "> #idea quote\n");
    assert_eq!(
        toggled("3. third\n", 4, "idea").unwrap(),
        "3. #idea third\n"
    );
}

#[test]
fn a_tag_in_the_middle_or_at_the_end_comes_off_cleanly() {
    assert_eq!(toggled("a #idea b\n", 1, "idea").unwrap(), "a b\n");
    assert_eq!(toggled("a b #idea\n", 1, "idea").unwrap(), "a b\n");
    assert_eq!(toggled("#idea\n", 2, "idea").unwrap(), "\n");
}

#[test]
fn a_tag_after_a_tag_only_paragraph_follows_it() {
    // Only tags on the line: the new one follows them.
    assert_eq!(
        toggled("#important\n", 3, "idea").unwrap(),
        "#important #idea\n"
    );
}

#[test]
fn an_empty_paragraph_gets_the_tag_and_a_space_for_typing() {
    let edit = toggle_tag(&ctx("\n", 0, 0), "idea").unwrap();
    assert_eq!(apply("\n", &edit), "#idea \n");
    assert_eq!(
        (edit.anchor, edit.head),
        (6, 6),
        "the caret follows the tag"
    );
}

#[test]
fn every_paragraph_of_a_selection_is_tagged_once() {
    let text = "one\ntwo\n\nthree\n- four\n";
    let edit = toggle_tag(&ctx(text, 0, text.len()), "idea").unwrap();
    assert_eq!(
        apply(text, &edit),
        "#idea one\ntwo\n\n#idea three\n- #idea four\n"
    );
    // A selection where only some have it adds it to the rest instead of removing.
    let mixed = "#idea one\n\ntwo\n";
    let edit = toggle_tag(&ctx(mixed, 0, mixed.len()), "idea").unwrap();
    assert_eq!(apply(mixed, &edit), "#idea one\n\n#idea two\n");
}

#[test]
fn a_wrapped_paragraph_is_tagged_at_its_first_line_and_found_from_any_line() {
    let text = "first line\nsecond line\n";
    let tagged = toggled(text, 15, "idea").unwrap();
    assert_eq!(tagged, "#idea first line\nsecond line\n");
    assert_eq!(toggled(&tagged, 20, "idea").unwrap(), text);
    let in_second = "first\nsecond #idea\n";
    assert_eq!(toggled(in_second, 2, "idea").unwrap(), "first\nsecond\n");
}

#[test]
fn code_fences_are_left_alone() {
    let text = "```\ncode\n```\n";
    assert!(toggle_tag(&ctx(text, 5, 5), "idea").is_none());
}

#[test]
fn remove_takes_known_tags_and_the_check_box() {
    let known = |n: &str| matches!(n, "important" | "question");
    let text = "#important #question #other keep\n";
    let edit = remove_tags(&ctx(text, 0, 0), &known).unwrap();
    assert_eq!(apply(text, &edit), "#other keep\n");
    let text = "- [ ] #important task\n";
    let edit = remove_tags(&ctx(text, 8, 8), &known).unwrap();
    assert_eq!(apply(text, &edit), "task\n");
    assert!(remove_tags(&ctx("plain\n", 0, 0), &known).is_none());
    let all = "#important #question\n";
    let edit = remove_tags(&ctx(all, 0, 0), &known).unwrap();
    assert_eq!(apply(all, &edit), "\n");
}
