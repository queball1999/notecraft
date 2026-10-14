//! `cargo xtask assets`: every bundled asset has an ATTRIBUTION.md row (AGENTS.md section 1).
//!
//! Files under `assets/` and `testdata/` must match a row in the "Assets" table of
//! ATTRIBUTION.md, either exactly or through a row naming a folder (ending in `/`). Rows that
//! point at paths that do not exist are reported too.

use std::path::Path;

/// Folders whose files need attribution.
const ASSET_DIRS: &[&str] = &["assets", "testdata"];

/// Paths listed in the first column of the "## Assets" table. Backticks are stripped;
/// placeholder rows such as "(none yet)" are skipped.
pub fn parse_rows(attribution: &str) -> Vec<String> {
    let mut rows = Vec::new();
    let mut in_section = false;
    for line in attribution.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            in_section = heading.trim() == "Assets";
            continue;
        }
        if !in_section || !line.starts_with('|') {
            continue;
        }
        let cell = line.split('|').nth(1).unwrap_or("").trim();
        let cell = cell.trim_matches('`');
        if cell.is_empty() || cell == "Path" || cell.starts_with('-') || cell.starts_with('(') {
            continue;
        }
        rows.push(cell.to_string());
    }
    rows
}

/// True if `file` (relative, `/`-separated) is covered by a row.
pub fn covered(file: &str, rows: &[String]) -> bool {
    rows.iter()
        .any(|r| r == file || (r.ends_with('/') && file.starts_with(r.as_str())))
}

fn walk(root: &Path, rel: &str, out: &mut Vec<String>) -> Result<(), String> {
    let dir = root.join(rel);
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let child = format!("{rel}/{name}");
        let kind = entry.file_type().map_err(|e| format!("{child}: {e}"))?;
        if kind.is_dir() {
            walk(root, &child, out)?;
        } else {
            out.push(child);
        }
    }
    Ok(())
}

pub fn run(root: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(root.join("ATTRIBUTION.md"))
        .map_err(|e| format!("ATTRIBUTION.md: {e}"))?;
    let rows = parse_rows(&text);
    let mut files = Vec::new();
    for dir in ASSET_DIRS {
        if root.join(dir).is_dir() {
            walk(root, dir, &mut files)?;
        }
    }
    let mut problems = Vec::new();
    for f in &files {
        if !covered(f, &rows) {
            problems.push(format!("{f}: no ATTRIBUTION.md row"));
        }
    }
    for r in &rows {
        if !root.join(r.trim_end_matches('/')).exists() {
            problems.push(format!("ATTRIBUTION.md row `{r}`: path does not exist"));
        }
    }
    if problems.is_empty() {
        println!(
            "assets: {} files, {} attribution rows, all covered",
            files.len(),
            rows.len()
        );
        Ok(())
    } else {
        for p in &problems {
            println!("  - {p}");
        }
        Err(format!("{} attribution problem(s)", problems.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# Attribution

## Assets

| Path | Author | Source | License | Notes |
|---|---|---|---|---|
| `assets/app-icon/` | Contributors | original | MIT | |
| `testdata/a.md` | Contributors | original | CC0 | |
| (none yet) | | | | |

## Third-party code

| Path | Origin | License | Notes |
|---|---|---|---|
| `xtask/src/` | GridCraft | MIT | |
";

    #[test]
    fn parses_only_the_assets_table() {
        assert_eq!(
            parse_rows(SAMPLE),
            vec!["assets/app-icon/".to_string(), "testdata/a.md".to_string()]
        );
    }

    #[test]
    fn folder_rows_cover_their_files() {
        let rows = parse_rows(SAMPLE);
        assert!(covered(
            "assets/app-icon/hicolor/32x32/notecraft.png",
            &rows
        ));
        assert!(covered("testdata/a.md", &rows));
        assert!(!covered("testdata/b.md", &rows));
        assert!(!covered("assets/app-iconx.svg", &rows));
    }

    #[test]
    fn placeholder_only_table_has_no_rows() {
        assert!(parse_rows("## Assets\n\n| Path |\n|---|\n| (none yet) |\n").is_empty());
    }
}
