//! Tests for the formatting edits. A `|` in a test string marks the caret, or two of them the
//! ends of the selection.

use super::*;
use crate::markup::Mark;
use crate::Align;

fn parse_marked(marked: &str) -> (String, usize, usize) {
    let positions: Vec<usize> = marked.match_indices('|').map(|(i, _)| i).collect();
    let text = marked.replace('|', "");
    match positions.as_slice() {
        [a] => (text, *a, *a),
        [a, b] => (text, *a, b - 1),
        _ => panic!("mark the caret with one | or the selection with two"),
    }
}

fn show(text: &str, anchor: usize, head: usize) -> String {
    if anchor == head {
        format!("{}|{}", &text[..anchor], &text[anchor..])
    } else {
        let (lo, hi) = (anchor.min(head), anchor.max(head));
        format!("{}|{}|{}", &text[..lo], &text[lo..hi], &text[hi..])
    }
}

fn ctx<'a>(text: &'a str, a: usize, h: usize) -> Ctx<'a> {
    Ctx {
        text,
        floor: 0,
        anchor: a,
        head: h,
        eol: "\n",
    }
}

/// Run `f` on the marked text and show the result with the selection marked.
fn run(marked: &str, f: impl Fn(&Ctx<'_>) -> Option<FormatEdit>) -> String {
    let (text, a, h) = parse_marked(marked);
    match f(&ctx(&text, a, h)) {
        None => format!("unchanged {}", show(&text, a, h)),
        Some(edit) => {
            let mut out = text.clone();
            out.replace_range(edit.at..edit.at + edit.remove, &edit.insert);
            show(&out, edit.anchor, edit.head)
        }
    }
}

fn toggle(marked: &str, mark: Mark) -> String {
    run(marked, |c| toggle_mark(c, mark))
}

#[test]
fn bold_wraps_the_selection_and_keeps_it_selected() {
    assert_eq!(toggle("a |big| day", Mark::Bold), "a **|big|** day");
}

#[test]
fn bold_on_a_caret_takes_the_word() {
    assert_eq!(toggle("a bi|g day", Mark::Bold), "a **bi|g** day");
    assert_eq!(toggle("a big| day", Mark::Bold), "a **big|** day");
}

#[test]
fn toggling_twice_gives_back_the_text() {
    let once = toggle("a |big| day", Mark::Bold);
    assert_eq!(toggle(&once, Mark::Bold), "a |big| day");
    let once = toggle("a bi|g day", Mark::Italic);
    assert_eq!(toggle(&once, Mark::Italic), "a bi|g day");
}

#[test]
fn removing_a_mark_from_part_of_a_span_splits_it() {
    assert_eq!(
        toggle("**one |two| three**", Mark::Bold),
        "**one** |two| **three**"
    );
    assert_eq!(toggle("**|one| two**", Mark::Bold), "|one| **two**");
    assert_eq!(toggle("**one |two|**", Mark::Bold), "**one** |two|");
}

#[test]
fn selecting_the_delimiters_too_still_toggles_off() {
    assert_eq!(toggle("a |**big**| day", Mark::Bold), "a |big| day");
}

#[test]
fn a_selection_that_is_partly_bold_becomes_all_bold() {
    assert_eq!(toggle("|**one** two|", Mark::Bold), "**|one two|**");
}

#[test]
fn spaces_at_the_edges_stay_outside() {
    assert_eq!(toggle("a| big |day", Mark::Bold), "a| **big** |day");
}

#[test]
fn italic_inside_bold_nests() {
    assert_eq!(
        toggle("**one |two| three**", Mark::Italic),
        "**one *|two|* three**"
    );
}

#[test]
fn strike_underline_and_scripts_use_the_format_spec_syntax() {
    assert_eq!(toggle("|x|", Mark::Strike), "~~|x|~~");
    assert_eq!(toggle("|x|", Mark::Underline), "<u>|x|</u>");
    assert_eq!(toggle("H|2|O", Mark::Subscript), "H<sub>|2|</sub>O");
    assert_eq!(toggle("x|2|", Mark::Superscript), "x<sup>|2|</sup>");
}

#[test]
fn subscript_replaces_superscript() {
    assert_eq!(toggle("<sup>|2|</sup>", Mark::Subscript), "<sub>|2|</sub>");
}

#[test]
fn highlight_and_color_marks() {
    assert_eq!(toggle("|hi|", Mark::Highlight), "==|hi|==");
    assert_eq!(
        run("|hi|", |c| set_mark(c, Mark::Highlight, Some("#00ff00"))),
        "<mark style=\"background:#00FF00\">|hi|</mark>"
    );
    assert_eq!(
        run("|hi|", |c| set_mark(c, Mark::Color, Some("#e53935"))),
        "<span style=\"color:#E53935\">|hi|</span>"
    );
    assert_eq!(
        run("|hi|", |c| set_mark(c, Mark::Size, Some("14"))),
        "<span style=\"font-size:14pt\">|hi|</span>"
    );
    assert_eq!(
        run("|hi|", |c| set_mark(c, Mark::Family, Some("Courier New"))),
        "<span style=\"font-family:Courier New\">|hi|</span>"
    );
}

#[test]
fn setting_a_value_replaces_the_old_one_and_none_removes_it() {
    assert_eq!(
        run("<span style=\"color:#FF0000\">|hi|</span>", |c| set_mark(
            c,
            Mark::Color,
            Some("#0000ff")
        )),
        "<span style=\"color:#0000FF\">|hi|</span>"
    );
    assert_eq!(
        run("<span style=\"color:#FF0000\">|hi|</span>", |c| set_mark(
            c,
            Mark::Color,
            None
        )),
        "|hi|"
    );
}

#[test]
fn a_bad_value_changes_nothing() {
    assert_eq!(
        run("|hi|", |c| set_mark(c, Mark::Color, Some("red"))),
        "unchanged |hi|"
    );
    assert_eq!(
        run("|hi|", |c| set_mark(c, Mark::Family, Some("a\"><b"))),
        "unchanged |hi|"
    );
}

#[test]
fn a_caret_off_a_word_types_into_a_new_pair() {
    assert_eq!(toggle("a |", Mark::Bold), "a **|**");
    assert_eq!(toggle("a **|**", Mark::Bold), "a |");
}

#[test]
fn multi_line_selection_formats_each_line() {
    assert_eq!(toggle("|one\ntwo|", Mark::Bold), "**|one**\n**two|**");
    assert_eq!(
        toggle("|- one\n- two|", Mark::Italic),
        "|- *one*
- *two|*"
    );
}

#[test]
fn the_selection_end_at_a_line_start_leaves_that_line_alone() {
    assert_eq!(
        toggle(
            "|one
|two",
            Mark::Bold
        ),
        "**|one**
|two"
    );
}

#[test]
fn code_blocks_and_markers_are_left_alone() {
    assert_eq!(
        toggle("```\n|code|\n```", Mark::Bold),
        "unchanged ```\n|code|\n```"
    );
    assert_eq!(
        toggle("<!-- notecraft:container id=c1 -->|\n|text", Mark::Bold),
        "unchanged <!-- notecraft:container id=c1 -->|\n|text"
    );
}

#[test]
fn clearing_removes_marks_headings_and_alignment() {
    let f = |c: &Ctx<'_>| clear_formatting(c);
    assert_eq!(run("|**a** ~~b~~ <u>c</u>|", f), "|a b c|");
    assert_eq!(run("## He|llo **x**", f), "He|llo x");
    assert_eq!(run("<!-- notecraft:align=center -->\nhi|", f), "hi|");
}

#[test]
fn headings_replace_the_prefix() {
    let h = |n| move |c: &Ctx<'_>| apply_style(c, Style::Heading(n));
    assert_eq!(run("Tit|le", h(2)), "## Tit|le");
    assert_eq!(run("# Tit|le", h(3)), "### Tit|le");
    assert_eq!(run("- It|em", h(1)), "# It|em");
    assert_eq!(run("## Tit|le", h(2)), "unchanged ## Tit|le");
    assert_eq!(run("|", h(1)), "# |");
}

#[test]
fn normal_removes_heading_quote_and_fences() {
    let n = |c: &Ctx<'_>| apply_style(c, Style::Normal);
    assert_eq!(run("## Ti|tle", n), "Ti|tle");
    assert_eq!(run("> qu|ote", n), "qu|ote");
    assert_eq!(run("```\nco|de\n```", n), "co|de");
}

#[test]
fn quote_and_code_styles() {
    let q = |c: &Ctx<'_>| apply_style(c, Style::Quote);
    assert_eq!(run("|a\nb|", q), "> |a\n> b|");
    assert_eq!(run("> a|", q), "unchanged > a|");
    let code = |c: &Ctx<'_>| apply_style(c, Style::Code);
    assert_eq!(run("|a\nb|\n", code), "```\n|a\nb|\n```\n");
    assert_eq!(run("a|", code), "```\na|\n```");
    assert_eq!(run("```\na|\n```", code), "unchanged ```\na|\n```");
}

#[test]
fn citation_is_italic() {
    let c = |c: &Ctx<'_>| apply_style(c, Style::Citation);
    assert_eq!(run("Some so|urce", c), "*|Some source|*");
}

#[test]
fn bullets_toggle() {
    assert_eq!(run("|a\nb|", toggle_bullets), "- |a\n- b|");
    assert_eq!(run("- a|", toggle_bullets), "a|");
    assert_eq!(run("1. a|", toggle_bullets), "- a|");
    assert_eq!(run("- [ ] a|", toggle_bullets), "- a|");
    assert_eq!(run("# a|", toggle_bullets), "- a|");
}

#[test]
fn numbering_toggles_and_continues() {
    assert_eq!(run("|a\nb|", toggle_numbering), "1. |a\n2. b|");
    assert_eq!(run("1. a\n2. b|", toggle_numbering), "1. a\nb|");
    assert_eq!(run("- a|", toggle_numbering), "1. a|");
    assert_eq!(run("3. a\nb|", toggle_numbering), "3. a\n4. b|");
}

#[test]
fn todo_toggles() {
    assert_eq!(run("a|", toggle_todo), "- [ ] a|");
    assert_eq!(run("- a|", toggle_todo), "- [ ] a|");
    assert_eq!(run("- [ ] a|", toggle_todo), "a|");
    assert_eq!(run("- [x] a|", toggle_todo), "a|");
    assert_eq!(run("|a\nb|", toggle_todo), "- [ ] |a\n- [ ] b|");
}

#[test]
fn indent_moves_list_items_and_quotes() {
    let i = |c: &Ctx<'_>| indent(c, false);
    let o = |c: &Ctx<'_>| indent(c, true);
    assert_eq!(run("- a|", i), "  - a|");
    assert_eq!(run("1. a|", i), "   1. a|");
    assert_eq!(run("  - a|", o), "- a|");
    assert_eq!(run("- a|", o), "unchanged - a|");
    assert_eq!(run("> a|", i), "> > a|");
    assert_eq!(run("> > a|", o), "> a|");
    assert_eq!(run("plain|", i), "unchanged plain|");
}

#[test]
fn alignment_comment_above_the_paragraph() {
    let center = |c: &Ctx<'_>| set_align(c, Align::Center);
    let left = |c: &Ctx<'_>| set_align(c, Align::Left);
    assert_eq!(
        run("hel|lo", center),
        "<!-- notecraft:align=center -->\nhel|lo"
    );
    assert_eq!(
        run("<!-- notecraft:align=right -->\nhel|lo", center),
        "<!-- notecraft:align=center -->\nhel|lo"
    );
    assert_eq!(
        run("<!-- notecraft:align=right -->\nhel|lo", left),
        "hel|lo"
    );
    assert_eq!(run("hel|lo", left), "unchanged hel|lo");
    // A paragraph of two lines gets one comment, above its first line.
    assert_eq!(
        run("one\ntw|o", center),
        "<!-- notecraft:align=center -->\none\ntw|o"
    );
    assert_eq!(run("- it|em", center), "unchanged - it|em");
}

#[test]
fn state_reports_marks_blocks_and_alignment() {
    let at = |marked: &str| {
        let (text, a, h) = parse_marked(marked);
        state_at(&ctx(&text, a, h))
    };
    let s = at("**bo|ld** text");
    assert!(s.bold && !s.italic);
    let s = at("**bold**| text");
    assert!(!s.bold);
    let s = at("**|bold|** text");
    assert!(s.bold);
    let s = at("**bold** |text|");
    assert!(!s.bold);
    let s = at("## ***hi|***");
    assert!(s.bold && s.italic);
    assert_eq!(s.heading, 2);
    let s = at("<span style=\"color:#FF0000\"><u>x|</u></span>");
    assert_eq!(s.color.as_deref(), Some("#FF0000"));
    assert!(s.underline);
    let s = at("==hi|==");
    assert_eq!(s.highlight.as_deref(), Some("#FFF176"));
    let s = at("> - [x] a|");
    assert!(s.quote);
    assert_eq!(s.list, ListState::Task);
    let s = at("<!-- notecraft:align=center -->\nx|");
    assert_eq!(s.align, Align::Center);
    let s = at("```\nco|de\n```");
    assert!(s.code);
}

#[test]
fn edits_are_trimmed_to_the_bytes_that_change() {
    let (text, a, h) = parse_marked("one two three four |five|");
    let edit = toggle_mark(&ctx(&text, a, h), Mark::Bold).expect("edit");
    assert_eq!(edit.at, text.len() - 4);
    assert_eq!(edit.remove, 4);
    assert_eq!(edit.insert, "**five**");
}

#[test]
fn hostile_input_does_not_panic() {
    let nasty = "**".repeat(5000);
    let marked = format!("|{nasty}|");
    for mark in Mark::ALL {
        let (text, a, h) = parse_marked(&marked);
        let _ = toggle_mark(&ctx(&text, a, h), mark);
    }
    let text = "\u{1F600}**\u{301}*_~~==<u>`";
    for at in 0..=text.len() {
        for end in at..=text.len() {
            let c = ctx(text, at, end);
            let _ = toggle_mark(&c, Mark::Bold);
            let _ = clear_formatting(&c);
            let _ = apply_style(&c, Style::Code);
            let _ = toggle_numbering(&c);
            let _ = set_align(&c, Align::Right);
            let _ = state_at(&c);
        }
    }
}
