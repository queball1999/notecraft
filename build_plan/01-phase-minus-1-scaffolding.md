# Phase -1: scaffolding and CI/CD

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-15. Change: reviewed at the Phase 0 close; no change.

Goal: an empty but complete workspace. Every crate exists and builds, `cargo xtask ci` runs every gate, CI matches the related craft apps, and the release workflow can build packages from the `release` branch. No features.

| | |
|---|---|
| Phase branch | `phase/m1-scaffolding` |
| Hours | 45-80 |
| Calendar | 2-3 weeks |
| Checklist rows | SC-01 to SC-10 |
| Gaps | ranks 1 to 5 |

## Entry criteria

- The docs set exists (this repo as of 2026-10-14): SCRD.md, AGENTS.md, ROADMAP.md, docs/, build_plan/, root files, `.github/`.
- The maintainer has created (or approved creating) the GitHub repo `Queball1999/notecraft`.

## Already done before this phase

These exist on disk and go into the first commit (section 0): README.md, CONTRIBUTING.md, CLAUDE.md, AGENTS.md, ATTRIBUTION.md, ROADMAP.md, SCRD.md, NOTICE, LICENSE-MIT, LICENSE-APACHE, `.gitignore`, `.gitattributes`, `.editorconfig`, `.shellcheckrc`, `rustfmt.toml`, `clippy.toml`, `.cargo/config.toml`, `.github/` (workflows: ci, release, packaging-lint, freebsd, windows-arm64, printing; issue templates; PR template; code of conduct; dependabot), docs/, build_plan/.

## 0. Bootstrap the repository (maintainer or agent when asked)

The repo is not under git yet. This is the only time anything is committed straight to `main`.

```sh
cd notecraft
git init -b main
git add -A
git status            # check: no target/, no secrets, no local settings
git commit -m "docs: add spec, rules, docs set, and build plan" \
  -m "Initial commit. No code. Adds SCRD, AGENTS, ROADMAP, docs/, build_plan/, licenses, and CI/CD that mirrors the related craft apps." \
  -m "Refs: build_plan/01-phase-minus-1-scaffolding.md section 0"
gh repo create Queball1999/notecraft --public --source . --remote origin \
  --description "OneNote-style canvas and ink on a markdown-first file format. Rust + egui."
git push -u origin main
git branch release main
git push -u origin release
git checkout -b phase/m1-scaffolding
git push -u origin phase/m1-scaffolding
```

Expected: the CI workflow runs on `main` and passes (only the text check runs; the Rust steps skip because there is no `Cargo.toml`). The nightly FreeBSD and Windows ARM64 workflows will fail until PR -1.1 reaches `main`; that is expected and why PR -1.1 is small and first.

Then do section 6 (GitHub settings).

## PR -1.1: Cargo workspace and crate skeletons

| | |
|---|---|
| Branch | `chore/workspace` off `phase/m1-scaffolding` |
| Hours | 6-10 |
| Depends on | section 0 |
| Rows | SC-01 |

Files:

```text
Cargo.toml                           workspace root
Cargo.lock
crates/notecraft-core/{Cargo.toml,src/lib.rs}
crates/notecraft-storage/{Cargo.toml,src/lib.rs}
crates/notecraft-model/{Cargo.toml,src/lib.rs}
crates/notecraft-markdown/{Cargo.toml,src/lib.rs}
crates/notecraft-canvas/{Cargo.toml,src/lib.rs}
crates/notecraft-engine/{Cargo.toml,src/lib.rs}
crates/notecraft-ui-egui/{Cargo.toml,src/lib.rs}
apps/notecraft/{Cargo.toml,src/main.rs}
apps/notecraft-cli/{Cargo.toml,src/main.rs}
xtask/{Cargo.toml,src/main.rs}       stub only; filled in by PR -1.2
```

`notecraft-web` is not created until Phase 4. `notecraft-mcp` is not created at all; MCP is a post-parity option (docs/post-parity.md).

Steps:

1. Write the root `Cargo.toml`:
   ```toml
   [workspace]
   resolver = "2"
   # Members are listed explicitly: an empty glob such as "crates/*" is an error,
   # and an explicit list lets each early commit build with only the crates it has.
   members = [
       "crates/notecraft-core",
       "crates/notecraft-storage",
       "crates/notecraft-model",
       "crates/notecraft-markdown",
       "crates/notecraft-canvas",
       "crates/notecraft-engine",
       "crates/notecraft-ui-egui",
       "apps/notecraft",
       "apps/notecraft-cli",
       "xtask",
   ]

   [workspace.package]
   version = "0.0.0"
   edition = "2021"
   rust-version = "1.95"
   license = "MIT OR Apache-2.0"
   repository = "https://github.com/Queball1999/notecraft"
   authors = ["The NoteCraft Contributors"]

   [workspace.lints.rust]
   unsafe_code = "forbid"

   [workspace.lints.clippy]
   unwrap_used = "deny"
   expect_used = "deny"
   panic = "deny"
   unimplemented = "deny"
   todo = "deny"
   unreachable = "deny"
   indexing_slicing = "warn"

   [workspace.dependencies]
   notecraft-core = { path = "crates/notecraft-core" }
   notecraft-storage = { path = "crates/notecraft-storage" }
   notecraft-model = { path = "crates/notecraft-model" }
   notecraft-markdown = { path = "crates/notecraft-markdown" }
   notecraft-canvas = { path = "crates/notecraft-canvas" }
   notecraft-engine = { path = "crates/notecraft-engine" }
   notecraft-ui-egui = { path = "crates/notecraft-ui-egui" }
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   tracing = "0.1"

   [profile.release]
   lto = "thin"
   codegen-units = 1
   strip = "debuginfo"
   ```
2. For each crate, `Cargo.toml` with `version.workspace = true` (and the other `workspace = true` fields), `[lints] workspace = true`, and only the path dependencies its layer allows (AGENTS.md section 5). Example for `notecraft-model`:
   ```toml
   [package]
   name = "notecraft-model"
   description = "NoteCraft L2: notebook, section, page, canvas objects, links"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   license.workspace = true
   repository.workspace = true

   [lints]
   workspace = true

   [dependencies]
   notecraft-core.workspace = true
   ```
3. Each `lib.rs`:
   ```rust
   //! NoteCraft L2: notebook, section, page, canvas objects, links.
   #![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
   #![forbid(unsafe_code)]

   /// Layer number, read by `cargo xtask layers`.
   pub const LAYER: u8 = 2;
   ```
4. `apps/notecraft/src/main.rs` and `apps/notecraft-cli/src/main.rs`: print the name and version and exit 0. No egui yet.
5. `xtask/src/main.rs`: `fn main() {}` placeholder so the alias in `.cargo/config.toml` resolves.
6. `cargo build --workspace`, `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.

Commits:

1. `chore: add cargo workspace root with shared lints`
2. `chore: add L0 to L5 engine crate skeletons`
3. `chore: add notecraft-ui-egui crate skeleton`
4. `chore: add notecraft and notecraft-cli app skeletons`
5. `chore(xtask): add placeholder xtask crate`

PR:

- Title: `chore: add cargo workspace and crate skeletons`
- Labels: `phase:m1`, `area:ci`
- Body:
  ```markdown
  ## What this changes
  Adds the Cargo workspace and an empty crate for every layer in AGENTS.md section 5, each with the never-crash lint header and a LAYER constant. No features.

  ## Related
  - Build plan: build_plan/01-phase-minus-1-scaffolding.md, PR -1.1

  ## Checklist
  - [x] cargo fmt, clippy -D warnings, and cargo test pass locally
  - [x] No unwrap, expect, panic, or unsafe
  - [x] Layering: each crate depends only on lower layers
  - [x] Text is plain ASCII, US English

  ## Feature status
  - [x] Not user-visible

  ## Testing performed
  cargo build, clippy, test on <OS>.
  ```

Acceptance:

- `cargo build --workspace` and `cargo test --workspace` pass on Windows, macOS, Linux.
- Every `lib.rs` has the lint header and `LAYER`.
- No crate below L6 depends on anything egui-related (nothing depends on it yet).

Docs to update: parity-checklist SC-01 to D; gaps rank 1 removed; ROADMAP log line.

## PR -1.2: xtask with `ci`

| | |
|---|---|
| Branch | `chore/xtask-ci` off `phase/m1-scaffolding` |
| Hours | 6-10 |
| Depends on | -1.1 |
| Rows | SC-02 (partial until -1.4) |

Files: `xtask/Cargo.toml`, `xtask/src/main.rs`, `xtask/src/ci.rs`, `xtask/src/util.rs`.

Steps:

1. Model on GridCraft's xtask (same subcommand names and output style). Use plain `std::env::args` matching or `clap`; follow whatever GridCraft uses.
2. `cargo xtask ci` runs, in order, stopping at the first failure:
   1. `cargo fmt --all -- --check`
   2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
   3. `cargo test --workspace --locked`
   4. then the xtask subcommands added in later PRs (text, assets, layers, wasm, parity --check), each behind a check that it exists yet.
3. Print one line per step: `ci: fmt ... ok` with timing. Exit with the failing step's code.
4. xtask itself follows never-crash rules (it is in the workspace).

Commits:

1. `feat(xtask): add command dispatch and process helpers`
2. `feat(xtask): add ci subcommand running fmt, clippy, and test`

PR title: `feat(xtask): add cargo xtask ci`. Labels: `phase:m1`, `area:ci`.

Acceptance: `cargo xtask ci` passes locally and in CI (the CI workflow's Rust steps now run, since `Cargo.toml` exists). Breaking formatting in a scratch commit makes it fail.

Docs to update: architecture.md section 9 (mark which steps exist); ROADMAP log line.

## PR -1.3: xtask `text`, `assets`, `layers`

| | |
|---|---|
| Branch | `chore/xtask-gates` off `phase/m1-scaffolding` |
| Hours | 10-16 |
| Depends on | -1.2 |
| Rows | SC-03, SC-04, SC-05 |

Files: `xtask/src/text.rs`, `xtask/src/assets.rs`, `xtask/src/layers.rs`, tests for each under `xtask/tests/`.

Steps:

1. `text`: walk `git ls-files`; skip `testdata/`, `locales/`, and binary files (by NUL byte in the first 8 KiB); report every non-ASCII byte as `path:line:col U+XXXX`. Same rule as the CI shell step. Keep the CI shell step too; it runs before Rust is installed.
2. `assets`: every file under `assets/` and `testdata/` must match a path or directory row in ATTRIBUTION.md's Assets table. Report missing rows and rows pointing at missing files.
3. `layers`: run `cargo metadata --format-version 1 --no-deps`, read each workspace crate's `LAYER` (parse `pub const LAYER: u8 = N;` from `lib.rs`, or the table in AGENTS.md for binaries: apps are L7, xtask is exempt). Fail if a crate depends on a workspace crate of a higher or equal layer, or if a crate below L6 has `egui`, `eframe`, `winit`, `rfd`, or `craft-ui` anywhere in its dependency tree (`cargo tree -p <crate> -e normal`).
4. Wire all three into `cargo xtask ci` after `test`.

Commits:

1. `feat(xtask): add text check for plain ascii`
2. `feat(xtask): add assets check against ATTRIBUTION.md`
3. `feat(xtask): add layers check for crate dependencies`
4. `feat(xtask): run text, assets, and layers in ci`
5. `test(xtask): cover text, assets, and layers failures`

PR title: `feat(xtask): add text, assets, and layers gates`.

Acceptance: each check fails on a deliberately bad fixture (a temp crate with a backward dependency; an asset with no row; a file with an em dash) and passes on the real tree.

## PR -1.4: xtask `wasm` and `parity`

| | |
|---|---|
| Branch | `chore/xtask-wasm-parity` off `phase/m1-scaffolding` |
| Hours | 8-14 |
| Depends on | -1.3 |
| Rows | SC-06, SC-02 (now D) |

Steps:

1. `wasm`: `cargo check --target wasm32-unknown-unknown -p notecraft-core -p notecraft-storage -p notecraft-model -p notecraft-markdown -p notecraft-canvas -p notecraft-engine`. (The list grows when crates gain wasm-incompatible deps; gate those with `cfg`.)
2. `parity`: parse docs/parity-checklist.md rows (`| ID | Area | Feature | Phase | Score | Evidence |`), count D, P, M per Area in first-seen order, and rewrite the block between `<!-- parity:summary:start -->` and `<!-- parity:summary:end -->`. `--check` compares instead of writing and fails if stale. Done % = D / Total, rounded down.
3. `parity --check` also checks the hour estimates: each `build_plan/0N-*.md` "Total" row equals the sum of its PR rows, and the "By phase" table in ROADMAP.md matches those totals.
4. Add both to `cargo xtask ci` (`parity --check` last).

Commits:

1. `feat(xtask): add wasm32 check for engine crates`
2. `feat(xtask): add parity summary generator and check`
3. `feat(xtask): check build plan hour totals against ROADMAP`
4. `feat(xtask): run wasm and parity check in ci`

PR title: `feat(xtask): add wasm and parity gates`.

Acceptance: `cargo xtask parity` on the current checklist produces exactly the committed summary (all M, 113 rows). Changing one PR's hours without updating the totals fails `cargo xtask ci`. Changing one score without regenerating fails `cargo xtask ci`.

## PR -1.5: release contract (version set and packaging)

| | |
|---|---|
| Branch | `chore/packaging` off `phase/m1-scaffolding` |
| Hours | 16-30 |
| Depends on | -1.2 |
| Rows | SC-07, SC-08 |

Files (see docs/releasing.md section 4 for the contract):

```text
xtask/src/version.rs
packaging/env.sh
packaging/macos/{import-cert.sh,package.sh,verify.sh,Info.plist,entitlements.plist}
packaging/windows/{package.ps1,notecraft.wxs}
packaging/linux/{package.sh,flatpak-bundle.sh,io.github.queball1999.notecraft.desktop,io.github.queball1999.notecraft.metainfo.xml,nfpm.yaml}
packaging/freebsd/package.sh
assets/app-icon/{notecraft.svg,notecraft.ico,notecraft.icns,hicolor/...}
apps/notecraft/build.rs
```

Steps:

1. `cargo xtask version set <v>`: set `[workspace.package] version` and refresh `Cargo.lock` (`cargo update --workspace`). Validate SemVer; refuse anything else.
2. Copy the packaging scripts from GridCraft (newest of the related craft apps at the time). Replace `gridcraft` with `notecraft`, `GridCraft` with `NoteCraft`, the app id with `io.github.queball1999.notecraft`, and binary names. Remove anything specific to spreadsheets. Keep comments, convert any non-ASCII characters to ASCII.
3. App icon: an original design (for example a rounded square with a stylized page and pen stroke in the 2016 accent purple), drawn as SVG and exported to `.ico`, `.icns`, and the hicolor sizes by a script under `packaging/icons.sh`. Add ATTRIBUTION.md rows (author: The NoteCraft Contributors, license: MIT OR Apache-2.0).
4. `apps/notecraft/build.rs`: read `VERSION`, `BUILD_DATE`, `BUILD_SHA` (generic names; NoteCraft is not in the shared workflow's per-app list) with fallbacks to the Cargo version, today, and `git rev-parse --short HEAD`. Expose as `env!` constants.
5. Test with a manual Release workflow run from `chore/packaging` with version `0.0.0-test.1`. Signing is skipped off the `release` branch.

Commits:

1. `feat(xtask): add version set subcommand`
2. `build(packaging): add shared env and macos scripts from gridcraft`
3. `build(packaging): add windows wix installer and script`
4. `build(packaging): add linux and flatpak packaging`
5. `build(packaging): add freebsd packaging script`
6. `feat(app): add original app icon and icon export script`
7. `build(app): embed version, date, and sha from build env`
8. `docs: record app icon in ATTRIBUTION.md`

PR title: `build(packaging): meet the craft-common-actions release contract`. Labels: `phase:m1`, `area:packaging`.

Acceptance: Packaging lint workflow passes; the manual Release run produces artifacts for macOS, Windows x64 and arm64, Linux, Flatpak, riscv64, and FreeBSD, and a draft release (delete the draft afterward).

## PR -1.6: logging and build info

| | |
|---|---|
| Branch | `feat/logging` off `phase/m1-scaffolding` |
| Hours | 4-8 |
| Depends on | -1.5 (build.rs) |
| Rows | SC-09 |

Steps:

1. `apps/notecraft/src/logging.rs`: `tracing-subscriber` to stderr and to `logs/notecraft.log` in the settings folder; rotate `notecraft.log` to `notecraft.1.log` and `notecraft.2.log` on launch; cap at 16 MiB (stop writing and log one warning); default filter `info` for `notecraft*`, `warn` otherwise; `RUST_LOG` overrides; `NOTECRAFT_NO_PREFS=1` disables file logging.
2. Log the version, build SHA, OS, and GPU backend at startup. Never log note content (add a comment saying so at the top of the module).
3. `notecraft --version` and `notecraft-cli --version` print `NoteCraft <version> (<sha>, <date>)`.

Commits:

1. `feat(app): add file and stderr logging with rotation`
2. `feat(app): print version and build info`
3. `feat(cli): print version and build info`
4. `test(app): cover log rotation and size cap`

PR title: `feat(app): add logging and version output`.

## PR -1.7: close Phase -1

Follow 00-conventions.md section 6. Branch `docs/phase-m1-close`. Commit `docs: close phase -1`. Then the phase PR `Phase -1: scaffolding` into `main`, merge commit. Then:

```sh
git checkout main && git pull
git checkout -b phase/0-ui-shell
git push -u origin phase/0-ui-shell
```

## 6. GitHub settings (maintainer)

Done once, right after section 0. Uses `gh` with admin rights.

1. Branch protection for `main`: require PR, require the `cargo xtask ci` check, require 1 approval, no force pushes, no deletions.
2. Branch protection for `release`: same, plus "require linear history" (fast-forward only); only maintainers can push.
3. Rulesets for `phase/*`: require PR and the `cargo xtask ci` check; allow merge commits from `main`.
4. Labels from 00-conventions.md section 3.2:
   ```sh
   for l in phase:m1 phase:0 phase:1 phase:2 phase:3 phase:4 area:core area:storage area:model area:markdown area:canvas area:engine area:ui area:cli area:ci area:docs area:packaging coming-soon breaking; do
     gh label create "$l" --force
   done
   ```
5. Environment `release` with the secrets in docs/releasing.md section 5.
6. Enable Discussions (the question issue template points there) and private vulnerability reporting.

## Estimate

| PR | Hours |
|---|---|
| Section 0 bootstrap | 1-2 |
| -1.1 workspace and crates | 6-10 |
| -1.2 xtask ci | 6-10 |
| -1.3 text, assets, layers | 10-16 |
| -1.4 wasm, parity | 8-14 |
| -1.5 release contract | 16-30 |
| -1.6 logging | 4-8 |
| -1.7 close | 2-4 |
| Section 6 GitHub settings | 2-4 |
| Total | 55-98 |

## Phase close notes (2026-10-14)

- Release workflow: replaced the shared craft-common-actions release workflow with NoteCraft's own (PR #1), because the shared one builds macOS on a paid `macos-15-xlarge` runner and has no license to copy. The dry run (0.0.0-test.2) produced all 20 artifacts on free standard runners; its draft release was deleted.
- GitHub settings as applied: `main` requires a PR and the `cargo xtask ci` check with 0 approvals (a single maintainer cannot approve their own PRs); `release` is linear history only, no force pushes or deletion; a `phase/*` ruleset requires a PR and the CI check, with repository admins allowed to bypass. Labels, Discussions, and an empty `release` environment exist; signing secrets are still to be added.
- The repo is `github.com/queball1999/notecraft` (GitHub lowercases the owner; links with `Queball1999` resolve to it).

## Exit criteria

- SC-01 to SC-10 scored D.
- CI green on `main`; FreeBSD and Windows ARM64 nightlies green.
- A manual Release run produced every artifact.
- docs/roadmap.md "Current focus" says Phase 0.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | PR -1.1 landed; sample Cargo.toml now lists members explicitly. |
| 2026-10-14 | -1 | Repo and app ID moved off the ArtCraft namespace. |
| 2026-10-14 | -1 | Phase closed; release workflow change and GitHub settings recorded. |
| 2026-10-15 | 0 | Reviewed at the Phase 0 close; no content change. |
