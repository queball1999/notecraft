//! `cargo xtask ci`: every gate CI runs, in order, stopping at the first failure.

use crate::util::{cargo, root, run};

type Step = (&'static str, fn() -> Result<(), String>);

fn fmt() -> Result<(), String> {
    let mut c = cargo();
    c.args(["fmt", "--all", "--", "--check"]);
    run(c, "cargo fmt --all -- --check")
}

fn clippy() -> Result<(), String> {
    let mut c = cargo();
    c.args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]);
    run(c, "cargo clippy --workspace --all-targets -- -D warnings")
}

fn test() -> Result<(), String> {
    let mut c = cargo();
    c.args(["test", "--workspace"]);
    run(c, "cargo test --workspace")
}

/// The gates, in the order AGENTS.md section 7 lists them.
const STEPS: &[Step] = &[
    ("fmt", fmt),
    ("clippy", clippy),
    ("test", test),
    ("text", || crate::text::run(&root())),
    ("assets", || crate::assets::run(&root())),
    ("layers", || crate::layers::run(&root())),
];

pub fn run_all() -> Result<(), String> {
    let mut done: Vec<&str> = Vec::new();
    for (name, step) in STEPS {
        eprintln!("\n=== ci: {name} ===");
        let started = std::time::Instant::now();
        if let Err(e) = step() {
            println!("\nCI summary:");
            for d in &done {
                println!("  ok    {d}");
            }
            println!("  FAIL  {name}: {e}");
            for (rest, _) in STEPS.iter().skip(done.len() + 1) {
                println!("  skip  {rest}");
            }
            return Err(format!("ci failed at `{name}`"));
        }
        eprintln!(
            "ci: {name} ... ok ({:.1}s)",
            started.elapsed().as_secs_f64()
        );
        done.push(name);
    }
    println!(
        "\nCI summary: all {} steps passed ({})",
        done.len(),
        done.join(", ")
    );
    Ok(())
}
