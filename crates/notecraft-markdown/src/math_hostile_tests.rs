//! Hostile equation input (PR 2.7): none of it may panic, hang, or grow without bound.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::block::Math;
use crate::math::{self, MathError, MAX_FORMULA_BYTES, MAX_NESTING_DEPTH};
use crate::{parse, runs_to_markdown, BlockKind, Run};

/// Run `f` on a helper thread and fail the test if it does not finish in `limit`.
fn within<T: Send + 'static>(limit: Duration, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(limit)
        .expect("finished within the limit without panicking")
}

fn every_run(text: &str) -> Vec<Run> {
    let mut out = Vec::new();
    for block in parse(text).blocks {
        if let BlockKind::Paragraph { runs } | BlockKind::Heading { runs, .. } = block.kind {
            out.extend(runs);
        }
    }
    out
}

/// A tiny deterministic generator so the "random" tests repeat.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
}

#[test]
fn unterminated_and_stray_dollars_parse_without_a_formula() {
    for text in [
        "$", "$$", "$$$", "$$$$", "$x", "$$x", "x$", "x$$", "$ $", "$\n$", "$$\n\n$$", "\\$x\\$",
        "$x\\$", "$`x$`",
    ] {
        let runs = every_run(&format!("{text}\n"));
        for run in &runs {
            if run.math != Math::None {
                assert!(!run.text.trim().is_empty(), "{text:?}");
            }
        }
        // Whatever was read, writing it back gives something that reads the same way.
        let written = runs_to_markdown(&runs);
        let again = every_run(&format!("{written}\n"));
        assert_eq!(
            again.iter().filter(|r| r.math != Math::None).count(),
            runs.iter().filter(|r| r.math != Math::None).count(),
            "{text:?} -> {written:?}"
        );
    }
}

#[test]
fn a_hundred_thousand_dollar_signs_parse_quickly() {
    let started = Instant::now();
    for text in [
        "$".repeat(100_000),
        "$a".repeat(50_000),
        "$a$ ".repeat(25_000),
        "$$a$$ ".repeat(16_000),
        "$$\n".repeat(30_000),
        "$ a$".repeat(25_000),
    ] {
        within(Duration::from_secs(60), move || {
            let _ = parse(&text);
        });
    }
    assert!(
        started.elapsed() < Duration::from_secs(120),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn the_size_limit_is_exact() {
    let at_limit = "x".repeat(MAX_FORMULA_BYTES);
    let over = "x".repeat(MAX_FORMULA_BYTES + 1);
    assert!(math::typeset(&at_limit, false, 16.0, 0).is_ok());
    assert_eq!(
        math::typeset(&over, false, 16.0, 0),
        Err(MathError::OutOfBounds)
    );
    // A multibyte formula is counted in bytes, not characters.
    let wide = "\u{3b1}".repeat(MAX_FORMULA_BYTES / 2 + 1);
    assert_eq!(
        math::typeset(&wide, false, 16.0, 0),
        Err(MathError::OutOfBounds)
    );
    for display in [false, true] {
        let marks = if display { "$$" } else { "$" };
        let runs = every_run(&format!("{marks}{at_limit}{marks}\n"));
        assert!(runs.iter().any(|r| r.math != Math::None));
        let runs = every_run(&format!("{marks}{over}{marks}\n"));
        assert!(runs.iter().all(|r| r.math == Math::None));
        // The oversize source is still all there, as text.
        assert!(runs.iter().any(|r| r.text.contains(&over)));
    }
}

#[test]
fn nesting_past_the_limit_is_an_error_and_below_it_is_typeset() {
    let nest = |open: &str, close: &str, depth: usize| {
        format!("{}x{}", open.repeat(depth), close.repeat(depth))
    };
    for (open, close) in [
        ("{", "}"),
        ("\\frac{", "}{1}"),
        ("\\sqrt{", "}"),
        ("x^{", "}"),
    ] {
        let deep = nest(open, close, MAX_NESTING_DEPTH * 4);
        if deep.len() <= MAX_FORMULA_BYTES {
            let result = within(Duration::from_secs(60), move || {
                math::typeset(&deep, false, 16.0, 0)
            });
            assert!(
                matches!(result, Err(MathError::Invalid(_))),
                "{open}: {:?}",
                result.map(|t| t.width_em)
            );
        }
        let shallow = nest(open, close, 8);
        let result = within(Duration::from_secs(60), move || {
            math::typeset(&shallow, false, 16.0, 0)
        });
        assert!(result.is_ok(), "{open}: {:?}", result.err());
    }
}

#[test]
fn ten_thousand_open_braces_do_not_overflow_the_stack() {
    let text = format!("${}$", "{".repeat(2000));
    let runs = within(Duration::from_secs(60), move || every_run(&text));
    assert!(runs.iter().any(|r| r.math == Math::Inline));
    let bomb = "{".repeat(MAX_FORMULA_BYTES);
    let result = within(Duration::from_secs(60), move || {
        math::typeset(&bomb, false, 16.0, 0)
    });
    assert!(result.is_err());
    let bomb = "\\frac".repeat(MAX_FORMULA_BYTES / 5);
    let result = within(Duration::from_secs(60), move || {
        math::typeset(&bomb, false, 16.0, 0)
    });
    assert!(result.is_err());
}

#[test]
fn absurd_sizes_and_numbers_are_errors_or_finite_results() {
    for src in [
        "\\hspace{99999999999999999999999999999999em}x",
        "\\hspace{-99999999999999999999999999999999em}x",
        "\\rule{1e999}{1e999}",
        "\\rule{0.00000000000000000000000000001em}{9999999999999em}",
        "\\kern 9999999999999999999999pt x",
        "x^{x^{x^{x^{x^{x^{x^{x^{x^{x}}}}}}}}}",
        "\\sqrt[9999999999999999999999]{x}",
        "\\underbrace{\\overbrace{\\underbrace{x}}}",
        "\\begin{matrix}a&b\\\\c&d\\end{matrix}",
        "\\begin{matrix}a&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&&b\\end{matrix}",
        "\\left( \\left( \\left( x",
        "\\right) x \\left",
        "\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\",
        "&&&&&&&&&&&&",
        "\\frac{\\frac{\\frac{1}{2}}{\\frac{3}{4}}}{\\frac{\\frac{5}{6}}{\\frac{7}{8}}}",
        "\\mathrm{\\mathbf{\\mathit{\\mathcal{x}}}}",
        "\\text{\\text{\\text{x}}}",
        "\\\u{0}\u{0}\u{1}",
        "\u{10FFFF}\u{FFFD}\u{200B}\u{202E}x",
    ] {
        let owned = src.to_owned();
        let result = within(Duration::from_secs(120), move || {
            math::typeset(&owned, true, 16.0, 0)
        });
        if let Ok(t) = result {
            assert!(t.width_em.is_finite() && t.height_em.is_finite() && t.depth_em.is_finite());
            assert!(t.svg.len() < 32 * 1024 * 1024, "{src:?}");
        }
    }
}

#[test]
fn macro_definitions_cannot_blow_up() {
    // A doubling macro chain: 4 KiB of definitions would expand to 2^100 symbols if followed.
    let mut src = String::from("\\newcommand{\\a}{xx}");
    for i in 0..60u8 {
        let (prev, next) = (char::from(b'a' + i % 25), char::from(b'a' + (i + 1) % 25));
        src.push_str(&format!("\\newcommand{{\\{next}{i}}}{{\\{prev}\\{prev}}}"));
    }
    src.push_str("\\a");
    src.truncate(MAX_FORMULA_BYTES);
    let started = Instant::now();
    let result = within(Duration::from_secs(120), move || {
        math::typeset(&src, false, 16.0, 0)
    });
    assert!(started.elapsed() < Duration::from_secs(120));
    if let Ok(t) = result {
        assert!(t.svg.len() < 32 * 1024 * 1024);
    }
    for src in [
        "\\def\\a{\\a}\\a",
        "\\newcommand{\\a}{\\a\\a}\\a",
        "\\let\\a\\a\\a",
        "\\input{/etc/passwd}",
        "\\include{x}",
        "\\write18{x}",
        "\\url{http://example.com}",
        "\\href{javascript:alert(1)}{x}",
    ] {
        let owned = src.to_owned();
        let result = within(Duration::from_secs(120), move || {
            math::typeset(&owned, false, 16.0, 0)
        });
        // Nothing here may reach the file system or the network, so the SVG is only shapes.
        if let Ok(t) = result {
            assert!(
                !t.svg.contains("<script") && !t.svg.contains("href"),
                "{src}"
            );
            assert!(
                !t.svg.contains("<image") && !t.svg.contains("<text"),
                "{src}"
            );
        }
    }
}

#[test]
fn typeset_output_is_only_paths_and_rectangles() {
    let t = math::typeset(
        "\\frac{\\sqrt{a^2+b^2}}{\\sum_{i}^{n}\\int x\\,dx} \\color{red}{y}",
        true,
        20.0,
        0xABCDEF,
    )
    .expect("typeset");
    for forbidden in [
        "<script",
        "<image",
        "<text",
        "<foreignObject",
        "href",
        "onload",
        "<use",
    ] {
        assert!(!t.svg.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn random_bytes_never_panic_the_parser_or_the_typesetter() {
    let alphabet: Vec<&str> = vec![
        "$",
        "$$",
        "\\",
        "{",
        "}",
        "^",
        "_",
        "&",
        "\\\\",
        "\\frac",
        "\\sqrt",
        "\\left",
        "\\right",
        "\\begin{matrix}",
        "\\end{matrix}",
        "x",
        "1",
        " ",
        "\n",
        "`",
        "*",
        "[",
        "]",
        "\u{3b1}",
        "\u{1F600}",
        "%",
        "#",
        "~",
        "<",
        ">",
        "\\text",
        "\\color",
        "\\sum",
        "\\int",
        "\\over",
    ];
    let mut rng = Lcg(0x5EED);
    for round in 0..300 {
        let mut text = String::new();
        let pieces = 1 + (rng.next() % 60) as usize;
        for _ in 0..pieces {
            let pick = (rng.next() as usize) % alphabet.len();
            text.push_str(alphabet.get(pick).copied().unwrap_or("x"));
        }
        let page = format!("{text}\n");
        let formula = text.clone();
        within(Duration::from_secs(120), move || {
            let runs = every_run(&page);
            let written = runs_to_markdown(&runs);
            let _ = every_run(&format!("{written}\n"));
            let _ = math::typeset(&formula, round % 2 == 0, 16.0, 0);
        });
    }
}

#[test]
fn formulas_inside_other_blocks_do_not_confuse_the_parser() {
    for text in [
        "> $x$ and $$y$$\n",
        "- $x$\n- $$y$$\n",
        "| a | b |\n|---|---|\n| $x$ | $$y$$ |\n",
        "# $x$\n",
        "**$x$** and *$y$* and ~~$z$~~ and [$w$](https://example.com)\n",
        "![$x$](a.png)\n",
        "<u>$x$</u>\n",
        "```\n$$\nx\n```\n",
        "    $$x$$\n",
    ] {
        let doc = parse(text);
        let _ = crate::blocks_to_markdown(&doc.blocks, crate::LineEnding::Lf);
    }
}
