# AGENTS.md: rules for every agent and contributor

NoteCraft is a clean-room, open-source note-taking app. It is related to the ArtCraft apps (WordCraft, DeckCraft, GridCraft) and follows their patterns and shared standards, but it is an independent project, not an official ArtCraft app. These rules bind every agent and human contributor. The shared standards live in [craftrules](https://github.com/storytold/craftrules) (`standards/never-crash.md`, `standards/engineering.md`, `standards/licensing.md`, `standards/progress-docs.md`). Read them before writing code.

## 1. Clean-room and asset policy

- No Microsoft, Adobe, or any third-party artwork, icons, fonts, themes, or templates. NoteCraft's ribbon icons, theme palettes, and sample notes are original work, drawn or generated in code.
- "OneNote" is used only to describe the workflow NoteCraft is compatible with. It is a trademark of its owner. NoteCraft is not affiliated with, sponsored by, or endorsed by it.
- NoteCraft compares itself only to OneNote. Do not name or compare against any other note-taking app in docs, code, commits, or PRs. Wikilinks, backlinks, and the graph view are described as NoteCraft features in their own right.
- Every bundled asset (fonts, icons, images, test data) needs a row in [ATTRIBUTION.md](ATTRIBUTION.md) with author, source, and license. `cargo xtask assets` fails if one is missing.
- Use original or openly licensed (MIT, Apache-2.0, CC0, OFL) assets only.

### Scope: OneNote parity first

- The build plan covers OneNote features only. Do not add features OneNote does not have (backlinks, graph view, command palette, plugins, and similar) to any phase. Exception: the command line (`notecraft-cli`, including its MCP server) follows the related craft apps (maintainer decision, 2026-10-15).
- Those ideas go in [docs/post-parity.md](docs/post-parity.md) and are considered only after full parity.

## 2. Plain text style (US English, ASCII only)

This rule applies to every file in the repo and to every commit message, PR title, PR body, issue, and code review comment.

- Write in US English: color, behavior, organize, center, license (noun and verb), canceled, gray.
- Use plain ASCII characters only. Specifically, never use:
  - em dashes or en dashes (use a comma, colon, period, or parentheses instead; use a plain hyphen for ranges, as in `50-90 hours`)
  - emoji or pictographs of any kind
  - typographic arrows (write "to", or `->` inside code only)
  - curly quotes or curly apostrophes (use `"` and `'`)
  - middle dots, bullets, section signs, multiplication signs, ellipsis characters, or comparison symbols (write `x`, `...`, `<=`, "at most")
  - box-drawing characters (draw diagrams and trees with `+`, `-`, `|`, and backticks)
- Keep formatting plain. Use headings, short paragraphs, tables, lists, and code blocks. Do not use decorative bold on every line, ALL-CAPS shouting, or ornate phrasing. Bold is for the one phrase a reader must not miss.
- Exemptions: fixtures under `testdata/` and translation files under `locales/` may contain any Unicode they need to test or translate. In Rust source, write non-ASCII test input with `\u{...}` escapes.
- Enforcement: the `text-check` CI job fails on any non-ASCII byte outside the exemptions. In Phase -1 it moves into `cargo xtask text`, which `cargo xtask ci` runs.

## 3. Never crash

- Every crate's `lib.rs` starts with:
  ```rust
  #![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
  ```
- No `unsafe` (the workspace lints forbid it).
- Every parser is lenient (unknown structures skipped, bad numbers fall back to defaults) and bounded (sizes, depths, counts capped), so hostile input yields an error or a best-effort result, never a panic.
- The engine's command guard catches an escaped panic and restores the document.

## 4. Everything is a command

- Every user-visible action (ribbon button, menu item, shortcut, gesture) is a command with a stable id in `crates/notecraft-engine/src/cmd/*`.
- The ribbon, shortcuts, gestures, and the test control channel all dispatch by id.
- To add a feature: add a command, wire it to the UI, and it is automatically reachable from the control channel used by UI tests. (This is also what would make post-parity automation cheap to add later.)

## 5. Architecture and layering

| Layer | Crates | What they do |
|---|---|---|
| L0 | `notecraft-core` | geometry, colors, units, ink stroke types, input normalization |
| L1 | `notecraft-storage` | SQLite, markdown I/O, `.notecraft` I/O, attachment store, auto-save |
| L2 | `notecraft-model` | notebook, section, page, canvas objects |
| L3 | `notecraft-markdown` | markdown parse and render, page links, note tags, equations |
| L4 | `notecraft-canvas` | ink engine, shapes, layout, hit testing, zoom and pan |
| L5 | `notecraft-engine` | session, command registry, undo/redo, clipboard, export |
| L6 | `notecraft-ui-egui` | the OneNote-style egui front end (swappable) |
| L7 | `notecraft`, `notecraft-cli`, `notecraft-web` | desktop, command line (see docs/cli.md), browser (Phase 4) |

- Layering is enforced by `cargo xtask layers`: a crate may only depend on lower layers, and nothing below L6 depends on egui, eframe, winit, or rfd.
- Engine crates are UI-agnostic. The egui front end is a separate, swappable layer.

## 6. Shared libraries (craft-libs)

Use the shared [craft-libs](https://github.com/storytold/craft-libs) crates where they fit. Do not reinvent them. See [docs/craft-libs.md](docs/craft-libs.md) for the full mapping.

- Depend by git, pinned to a commit:
  ```toml
  craft-atomic   = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
  craft-settings = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
  craft-ui       = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
  craft-print    = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
  ```
- Printing goes through `craft-print`, which wraps `winprint` (Windows) and CUPS (macOS and Linux). Do not depend on `winprint` directly. See [docs/printing.md](docs/printing.md).
- `craft-ui` needs Rust 1.95 or newer, so the workspace MSRV is 1.95.

## 7. Quality gates

`cargo xtask ci` runs, in order: `rustfmt --check`, `clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo xtask text` (plain-text style), `cargo xtask assets` (attribution), `cargo xtask layers` (layering), and the wasm32 check. CI runs it on every PR and every push to `main`.

## 8. Planning and progress docs

The committed progress docs follow craftrules `standards/progress-docs.md`:

| File | Holds |
|---|---|
| `ROADMAP.md` | Stage, headline numbers, dimensions, progress log |
| `docs/target-app-parity.md` | The parity assessment vs OneNote |
| `docs/gaps.md` | Every known shortfall, ranked: the work list. Pick from the top |
| `docs/roadmap.md` | Current focus, the alpha plan, milestones |
| `docs/parity-checklist.md` | Feature rows scored D/P/M; `cargo xtask parity` regenerates its summary |
| `docs/architecture.md` | How NoteCraft is built today |

Every doc in `docs/` opens with a status line and ends with a revision history table:

```markdown
> Stage: pre-alpha (scaffolding). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: <one line>.
```

When a task lands: re-score its checklist row, delete or update its `gaps.md` entry, add a line to the `ROADMAP.md` progress log, and update each touched doc's status line and revision history. At the end of each phase, review every doc in `docs/`, even ones the phase did not touch, and bump its "Last reviewed" date.

The execution plan (phases, branches, commits, PRs) is in [`build_plan/`](build_plan/README.md).

## 9. Branching, commits, and PRs

Follow the conventions in [`build_plan/00-conventions.md`](build_plan/00-conventions.md):

- Branches: `main` (protected) and `release` (protected; fast-forward from `main` to ship), then `phase/<n>-<slug>` off `main`, then `feat/<slug>`, `fix/<slug>`, `docs/<slug>`, `chore/<slug>` off the phase branch.
- Commits: Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`, `test:`, `refactor:`, `ci:`), imperative mood, scoped, subject at most 72 characters.
- PRs: one logical change per PR, small enough to review in one sitting. Every PR passes `cargo xtask ci`. UI changes require screenshots. Link the issue and the `build_plan` phase.
- Agents never commit, push, or open PRs unless the maintainer asks for that in the current session.

## 10. Clean up before you finish

Before an agent reports a task done or hands off, it removes what it created and no longer needs. Leave the repo, the disk, and the running system as you found them, plus the intended change.

- Delete scratch files, throwaway test notebooks, temporary scripts, logs, screenshots, and probe or debug code that is not part of the change. Remove temporary tests that only served an investigation; keep tests that guard the change.
- Remove debug output (`dbg!`, stray `println!`, extra log lines) and commented-out experiments before committing.
- Delete build output you made outside the shared target folders, and the build output of worktrees whose work has landed. Remove worktrees and local branches whose PRs have merged.
- Stop every process you started: the app, servers, watchers, and background CI runs. Close any OS dialog you opened.
- Drop any stash entry you created (never use bare `git stash`), and do not leave stray WIP commits.
- Keep what later work needs: work in progress, unmerged branches, notes a handoff depends on, and logs a maintainer asked for. Say in the final report or handoff what was kept and why, where it is, and when it can be deleted.
- Never delete what you did not create, or what another session or agent is using. If unsure, list it in the report instead of deleting it.
