//! `cargo xtask layers`: crate layering (AGENTS.md section 5).
//!
//! Rules:
//! - Each library crate declares `pub const LAYER: u8 = N;` in `src/lib.rs`. Apps under `apps/`
//!   are layer 7. `xtask` is exempt.
//! - A crate may depend only on workspace crates in strictly lower layers.
//! - Nothing below layer 6 may have a UI or windowing crate anywhere in its dependency tree.
//!
//! The rule checks work on a small model so they can be unit-tested; `from_metadata` builds the
//! model from `cargo metadata`.

use crate::util::cargo;
use serde_json::Value;
use std::path::Path;

/// External crates that mean "UI toolkit or windowing". A trailing `*` is a prefix match.
pub const UI_CRATES: &[&str] = &["egui*", "eframe", "winit", "rfd", "wgpu*", "craft-ui"];

/// First layer allowed to use UI crates.
pub const UI_MIN_LAYER: u8 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    L(u8),
    Exempt,
}

#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    pub layer: Result<Layer, String>,
    /// Workspace dependencies: (name, kind) with kind "normal", "dev", or "build".
    pub deps: Vec<(String, String)>,
}

/// Read `pub const LAYER: u8 = N;` from a lib.rs source.
pub fn parse_layer(lib_rs: &str) -> Option<u8> {
    lib_rs.lines().find_map(|l| {
        let rest = l.trim().strip_prefix("pub const LAYER: u8 =")?;
        rest.trim().trim_end_matches(';').trim().parse().ok()
    })
}

fn layer_for(name: &str, manifest_dir: &Path, root: &Path) -> Result<Layer, String> {
    if name == "xtask" {
        return Ok(Layer::Exempt);
    }
    let rel = manifest_dir.strip_prefix(root).unwrap_or(manifest_dir);
    if rel.starts_with("apps") {
        return Ok(Layer::L(7));
    }
    let lib = manifest_dir.join("src").join("lib.rs");
    let src = std::fs::read_to_string(&lib).map_err(|e| format!("{}: {e}", lib.display()))?;
    parse_layer(&src)
        .map(Layer::L)
        .ok_or_else(|| format!("{}: no `pub const LAYER: u8 = N;`", lib.display()))
}

pub fn from_metadata(meta: &Value, root: &Path) -> Result<Vec<Crate>, String> {
    let packages = meta
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("cargo metadata: no packages")?;
    let names: Vec<&str> = packages
        .iter()
        .filter_map(|p| p.get("name").and_then(Value::as_str))
        .collect();
    let mut out = Vec::new();
    for p in packages {
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let manifest = p.get("manifest_path").and_then(Value::as_str).unwrap_or("");
        let dir = Path::new(manifest).parent().unwrap_or(Path::new(""));
        let deps = p
            .get("dependencies")
            .and_then(Value::as_array)
            .map(|ds| {
                ds.iter()
                    .filter_map(|d| {
                        let n = d.get("name").and_then(Value::as_str)?;
                        if !names.contains(&n) {
                            return None;
                        }
                        let kind = d.get("kind").and_then(Value::as_str).unwrap_or("normal");
                        Some((n.to_string(), kind.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(Crate {
            layer: layer_for(&name, dir, root),
            name,
            deps,
        });
    }
    out.sort_by(|a, b| {
        let key = |c: &Crate| match c.layer {
            Ok(Layer::L(l)) => l,
            _ => u8::MAX,
        };
        key(a).cmp(&key(b)).then(a.name.cmp(&b.name))
    });
    Ok(out)
}

/// Layering violations between workspace crates.
pub fn check(crates: &[Crate]) -> Vec<String> {
    let layer_of = |n: &str| crates.iter().find(|c| c.name == n).map(|c| c.layer.clone());
    let mut v = Vec::new();
    for c in crates {
        let me = match &c.layer {
            Ok(Layer::L(l)) => *l,
            Ok(Layer::Exempt) => continue,
            Err(e) => {
                v.push(e.clone());
                continue;
            }
        };
        for (dep, kind) in &c.deps {
            if let Some(Ok(Layer::L(theirs))) = layer_of(dep) {
                if theirs >= me {
                    v.push(format!(
                        "{} (L{me}) depends on {dep} (L{theirs}) [{kind}]: dependencies must point to lower layers",
                        c.name
                    ));
                }
            }
        }
    }
    v
}

pub fn is_ui_crate(name: &str) -> bool {
    UI_CRATES.iter().any(|p| match p.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == *p,
    })
}

/// UI crates in `cargo tree --prefix none --format {p}` output (one "name vX.Y.Z ..." per line).
pub fn ui_crates_in_tree(tree: &str) -> Vec<String> {
    let mut found: Vec<String> = tree
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter(|n| is_ui_crate(n))
        .map(str::to_string)
        .collect();
    found.sort();
    found.dedup();
    found
}

fn tree_of(name: &str) -> Result<String, String> {
    let out = cargo()
        .args([
            "tree",
            "-p",
            name,
            "-e",
            "normal,build",
            "--target",
            "all",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .output()
        .map_err(|e| format!("cargo tree: failed to start: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo tree -p {name} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn metadata() -> Result<Value, String> {
    let out = cargo()
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .map_err(|e| format!("cargo metadata: failed to start: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata: bad JSON: {e}"))
}

pub fn run(root: &Path) -> Result<(), String> {
    let crates = from_metadata(&metadata()?, root)?;
    println!("{:<22} {:<7} workspace deps", "crate", "layer");
    for c in &crates {
        let layer = match &c.layer {
            Ok(Layer::L(l)) => format!("L{l}"),
            Ok(Layer::Exempt) => "exempt".into(),
            Err(_) => "?".into(),
        };
        let deps: Vec<String> = c
            .deps
            .iter()
            .map(|(n, k)| {
                if k == "normal" {
                    n.clone()
                } else {
                    format!("{n} ({k})")
                }
            })
            .collect();
        println!("{:<22} {:<7} {}", c.name, layer, deps.join(", "));
    }
    let mut violations = check(&crates);
    for c in &crates {
        if let Ok(Layer::L(l)) = c.layer {
            if l < UI_MIN_LAYER {
                for ui in ui_crates_in_tree(&tree_of(&c.name)?) {
                    violations.push(format!(
                        "{} (L{l}) depends on UI crate {ui}: only L{UI_MIN_LAYER} and above may",
                        c.name
                    ));
                }
            }
        }
    }
    if violations.is_empty() {
        println!("\nlayers: {} crates, no violations", crates.len());
        Ok(())
    } else {
        for v in &violations {
            println!("  - {v}");
        }
        Err(format!("{} layering violation(s)", violations.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(name: &str, layer: Layer, deps: &[&str]) -> Crate {
        Crate {
            name: name.into(),
            layer: Ok(layer),
            deps: deps
                .iter()
                .map(|d| (d.to_string(), "normal".into()))
                .collect(),
        }
    }

    #[test]
    fn parses_layer_constant() {
        assert_eq!(parse_layer("//! doc\npub const LAYER: u8 = 3;\n"), Some(3));
        assert_eq!(parse_layer("pub const LAYER: u8 = x;"), None);
        assert_eq!(parse_layer("fn main() {}"), None);
    }

    #[test]
    fn downward_dependencies_pass() {
        let cs = vec![
            c("core", Layer::L(0), &[]),
            c("engine", Layer::L(5), &["core"]),
            c("app", Layer::L(7), &["engine"]),
            c("xtask", Layer::Exempt, &["app"]),
        ];
        assert!(check(&cs).is_empty());
    }

    #[test]
    fn upward_and_same_layer_dependencies_fail() {
        let cs = vec![
            c("core", Layer::L(0), &["engine"]),
            c("model", Layer::L(2), &["calc"]),
            c("calc", Layer::L(2), &[]),
            c("engine", Layer::L(5), &[]),
        ];
        let v = check(&cs);
        assert_eq!(v.len(), 2, "{v:?}");
    }

    #[test]
    fn missing_layer_is_a_violation() {
        let cs = vec![Crate {
            name: "x".into(),
            layer: Err("x: no LAYER".into()),
            deps: vec![],
        }];
        assert_eq!(check(&cs), vec!["x: no LAYER".to_string()]);
    }

    #[test]
    fn finds_ui_crates_in_tree_output() {
        let tree = "notecraft-model v0.0.0 (C:\\x)\nnotecraft-core v0.0.0\negui v0.31.0\nwgpu-core v25.0.0\nserde v1.0.0\negui v0.31.0\n";
        assert_eq!(ui_crates_in_tree(tree), vec!["egui", "wgpu-core"]);
        assert!(is_ui_crate("egui_extras"));
        assert!(is_ui_crate("craft-ui"));
        assert!(!is_ui_crate("craft-uix"));
    }
}
