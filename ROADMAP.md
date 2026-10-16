# NoteCraft roadmap

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-15. Change: Phase 1 PR 1.1 landed (notebook folder reader); progress log updated; PR 1.10 (ink engine and pen) logged; PR 1.3 landed (markdown read, frontmatter read, flow-layout rendering); progress log updated. Target: OneNote 2020 era and Microsoft 365 (canvas and ink), on a markdown-first file format that is easy to edit in any text editor; page text read from disk when a page is shown (FM-01 scored D).


NoteCraft is a free, open-source, clean-room note-taking app that remakes OneNote one to one (freeform canvas, ink, ribbon, notebook structure, themes), backed by a markdown-first, folder-based file format that is easy to manage and edit in any other editor. Built in pure Rust with egui.

This file is the stage summary and progress log. The detailed assessments live in `docs/`:

| File | Holds |
|---|---|
| [docs/target-app-parity.md](docs/target-app-parity.md) | Parity assessment vs OneNote (canvas, ink, themes, structure) |
| [docs/markdown.md](docs/markdown.md) | The markdown-first file format and page links |
| [docs/gaps.md](docs/gaps.md) | Every known shortfall, ranked: the work list. Pick from the top |
| [docs/roadmap.md](docs/roadmap.md) | Current focus, the alpha plan, milestones |
| [docs/parity-checklist.md](docs/parity-checklist.md) | Feature rows scored D/P/M; `cargo xtask parity` regenerates its summary |
| [docs/architecture.md](docs/architecture.md) | How NoteCraft is built (layers, data flow, quality gates) |
| [docs/craft-libs.md](docs/craft-libs.md) | Which shared craft-libs crates NoteCraft uses and how |
| [docs/printing.md](docs/printing.md) | Print and PDF strategy (craft-print, which wraps winprint) |
| [docs/themes.md](docs/themes.md) | The four OneNote theme presets |
| [docs/releasing.md](docs/releasing.md) | Release, signing, notarization, packaging |
| [docs/post-parity.md](docs/post-parity.md) | Optional features OneNote does not have; considered only after full parity |
| [docs/mcp.md](docs/mcp.md), [docs/control-protocol.md](docs/control-protocol.md), [docs/cli.md](docs/cli.md) | Automation specs (CLI and MCP server: build plan; control channel: mostly post-parity) |

The execution plan for agents (phases, branches, commit messages, PRs) is in [`build_plan/`](build_plan/README.md). It covers OneNote parity only.

## By dimension (estimated, pre-alpha)

Hours are summed bottom-up from every PR in `build_plan/` (each PR carries a generous range that includes tests, docs, and review). "To 1.0" covers Phases -1 to 3 plus beta hardening. "Phase 4" is the rest of full OneNote parity.

| Dimension | Ready | To 1.0 | Phase 4 | Full parity | Doc |
|---|---:|---|---|---|---|
| Canvas and freeform layout | 0% | 90-152 | 0 | 90-152 | [parity](docs/target-app-parity.md#1-canvas-and-freeform-layout) |
| Ink and drawing | 0% | 154-262 | 140-256 | 294-518 | [parity](docs/target-app-parity.md#2-ink-and-drawing) |
| Text and markdown editing | 0% | 156-268 | 0 | 156-268 | [markdown](docs/markdown.md) |
| Notebook structure | 0% | 118-198 | 40-80 | 158-278 | [parity](docs/target-app-parity.md#4-notebook-structure) |
| Ribbon, chrome, accessibility | 0% | 192-318 | 16-28 | 208-346 | [parity](docs/target-app-parity.md#5-ribbon-and-chrome) |
| Themes (4 presets x 2 modes) | 0% | 40-68 | 0 | 40-68 | [themes](docs/themes.md) |
| Attachments and media | 0% | 52-91 | 100-180 | 152-271 | [parity](docs/target-app-parity.md#7-attachments-and-media) |
| Page links, search, note tags | 0% | 92-154 | 0 | 92-154 | [markdown](docs/markdown.md#5-page-links) |
| Print and export | 0% | 88-153 | 0 | 88-153 | [printing](docs/printing.md) |
| File formats (.md, .notecraft, .one import) | 0% | 104-176 | 160-320 | 264-496 | [gaps](docs/gaps.md#file-formats) |
| Command registry, undo/redo, command line | 0% | 88-148 | 0 | 88-148 | [architecture](docs/architecture.md) |
| Localization | 0% | 30-50 | 80-160 | 110-210 | [gaps](docs/gaps.md#localization) |
| Performance and 1.0 hardening | 0% | 110-210 | 0 | 110-210 | [roadmap](docs/roadmap.md) |
| Browser build and sync research | 0% | 0 | 100-190 | 100-190 | [Phase 4](build_plan/06-phase-4-advanced.md) |
| Scaffolding, CI, release | 100% | 87-156 | 8-14 | 95-170 | [Phase -1](build_plan/01-phase-minus-1-scaffolding.md) |
| Total | 0% | 1,401-2,404 | 644-1,228 | 2,045-3,632 | |

Calendar, at about 30 focused hours a week: v1.0 in 47-80 weeks; full parity in 68-121 weeks.

**Overall: 0% ready for users.** Phase -1 is closed: the workspace, the eight `cargo xtask ci` gates, packaging, and logging exist, and CI runs on GitHub. Every feature is "Coming Soon" until Phase 0 (UI shell) and Phase 1 (core engine) land.

## By phase

| Phase | Theme | Hours | Ends with |
|---|---|---|---|
| -1 | Scaffolding and CI/CD | 55-98 | CI green, release pipeline proven |
| 0 | UI shell | 284-474 | internal 0.0.1 |
| 1 | Core engine | 456-776 | v0.1.0 alpha |
| 2 | Page links, search, note tags | 134-230 | v0.2.0 |
| 3 | Polish and parity | 392-666 | v0.9.0 beta |
| Hardening | Beta to 1.0 | 80-160 | v1.0.0 |
| 4 | Remaining parity | 644-1,228 | full OneNote parity (1.x) |

## Alpha gate (v0.1.0)

The workflows that must work end to end for v0.1.0. Full table with per-row evidence: [docs/roadmap.md](docs/roadmap.md#alpha-gate) (mirrors [SCRD.md section 5](SCRD.md)).

## How estimates are maintained

- Each PR in `build_plan/` has an hour range; phase totals are the sum of their PRs, and the tables above are the sum of the phases. `cargo xtask parity --check` (part of `cargo xtask ci`) fails if a phase total or a By phase row drifts.
- At each phase close, replace estimates for finished work with actual hours and re-estimate the remaining phases.

## Progress log

| Date | What landed |
|---|---|
| 2026-10-13 | Docs set started to the craftrules progress-docs standard; craft-libs and winprint (via craft-print) integration decided. No code yet. |
| 2026-10-14 | Docs set completed (13 docs plus post-parity.md), `build_plan/` written with six phase files, root files and licenses added, CI/CD aligned with the related craft apps (shared craft-common-actions workflows), plain-ASCII US English rule added, scope narrowed to OneNote parity, and all hours re-assessed bottom-up. No code yet. |
| 2026-10-14 | Project renamed from QNote 2.0 to NoteCraft: crates, binaries, the `.notecraft` page format, the notebook data folder, URL scheme, env vars, app id, and repo name. Docs now state NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | Phase -1 PR -1.1: Cargo workspace with shared never-crash lints, crate skeletons for L0 to L6, notecraft and notecraft-cli apps, placeholder xtask. |
| 2026-10-14 | Phase -1 PR -1.2: cargo xtask ci runs fmt, clippy -D warnings, and test, with a step summary. |
| 2026-10-14 | Phase -1 PR -1.3: cargo xtask text, assets, and layers gates, run by cargo xtask ci. |
| 2026-10-14 | Phase -1 PR -1.4: cargo xtask wasm (L0 to L5 on wasm32) and cargo xtask parity (checklist summary and build plan hour totals); cargo xtask ci now runs all 8 gates. |
| 2026-10-14 | Phase -1 PR -1.5: release contract for craft-common-actions: cargo xtask version, ico, and icns; packaging scripts adapted from GridCraft with new MSI GUIDs and NoteCraft file types; original app icon; build SHA and date embedded in both binaries. |
| 2026-10-14 | App ID changed from ai.storyteller.notecraft to io.github.queball1999.notecraft and the repo moved to github.com/Queball1999/notecraft; NoteCraft no longer uses the ArtCraft namespace or links. |
| 2026-10-14 | Phase -1 PR -1.6: desktop app logging (stderr plus rotated, size-capped notecraft.log; NOTECRAFT_NO_PREFS), --version on both binaries, build SHA refreshed when git HEAD moves. |
| 2026-10-14 | Release workflow is now NoteCraft's own (free standard runners only; the shared one used a paid macOS runner and has no license). |
| 2026-10-14 | Phase -1 closed. Repo on GitHub (queball1999/notecraft); CI, Printing, FreeBSD, and packaging lint green; release dry run built 20 artifacts on free runners; branch protection, phase ruleset, labels, Discussions, and the release environment set up. Phase 0 (UI shell) is next. |
| 2026-10-14 | Phase 0 PR 0.1: the main window opens on wgpu with an empty app shell (ribbon, notebooks, pages, canvas, status regions); window geometry saved with craft-settings; craft-quit close handling; egui 0.36 and craft-libs pinned at 457a0788. |
| 2026-10-14 | Phase 0 PR 0.2: command registry in notecraft-engine (118 Coming Soon ids for every Phase 0 ribbon and shortcut command, argument checks, panic guard that leaves the session unchanged) and the "Coming Soon: <label>" toast (PR #6). |
| 2026-10-14 | Phase 0 PR 0.3: theme token system (tokens, egui style mapping through craft-ui, WCAG AA contrast check), Ready theme commands (view.theme.set, view.mode.toggle, view.mode.set), theme saved with craft-settings, and system dark mode followed live (PR #7). |
| 2026-10-14 | Phase 0 PR 0.4: the four theme presets (OneNote 2016, 2019, 2021, Microsoft 365) in light and dark, checked against docs/themes.md and WCAG AA; two dark accents tuned for contrast; OS UI font loaded with egui's built-in fonts as the fallback (no bundled font) (PR #8). |
| 2026-10-14 | Phase 0 PR 0.5a: data-driven ribbon (spec table, craft-ui buttons and tabs, Coming Soon in tooltips; the per-button badge was dropped), original vector icons drawn in code, the File backstage, and the Home tab; every button bound to a registered command (PR #9). |
| 2026-10-14 | Phase 0 PR 0.5b: Insert, Draw, History, Review, and View tabs with original icons (every OneNote ribbon tab now present); ribbon collapse (Ctrl+F1, chevron, double-click, saved across runs) and key tips; the View tab's Theme and Dark Mode buttons switch the theme live (PR #10). |
| 2026-10-14 | Phase 0 PR 0.6: notebook pane, colored section tabs, and page list on mock notebooks (two notebooks, five sections, a section group, twenty pages); nav.notebook.select, nav.section.select, and nav.page.select; the page list follows the preset's side (PR #11). |
| 2026-10-14 | Ribbon fixes: buttons sit on the ribbon background in every preset; color choices (font, highlight, ink, page) are real pickers with a swatch grid and a full custom color picker; Insert Table is a size grid; pen, highlighter, and width menus show previews (PR #12). |
| 2026-10-14 | Phase 0 PR 0.7: the page canvas pans (wheel, scroll bars, middle-drag, Space+drag) and zooms 10% to 400% (ribbon, Ctrl+wheel, pinch, all through view.zoom); rule lines and grids, page color, and paper size are Ready and drawn for the visible area only; the page title, date, and time area is drawn on mock data; Viewport in notecraft-canvas with property tests (PR #13). |
| 2026-10-14 | Phase 0 PR 0.8: start page while nothing is open (New Notebook, Open Notebook, Open File, recent list); File backstage New and Open pages; file.new_notebook, file.open_notebook, file.open, and file.recent.clear Ready; OS open dialogs through rfd in the app; the last 10 notebooks and pages saved in config.toml, and the newest reopened at launch (PR #14). |
| 2026-10-14 | Phase 0 PR 0.9: status bar (Page n of m, zoom menu); Full Page View (F11, Esc, or the Normal View button to leave); OneNote's default shortcut map in the engine (Ctrl+2 to Ctrl+9 note tags included, Ctrl+E bound to a Coming Soon search.all) dispatched by the shell, with Ctrl as Cmd on macOS; egui's own Ctrl+= UI scaling turned off (PR #15). |
| 2026-10-14 | Phase 0 PR 0.10: every actionable control has a screen reader name (the panel splitters were the only gap); Tab moves through the ribbon, navigation, section tabs, page, and status bar in that order; every control, the canvas included, shows an accent focus ring; tests guard both. Narrator and VoiceOver runs are still to do (PR #16). |
| 2026-10-15 | Phase 0 PR 0.10 follow-up, checked in the running app through Windows UI Automation: ribbon and other icon buttons are push buttons (craft-ui's IconButton reported every one as a toggle, so Narrator heard "toggle button, off" and UIA offered no Invoke); notebook, section, and page rows are list items with a selection state; the panel splitters draw a focus ring; the canvas scroll bars are named Horizontal and Vertical (PR #16). |
| 2026-10-15 | Phase 0 PR 0.5c: Ribbon Display Options on the ribbon chevron, as in OneNote: Classic or Simplified Ribbon, and Full-screen mode, Show tabs only, or Always show Ribbon (view.ribbon.layout, view.ribbon.show, both saved). The simplified ribbon is one row of icon buttons with a More Options menu; full-screen mode shows the ribbon while the pointer is at the top of the window. The canvas now reports only its visible part to screen readers. |
| 2026-10-15 | Phase 0 PR 0.5d: Font and Font Size are editable boxes, as in OneNote: they show the current value (Calibri, 11), the font box lists the installed families from a background scan that reads only each font's name table, and typing filters it; the size box takes any size from 1 to 1638. |
| 2026-10-15 | Phase 0 PR 0.5e: color menus in Office's layout: Automatic or No Color, Theme Colors, Standard Colors, Recent Colors (saved in settings), and More Colors, which opens a Colors dialog with Standard and Custom pages and New over Current. Split color buttons show the last color as the bar under their icon and apply it; Text Highlight Color offers fifteen colors with No Color last. |
| 2026-10-15 | Open at launch: `notecraft <notebook folder or page file>` opens it instead of the last recent item (`--help` added; a bad path toasts and falls back); documented in docs/cli.md. |
| 2026-10-15 | Scope change (maintainer decision): the command line works like the related craft apps' (WordCraft's wordcraft-cli has convert, info, text, inspect, render, run, commands, mcp), an exception to the OneNote-only rule for the CLI and its MCP server only. The Full CLI and MCP server rows leave docs/post-parity.md; PR 1.16, 2.11, 3.14, and 3.15 added (56-94 hours). Phase 0 PR 0.13 landed `notecraft-cli commands [--json]` and `notecraft-cli run [--sample] --cmd 'id={json}'`. Totals: 1,399-2,400 to 1.0 and 2,043-3,628 to full parity. |
| 2026-10-15 | Phase 0 PR 0.11 landed on feat/control-minimal: `notecraft --control <port>` serves a minimal JSON-RPC control channel on 127.0.0.1 (`auth` with a random token in `control.token`, `command.run` through the shell's own dispatcher, `screenshot`, `wait.idle`); tests drive it over TCP headlessly, and an ignored real-window test saves a screenshot of every theme. |
| 2026-10-15 | Phase 0 PR 0.5f: Styles and Tags are in-ribbon galleries, as in OneNote: a bordered box with two entries drawn in their own style (Heading 1 and Heading 2; To Do and Important with their icons and Ctrl+number shortcuts), arrows that scroll two entries at a time, and a More arrow for the full list; the simplified ribbon shows each as one dropdown. The Help tab (Help, Support, Feedback, Show Training) follows View; its commands are Coming Soon. |
| 2026-10-15 | Phase 0: the File backstage About page is real (feat/about-page): version, commit, build date, profile, target, app ID, copyright, MIT and Apache texts, third-party notices, links, settings and log folders, a Copy button for bug reports (file.copy_about_info), and the Microsoft trademark notice. |
| 2026-10-15 | Phase 0 PR 0.7 follow-up: dotted paper (a kept extra, `view.rule_lines` style `dotted`) and the thin red margin line on ruled paper (theme token `canvas_margin_rule`); CV-03 scored D. |
| 2026-10-15 | Phase 0 PR 0.7 follow-up: the View > Rule Lines dropdown is a gallery of thumbnail cards, as in OneNote: each card is a miniature page drawn by the same paper code as the canvas (rules, margin line, dots; current theme), grouped under Ruled Lines and Grid Lines, with the current style highlighted; every card is keyboard reachable and named for screen readers. |
| 2026-10-15 | Phase 0 closed (PR 0.12, docs/phase-0-close): every Phase 0 PR (#6 to #25) is merged into phase/0-ui-shell; SH-01, SH-04, SH-05, SH-12, SH-15, and TH-06 scored D (maintainer decisions, 2026-10-15; the maintainer also ran NoteCraft on Linux and it works like Windows); parity is 33 of 113 rows done (29%); alpha gate evidence filled for the Phase 0 rows; the hands-on checks agents cannot run (macOS, Linux dark mode, physical keyboard, Narrator and VoiceOver, Cmd on a Mac, the real OS file dialogs) are one gap to do before v0.1.0 (docs/gaps.md rank 25, 2-4 hours, added to PR 1.15). No actual hours were recorded, so Phases 1 to 4 keep their estimates; Phase 1 is 456-776 hours, 1,401-2,404 to 1.0, 2,045-3,632 to full parity. Phase 1 (core engine) is next. |
| 2026-10-15 | Phase 1 PR 1.1: notecraft-storage scans a notebook folder into groups, sections, and pages (notebook.toml and .section.toml read leniently, listed order first, depth 8, warning at 50,000 files), with cross-platform file name rules; file.open_notebook shows the real tree in the navigation instead of the mock; original sample notebook in testdata/notebooks/basic; ST-01 scored D. |
| 2026-10-15 | Phase 1 PR 1.10 (feat/ink-pen): draw with the pen. Pointer samples normalized in notecraft-core; one-euro smoothing and speed-based width when a device reports no pressure; variable-width stroke meshes and a grid index for hit testing in notecraft-canvas; draw.type, draw.pen, draw.color, draw.thickness, and ink.add_stroke Ready; strokes drawn on the canvas and kept in the session (saving them is PR 1.9). IN-03 scored D, IN-01 and IN-02 P; no real pen was tried, and from the winit source only Windows reports pen pressure. |
| 2026-10-15 | Phase 1 PR 1.3 (feat/markdown-read): notecraft-markdown wraps comrak and parses pages into blocks with exact source spans, a bounded frontmatter reader, and the canvas renders blocks in flow layout (FM-01 and FM-04 scored P). |
| 2026-10-15 | Phase 1 page text from disk (feat/page-text-from-disk): notecraft-storage reads one page file (64 MiB cap, lossy UTF-8, truncated and lossy flags); Session::load_current_page reads the shown .md page of an opened folder after every command and calls set_page_source; a cut page is read-only and an unreadable page shows a short error line on the canvas. FM-01 scored D. Gaps ranks 15 and 16 deduplicated. |
