//! `cargo xtask parity [--check]`: the parity checklist summary and the build plan hour totals.
//!
//! - Rewrites the block between `<!-- parity:summary:start -->` and `<!-- parity:summary:end -->`
//!   in docs/parity-checklist.md from the rows below it (`--check` compares instead of writing).
//! - Checks that every `build_plan/0N-*.md` "Total" row equals the sum of its PR rows, and that
//!   the "By phase" table in ROADMAP.md shows the same totals.

use std::path::Path;

const CHECKLIST: &str = "docs/parity-checklist.md";
const START: &str = "<!-- parity:summary:start -->";
const END: &str = "<!-- parity:summary:end -->";

/// Build plan phase files and the phase label ROADMAP.md uses for each.
const PHASE_FILES: &[(&str, &str)] = &[
    ("01-phase-minus-1-scaffolding.md", "-1"),
    ("02-phase-0-ui-shell.md", "0"),
    ("03-phase-1-core-engine.md", "1"),
    ("04-phase-2-links-search-tags.md", "2"),
    ("05-phase-3-polish-parity.md", "3"),
    ("06-phase-4-advanced.md", "4"),
];

/// Split a markdown table row into trimmed cells (without the outer pipes).
fn cells(line: &str) -> Option<Vec<&str>> {
    let inner = line.trim().strip_prefix('|')?.strip_suffix('|')?;
    Some(inner.split('|').map(str::trim).collect())
}

fn is_row_id(id: &str) -> bool {
    let b = id.as_bytes();
    b.len() == 5
        && b.iter().take(2).all(u8::is_ascii_uppercase)
        && b.get(2) == Some(&b'-')
        && b.iter().skip(3).all(u8::is_ascii_digit)
}

/// The summary table for the checklist rows in `text`, areas in first-seen order.
pub fn summary(text: &str) -> String {
    let mut areas: Vec<(String, [usize; 3])> = Vec::new();
    for line in text.lines() {
        let Some(c) = cells(line) else { continue };
        let (Some(id), Some(area), Some(score)) = (c.first(), c.get(1), c.get(4)) else {
            continue;
        };
        if !is_row_id(id) {
            continue;
        }
        let slot = match *score {
            "D" => 0,
            "P" => 1,
            "M" => 2,
            _ => continue,
        };
        let pos = match areas.iter().position(|(a, _)| a == area) {
            Some(p) => p,
            None => {
                areas.push((area.to_string(), [0; 3]));
                areas.len() - 1
            }
        };
        if let Some((_, counts)) = areas.get_mut(pos) {
            if let Some(n) = counts.get_mut(slot) {
                *n += 1;
            }
        }
    }
    let mut out =
        String::from("| Area | D | P | M | Total | Done % |\n|---|---:|---:|---:|---:|---:|\n");
    let mut total = [0usize; 3];
    let row = |name: &str, c: [usize; 3]| {
        let sum = c[0] + c[1] + c[2];
        let pct = (c[0] * 100).checked_div(sum).unwrap_or(0);
        format!(
            "| {name} | {} | {} | {} | {sum} | {pct}% |\n",
            c[0], c[1], c[2]
        )
    };
    for (name, c) in &areas {
        for (t, n) in total.iter_mut().zip(c) {
            *t += n;
        }
        out.push_str(&row(name, *c));
    }
    out.push_str(&row("Total", total));
    out
}

/// `text` with the summary block replaced, or None if the markers are missing.
pub fn with_summary(text: &str) -> Option<String> {
    let start = text.find(START)? + START.len();
    let end = text.find(END)?;
    let before = text.get(..start)?;
    let after = text.get(end..)?;
    Some(format!("{before}\n{}{after}", summary(text)))
}

/// Parse "55-98" or "1,279-2,202" into (low, high).
pub fn range(s: &str) -> Option<(u32, u32)> {
    let (a, b) = s.split_once('-')?;
    let n = |x: &str| x.trim().replace(',', "").parse::<u32>().ok();
    Some((n(a)?, n(b)?))
}

/// (sum of PR rows, stated Total) from a phase file's "## Estimate" table.
pub fn estimate(text: &str) -> Option<((u32, u32), (u32, u32))> {
    let section = text.split("## Estimate").nth(1)?;
    let section = section.split("\n## ").next()?;
    let mut sum = (0u32, 0u32);
    let mut total = None;
    for line in section.lines() {
        let Some(c) = cells(line) else { continue };
        let (Some(name), Some(hours)) = (c.first(), c.get(1)) else {
            continue;
        };
        let Some(r) = range(hours) else { continue };
        if *name == "Total" {
            total = Some(r);
        } else {
            sum = (sum.0 + r.0, sum.1 + r.1);
        }
    }
    Some((sum, total?))
}

/// ROADMAP.md "By phase" hours for a phase label such as "-1".
pub fn roadmap_hours(roadmap: &str, phase: &str) -> Option<(u32, u32)> {
    let section = roadmap.split("## By phase").nth(1)?;
    let section = section.split("\n## ").next()?;
    section.lines().find_map(|l| {
        let c = cells(l)?;
        if c.first() == Some(&phase) {
            range(c.get(2)?)
        } else {
            None
        }
    })
}

fn check_estimates(root: &Path) -> Result<Vec<String>, String> {
    let roadmap =
        std::fs::read_to_string(root.join("ROADMAP.md")).map_err(|e| format!("ROADMAP.md: {e}"))?;
    let mut problems = Vec::new();
    for (file, phase) in PHASE_FILES {
        let path = root.join("build_plan").join(file);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let Some((sum, total)) = estimate(&text) else {
            problems.push(format!(
                "build_plan/{file}: no Estimate table with a Total row"
            ));
            continue;
        };
        if sum != total {
            problems.push(format!(
                "build_plan/{file}: PR rows sum to {}-{} but Total says {}-{}",
                sum.0, sum.1, total.0, total.1
            ));
        }
        match roadmap_hours(&roadmap, phase) {
            Some(r) if r == total => {}
            Some(r) => problems.push(format!(
                "ROADMAP.md By phase: Phase {phase} says {}-{} but build_plan/{file} totals {}-{}",
                r.0, r.1, total.0, total.1
            )),
            None => problems.push(format!("ROADMAP.md By phase: no row for Phase {phase}")),
        }
    }
    Ok(problems)
}

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    let check = args.contains(&"--check");
    let path = root.join(CHECKLIST);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{CHECKLIST}: {e}"))?;
    let updated = with_summary(&text).ok_or(format!("{CHECKLIST}: summary markers missing"))?;
    let mut problems = Vec::new();
    if updated != text {
        if check {
            problems.push(format!(
                "{CHECKLIST}: summary is stale (run `cargo xtask parity`)"
            ));
        } else {
            std::fs::write(&path, &updated).map_err(|e| format!("{CHECKLIST}: {e}"))?;
            println!("parity: summary rewritten");
        }
    }
    problems.extend(check_estimates(root)?);
    if problems.is_empty() {
        println!("parity: summary current; build plan totals match ROADMAP.md");
        Ok(())
    } else {
        for p in &problems {
            println!("  - {p}");
        }
        Err(format!("{} parity problem(s)", problems.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "\
<!-- parity:summary:start -->
stale
<!-- parity:summary:end -->

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| SC-01 | Scaffolding | Workspace | -1 | D | PR -1.1 |
| SC-02 | Scaffolding | CI | -1 | P | |
| SH-01 | Shell | Window | 0 | M | |
";

    #[test]
    fn builds_summary_per_area_in_order() {
        let s = summary(DOC);
        assert!(s.contains("| Scaffolding | 1 | 1 | 0 | 2 | 50% |"), "{s}");
        assert!(s.contains("| Shell | 0 | 0 | 1 | 1 | 0% |"), "{s}");
        assert!(s.contains("| Total | 1 | 1 | 1 | 3 | 33% |"), "{s}");
        assert!(s.find("Scaffolding") < s.find("Shell"));
    }

    #[test]
    fn rewrite_is_stable() {
        let once = with_summary(DOC).unwrap();
        assert_eq!(with_summary(&once).unwrap(), once);
        assert!(!once.contains("stale"));
    }

    #[test]
    fn missing_markers_are_reported() {
        assert!(with_summary("| SC-01 | A | x | -1 | D | |").is_none());
    }

    #[test]
    fn parses_ranges_with_commas() {
        assert_eq!(range("55-98"), Some((55, 98)));
        assert_eq!(range("1,279-2,202"), Some((1279, 2202)));
        assert_eq!(range("n/a"), None);
    }

    #[test]
    fn estimate_sums_rows_and_reads_total() {
        let t = "## Estimate\n\n| PR | Hours |\n|---|---|\n| 1 a | 1-2 |\n| 2 b | 3-4 |\n| Total | 4-6 |\n\n## Exit\n| x | 9-9 |\n";
        assert_eq!(estimate(t), Some(((4, 6), (4, 6))));
    }

    #[test]
    fn roadmap_hours_by_phase_label() {
        let r = "## By phase\n\n| Phase | Theme | Hours |\n|---|---|---|\n| -1 | S | 55-98 |\n| 4 | R | 644-1,228 |\n\n## Next\n";
        assert_eq!(roadmap_hours(r, "-1"), Some((55, 98)));
        assert_eq!(roadmap_hours(r, "4"), Some((644, 1228)));
        assert_eq!(roadmap_hours(r, "2"), None);
    }
}
