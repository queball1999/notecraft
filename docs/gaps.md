# Gaps: the ranked work list

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-18. Last updated: 2026-10-18. Change: rank 20 removed (FM-03 scored D: pictures and files in the canvas block; shapes belong to IN-07 in Phase 3), rank 22 narrowed to the real-OS checks; rank 21 shrunk, the textured pens and the pencil landed (IN-01 scored D); earlier: rank 1 added (startup crash on Linux X11 with only the software renderer; wgpu messages now reach the log); PR 1.15 close-out: rank 19 removed (ST-08 scored D); ranks 17, 21, 22, and 25 point at the hands-on checklist in docs/roadmap.md; highlighter true multiply, Always Create Pages with Rule Lines, and Eyedropper page colors landed (rank 21 narrowed); PR 1.9 landed (.notecraft pages and saving ink; one save path with auto-save); PR 1.7: rank 18 removed (auto-save, crash recovery, outside edits landed); duplicate rank 15 and 16 rows from a merge removed; page text read from disk when a page is shown (FM-01); PR 1.1 landed; rank 15 narrowed to the notebook index (ST-07); PR 1.10 landed; rank 21 narrowed to the highlighter, erasers, saving strokes, and real-pen checks; PR 1.3 landed; rank 16 shrunk to the minimal-diff write; rank 15 narrowed to reading the index in the navigation (PR 1.2 landed); rank 16 shrunk to the minimal-diff write. PR 1.8 landed; rank 19 narrowed to the callers of undo; also: PR 1.4 landed; rank 16 removed, the write is wired to saving under rank 18; PR 1.10 follow-ups (anti-aliased ink edges, marked current pen, WordCraft survey notes); Ink Replay scheduled in PR 3.2 (IN-13); PR 1.11 follow-ups (eraser sizes, grazing point eraser); PR 1.11 landed; rank 21 narrowed to saving strokes, true multiply, and real-pen checks; rank 19 no longer waits for the eraser; rank 23 removed: structure editing landed (PR 1.13); rank 19 narrowed; Ink Replay scheduled in PR 3.2 (IN-13). PR 1.5: text editing in containers; PR 1.14: rank 24 removed (fuzz targets landed; the 30-minute runs are noted in CONTRIBUTING.md). PR 1.6: text formatting (bold, italic, strikethrough, underline, sub and superscript, headings, lists, indent, tasks, quote, code, alignment as commands; TX-02 to TX-06 scored D); Draw tab parity (feat/draw-tab-parity): OneNote Draw groups, editable pen list saved in settings, pen menu with palette and Eyedropper, Format Background, smooth curves for ink, Rule Lines thumbnail margin fix; PR 1.12 landed (pictures and attachments, AT-01 to AT-05 scored P); rank 22 narrowed: SVG pictures, picture resize handles, and Ctrl+V images landed; container positions saved in the canvas block, broken canvas block repair, warning bar and Convert button (PR 1.9 follow-ups); ST-07: the navigation reads the notebook index, frontmatter titles show, and a background thread keeps the index current (feat/nav-from-index).

Every known shortfall against OneNote parity, ranked. Pick from the top. When an item lands, delete it here (or shrink it if only part landed), re-score its rows in [parity-checklist.md](parity-checklist.md), and add a line to the [ROADMAP.md](../ROADMAP.md) progress log.

Ranking rule: blockers first (things other work waits on), then alpha gate items, then by user impact per hour. Hour ranges match the build plan PRs. Optional features OneNote does not have are not gaps; they are in [post-parity.md](post-parity.md).

## Now (Phase 0: UI shell)

| Rank | Gap | Rows | Hours | Plan |
|---:|---|---|---|---|
| 1 | Startup crash on Linux X11 when only the software renderer (llvmpipe) is available. Seen in the Flatpak on Ubuntu 24.04, Xorg session, NVIDIA RTX 2070 SUPER, driver 580.178.04, before the matching `org.freedesktop.Platform.GL.nvidia-580-178-04` extension was installed. Reproduce on any Linux X11 box with `flatpak run --nodevice=dri io.github.queball1999.notecraft`; 0.0.0 and 0.0.1 crash the same way, and with the GPU driver present 0.0.1 runs fine. Cause: while wgpu sets up the Vulkan surface, Mesa's X11 code sends a request that fails with `BadDrawable` (request 149, minor 4, most likely Present QueryCapabilities). winit records that error and reports it at its next check, the `XMapRaised` in `set_visible` when eframe first shows the window (winit-0.30.13 `x11/window.rs:1145`), and panics there; a poisoned mutex then panics in a destructor and the process aborts. The panic is inside winit, not our code. `WGPU_BACKEND=gl` with `--nodevice=dri` fails with `FailedToCreateSurfaceForAnyBackend({})`: the GL backend never started, reason unknown, because wgpu messages were not logged. Done: `tracing-log` feature on `tracing-subscriber` (Cargo.toml), so wgpu and naga messages now reach stderr and `notecraft.log`. Next: (a) build a bundle with that change and run `flatpak run --nodevice=dri --env=WGPU_BACKEND=gl --env=RUST_LOG=info,wgpu_hal=debug,wgpu_core=info io.github.queball1999.notecraft` to see why GL did not start (`RUST_LOG` replaces the default filter, so keep the leading `info,`); (b) if GL can work, on Linux X11 fall back to the GL backend when the only Vulkan adapter is `DeviceType::Cpu`; (c) check winit master for a fix and file a winit issue if there is none; (d) add a Linux troubleshooting entry: on an NVIDIA machine where the log shows `adapter=llvmpipe`, run `flatpak update` to get the matching GL extension. Not the cause: the `bwrap: setting up uid map: Permission denied` from plain `/usr/bin/bwrap` (Ubuntu AppArmor; Flatpak has its own profile), the zbus `ServiceUnknown` warning, and our finish-args (the installer lists `fallback-x11` as `x11`) | SH-01 | 4-10 | build_plan/02, PR 0.1 |

## Next (Phase 1: core engine, rest of alpha gate)

| Rank | Gap | Rows | Hours | Plan |
|---:|---|---|---|---|
| 17 | Text editing and formatting in containers landed (PR 1.5: caret, selection, IME, clipboard, new, move, and resize; PR 1.6: bold, italic, strikethrough, underline, sub and superscript, highlight, color, size, family, headings, lists, indent, tasks, quote, code, alignment, tables drawn read-only); container positions are saved in the canvas block (PR 1.9 follow-up); still missing a bold face, and hand testing with a real keyboard, IME, and mouse on all three systems | TX-01, CV-05 (both P; CV-04 and CV-06 scored D) | the checks (hands-on checklist steps 4 to 6 in docs/roadmap.md), 4-8 | build_plan/03, PR 1.15 |
| 21 | Ink: pen, smoothing, anti-aliased meshes, the pressure path (PR 1.10), the highlighter, and the stroke and point erasers (PR 1.11) landed; still missing pressure on macOS and Linux (needs a wrapper crate or newer winit), and trying a real pen or tablet on all three systems; Draw tab leftovers: the textured pens (rainbow, galaxy, lava, ocean, and four metals) and the pencil landed (procedural textures, a `texture` key on the stroke) and need a hands-on look on all three systems; the highlighter multiply (an egui_wgpu paint callback, with the opaque pre-blend as the fallback), Always Create Pages with Rule Lines, and the Eyedropper reading page colors landed, and the multiply still needs a hands-on look on all three systems (including an sRGB surface, which keeps the fallback) | IN-01 to IN-06, CV-03 | the checks (hands-on checklist step 9 in docs/roadmap.md) | build_plan/03, PR 1.11 |
| 22 | Pictures and attachments landed (PR 1.12; SVG pictures, resize handles, Ctrl+V for images, and placing, dragging, and saving them at a place of their own in the canvas block followed); still missing a real-OS check of open, drag and drop, and clipboard paste (Ctrl+V of an image is detected from the key release because the window layer swallows the press); the hands-on checks are steps 7 and 8 of the checklist in docs/roadmap.md | AT-01 to AT-05 | 8-14 | build_plan/03, PR 1.12 (done), 1.7 |
| 25 | Hands-on checks deferred from Phase 0 (one pass, 2-4 hours, before the v0.0.2 alpha): now step 12 to 19 of the hands-on checklist in docs/roadmap.md, which also lists the Phase 1 checks (drag gestures, IME, real pen, attachments); launch the window and follow system dark mode and the OS UI font on macOS (SH-01, TH-06); follow system dark mode on Linux (TH-06; Linux launch was checked by the maintainer); shortcuts using Cmd on a Mac (SH-14); ribbon key tips on a physical keyboard (SH-05); Narrator on Windows and VoiceOver on macOS runs (SH-15); the real OS folder and file dialogs on Windows, macOS, and Linux (SH-12); also pinch zoom on a real touchpad (CV-02), typing into the font and size boxes by hand (SH-02), and drawing with a real pen or tablet on Windows, macOS, and Linux (IN-01 to IN-03: does pressure arrive, does smoothing feel right). Record each result in the row evidence. Note: the screen reader checks may be partly automated in Phase 1 (UI Automation, AX, and AT-SPI tree checks in CI, and VoiceOver through Guidepup); a note only, not a build plan item | SH-01, SH-05, SH-12, SH-14, SH-15, TH-06 (plus CV-02, SH-02 notes) | 2-4 | build_plan/03, PR 1.15 |

## Later

Phase 2 (page links, search, note tags), Phase 3 (polish and parity), and Phase 4 (remaining parity) items are listed in their build_plan files and in the checklist. They move up into this list when the phase before them is under way.

## File formats

| Gap | Phase | Hours |
|---|---|---|
| `.md` page files wired to the editor (auto-save, recovery, and outside edits landed in PR 1.7; typing into containers is PR 1.5) | 1 | part of 104-180 (PR 1.5) |
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
| 2026-10-15 | 1 | Rank 16 shrunk: markdown read, frontmatter read, and flow-layout rendering landed (PR 1.3); only the minimal-diff write remains. Rank 15 now also carries FM-01 (reading page files into the canvas). |
| 2026-10-15 | 1 | Rank 15 narrowed (PR 1.2, the notebook index): the index, its refresh, and rebuild landed; what remains is reading the index in the navigation and sending page text to the canvas. The duplicate rank 15 and 16 rows left by the earlier merges were removed. |
| 2026-10-15 | 1 | Page text from disk landed (FM-01 D); duplicate rank 15 and 16 rows removed. |
| 2026-10-15 | 1 | Rank 16 shrunk: markdown read, frontmatter read, and flow-layout rendering landed (PR 1.3); only the minimal-diff write remains. |
| 2026-10-15 | 1 | Rank 19 narrowed: undo and redo landed (PR 1.8) for ink and page setup, with typing, merge, and notebook-level steps built against the engine API; text, eraser, and structure callers remain (PR 1.5, 1.11, 1.13). |
| 2026-10-15 | 1 | Rank 21: anti-aliased stroke edges and the marked current pen landed; highlighter and eraser design recorded (docs/architecture.md section 10). |
| 2026-10-15 | 1 | Rank 16 removed: the minimal-diff markdown write landed (PR 1.4: span-replacement editor, canonical serializer, atomic page writes). FM-02 and FM-04 stay P until PR 1.7 saves pages from the editor; that is noted on rank 18. The duplicated rank 15 and 16 lines left by the merge were dropped. |
| 2026-10-16 | 1 | Rank 18 removed: auto-save, the recovery journal, and outside edit handling landed (PR 1.7). FM-05 to FM-07, FM-02, and FM-04 are D. Still open there: strokes are not written to the page file until PR 1.9 (rank 20), and nothing calls `mark_page_dirty` from typing until PR 1.5 (rank 17), though text edits made through the undo API already mark pages. |
| 2026-10-16 | 1 | PR 1.9: rank 20 narrowed: the canvas block (ink), container markers, md promotion, and convert to markdown landed; positions for other objects, UI wiring, remain (auto-save writes ink: PR 1.7 merged). Rank 21 no longer lists saving strokes. |
| 2026-10-16 | 1 | Rank 21 narrowed: the highlighter and both erasers landed (PR 1.11); rank 19 no longer waits for the eraser. |
| 2026-10-16 | 1 | PR 1.11 follow-ups: Small, Medium, Large point erasers; the point eraser cuts wherever its circle touches the outline; highlighter multiply gap now says what OneNote does. |
| 2026-10-16 | 1 | Rank 23 removed: create, rename, move, color, and delete for notebooks, groups, sections, and pages landed (PR 1.13; ST-02 P, ST-03 to ST-05 D, ST-06 P). Rank 19 narrowed: structure edits are undoable. |
| 2026-10-16 | 1 | Rank 17 narrowed: text editing in containers landed (PR 1.5); formatting (PR 1.6), saving (PR 1.7, 1.9), and hand tests remain. Rank 19 loses its text-edit caller. |
| 2026-10-16 | 1 | PR 1.14: rank 24 removed: fuzz targets and bounded property tests landed for every page parser (FM-08 P). The 30-minute run of each target is a manual step in CONTRIBUTING.md. |
| 2026-10-16 | 1 | Rank 17 narrowed again: text formatting landed (PR 1.6; TX-02 to TX-06 done); saving positions and hand tests remain. |
| 2026-10-16 | 1 | Rank 21 narrowed (Draw tab parity): the pen list, pen menu, Eyedropper, Format Background, and smooth curves landed; textured pens and Always Create Pages with Rule Lines recorded. |
| 2026-10-16 | 1 | PR 1.12 landed: rank 22 narrowed to SVG, resize handles, real-OS checks, Ctrl+V images, and the page write. |
| 2026-10-17 | 1 | Rank 21 narrowed: the highlighter multiplies over the page through an egui_wgpu paint callback (rule lines show through, crossing strokes darken); Always Create Pages with Rule Lines and Eyedropper page colors landed. Textured pens, pen pressure off Windows, and real-pen checks remain. |
| 2026-10-17 | 1 | Rank 22 narrowed: SVG pictures (resvg), resize handles (picture.resize, width kept as a comment after the image), and Ctrl+V of an image landed; real-OS checks and click-point placement remain. |
| 2026-10-17 | 1 | Ranks 17 and 20 narrowed: container positions are saved in the canvas block, a block that did not parse can be repaired, and the UI shows the warning bar and the Convert button. Rank 20 keeps images, files, and shapes in the block. |
| 2026-10-17 | 1 | Rank 15 removed: the navigation reads the notebook index (ST-07 scored D). |
| 2026-10-17 | 1 | Rank 1 added: startup crash on Linux X11 with only the software renderer (llvmpipe); the `tracing-log` feature now forwards wgpu and naga messages to the log. |
| 2026-10-18 | 1 | PR 1.15 close-out: rank 19 removed (ST-08 scored D, nothing left to build); rank 17 now lists CV-05 and TX-01 only (CV-04 and CV-06 scored D); ranks 21, 22, and 25 point at the hands-on checklist in docs/roadmap.md. |
| 2026-10-18 | 1 | Rank 21 shrunk: the textured pens (rainbow, galaxy, lava, ocean, gold, silver, bronze, rose gold) and the pencil are built; only the hands-on look at them (checklist step 9) remains, about 16-30 hours of build work removed. |
| 2026-10-18 | 1 | Rank 20 removed and the `.notecraft` read and write line under File formats removed: pictures and file attachments are placed, dragged, and saved in the canvas block (FM-03 scored D); shapes are tracked by IN-07. Rank 22 narrowed to the hands-on checks. |
