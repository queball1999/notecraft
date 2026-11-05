//! Equation tests: parsing, writing back, bounds, and typesetting.

use crate::block::{BlockKind, Math};
use crate::math::{self, MathError, MAX_FORMULA_BYTES};
use crate::{parse, runs_to_markdown, Run};

fn first_runs(text: &str) -> Vec<Run> {
    match parse(text).blocks.into_iter().next().map(|b| b.kind) {
        Some(BlockKind::Paragraph { runs }) => runs,
        other => panic!("expected a paragraph, got {other:?}"),
    }
}

#[test]
fn inline_math_is_a_run_with_its_source() {
    let runs = first_runs("Area is $\\pi r^2$ here.\n");
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[1].math, Math::Inline);
    assert_eq!(runs[1].text, "\\pi r^2");
    assert_eq!(runs[0].text, "Area is ");
    assert_eq!(runs[2].math, Math::None);
}

#[test]
fn display_math_is_a_display_run() {
    let runs = first_runs("$$\\frac{a}{b}$$\n");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].math, Math::Display);
    assert_eq!(runs[0].text, "\\frac{a}{b}");
}

#[test]
fn display_math_over_several_lines_keeps_its_line_breaks() {
    let runs = first_runs("$$\nx^2 +\ny^2\n$$\n");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].math, Math::Display);
    assert_eq!(runs[0].text, "\nx^2 +\ny^2\n");
    assert_eq!(runs_to_markdown(&runs), "$$\nx^2 +\ny^2\n$$");
}

#[test]
fn prices_are_not_math() {
    for text in ["It costs $5 and $10 today.\n", "Only $5.\n", "a $ b $ c\n"] {
        let runs = first_runs(text);
        assert!(runs.iter().all(|r| r.math == Math::None), "{text}");
    }
}

#[test]
fn an_escaped_dollar_is_text_and_survives_writing() {
    let runs = first_runs("Pay \\$5 and \\$6.\n");
    assert!(runs.iter().all(|r| r.math == Math::None));
    let written = runs_to_markdown(&runs);
    assert_eq!(written, "Pay \\$5 and \\$6.");
    let again = first_runs(&format!("{written}\n"));
    assert!(again.iter().all(|r| r.math == Math::None));
}

#[test]
fn formulas_round_trip_through_the_canonical_writer() {
    for text in [
        "Area is $\\pi r^2$ here.",
        "$$\\int_0^1 x\\,dx$$",
        "a $x$ b $$y$$ c",
    ] {
        let runs = first_runs(&format!("{text}\n"));
        assert_eq!(runs_to_markdown(&runs), text);
    }
}

#[test]
fn code_spans_and_code_blocks_keep_their_dollars() {
    let runs = first_runs("`$x$` and `$$y$$`\n");
    assert!(runs.iter().all(|r| r.math == Math::None));
    let doc = parse("```\n$x$\n```\n");
    assert!(matches!(
        doc.blocks.first().map(|b| &b.kind),
        Some(BlockKind::Code { .. })
    ));
}

#[test]
fn a_formula_over_the_limit_stays_plain_text() {
    let big = "x".repeat(MAX_FORMULA_BYTES + 1);
    let runs = first_runs(&format!("${big}$\n"));
    assert!(runs.iter().all(|r| r.math == Math::None));
    assert!(runs.iter().any(|r| r.text.contains(&big)));
    let ok = "x".repeat(MAX_FORMULA_BYTES);
    let runs = first_runs(&format!("${ok}$\n"));
    assert!(runs.iter().any(|r| r.math == Math::Inline));
}

#[test]
fn typesetting_gives_a_size_and_an_svg() {
    let t = math::typeset("\\frac{a}{b} + \\sqrt{x}", false, 16.0, 0x112233).expect("typeset");
    assert!(t.svg.starts_with("<?xml"));
    assert!(t.svg.contains("<path"));
    assert!(t.svg.contains("#112233"));
    assert!(t.width_em > 1.0 && t.height_em > 0.5);
    let display = math::typeset("\\sum_{i=0}^n i", true, 16.0, 0).expect("display");
    let text = math::typeset("\\sum_{i=0}^n i", false, 16.0, 0).expect("text");
    assert!(display.height_em + display.depth_em > text.height_em + text.depth_em);
}

#[test]
fn a_formula_that_is_not_math_is_an_error_not_a_panic() {
    assert_eq!(
        math::typeset("   ", false, 16.0, 0),
        Err(MathError::OutOfBounds)
    );
    assert!(matches!(
        math::typeset("\\notacommand{", false, 16.0, 0),
        Err(MathError::Invalid(_))
    ));
}

mod formula_at_tests {
    use crate::math::{formula_at, MAX_FORMULA_BYTES};

    fn at(text: &str, marker: &str, extra: usize) -> Option<crate::math::FormulaAt> {
        formula_at(text, text.find(marker)? + extra)
    }

    #[test]
    fn the_caret_inside_an_inline_formula_finds_it() {
        let text = "Area is $\\pi r^2$ here.\n";
        let f = at(text, "r^2", 1).expect("formula");
        assert_eq!(f.text, "\\pi r^2");
        assert!(!f.display);
        assert_eq!(&text[f.span.clone()], "$\\pi r^2$");
        assert_eq!(&text[f.source.clone()], "\\pi r^2");
    }

    #[test]
    fn the_edges_just_after_the_opening_and_closing_dollar_count() {
        let text = "a $x$ b\n";
        assert!(formula_at(text, 2).is_none(), "before the opening dollar");
        assert!(formula_at(text, 3).is_some(), "after the opening dollar");
        assert!(formula_at(text, 4).is_some(), "before the closing dollar");
        assert!(formula_at(text, 5).is_some(), "after the closing dollar");
        assert!(formula_at(text, 6).is_none(), "past it");
    }

    #[test]
    fn a_display_formula_over_several_lines_is_found() {
        let text = "intro\n\n$$\n\\frac{a}{b}\n$$\n\nafter $y$\n";
        let f = at(text, "frac", 2).expect("formula");
        assert!(f.display);
        assert_eq!(&text[f.span], "$$\n\\frac{a}{b}\n$$");
    }

    #[test]
    fn the_right_formula_of_two_in_a_paragraph() {
        let text = "$a$ and $b$ and $c$\n";
        assert_eq!(at(text, "b$", 1).expect("b").text, "b");
        assert_eq!(at(text, "c$", 1).expect("c").text, "c");
        assert_eq!(formula_at(text, 1).expect("a").text, "a");
        assert!(at(text, " and $b", 2).is_none());
    }

    #[test]
    fn formulas_in_lists_quotes_headings_and_tables_are_found() {
        for text in [
            "- item $q$ end\n",
            "> quote $q$ end\n",
            "# Head $q$ end\n",
            "| a |\n|---|\n| $q$ |\n",
        ] {
            let f = at(text, "q$", 1).unwrap_or_else(|| panic!("no formula in {text:?}"));
            assert_eq!(f.text, "q");
        }
    }

    #[test]
    fn no_formula_in_code_escapes_or_plain_text() {
        assert!(at("```\n$x$\n```\n", "x", 1).is_none());
        assert!(at("use `$x$` here\n", "x", 1).is_none());
        assert!(at("cost \\$5 and \\$6\n", "5", 1).is_none());
        assert!(formula_at("no math at all\n", 4).is_none());
        assert!(formula_at("", 0).is_none());
    }

    #[test]
    fn other_paragraphs_are_not_searched() {
        let text = "one $a$\n\ntwo\n";
        assert!(at(text, "two", 1).is_none());
    }

    #[test]
    fn bad_offsets_and_hostile_text_do_not_panic() {
        let text = "caf\u{e9} $x$\n";
        for offset in 0..text.len() + 3 {
            let _ = formula_at(text, offset);
        }
        let long = format!("${}$", "x".repeat(MAX_FORMULA_BYTES + 10));
        assert!(formula_at(&long, 10).is_none());
        let huge = "$a$ ".repeat(40_000);
        assert!(formula_at(&huge, 10).is_none());
        let deep = format!("${}x{}$", "{".repeat(500), "}".repeat(500));
        let _ = formula_at(&deep, 5);
    }
}
