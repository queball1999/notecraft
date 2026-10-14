# Conventions: branches, commits, PRs, reviews

> Stage: pre-alpha (scaffolding). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: project renamed to NoteCraft; earlier changes are in the revision history.

These conventions match the related craft apps, plus NoteCraft's plain-text rule. AGENTS.md section 9 is the short version.

## 1. Branches

```text
main                                   protected; always green; CI on every push
 |-- release                           protected; fast-forward from main only; push = release build
 |
 `-- phase/<n>-<slug>                  one per phase, off main, merged back at phase close
      |-- feat/<slug>                  new feature
      |-- fix/<slug>                   bug fix
      |-- docs/<slug>                  docs only
      |-- chore/<slug>                 tooling, deps, CI, scaffolding
      |-- test/<slug>                  tests only
      `-- refactor/<slug>              no behavior change
```

Phase branch names:

| Phase | Branch |
|---|---|
| -1 | `phase/m1-scaffolding` ("m1" means minus one; a leading dash breaks git commands) |
| 0 | `phase/0-ui-shell` |
| 1 | `phase/1-core-engine` |
| 2 | `phase/2-links-search-tags` |
| 3 | `phase/3-polish-parity` |
| 4 | `phase/4-advanced` |

Rules:

- Slugs are lowercase, words joined by hyphens, at most 40 characters: `feat/ribbon-home-tab`.
- Work branches come off the current phase branch, never off `main` directly (except hotfixes, section 7).
- Keep the phase branch current: after each merge into `main` from elsewhere (hotfix, dependency bump), merge `main` into the phase branch with a merge commit `chore: merge main into phase/<n>-<slug>`.
- Delete work branches after merge.

## 2. Commits

Conventional Commits:

```text
<type>(<scope>): <subject>

<body: what and why, wrapped at 72 columns>

Refs: build_plan/<file>.md PR <n>
<trailers>
```

- Types: `feat`, `fix`, `docs`, `chore`, `test`, `refactor`, `ci`, `perf`, `build`.
- Scopes: the crate without the `notecraft-` prefix (`core`, `storage`, `model`, `markdown`, `canvas`, `engine`, `ui`, `app`, `cli`, `web`), or `xtask`, `ci`, `docs`, `deps`, `release`, `packaging`.
- Subject: imperative mood ("add", not "added" or "adds"), lowercase first word, no period, at most 72 characters including the prefix.
- Body: explain why when it is not obvious. Plain ASCII.
- One logical change per commit. Each commit should build and pass tests on its own; this keeps `git bisect` useful.
- Breaking changes to the file format or a command id: add `BREAKING CHANGE: <what>` in the footer.
- Agent commits end with the co-author trailer the session gives, for example `Co-Authored-By: Claude <noreply@anthropic.com>`.

Good:

```text
feat(canvas): add zoom to fit page width

Zoom to fit is the default when a page opens, matching OneNote. The
command is view.zoom_fit_width and is bound to Ctrl+Alt+W.

Refs: build_plan/02-phase-0-ui-shell.md PR 0.7
```

Bad: `Added zoom stuff.`, `feat: canvas zoom, palette, and theme fixes` (three changes), any subject with an em dash or emoji.

## 3. Pull requests

### 3.1 Opening

```sh
git push -u origin feat/<slug>
gh pr create \
  --base phase/<n>-<slug> \
  --head feat/<slug> \
  --title "feat(<scope>): <subject>" \
  --body-file .git/PR_BODY.md \
  --label "phase:<n>" --label "<area label>"
```

Write the body to `.git/PR_BODY.md` first (inside `.git/`, so it is never committed). Use the repo's PR template (`.github/pull_request_template.md`); every phase file gives the filled-in body for each PR.

- Title: same format as a commit subject. For a single-commit PR, the commit subject.
- Base: the phase branch. Only phase-close PRs and release-prep PRs target `main`.
- Size: aim for under 400 changed lines excluding generated files and test fixtures. Split bigger work.
- UI changes: screenshots in light and dark, at least one theme preset. Attach them in the PR description.
- Link the issue (`Closes #<n>`) and the build plan PR number.
- Agent-written PRs end with the line `Generated with [Claude Code](https://claude.com/claude-code)`, with no emoji.

### 3.2 Labels

| Label | Meaning |
|---|---|
| `phase:m1`, `phase:0` to `phase:4` | which phase |
| `area:core`, `area:storage`, `area:model`, `area:markdown`, `area:canvas`, `area:engine`, `area:ui`, `area:cli`, `area:ci`, `area:docs`, `area:packaging` | which part |
| `coming-soon` | adds a button whose engine side is not done |
| `breaking` | changes the file format or a command id |

### 3.3 Merging

- Work PRs into a phase branch: squash merge. The squash commit message is the PR title plus the `Refs:` line.
- Phase-close PRs into `main`: merge commit (no squash), so the phase history stays visible.
- Never force-push a phase branch, `main`, or `release`.
- Required: CI green, one approving review (maintainer, or a second agent session when the maintainer allows it), conversations resolved.

## 4. Reviews

The reviewer checks, in order:

1. The PR does what its build plan section says, no more.
2. `cargo xtask ci` is green in CI.
3. No `unwrap`, `expect`, `panic!`, `unsafe`, or indexing that can panic outside tests.
4. New user actions are commands with ids; ids follow `area.verb`.
5. Layering holds; engine crates do not see egui.
6. Tests cover the change, including a hostile-input test for any parser change.
7. Text is plain ASCII, US English.
8. Docs listed under "Docs to update" are updated, with status lines and revision history.

## 5. Docs updates per PR

When a PR lands a feature (AGENTS.md section 8):

1. Re-score its rows in docs/parity-checklist.md and run `cargo xtask parity` to refresh the summary.
2. Delete or shrink its entry in docs/gaps.md.
3. Add one line to the ROADMAP.md progress log: `| YYYY-MM-DD | <what landed> (PR #<n>) |`.
4. Update the status line of each doc you touched: bump "Last updated", write a one-line "Change".
5. Add a row to each touched doc's revision history.

These doc edits go in the same PR as the code, in a final commit `docs: update progress docs for <feature>`.

## 6. Phase close

1. Confirm every PR in the phase file is merged or explicitly moved to a later phase (with a note in that phase file).
2. On a `docs/phase-<n>-close` branch off the phase branch: bump "Last reviewed" on every doc in `docs/` and `build_plan/`, update ROADMAP.md stage and numbers, update docs/roadmap.md "Current focus". Commit `docs: close phase <n>`. PR into the phase branch.
3. Open the phase PR: base `main`, head `phase/<n>-<slug>`, title `Phase <n>: <name>`, body listing every merged PR. Merge with a merge commit.
4. Create the next phase branch from the new `main`.

## 7. Hotfixes

A bug on `main` that blocks everyone: branch `fix/<slug>` off `main`, PR into `main`, squash merge, then merge `main` into the active phase branch.

## 8. Releases

See docs/releasing.md. Release prep is a `chore/release-<version>` PR into `main`; the release itself is a fast-forward of `release` to `main`.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
