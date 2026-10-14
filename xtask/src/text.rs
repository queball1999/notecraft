//! `cargo xtask text`: plain ASCII, US English text (AGENTS.md section 2).
//!
//! Every tracked or new (not ignored) file outside `testdata/` and `locales/` must be pure
//! ASCII. Binary files (a NUL byte in the first 8 KiB) are skipped. The CI workflow runs the
//! same rule as a shell step before Rust is installed.

use std::path::Path;
use std::process::Command;

/// Folders whose files may hold any Unicode (fixtures and translations).
const EXEMPT_DIRS: &[&str] = &["testdata/", "locales/"];

/// Stop listing after this many hits; the count is still reported.
const MAX_REPORTS: usize = 200;

#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub line: usize,
    pub col: usize,
    pub ch: char,
}

/// Every non-ASCII character in `bytes`, with 1-based line and column. Binary data yields none.
pub fn scan(bytes: &[u8]) -> Vec<Hit> {
    if is_binary(bytes) {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (i, line) in bytes.split(|b| *b == b'\n').enumerate() {
        if line.is_ascii() {
            continue;
        }
        for (col, ch) in String::from_utf8_lossy(line).chars().enumerate() {
            if !ch.is_ascii() {
                hits.push(Hit {
                    line: i + 1,
                    col: col + 1,
                    ch,
                });
            }
        }
    }
    hits
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|b| *b == 0)
}

pub fn is_exempt(path: &str) -> bool {
    EXEMPT_DIRS.iter().any(|d| path.starts_with(d))
}

/// Tracked files plus new files that are not ignored, relative to `root`, with `/` separators.
fn files(root: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .map_err(|e| format!("git ls-files: failed to start: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git ls-files failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect())
}

pub fn run(root: &Path) -> Result<(), String> {
    let mut total = 0usize;
    let mut checked = 0usize;
    for rel in files(root)? {
        if is_exempt(&rel) {
            continue;
        }
        // Deleted-but-tracked files have nothing to check.
        let Ok(bytes) = std::fs::read(root.join(&rel)) else {
            continue;
        };
        checked += 1;
        for hit in scan(&bytes) {
            total += 1;
            if total <= MAX_REPORTS {
                println!(
                    "{rel}:{}:{}: U+{:04X} {:?}",
                    hit.line, hit.col, hit.ch as u32, hit.ch
                );
            }
        }
    }
    if total == 0 {
        println!("text: {checked} files, all plain ASCII");
        Ok(())
    } else {
        if total > MAX_REPORTS {
            println!("... and {} more", total - MAX_REPORTS);
        }
        Err(format!(
            "{total} non-ASCII character(s); use plain ASCII (AGENTS.md section 2)"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ascii_has_no_hits() {
        assert!(scan(b"fn main() {}\n// colors, behavior\n").is_empty());
    }

    #[test]
    fn finds_em_dash_with_position() {
        let hits = scan("ok\na \u{2014} b\n".as_bytes());
        assert_eq!(
            hits,
            vec![Hit {
                line: 2,
                col: 3,
                ch: '\u{2014}'
            }]
        );
    }

    #[test]
    fn finds_emoji_and_curly_quotes() {
        let hits = scan("\u{201C}hi\u{201D} \u{1F600}".as_bytes());
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn invalid_utf8_is_reported_not_panicked() {
        let hits = scan(&[b'a', 0xFF, b'b']);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn binary_files_are_skipped() {
        assert!(scan(&[0x89, b'P', b'N', b'G', 0, 0xFF]).is_empty());
    }

    #[test]
    fn exemptions() {
        assert!(is_exempt("testdata/notebooks/a.md"));
        assert!(is_exempt("locales/fr-FR.ftl"));
        assert!(!is_exempt("docs/testdata.md"));
    }
}
