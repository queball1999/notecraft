# Build plan

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-17. Change: OneNote import of every OneNote file type (`.one`, `.onex`, `.onetoc2`, `.onepkg`) moved into Phase 3 with full fidelity (PR 3.16, 3.17; maintainer decision, 2026-10-16); Phase 0 done, Phase 1 next; totals updated.


This folder is the execution plan for NoteCraft. It is written so an agent or a new contributor can pick up any phase cold and know exactly which branch to create, which files to touch, which commits to make, and how to open each PR.

The spec is [SCRD.md](../SCRD.md). The rules are [AGENTS.md](../AGENTS.md). Progress is tracked in [ROADMAP.md](../ROADMAP.md) and [docs/](../docs/).

## Files

| File | Phase | Hours | Status |
|---|---|---|---|
| [00-conventions.md](00-conventions.md) | all | n/a | branch, commit, PR, and review rules |
| [01-phase-minus-1-scaffolding.md](01-phase-minus-1-scaffolding.md) | -1 | 55-98 | done 2026-10-14 |
| [02-phase-0-ui-shell.md](02-phase-0-ui-shell.md) | 0 | 284-474 | done 2026-10-15 |
| [03-phase-1-core-engine.md](03-phase-1-core-engine.md) | 1 | 456-776 | next |
| [04-phase-2-links-search-tags.md](04-phase-2-links-search-tags.md) | 2 | 134-230 | not started |
| [05-phase-3-polish-parity.md](05-phase-3-polish-parity.md) | 3, plus beta-to-1.0 hardening | 664-1,126, plus 80-160 | not started |
| [06-phase-4-advanced.md](06-phase-4-advanced.md) | 4 | 508-952 | not started |

Totals: 1,673-2,864 hours to v1.0; 2,181-3,816 hours to full OneNote parity. Each file's "Estimate" table is the source; ROADMAP.md sums them.

## Scope

The build plan covers OneNote parity only, with one exception: the command line and its MCP server follow the related craft apps (maintainer decision, 2026-10-15; docs/cli.md). Other features OneNote does not have (backlinks, graph view, command palette, plugins, and others) are listed in [docs/post-parity.md](../docs/post-parity.md) and are considered only after full parity.

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
| 2026-10-15 | 0 | Command line PRs added (0.13, 1.16, 2.11, 3.14, 3.15); phase and total hours updated. |
| 2026-10-15 | 0 | Phase 0 marked done, Phase 1 next; Phase 1 estimate 456-776 hours (PR 1.15 +2-4 for deferred hands-on checks); totals 1,401-2,404 to 1.0 and 2,045-3,632 to full parity. |
| 2026-10-16 | 1 | OneNote import moved to Phase 3 with full fidelity (PR 3.16, 3.17); PR 4.3 narrowed; phase and total hours updated. |
