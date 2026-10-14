//! `cargo xtask version [set X.Y.Z[-pre]]`: the workspace version, its single source of truth.
//!
//! The version lives in `[workspace.package] version` in the root Cargo.toml; every crate
//! inherits it. The release workflow (.github/workflows/release.yml) calls `version set` for test
//! runs, then builds with `--locked`, so `set` also refreshes the workspace entries in Cargo.lock.

use crate::util::{cargo, run as run_cmd};
use std::path::Path;

/// Index of the `version` line inside `[workspace.package]`, and its value.
fn find(manifest: &str) -> Result<(usize, String), String> {
    let mut in_section = false;
    for (i, line) in manifest.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t == "[workspace.package]";
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some(rest) = t.strip_prefix("version") {
            if let Some(value) = rest.trim_start().strip_prefix('=') {
                let v = value.trim().trim_matches('"').to_string();
                if v.is_empty() {
                    return Err("empty [workspace.package] version".into());
                }
                return Ok((i, v));
            }
        }
    }
    Err("no version in [workspace.package]".into())
}

pub fn read(manifest: &str) -> Result<String, String> {
    find(manifest).map(|(_, v)| v)
}

/// Semver without build metadata: MAJOR.MINOR.PATCH plus an optional `-pre.release` tag.
pub fn validate(v: &str) -> Result<(), String> {
    let bad = || Err(format!("`{v}` is not a version like 1.2.3 or 1.2.3-rc.1"));
    let (core, pre) = match v.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (v, None),
    };
    let nums: Vec<&str> = core.split('.').collect();
    let numeric = |n: &&str| {
        !n.is_empty()
            && n.bytes().all(|b| b.is_ascii_digit())
            && !(n.len() > 1 && n.starts_with('0'))
    };
    if nums.len() != 3 || !nums.iter().all(numeric) {
        return bad();
    }
    if let Some(p) = pre {
        let ok_id =
            |id: &str| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if !p.split('.').all(ok_id) {
            return bad();
        }
    }
    Ok(())
}

/// `manifest` with the workspace version replaced; every other byte is unchanged.
pub fn replace(manifest: &str, new: &str) -> Result<String, String> {
    validate(new)?;
    let (idx, _) = find(manifest)?;
    let mut out = String::with_capacity(manifest.len() + 8);
    for (i, line) in manifest.split_inclusive('\n').enumerate() {
        if i == idx {
            let eol = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.push_str(&format!("version = \"{new}\"{eol}"));
        } else {
            out.push_str(line);
        }
    }
    Ok(out)
}

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    let path = root.join("Cargo.toml");
    let manifest = std::fs::read_to_string(&path).map_err(|e| format!("Cargo.toml: {e}"))?;
    match args {
        [] => {
            println!("{}", read(&manifest)?);
            Ok(())
        }
        ["set", new] => {
            let updated = replace(&manifest, new)?;
            std::fs::write(&path, updated).map_err(|e| format!("Cargo.toml: {e}"))?;
            let mut c = cargo();
            c.args(["update", "--workspace"]);
            run_cmd(c, "cargo update --workspace")?;
            println!("version set to {new}");
            Ok(())
        }
        _ => Err("usage: cargo xtask version [set X.Y.Z[-pre]]".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: &str = "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nversion = \"9\"\n";

    #[test]
    fn reads_only_the_workspace_package_version() {
        assert_eq!(read(M).unwrap(), "0.0.0");
    }

    #[test]
    fn replaces_in_place() {
        let out = replace(M, "0.1.0-rc.1").unwrap();
        assert_eq!(
            out,
            M.replace("version = \"0.0.0\"", "version = \"0.1.0-rc.1\"")
        );
    }

    #[test]
    fn keeps_crlf() {
        let crlf = M.replace('\n', "\r\n");
        assert!(replace(&crlf, "1.2.3")
            .unwrap()
            .contains("version = \"1.2.3\"\r\n"));
    }

    #[test]
    fn validates() {
        for ok in ["0.0.1", "1.2.3", "10.20.30", "1.0.0-rc.1", "0.2.0-alpha-2"] {
            assert!(validate(ok).is_ok(), "{ok}");
        }
        for bad in [
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.3-",
            "1.2.3-rc..1",
            "v1.2.3",
            "1.2.3+b1",
            "",
        ] {
            assert!(validate(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn missing_version_is_an_error() {
        assert!(read("[workspace]\n").is_err());
        assert!(replace("[workspace]\n", "1.0.0").is_err());
    }
}
