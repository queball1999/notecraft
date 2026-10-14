# Parity checklist

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: SH-03 and SH-04 scored P (command registry and Coming Soon toast).

Every OneNote feature NoteCraft must have, one row each, scored:

- D (done): works end to end, has a test, and is reachable as a command.
- P (partial): some of it works, or it works without a test; the Evidence column says what is missing.
- M (missing): not started, or only a "Coming Soon" badge.

Only features OneNote has are listed. Optional features OneNote does not have are in [post-parity.md](post-parity.md) and get rows only when someone picks them up.

`cargo xtask parity` regenerates the summary block from the rows below; `cargo xtask parity --check` (part of `cargo xtask ci`) fails if the block is stale. Do not edit the summary by hand.

Row format, which the xtask parses: `| ID | Area | Feature | Phase | Score | Evidence |`. Ids are permanent once committed. Unused ids: SH-10, TX-08, AT-07 (their features moved to post-parity.md or were merged into LK-08 before the first commit).

<!-- parity:summary:start -->
| Area | D | P | M | Total | Done % |
|---|---:|---:|---:|---:|---:|
| Scaffolding | 10 | 0 | 0 | 10 | 100% |
| Shell | 0 | 3 | 17 | 20 | 0% |
| Themes | 0 | 0 | 6 | 6 | 0% |
| Canvas | 0 | 0 | 12 | 12 | 0% |
| Ink | 0 | 0 | 12 | 12 | 0% |
| Text | 0 | 0 | 12 | 12 | 0% |
| Structure | 0 | 0 | 12 | 12 | 0% |
| Attachments | 0 | 0 | 8 | 8 | 0% |
| Format | 0 | 0 | 9 | 9 | 0% |
| Links | 0 | 0 | 8 | 8 | 0% |
| Export | 0 | 0 | 4 | 4 | 0% |
| Total | 10 | 3 | 100 | 113 | 8% |
<!-- parity:summary:end -->

## Scaffolding (Phase -1)

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| SC-01 | Scaffolding | Cargo workspace with all crates and `xtask` | -1 | D | PR -1.1: workspace, 7 crates, 2 apps, xtask; build, clippy, test pass |
| SC-02 | Scaffolding | `cargo xtask ci` runs every gate | -1 | D | PR -1.4: cargo xtask ci runs fmt, clippy, test, text, assets, layers, wasm, parity |
| SC-03 | Scaffolding | `cargo xtask layers` | -1 | D | PR -1.3: cargo xtask layers; unit tests plus a failing upward-dependency check |
| SC-04 | Scaffolding | `cargo xtask assets` | -1 | D | PR -1.3: cargo xtask assets; unit tests plus a failing unattributed-file check |
| SC-05 | Scaffolding | `cargo xtask text` | -1 | D | PR -1.3: cargo xtask text; unit tests plus a failing em dash check; CI shell step kept |
| SC-06 | Scaffolding | `cargo xtask parity` and `--check` | -1 | D | PR -1.4: cargo xtask parity and --check; also checks build plan hour totals |
| SC-07 | Scaffolding | `cargo xtask version set` (release contract) | -1 | D | PR -1.5: cargo xtask version set; validates semver and refreshes Cargo.lock for --locked builds |
| SC-08 | Scaffolding | Packaging scripts and the release workflow | -1 | D | PR -1.5 and PR #1: packaging scripts plus NoteCraft's own release workflow; dry run 0.0.0-test.2 built 20 artifacts on free runners |
| SC-09 | Scaffolding | Logging to file with rotation and 16 MiB cap | -1 | D | PR -1.6: stderr and file logging, rotation to .1 and .2, 16 MiB cap, RUST_LOG, NOTECRAFT_NO_PREFS; unit tests |
| SC-10 | Scaffolding | Branch protection and labels on GitHub | -1 | D | Phase -1 close: main and release protection, phase/* ruleset, labels, Discussions, release environment |

## Shell

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| SH-01 | Shell | Window opens on wgpu (DX12, Metal, Vulkan) | 0 | P | PR 0.1: window opens on wgpu (Windows DX12 verified; macOS and Linux launches to verify); geometry saved through craft-settings |
| SH-02 | Shell | Ribbon with File, Home, Insert, Draw, History, Review, View tabs | 0 | M | |
| SH-03 | Shell | Every ribbon button bound to a command id | 0 | P | PR 0.2: command registry with ids, labels, argument schemas, and shortcuts for all 118 Phase 0 ribbon and shortcut commands (Coming Soon); ids unit-tested (unique, pattern, labels); buttons bind to them in PR 0.5 |
| SH-04 | Shell | "Coming Soon" badge and toast for unimplemented commands | 0 | P | PR 0.2: Registry::run returns a coming_soon error and AppShell::dispatch shows "Coming Soon: <label>" for 3 s (egui_kittest test); the badge on ribbon buttons lands in PR 0.5 |
| SH-05 | Shell | Collapsible ribbon (Ctrl+F1) and key tips | 0 | M | |
| SH-06 | Shell | Navigation: notebook list | 0 | M | |
| SH-07 | Shell | Section tabs with colors | 0 | M | |
| SH-08 | Shell | Page list with subpage indent | 0 | M | |
| SH-09 | Shell | Status bar (zoom, page info) | 0 | M | |
| SH-11 | Shell | File backstage with recent notebooks and pages | 0 | M | |
| SH-12 | Shell | Open dialog for notebooks, `.md`, and `.notecraft` | 0 | M | |
| SH-13 | Shell | Full page view (F11) | 0 | M | |
| SH-14 | Shell | OneNote default shortcut map | 0 | M | |
| SH-15 | Shell | Keyboard navigation and screen reader labels (AccessKit) | 0 | M | |
| SH-16 | Shell | Quick Access Toolbar | 3 | M | |
| SH-17 | Shell | Docked window (Ctrl+Alt+D) | 4 | M | |
| SH-18 | Shell | Browser version | 4 | M | |
| SH-19 | Shell | Translated UI (first locales) | 4 | M | |
| SH-20 | Shell | Right-to-left layout | 4 | M | |
| SH-21 | Shell | All UI strings in a string table (en-US) | 3 | M | |

## Themes

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| TH-01 | Themes | OneNote 2016 preset, light and dark | 0 | M | |
| TH-02 | Themes | OneNote 2019 preset, light and dark | 0 | M | |
| TH-03 | Themes | OneNote 2021 preset, light and dark | 0 | M | |
| TH-04 | Themes | Microsoft 365 preset, light and dark | 0 | M | |
| TH-05 | Themes | Switch theme and mode live, persisted in settings | 0 | M | |
| TH-06 | Themes | Follow system dark mode | 0 | M | |

## Canvas

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| CV-01 | Canvas | Pan (scroll, middle drag, space drag) | 0 | M | |
| CV-02 | Canvas | Zoom 10% to 400%, Ctrl+wheel, pinch | 0 | M | |
| CV-03 | Canvas | Rule lines and grid (blank, lined, grid, dotted) | 0 | M | |
| CV-04 | Canvas | Click anywhere to create a note container | 1 | M | |
| CV-05 | Canvas | Move and resize containers | 1 | M | |
| CV-06 | Canvas | Page grows with content | 1 | M | |
| CV-07 | Canvas | Lasso and rectangle select | 3 | M | |
| CV-08 | Canvas | Z-order commands | 3 | M | |
| CV-09 | Canvas | Snap to grid and alignment guides | 3 | M | |
| CV-10 | Canvas | Insert Space tool | 3 | M | |
| CV-11 | Canvas | 60 FPS with 500 objects at 100% zoom | 3 | M | |
| CV-12 | Canvas | New window on the same notebook | 3 | M | |

## Ink

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| IN-01 | Ink | Pen with color and width | 1 | M | |
| IN-02 | Ink | Pressure from pen tablets | 1 | M | |
| IN-03 | Ink | Stroke smoothing | 1 | M | |
| IN-04 | Ink | Highlighter | 1 | M | |
| IN-05 | Ink | Stroke eraser | 1 | M | |
| IN-06 | Ink | Point eraser | 1 | M | |
| IN-07 | Ink | Shapes gallery (15 or more) | 3 | M | |
| IN-08 | Ink | Ink to shape | 3 | M | |
| IN-09 | Ink | Ruler | 3 | M | |
| IN-10 | Ink | Touch input and palm rejection | 3 | M | |
| IN-11 | Ink | Ink to text (handwriting recognition) | 4 | M | |
| IN-12 | Ink | Search handwritten ink | 4 | M | |

## Text

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| TX-01 | Text | Type plain text in a container | 1 | M | |
| TX-02 | Text | Bold, italic, strikethrough | 1 | M | |
| TX-03 | Text | Headings and styles | 1 | M | |
| TX-04 | Text | Bulleted and numbered lists, indent | 1 | M | |
| TX-05 | Text | To Do checkboxes (task lists) | 1 | M | |
| TX-06 | Text | Tables (render) | 1 | M | |
| TX-07 | Text | Table editing tools | 3 | M | |
| TX-09 | Text | Equations | 2 | M | |
| TX-10 | Text | Text highlight color | 2 | M | |
| TX-11 | Text | Find on page | 2 | M | |
| TX-12 | Text | Spell check | 3 | M | |
| TX-13 | Text | Insert date and time stamps | 2 | M | |

## Structure

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| ST-01 | Structure | Open a folder as a notebook | 1 | M | |
| ST-02 | Structure | Create, rename, delete notebooks | 1 | M | |
| ST-03 | Structure | Create, rename, delete, color sections | 1 | M | |
| ST-04 | Structure | Section groups | 1 | M | |
| ST-05 | Structure | Create, rename, delete pages; subpages | 1 | M | |
| ST-06 | Structure | Drag to reorder and move pages and sections | 1 | M | |
| ST-07 | Structure | Navigation loads 1,000 pages in under 500 ms | 1 | M | |
| ST-08 | Structure | Undo and redo | 1 | M | |
| ST-09 | Structure | Notebook Recycle Bin with restore | 3 | M | |
| ST-10 | Structure | Page versions | 3 | M | |
| ST-11 | Structure | Page templates | 3 | M | |
| ST-12 | Structure | Password-protected sections | 4 | M | |

## Attachments

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| AT-01 | Attachments | Insert pictures (PNG, JPEG, WebP, SVG) | 1 | M | |
| AT-02 | Attachments | Insert any file as an attachment icon | 1 | M | |
| AT-03 | Attachments | Drag and drop onto the page | 1 | M | |
| AT-04 | Attachments | Paste picture from clipboard | 1 | M | |
| AT-05 | Attachments | Open attachment in default app | 1 | M | |
| AT-06 | Attachments | File printout (PDF pages as images) | 3 | M | |
| AT-08 | Attachments | Audio and video recording | 4 | M | |
| AT-09 | Attachments | Copy text from picture (OCR) | 4 | M | |

## Format

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| FM-01 | Format | Read `.md` pages | 1 | M | |
| FM-02 | Format | Write `.md` pages with minimal diffs | 1 | M | |
| FM-03 | Format | Read and write `.notecraft` pages (containers, canvas block) | 1 | M | |
| FM-04 | Format | Frontmatter read and write | 1 | M | |
| FM-05 | Format | Auto-save every 30 s and on focus loss | 1 | M | |
| FM-06 | Format | Crash recovery of unsaved edits | 1 | M | |
| FM-07 | Format | Outside edit detection and conflict bar | 1 | M | |
| FM-08 | Format | Fuzzed parsers (no panic on any input) | 1 | M | |
| FM-09 | Format | Import OneNote `.one` sections | 4 | M | |

## Links

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| LK-01 | Links | Page links typed as `[[Page]]` | 2 | M | |
| LK-02 | Links | Link resolution, page picker, create missing page | 2 | M | |
| LK-03 | Links | Links update on rename and move | 2 | M | |
| LK-04 | Links | Copy link to page (`notecraft://`) | 3 | M | |
| LK-05 | Links | Search with scopes (page to all notebooks) | 2 | M | |
| LK-06 | Links | Note tags with icons, gallery, shortcuts | 2 | M | |
| LK-07 | Links | Find Tags summary pane and summary page | 2 | M | |
| LK-08 | Links | Search attachments by name and type | 2 | M | |

## Export

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| EX-01 | Export | Print with preview (craft-print) | 3 | M | |
| EX-02 | Export | Export page or section to PDF | 3 | M | |
| EX-03 | Export | Export to single-file HTML | 3 | M | |
| EX-04 | Export | Export notebook as zip | 3 | M | |

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Re-scoped the same day to OneNote features only: graph, automation, and plugin rows removed (docs/post-parity.md); Phase 4 parity rows added. 113 rows, all M. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | SC-01 scored D (workspace and crate skeletons). |
| 2026-10-14 | -1 | SC-02 scored P (cargo xtask ci with fmt, clippy, test). |
| 2026-10-14 | -1 | SC-03, SC-04, SC-05 scored D (text, assets, layers gates). |
| 2026-10-14 | -1 | SC-02 and SC-06 scored D; summary now generated by cargo xtask parity. |
| 2026-10-14 | -1 | SC-07 scored D and SC-08 scored P (release contract). |
| 2026-10-14 | -1 | SC-09 scored D (logging). |
| 2026-10-14 | -1 | SC-08 and SC-10 scored D; Phase -1 closed. |
| 2026-10-14 | -1 | SH-01 scored P (window opens; Windows verified). |
| 2026-10-14 | 0 | SH-03 and SH-04 scored P (command registry and Coming Soon toast). |
