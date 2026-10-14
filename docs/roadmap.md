# Roadmap: focus, alpha plan, milestones

> Stage: pre-alpha (scaffolding). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: project renamed to NoteCraft; earlier changes are in the revision history.

The stage summary and progress log are in [ROADMAP.md](../ROADMAP.md). This doc says what we are doing now, what the alpha needs, and the milestones after it. The step-by-step execution plan is in [build_plan/](../build_plan/README.md). Optional features OneNote does not have are in [post-parity.md](post-parity.md) and are not on this roadmap.

## Current focus

Pre-code. The docs set and build plan are being finished and reviewed. Phase -1 (scaffolding) starts when the maintainer approves them; no phase work has started.

Exit criteria for Phase -1, for reference:

- All crates from AGENTS.md section 5 exist and build, each with the never-crash lint header.
- `cargo xtask ci` runs fmt, clippy, test, text, assets, layers, wasm, parity, and passes.
- CI is green on `main`. The FreeBSD and Windows ARM64 nightlies are green.
- A test run of `release.yml` (manual dispatch with a version override) produces artifacts for every platform.
- Branch protection is on for `main` and `release`.

## Phases and estimates

Hours are summed from every PR in the build plan. They assume one experienced Rust developer working with agent help, and include tests, docs, and review. Calendar weeks assume about 30 focused hours a week.

| Phase | Theme | Hours | Calendar | Plan |
|---|---|---|---|---|
| -1 | Scaffolding and CI/CD | 55-98 | 2-4 weeks | [01](../build_plan/01-phase-minus-1-scaffolding.md) |
| 0 | UI shell | 214-360 | 7-12 weeks | [02](../build_plan/02-phase-0-ui-shell.md) |
| 1 | Core engine | 444-756 | 15-26 weeks | [03](../build_plan/03-phase-1-core-engine.md) |
| 2 | Page links, search, note tags | 130-222 | 4-8 weeks | [04](../build_plan/04-phase-2-links-search-tags.md) |
| 3 | Polish and parity | 356-606 | 12-21 weeks | [05](../build_plan/05-phase-3-polish-parity.md) |
| Hardening | Beta to 1.0 | 80-160 | 3-6 weeks | [05](../build_plan/05-phase-3-polish-parity.md#beta-to-10-hardening) |
| To 1.0 | | 1,279-2,202 | 43-74 weeks | |
| 4 | Remaining parity (post-1.0) | 644-1,228 | 22-41 weeks | [06](../build_plan/06-phase-4-advanced.md) |
| Full parity | | 1,923-3,430 | 64-115 weeks | |

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
