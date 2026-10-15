# Build plan

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: Phase -1 marked done, Phase 0 next.

This folder is the execution plan for NoteCraft. It is written so an agent or a new contributor can pick up any phase cold and know exactly which branch to create, which files to touch, which commits to make, and how to open each PR.

The spec is [SCRD.md](../SCRD.md). The rules are [AGENTS.md](../AGENTS.md). Progress is tracked in [ROADMAP.md](../ROADMAP.md) and [docs/](../docs/).

## Files

| File | Phase | Hours | Status |
|---|---|---|---|
| [00-conventions.md](00-conventions.md) | all | n/a | branch, commit, PR, and review rules |
| [01-phase-minus-1-scaffolding.md](01-phase-minus-1-scaffolding.md) | -1 | 55-98 | done 2026-10-14 |
| [02-phase-0-ui-shell.md](02-phase-0-ui-shell.md) | 0 | 278-464 | next |
| [03-phase-1-core-engine.md](03-phase-1-core-engine.md) | 1 | 444-756 | not started |
| [04-phase-2-links-search-tags.md](04-phase-2-links-search-tags.md) | 2 | 130-222 | not started |
| [05-phase-3-polish-parity.md](05-phase-3-polish-parity.md) | 3, plus beta-to-1.0 hardening | 356-606, plus 80-160 | not started |
| [06-phase-4-advanced.md](06-phase-4-advanced.md) | 4 | 644-1,228 | not started |

Totals: 1,343-2,306 hours to v1.0; 1,987-3,534 hours to full OneNote parity. Each file's "Estimate" table is the source; ROADMAP.md sums them.

## Scope

The build plan covers OneNote parity only. Features OneNote does not have (backlinks, graph view, command palette, MCP, full CLI, plugins, and others) are listed in [docs/post-parity.md](../docs/post-parity.md) and are considered only after full parity.

## How to use a phase file

1. Read 00-conventions.md once.
2. Open the phase file. Check its "Entry criteria". Do not start a phase whose entry criteria are not met.
3. Work through the PRs in order unless the file says two can run in parallel. Each PR section lists:
   - Branch name and the branch it comes off
   - Hours (generous range)
   - Depends on (earlier PRs)
   - Files created or changed
   - Steps
   - Commits, with exact messages, in order
   - PR title and body
   - Acceptance criteria (what the reviewer checks)
   - Docs to update when it merges
4. When the phase's last PR merges, do the "Phase close" section: merge the phase branch into `main`, update every doc's "Last reviewed" date, and mark the phase done in the table above.

## Rules that apply to every PR

- Plain ASCII, US English in code, comments, commit messages, and PR text (AGENTS.md section 2).
- Compare NoteCraft only to OneNote.
- `cargo xtask ci` passes before you ask for review. (Before PR -1.2 lands, the CI text check is the only gate.)
- Agents do not commit, push, or open PRs unless the maintainer asks in the current session. The commands in these files are what to run when asked.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Same day: scope narrowed to OneNote parity, Phase 2 replaced, hours re-assessed bottom-up. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | Phase -1 marked done, Phase 0 next. |
