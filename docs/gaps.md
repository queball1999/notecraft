# Gaps: the ranked work list

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: ranks 3 (release dry run) and 5 (GitHub settings) removed.

Every known shortfall against OneNote parity, ranked. Pick from the top. When an item lands, delete it here (or shrink it if only part landed), re-score its rows in [parity-checklist.md](parity-checklist.md), and add a line to the [ROADMAP.md](../ROADMAP.md) progress log.

Ranking rule: blockers first (things other work waits on), then alpha gate items, then by user impact per hour. Hour ranges match the build plan PRs. Optional features OneNote does not have are not gaps; they are in [post-parity.md](post-parity.md).

## Now (Phase 0: UI shell)

| Rank | Gap | Rows | Hours | Plan |
|---:|---|---|---|---|
| 6 | No window or app scaffold | SH-01 | 8-14 | build_plan/02, PR 0.1 |
| 7 | No command registry (needed before any button) | SH-03, SH-04 | 12-20 | build_plan/02, PR 0.2 |
| 8 | No theme system or presets | TH-01 to TH-06 | 40-68 | build_plan/02, PR 0.3, 0.4 |
| 9 | No ribbon or original icon set | SH-02, SH-05 | 60-100 | build_plan/02, PR 0.5 |
| 10 | No notebook list, section tabs, page list | SH-06 to SH-08 | 24-40 | build_plan/02, PR 0.6 |
| 11 | No canvas pan, zoom, rule lines | CV-01 to CV-03 | 24-40 | build_plan/02, PR 0.7 |
| 12 | No File backstage, recents, open dialog | SH-11, SH-12 | 12-20 | build_plan/02, PR 0.8 |
| 13 | No status bar, full page view, shortcut map | SH-09, SH-13, SH-14 | 12-20 | build_plan/02, PR 0.9 |
| 14 | No accessibility labels | SH-15 | 10-16 | build_plan/02, PR 0.10 |

## Next (Phase 1: core engine, rest of alpha gate)

| Rank | Gap | Rows | Hours | Plan |
|---:|---|---|---|---|
| 15 | No notebook folder reader or index | ST-01, ST-07 | 34-58 | build_plan/03, PR 1.1, 1.2 |
| 16 | No markdown read and minimal-diff write | FM-01, FM-02, FM-04 | 60-104 | build_plan/03, PR 1.3, 1.4 |
| 17 | No text editing in containers | TX-01 to TX-06, CV-04 to CV-06 | 104-180 | build_plan/03, PR 1.5, 1.6 |
| 18 | No auto-save, crash recovery, outside edit handling | FM-05 to FM-07 | 24-40 | build_plan/03, PR 1.7 |
| 19 | No undo and redo | ST-08 | 20-34 | build_plan/03, PR 1.8 |
| 20 | No `.notecraft` format | FM-03 | 30-50 | build_plan/03, PR 1.9 |
| 21 | No ink engine | IN-01 to IN-06 | 84-140 | build_plan/03, PR 1.10, 1.11 |
| 22 | No pictures or attachments | AT-01 to AT-05 | 40-70 | build_plan/03, PR 1.12 |
| 23 | No structure editing (create, rename, move) | ST-02 to ST-06 | 30-50 | build_plan/03, PR 1.13 |
| 24 | Parsers not fuzzed | FM-08 | 10-16 | build_plan/03, PR 1.14 |

## Later

Phase 2 (page links, search, note tags), Phase 3 (polish and parity), and Phase 4 (remaining parity) items are listed in their build_plan files and in the checklist. They move up into this list when the phase before them is under way.

## File formats

| Gap | Phase | Hours |
|---|---|---|
| `.md` read and minimal-diff write | 1 | 60-104 |
| `.notecraft` read and write | 1 | 30-50 |
| PDF export | 3 | 40-70 |
| HTML export and notebook zip | 3 | part of 40-70 (PR 3.8) |
| OneNote `.one` import (best effort, from the published spec) | 4 | 160-320 |

## Localization

| Gap | Phase | Hours |
|---|---|---|
| String table and `locales/` folder; every UI string through it | 3 | 30-50 |
| Right-to-left layout | 4 | 30-60 |
| First translations (with native-speaker review) | 4 | 50-100 plus review |

All UI strings are written in US English first (AGENTS.md section 2).

## Known risks

| Risk | Impact | Mitigation |
|---|---|---|
| Live markdown editing in egui (IME, selection, caret) is the largest single piece of work | Editing feels unlike OneNote; schedule slips | Budgeted at 80-140 hours (PR 1.5); split into editor widget and containers; prototype early |
| Minimal-diff markdown writing is hard with an AST parser | Outside edits get reformatted | Span-preserving edit layer in `notecraft-markdown`; byte-identical round-trip tests on a sample corpus |
| Pen pressure APIs differ per OS, winit exposes little, and `unsafe` is forbidden | Ink feels wrong | Safe wrapper crates per OS; prototype first in PR 1.10; fall back to velocity-based width |
| About 150 original ribbon icons | Ribbon looks unfinished or derivative | Budgeted in PR 0.5; drawn in code; never traced |
| Shared release workflow sets per-app env vars and has no NoteCraft entry | Version strings missing in builds | Read generic `VERSION`, `BUILD_DATE`, `BUILD_SHA` in `build.rs` (docs/releasing.md section 4) |
| `.one` spec is large | Import incomplete | Best effort with an import report; Phase 4 |

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Re-ranked the same day for OneNote-only scope and re-assessed hours; risks updated. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | Rank 1 (no workspace) removed. |
| 2026-10-14 | -1 | Rank 2 narrowed to the wasm and parity gates. |
| 2026-10-14 | -1 | Rank 2 (xtask gates) removed. |
| 2026-10-14 | -1 | Rank 3 narrowed to the GitHub release dry run. |
| 2026-10-14 | -1 | Rank 4 (logging) removed. |
| 2026-10-14 | -1 | Ranks 3 (release dry run) and 5 (GitHub settings) removed. |
