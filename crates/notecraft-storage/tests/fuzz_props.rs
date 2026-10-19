//! Bounded property tests that mirror the `cargo fuzz` targets in `fuzz/` (PR 1.14). They run
//! on stable in `cargo test`: seeded mutations of the testdata plus hostile shapes, the same
//! checks as the fuzz targets.

#[path = "../../../fuzz/checks/storage.rs"]
mod checks;
#[path = "../../../fuzz/checks/mutate.rs"]
mod mutate;

use mutate::{mutate, noise, root, seeds_in, Rng};

const CASES: usize = 3_000;
/// The journal check touches the file system, so it runs fewer cases.
const JOURNAL_CASES: usize = 150;

const CANVAS_TOKENS: &[&str] = &[
    "```notecraft-canvas\n",
    "```\n",
    "~~~\n",
    "{\"format\":1,\"ink\":{\"strokes\":[",
    "{\"id\":\"s1\",\"tool\":\"pen\",\"width\":2,\"points\":[[1,2,0.5],[3,4,0.5]]}",
    "{\"points\":[[1e999,-1e999,2]]}",
    "\"width\":NaN",
    "\"format\":999",
    "\"format\":-1",
    "]}}",
    "[[[[",
    "{\"a\":",
    "\\u00",
    "\"id\":\"",
    "\"images\":[{\"id\":\"i1\",\"src\":\"attachments/a.png\",\"x\":1,\"y\":2}]",
    "\"files\":[{\"src\":\"f.pdf\",\"nth\":3,\"x\":1e999,\"y\":5}]",
    "{\"id\":\"i1\",\"src\":\"\",\"x\":-1,\"y\":1}",
    "\"nth\":-1",
    "9999999999999999999",
    "\n",
];

const TOML_TOKENS: &[&str] = &[
    "format = 1\n",
    "format = \"x\"\n",
    "id = \"abc\"\n",
    "name = \"N\"\n",
    "color = \"#ff00aa\"\n",
    "kind = \"group\"\n",
    "sections = [\"A\", \"B\"]\n",
    "[[pages]]\nfile = \"a.md\"\nlevel = 9\n",
    "pages = [\"x.md\", 5, { file = \"y.md\", level = -3 }]\n",
    "[a.b.c]\n",
    "\"\"\"",
    "'''",
    "\\u",
    "= ",
    "[",
    "\n",
];

fn text_seeds(suffixes: &[&str]) -> Vec<Vec<u8>> {
    let data = root().join("testdata");
    let mut all = seeds_in(&data.join("notecraft"), &[".notecraft"]);
    all.extend(seeds_in(&data.join("notebooks"), suffixes));
    all
}

fn run(
    name: &str,
    seed: u64,
    seeds: &[Vec<u8>],
    tokens: &[&str],
    cases: usize,
    check: &dyn Fn(&[u8]),
) {
    for seed_file in seeds {
        check(seed_file);
    }
    let mut rng = Rng::new(seed);
    for i in 0..mutate::cases(cases) {
        let data = if i % 5 == 4 {
            noise(&mut rng, 2_000)
        } else {
            mutate(&mut rng, seeds, tokens)
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&data)));
        assert!(
            result.is_ok(),
            "{name}: case {i} failed; input: {:?}",
            String::from_utf8_lossy(&data)
        );
    }
}

#[test]
fn canvas_block_survives_mutated_input() {
    let seeds = text_seeds(&[".notecraft"]);
    assert!(!seeds.is_empty(), "no .notecraft seeds under testdata");
    run(
        "canvas",
        0x4356,
        &seeds,
        CANVAS_TOKENS,
        CASES,
        &checks::check_canvas,
    );
}

#[test]
fn notebook_toml_survives_mutated_input() {
    let seeds = text_seeds(&["notebook.toml"]);
    let seeds = if seeds.is_empty() {
        vec![b"format = 1\n".to_vec()]
    } else {
        seeds
    };
    run(
        "notebook.toml",
        0x4e42,
        &seeds,
        TOML_TOKENS,
        CASES,
        &checks::check_notebook_toml,
    );
}

#[test]
fn section_toml_survives_mutated_input() {
    let seeds = text_seeds(&[".section.toml"]);
    let seeds = if seeds.is_empty() {
        vec![b"format = 1\n".to_vec()]
    } else {
        seeds
    };
    run(
        "section.toml",
        0x5354,
        &seeds,
        TOML_TOKENS,
        CASES,
        &checks::check_section_toml,
    );
}

#[test]
fn recovery_journal_survives_mutated_input() {
    let dir = tempfile::TempDir::new().unwrap();
    let good = br#"{"version":1,"path":"/n/A.md","text":"hello\n"}"#.to_vec();
    let tokens: &[&str] = &[
        "{\"version\":1,",
        "\"path\":\"\",",
        "\"path\":\"/x/y.md\",",
        "\"text\":\"\\ud800\"",
        "\"version\":2",
        "\"text\":null",
        "}",
        "[",
        "\\u",
    ];
    run("journal", 0x4a4c, &[good], tokens, JOURNAL_CASES, &|data| {
        checks::check_journal(dir.path(), data)
    });
}

#[test]
fn hostile_shapes_stay_bounded() {
    let mut canvas: Vec<String> = Vec::new();
    // Deep JSON, a huge stroke, many strokes, a stroke with enormous coordinates.
    canvas.push(format!(
        "```notecraft-canvas\n{}{}\n```\n",
        "[".repeat(100_000),
        "]".repeat(100_000)
    ));
    canvas.push(format!(
        "```notecraft-canvas\n{}1{}\n```\n",
        "{\"a\":".repeat(50_000),
        "}".repeat(50_000)
    ));
    let points = (0..30_000)
        .map(|i| format!("[{i},{i},0.5]"))
        .collect::<Vec<_>>()
        .join(",");
    canvas.push(format!(
        "```notecraft-canvas\n{{\"format\":1,\"ink\":{{\"strokes\":[{{\"id\":\"s\",\"width\":1,\"points\":[{points}]}}]}}}}\n```\n"
    ));
    let many = (0..110_000)
        .map(|i| format!("{{\"id\":\"s{i}\",\"width\":1,\"points\":[[1,2,0.5]]}}"))
        .collect::<Vec<_>>()
        .join(",");
    canvas.push(format!(
        "```notecraft-canvas\n{{\"format\":1,\"ink\":{{\"strokes\":[{many}]}}}}\n```\n"
    ));
    canvas.push("```notecraft-canvas\n{\"format\":1,\"ink\":{\"strokes\":[{\"id\":\"s\",\"width\":1e40,\"points\":[[1e30,-1e30,9]]}]}}\n```\n".into());
    canvas.push("```notecraft-canvas\n".repeat(20_000));
    // Placed pictures and files: far more than the cap, a huge link, odd numbers.
    let objects = (0..12_000)
        .map(|i| format!("{{\"id\":\"i{i}\",\"src\":\"a{i}.png\",\"nth\":{i},\"x\":1,\"y\":2}}"))
        .collect::<Vec<_>>()
        .join(",");
    canvas.push(format!(
        "```notecraft-canvas\n{{\"format\":1,\"images\":[{objects}],\"files\":[{objects}]}}\n```\n"
    ));
    canvas.push(format!(
        "```notecraft-canvas\n{{\"format\":1,\"images\":[{{\"src\":\"{}\",\"x\":1e40,\"y\":-1e40,\"nth\":1e30}}],\"files\":{{\"a\":1}}}}\n```\n",
        "s".repeat(500_000)
    ));
    for (i, case) in canvas.iter().enumerate() {
        let result = std::panic::catch_unwind(|| checks::check_canvas(case.as_bytes()));
        assert!(result.is_ok(), "hostile canvas case {i} failed");
    }

    let mut toml: Vec<String> = Vec::new();
    toml.push(format!("sections = [{}]\n", "\"s\",".repeat(150_000)));
    toml.push(format!("pages = [{}]\n", "\"p.md\",".repeat(150_000)));
    toml.push(format!(
        "id = \"{}\"\nname = \"{}\"\n",
        "i".repeat(10_000),
        "n".repeat(10_000)
    ));
    toml.push(format!("{}x{}\n", "a=[".repeat(5_000), "]".repeat(5_000)));
    toml.push((0..20_000).map(|i| format!("[k{i}]\n")).collect());
    for (i, case) in toml.iter().enumerate() {
        let result = std::panic::catch_unwind(|| {
            checks::check_notebook_toml(case.as_bytes());
            checks::check_section_toml(case.as_bytes());
        });
        assert!(result.is_ok(), "hostile toml case {i} failed");
    }
}
