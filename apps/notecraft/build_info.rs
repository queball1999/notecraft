// Build-time version info shared by the notecraft and notecraft-cli build scripts
// (notecraft-cli includes this file). Emits NOTECRAFT_BUILD_SHA and NOTECRAFT_BUILD_DATE as
// compile-time env vars, taken in order from:
//   1. NOTECRAFT_BUILD_SHA / NOTECRAFT_BUILD_DATE (packaging/env.sh and package.ps1 set them)
//   2. BUILD_SHA / BUILD_DATE (generic names some CI systems set)
//   3. `git rev-parse --short HEAD` and today's UTC date
// Anything that cannot be found becomes "unknown"; a build never fails over version info.

fn first_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|n| {
        println!("cargo:rerun-if-env-changed={n}");
        std::env::var(n).ok().filter(|v| !v.trim().is_empty())
    })
}

fn git_short_sha() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let sha = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (!sha.is_empty()).then_some(sha)
}

/// Today's UTC date as YYYY-MM-DD (civil-from-days, Howard Hinnant's algorithm).
fn utc_today() -> Option<String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let z = i64::try_from(secs / 86_400).ok()? + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// Re-run when the checked-out commit changes, so a dev build never reports a stale SHA.
/// Both build scripts sit two levels below the repo root. Skipped outside a git checkout.
fn watch_git_head() {
    let git = std::path::Path::new("../../.git");
    if git.join("HEAD").is_file() {
        println!("cargo:rerun-if-changed=../../.git/HEAD");
        for extra in ["refs/heads", "packed-refs"] {
            if git.join(extra).exists() {
                println!("cargo:rerun-if-changed=../../.git/{extra}");
            }
        }
    }
}

fn emit_build_info() {
    watch_git_head();
    let sha = first_env(&["NOTECRAFT_BUILD_SHA", "BUILD_SHA"])
        .map(|s| s.chars().take(12).collect::<String>())
        .or_else(git_short_sha)
        .unwrap_or_else(|| "unknown".into());
    let date = first_env(&["NOTECRAFT_BUILD_DATE", "BUILD_DATE"])
        .or_else(utc_today)
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=NOTECRAFT_BUILD_SHA={sha}");
    println!("cargo:rustc-env=NOTECRAFT_BUILD_DATE={date}");
}
