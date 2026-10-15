# Roadmap: focus, alpha plan, milestones

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-15. Change: Phase 0 estimate raised for ribbon parity follow-ups (PR 0.5c to 0.5f).

The stage summary and progress log are in [ROADMAP.md](../ROADMAP.md). This doc says what we are doing now, what the alpha needs, and the milestones after it. The step-by-step execution plan is in [build_plan/](../build_plan/README.md). Optional features OneNote does not have are in [post-parity.md](post-parity.md) and are not on this roadmap.

## Current focus

Phase 0, UI shell ([build_plan/02](../build_plan/02-phase-0-ui-shell.md)): the whole OneNote interface, clickable, on mock data. Branch `phase/0-ui-shell`.

Phase -1 closed on 2026-10-14 with every exit criterion met. The Release workflow dry run (0.0.0-test.2) built all 20 artifacts on free runners with NoteCraft's own release workflow. Signing secrets are not set yet; add them to the `release` environment before the first real release.

## Phases and estimates

Hours are summed from every PR in the build plan. They assume one experienced Rust developer working with agent help, and include tests, docs, and review. Calendar weeks assume about 30 focused hours a week.

| Phase | Theme | Hours | Calendar | Plan |
|---|---|---|---|---|
| -1 | Scaffolding and CI/CD | 55-98 | 2-4 weeks | [01](../build_plan/01-phase-minus-1-scaffolding.md) |
| 0 | UI shell | 278-464 | 9-16 weeks | [02](../build_plan/02-phase-0-ui-shell.md) |
| 1 | Core engine | 444-756 | 15-26 weeks | [03](../build_plan/03-phase-1-core-engine.md) |
| 2 | Page links, search, note tags | 130-222 | 4-8 weeks | [04](../build_plan/04-phase-2-links-search-tags.md) |
| 3 | Polish and parity | 356-606 | 12-21 weeks | [05](../build_plan/05-phase-3-polish-parity.md) |
| Hardening | Beta to 1.0 | 80-160 | 3-6 weeks | [05](../build_plan/05-phase-3-polish-parity.md#beta-to-10-hardening) |
| To 1.0 | | 1,343-2,306 | 45-77 weeks | |
| 4 | Remaining parity (post-1.0) | 644-1,228 | 22-41 weeks | [06](../build_plan/06-phase-4-advanced.md) |
| Full parity | | 1,987-3,534 | 66-118 weeks | |

### What changed in the 2026-10-14 re-assessment

- Before: dimension hours were top-down guesses carried over from the SCRD (about 1,030-1,800 hours, with no phase totals).
- Now: every PR has a range, phases are sums of PRs, dimensions are sums of PRs mapped to them.
- Scope removed (not OneNote features, now in post-parity.md): backlinks, graph view, command palette, full CLI, MCP, full control channel, plugins, code highlighting, footnotes, embeds. About 300-530 hours.
- Scope added (OneNote features that were missing): Find Tags summary, date and time stamps, audio and video recording, password-protected sections, OCR, ink search.
- Raised after review of risk: ribbon icon set, live markdown editor, minimal-diff writer, pen pressure without `unsafe`, `.one` import, video capture.

## Alpha gate

v0.1.0 ships when every row below works end to end on Windows, macOS, and Linux. Evidence is a test name or a PR link with screenshots. SCRD section 5 is updated to match.

| Workflow | Must work | Phase | Checklist rows | Evidence |
|---|---|---|---|---|
| UI shell | Full ribbon, every OneNote tab, every button clickable; unimplemented ones show "Coming Soon" | 0 | SH-02 to SH-05 | |
| Canvas | Pan and zoom on an empty page, rule lines and grid | 0 | CV-01 to CV-03 | |
| Notebook navigation | Notebook list, section tabs, page list from a real folder | 0, 1 | SH-06 to SH-08, ST-01 | |
| Theme switching | All four presets switch live | 0 | TH-01 to TH-05 | |
| Markdown read | Open a `.md` file, render it on the canvas | 1 | FM-01 | |
| Markdown write | Type in a container, save to `.md` | 1 | TX-01, FM-02 | |
| Open dialog | Open a notebook folder, `.md`, or `.notecraft` | 0 | SH-12 | |
| Auto-save | Every 30 seconds and on focus loss | 1 | FM-05 | |
| Dark mode | Toggle light and dark | 0 | TH-01 to TH-05 | |
| Recent files | File backstage lists the last 10 | 0 | SH-11 | |

## Milestones

| Milestone | Version | Contents | Ends with |
|---|---|---|---|
| M-1 | none | Scaffolding | Phase -1 exit criteria |
| M0 | 0.0.1 (internal test release) | UI shell on mock data | Phase 0 done; first `release` branch build |
| Alpha | 0.1.0 | Alpha gate plus ink, pictures, attachments, undo | Phase 1 done |
| M2 | 0.2.0 | Page links, search, note tags, equations | Phase 2 done |
| Beta | 0.9.0 | Shapes, selection tools, tables, recycle bin, page versions, print, PDF and HTML export | Phase 3 done; parity about 70% |
| 1.0 | 1.0.0 | Beta plus fixes, performance targets (SCRD section 11) | parity about 90% |
| Full parity | 1.x | Handwriting, ink search, `.one` import, recording, password sections, OCR, docked window, web, translations | Phase 4 done |
| Post-parity | later | Optional features from post-parity.md, by demand | maintainer decision |

## How this doc is maintained

- Change "Current focus" when a phase starts.
- Fill the Evidence column of the alpha gate as rows land.
- At the end of each phase, replace estimates with actual hours for finished work, re-estimate the remaining phases, and record the change in the revision history.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Re-scoped the same day to OneNote parity; estimates re-assessed bottom-up (1,279-2,202 hours to 1.0, 1,923-3,430 to full parity); command palette removed from the alpha gate. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | Current focus moved to Phase 0; Phase -1 close recorded. |
| 2026-10-15 | 0 | Phase 0 estimate raised to 278-464 hours for ribbon parity follow-ups (PR 0.5c to 0.5f); totals 1,343-2,306 to 1.0 and 1,987-3,534 to full parity. |
