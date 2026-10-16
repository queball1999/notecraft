# Gaps: the ranked work list

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-15. Change: duplicate rank 15 and 16 rows from a merge removed; page text read from disk when a page is shown (FM-01); PR 1.1 landed; rank 15 narrowed to the notebook index (ST-07); PR 1.10 landed; rank 21 narrowed to the highlighter, erasers, saving strokes, and real-pen checks; PR 1.3 landed; rank 16 shrunk to the minimal-diff write; PR 1.10 follow-ups (anti-aliased ink edges, marked current pen, WordCraft survey notes); Ink Replay scheduled in PR 3.2 (IN-13); also: PR 1.4 landed; rank 16 removed, the write is wired to saving under rank 18; rank 15 narrowed to reading the index in the navigation (PR 1.2 landed).

Every known shortfall against OneNote parity, ranked. Pick from the top. When an item lands, delete it here (or shrink it if only part landed), re-score its rows in [parity-checklist.md](parity-checklist.md), and add a line to the [ROADMAP.md](../ROADMAP.md) progress log.

Ranking rule: blockers first (things other work waits on), then alpha gate items, then by user impact per hour. Hour ranges match the build plan PRs. Optional features OneNote does not have are not gaps; they are in [post-parity.md](post-parity.md).

## Now (Phase 0: UI shell)

| Rank | Gap | Rows | Hours | Plan |
|---:|---|---|---|---|

## Next (Phase 1: core engine, rest of alpha gate)

| Rank | Gap | Rows | Hours | Plan |
|---:|---|---|---|---|
| 15 | The navigation does not read the index yet (the folder reader, reading page text from disk, and the index have landed) | ST-07 | 6-10 | build_plan/03, PR 1.2 (done), 1.5 |
| 17 | No text editing in containers | TX-01 to TX-06, CV-04 to CV-06 | 104-180 | build_plan/03, PR 1.5, 1.6 |
| 18 | No auto-save, crash recovery, outside edit handling (also wires the minimal-diff write, PR 1.4, into page saving; FM-02 and FM-04 stay P until then) | FM-05 to FM-07 (plus FM-02, FM-04) | 24-40 | build_plan/03, PR 1.7 |
| 19 | No undo and redo | ST-08 | 20-34 | build_plan/03, PR 1.8 |
| 20 | No `.notecraft` format | FM-03 | 30-50 | build_plan/03, PR 1.9 |
| 21 | Ink: pen, smoothing, anti-aliased meshes, a marked current pen, and the pressure path landed (PR 1.10); still missing the highlighter and both erasers (PR 1.11), saving strokes to the page file (PR 1.9, rank 20), pressure on macOS and Linux (needs a wrapper crate or newer winit), and trying a real pen or tablet on all three systems | IN-01 to IN-06 | 24-40 plus the checks | build_plan/03, PR 1.11 |
| 22 | No pictures or attachments | AT-01 to AT-05 | 40-70 | build_plan/03, PR 1.12 |
| 23 | No structure editing (create, rename, move) | ST-02 to ST-06 | 30-50 | build_plan/03, PR 1.13 |
| 24 | Parsers not fuzzed | FM-08 | 10-16 | build_plan/03, PR 1.14 |
| 25 | Hands-on checks deferred from Phase 0 (one pass, 2-4 hours, before the v0.1.0 alpha): launch the window and follow system dark mode and the OS UI font on macOS (SH-01, TH-06); follow system dark mode on Linux (TH-06; Linux launch was checked by the maintainer); shortcuts using Cmd on a Mac (SH-14); ribbon key tips on a physical keyboard (SH-05); Narrator on Windows and VoiceOver on macOS runs (SH-15); the real OS folder and file dialogs on Windows, macOS, and Linux (SH-12); also pinch zoom on a real touchpad (CV-02), typing into the font and size boxes by hand (SH-02), and drawing with a real pen or tablet on Windows, macOS, and Linux (IN-01 to IN-03: does pressure arrive, does smoothing feel right). Record each result in the row evidence. Note: the screen reader checks may be partly automated in Phase 1 (UI Automation, AX, and AT-SPI tree checks in CI, and VoiceOver through Guidepup); a note only, not a build plan item | SH-01, SH-05, SH-12, SH-14, SH-15, TH-06 (plus CV-02, SH-02 notes) | 2-4 | build_plan/03, PR 1.15 |

## Later

Phase 2 (page links, search, note tags), Phase 3 (polish and parity), and Phase 4 (remaining parity) items are listed in their build_plan files and in the checklist. They move up into this list when the phase before them is under way.

## File formats

| Gap | Phase | Hours |
|---|---|---|
| `.md` page files wired to the editor and auto-save (read and minimal-diff write landed in PR 1.3 and 1.4) | 1 | part of 24-40 (PR 1.7) |
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
| Pen pressure APIs differ per OS, winit exposes little, and `unsafe` is forbidden | Ink feels wrong | Safe wrapper crates per OS; fall back to velocity-based width. PR 1.10 built the fallback and read the winit source: only Windows reports pressure (build_plan/03 PR 1.10 notes); no real pen tried yet |
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
| 2026-10-14 | -1 | Rank 6 narrowed to verifying the window on macOS and Linux. |
| 2026-10-14 | 0 | Rank 7 (command registry) removed; rank 9 now covers binding buttons and the badge. |
| 2026-10-14 | 0 | Rank 8 narrowed to the four presets (PR 0.3 landed the token system); rank 6 adds verifying system dark mode on macOS and Linux. |
| 2026-10-14 | 0 | Rank 8 (theme presets) removed; rank 6 adds verifying the OS UI font on macOS and Linux. |
| 2026-10-14 | 0 | Rank 9 narrowed: the ribbon widget, icons, File backstage, and Home tab landed (PR 0.5a); the other tabs, collapse, and key tips remain (PR 0.5b). |
| 2026-10-14 | 0 | Rank 9 narrowed: all ribbon tabs, collapse, and key tips landed (PR 0.5b); a physical keyboard check and the SH-04 badge decision remain. |
| 2026-10-14 | 0 | Rank 10 (notebook list, section tabs, page list) removed (PR 0.6). |
| 2026-10-14 | 0 | Rank 11 narrowed: canvas pan, zoom, rule lines, page color, and paper size landed (PR 0.7); the dotted paper decision and a real pinch check remain. |
| 2026-10-14 | 0 | Rank 12 narrowed: start page, recent list, and open dialogs landed (PR 0.8); trying the OS dialogs in the running app remains. |
| 2026-10-14 | 0 | Rank 13 (status bar, full page view, shortcut map) removed (PR 0.9); rank 6 adds trying Cmd shortcuts on a Mac. |
| 2026-10-14 | 0 | Rank 14 narrowed: screen reader names, keyboard focus order, and focus rings landed (PR 0.10); Narrator and VoiceOver runs remain. |
| 2026-10-15 | 0 | Rank 9 updated: Ribbon Display Options and the simplified ribbon landed (PR 0.5c); Alt key tips seen with OS keyboard input; 0.5d to 0.5f added. |
| 2026-10-15 | 0 | Rank 9 updated: font and size boxes landed (PR 0.5d); font names in their own face remain. |
| 2026-10-15 | 0 | Rank 9 updated: color pickers and the Colors dialog landed (PR 0.5e); the duplicated RGBA fields on its Custom page noted. |
| 2026-10-15 | 0 | Rank 9: font names now show in their own face (PR 0.5d) and the Colors dialog shows one set of R, G, B, A fields (PR 0.5e); a hand check of typing in the boxes added. |
| 2026-10-15 | 0 | Rank 9 narrowed: style and tag galleries and the Help tab landed (PR 0.5f); ribbon parity follow-ups 0.5c to 0.5f are done. |
| 2026-10-15 | 0 | Rank 11 narrowed: dotted paper kept as an extra and built with the ruled margin line (CV-03 done); only the real pinch check remains. |
| 2026-10-15 | 0 | Phase 0 close (maintainer decisions, 2026-10-15): ranks 6, 9, 11, and 14 removed; their rows (SH-01, SH-05, SH-14, SH-15, TH-06) are scored D on the Windows evidence and the hands-on checks move to one new gap, rank 25, to do before the v0.1.0 alpha (2-4 hours, PR 1.15). SH-04 scored D with the per-button badge dropped for good. Dotted paper stays a kept extra. Rank 12 (OS dialogs, SH-12) also removed on the same decision: SH-12 is scored D on the harness tests and the real-dialog check joins rank 25; the Linux launch was checked by the maintainer. |
| 2026-10-15 | 1 | Rank 15 narrowed to the index (ST-07, 18-30 hours): the notebook folder reader landed (PR 1.1). |
| 2026-10-15 | 1 | Page text from disk landed (FM-01 done): a page shown from an opened folder is read through notecraft-storage (64 MiB cap, lenient UTF-8) and fed to the canvas. Duplicate rank 15 and 16 rows from a merge removed; rank 15 is the index only, rank 16 the minimal-diff write only. |
| 2026-10-15 | 1 | Rank 21 narrowed: pen, smoothing, stroke capture, tessellation, and the spatial index landed (PR 1.10); real-pen checks added to rank 25; pressure risk updated. |
| 2026-10-15 | 1 | Rank 16 shrunk: markdown read, frontmatter read, and flow-layout rendering landed (PR 1.3); only the minimal-diff write remains. |
| 2026-10-15 | 1 | Rank 16 shrunk: markdown read, frontmatter read, and flow-layout rendering landed (PR 1.3); only the minimal-diff write remains. Rank 15 now also carries FM-01 (reading page files into the canvas). |
| 2026-10-15 | 1 | Rank 21: anti-aliased stroke edges and the marked current pen landed; highlighter and eraser design recorded (docs/architecture.md section 10). |
| 2026-10-15 | 1 | Rank 16 removed: the minimal-diff markdown write landed (PR 1.4: span-replacement editor, canonical serializer, atomic page writes). FM-02 and FM-04 stay P until PR 1.7 saves pages from the editor; that is noted on rank 18. The duplicated rank 15 and 16 lines left by the merge were dropped. |
| 2026-10-15 | 1 | Rank 15 narrowed (PR 1.2, the notebook index): the index, its refresh, and rebuild landed; what remains is reading the index in the navigation and sending page text to the canvas. The duplicate rank 15 and 16 rows left by the earlier merges were removed. |
| 2026-10-15 | 1 | Page text from disk landed (FM-01 D); duplicate rank 15 and 16 rows removed. |
