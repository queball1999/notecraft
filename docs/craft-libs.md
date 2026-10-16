# Shared craft libraries

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-16. Change: craft-atomic also writes the recovery journal (PR 1.7); craft-atomic adopted by notecraft-storage (PR 1.4).

NoteCraft uses the shared crates in [storytold/craft-libs](https://github.com/storytold/craft-libs) instead of writing its own versions. This doc says which ones, where, and how to update them. For each crate's API, read its docs at the pinned commit; this doc does not repeat them.

## 1. How to depend on them

Every craft-libs crate is a git dependency pinned to one commit, declared once in the root `Cargo.toml` under `[workspace.dependencies]`:

```toml
[workspace.dependencies]
craft-atomic   = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
craft-settings = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
craft-ui       = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
craft-print    = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
craft-quit     = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
craft-cms      = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
```

Member crates then use `craft-atomic = { workspace = true }`. All craft crates share one `rev`, so they always come from the same commit.

Currently pinned: `457a078809690994b22debdffb703ff36d3129df` (2026-10-14). In use: `craft-ui` (notecraft-ui-egui; `theme::WidgetPalette` sets widget fills from the theme tokens), `craft-quit` with the `egui` and `menu` features and `craft-settings` (the notecraft app). craft-libs uses egui 0.36, so NoteCraft does too.

## 2. Which crate goes where

| Crate | What it does | Used by | Layer | Adopted in |
|---|---|---|---|---|
| `craft-atomic` | Crash-safe file writes (temp file, flush, rename) | `notecraft-storage` for every page, `.toml`, and attachment write; `notecraft-engine` export | L1, L5 | Phase 1 |
| `craft-settings` | `config.toml` settings store in the platform settings folder | `notecraft-storage` (load and save), surfaced by `notecraft-engine` | L1 | Phase 0 |
| `craft-ui` | egui components: tabs, buttons, docking, print settings dialog, theme helpers | `notecraft-ui-egui` | L6 | Phase 0 |
| `craft-print` | Sends a PDF to the OS printer: winprint on Windows, CUPS on macOS and Linux | `notecraft` (File > Print) | L7 | Phase 3 |
| `craft-quit` | macOS quit handling and app menu | `notecraft` (macOS only, `cfg(target_os = "macos")`) | L7 | Phase 0 |
| `craft-cms` | ICC color management | `notecraft-canvas` image decode, `notecraft-engine` PDF export (optional feature `color-managed`) | L4, L5 | Phase 3 (optional) |

Notes:

- `craft-ui` needs Rust 1.95 or newer, which sets the workspace MSRV (`rust-version = "1.95"`).
- `craft-ui` depends on egui, so it may only be used at L6 and above. `cargo xtask layers` enforces this.
- `craft-settings` and `craft-atomic` must build for `wasm32-unknown-unknown` because L1 is in the wasm check. If a crate does not, gate it behind `cfg(not(target_arch = "wasm32"))` and provide a browser-storage fallback in `notecraft-storage`.

## 3. Not used directly

| Crate | Why not |
|---|---|
| `winprint` (storytold fork) | `craft-print` wraps it. Depending on it directly would bypass the cross-platform layer. Never add it to any `Cargo.toml`. |

## 4. Related shared repos

| Repo | Used for |
|---|---|
| [storytold/craft-common-actions](https://github.com/storytold/craft-common-actions) | Reusable GitHub workflows called by `freebsd.yml`, `windows-arm64.yml`, and `packaging-lint.yml`. Not used for releases: `release.yml` is NoteCraft's own, on free runners (docs/releasing.md) |
| [storytold/craft-fonts](https://github.com/storytold/craft-fonts) | Openly licensed fonts the related craft apps embed. Not used. Decided in PR 0.4: NoteCraft uses the OS UI font and falls back to egui's built-in fonts, so it bundles no font (docs/themes.md section 2). If that changes, take the font from craft-fonts and pass `craft-fonts-ref` in the workflows, as DeckCraft and GridCraft do |
| [storytold/craftrules](https://github.com/storytold/craftrules) | Shared engineering, never-crash, licensing, and progress-docs standards |

## 5. Updating the pinned commit

1. Branch `chore/bump-craft-libs` off the current phase branch.
2. Change `rev` for all craft crates to the new commit in one edit.
3. Run `cargo update -p craft-atomic -p craft-settings -p craft-ui -p craft-print -p craft-quit -p craft-cms`.
4. Read the craft-libs commit log between the old and new commits; note breaking changes in the PR body.
5. Run `cargo xtask ci`, and run the Printing workflow manually if `craft-print` changed.
6. Commit: `chore(deps): bump craft-libs to <short-sha>`.

Dependabot ignores `craft-*` on purpose (`.github/dependabot.yml`).

## 6. When craft-libs is missing something

Do not fork the code into NoteCraft. Open an issue or PR on craft-libs, and if NoteCraft needs a stopgap, put it behind a small trait in the NoteCraft crate that would use the shared crate, with a `// TODO(craft-libs#<n>)` comment, so it is easy to remove.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Crate mapping, pinning, bump procedure, related shared repos. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | Craft-common-actions now covers FreeBSD tests, Windows ARM64 checks, and packaging lint only. |
| 2026-10-14 | -1 | Craft-libs pinned at 457a0788; craft-ui, craft-quit, craft-settings adopted. |
| 2026-10-14 | 0 | craft-ui `theme::WidgetPalette` used by the theme system (PR 0.3). |
| 2026-10-14 | 0 | craft-fonts not adopted: NoteCraft uses the OS UI font with egui's built-in fallback (PR 0.4). |
| 2026-10-15 | 1 | craft-atomic adopted: `notecraft-storage::page_file` writes pages through it (PR 1.4). |
| 2026-10-16 | 1 | craft-atomic also writes the recovery journal entries (PR 1.7); the journal lives in the folder craft-settings uses (`recovery/`). |
