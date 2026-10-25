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
