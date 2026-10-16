//! Bounded property tests that mirror the `cargo fuzz` targets in `fuzz/` (PR 1.14). They run
//! on stable in `cargo test`: seeded mutations of the testdata corpus plus hostile shapes,
//! a few thousand cases per parser, the same checks as the fuzz targets.

#[path = "../../../fuzz/checks/markdown.rs"]
mod checks;
#[path = "../../../fuzz/checks/mutate.rs"]
mod mutate;

use mutate::{mutate, noise, root, seeds_in, Rng};

const CASES: usize = 3_000;

const TOKENS: &[&str] = &[
    "---\n",
    "...\n",
    "\r\n",
    "\u{feff}",
    "title: x\n",
    "tags: [a, b]\n",
    "tags:\n  - a\n",
    "key: |\n  text\n",
    "- ",
    "> ",
    "```",
    "~~~",
    "    ",
    "1. ",
    "- [ ] ",
    "| a | b |\n|---|---|\n",
    "# ",
    "<!-- notecraft:container id=c1 -->\n",
    "<!-- notecraft:container id=c0 -->\n",
    "<!-- notecraft:container id=\"x y\" -->\n",
    "<!-- notecraft:container -->\n",
    "[[page]]",
    "[a](b)",
    "![i](p.png)",
    "<div>",
    "**",
    "*",
    "~~",
    "`",
    "\\",
    "&amp;",
    "\u{e9}",
    "\u{1f600}",
];

fn seeds() -> Vec<Vec<u8>> {
    let root = root().join("testdata");
    let mut all = seeds_in(&root.join("markdown"), &[".md"]);
    all.extend(seeds_in(&root.join("notecraft"), &[".notecraft"]));
    all.extend(seeds_in(&root.join("notebooks"), &[".md", ".notecraft"]));
    assert!(!all.is_empty(), "no seed files found under testdata");
    all
}

fn run(name: &str, seed: u64, check: fn(&[u8])) {
    let seeds = seeds();
    for seed_file in &seeds {
        check(seed_file);
    }
    let mut rng = Rng::new(seed);
    for i in 0..mutate::cases(CASES) {
        let data = if i % 5 == 4 {
            noise(&mut rng, 2_000)
        } else {
            mutate(&mut rng, &seeds, TOKENS)
        };
        let result = std::panic::catch_unwind(|| check(&data));
        assert!(
            result.is_ok(),
            "{name}: case {i} failed; input: {:?}",
            String::from_utf8_lossy(&data)
        );
    }
}

#[test]
fn markdown_blocks_survive_mutated_input() {
    run("markdown", 0x4d44, checks::check_markdown);
}

#[test]
fn frontmatter_survives_mutated_input() {
    run("frontmatter", 0x464d, checks::check_frontmatter);
}

#[test]
fn container_markers_survive_mutated_input() {
    run("containers", 0x434f, checks::check_containers);
}

/// Shapes that stress the documented limits.
#[test]
fn hostile_shapes_stay_bounded() {
    let mut cases: Vec<String> = Vec::new();
    cases.push("> ".repeat(5_000) + "deep\n");
    cases.push((0..300).map(|i| format!("{}- x\n", "  ".repeat(i))).collect());
    cases.push("- ".repeat(3_000) + "x\n");
    cases.push("*".repeat(100_000));
    cases.push("[".repeat(50_000));
    cases.push("![".repeat(20_000));
    cases.push("`".repeat(50_000));
    cases.push("<div>".repeat(20_000));
    cases.push("| a ".repeat(5_000) + "|\n" + &"|---".repeat(5_000) + "|\n");
    // Frontmatter just under and just over 64 KiB, and with too many entries.
    cases.push(format!("---\ntitle: {}\n---\nbody\n", "a".repeat(64 * 1024 - 20)));
    cases.push(format!("---\ntitle: {}\n---\nbody\n", "a".repeat(64 * 1024 + 20)));
    cases.push(format!("---\n{}---\nbody\n", "k: v\n".repeat(5_000)));
    cases.push(format!("---\ntags:\n{}---\nbody\n", "  - t\n".repeat(5_000)));
    cases.push(format!("---\nm:\n{}---\nbody\n", "  a: b\n".repeat(5_000)));
    cases.push("---\n".repeat(10_000));
    // More container markers than the limit, with duplicate ids.
    cases.push("<!-- notecraft:container id=c1 -->\ntext\n".repeat(12_000));
    cases.push((0..12_000).map(|i| format!("<!-- notecraft:container id=c{i} -->\n")).collect());
    for (i, case) in cases.iter().enumerate() {
        let data = case.as_bytes();
        for check in [
            checks::check_markdown as fn(&[u8]),
            checks::check_frontmatter,
            checks::check_containers,
        ] {
            let result = std::panic::catch_unwind(|| check(data));
            assert!(result.is_ok(), "hostile case {i} failed");
        }
    }
}
