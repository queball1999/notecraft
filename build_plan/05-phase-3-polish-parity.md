# Phase 3: polish and parity

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-15. Change: Ink Replay scheduled in PR 3.2 (maintainer decision, 2026-10-15); Phase 3 estimate 404-686 hours.

Goal: close the gap to OneNote for everyday use. Shapes, ink to shape, selection tools, table editing, spell check, trash and page versions, templates, print and PDF, HTML export, copy link to page, new window, Quick Access Toolbar, touch, localization plumbing, and performance. Ends with the v0.9.0 beta.

The full control channel, MCP server, full CLI, and plugins are not OneNote features and are not in this plan (docs/post-parity.md).

| | |
|---|---|
| Phase branch | `phase/3-polish-parity` |
| Hours | see "Estimate" |
| Checklist rows | IN-07 to IN-10, CV-07 to CV-12, TX-07, TX-12, ST-09 to ST-11, AT-06, LK-04, EX-01 to EX-04, SH-16, SH-21 |

## Entry criteria

- Phase 2 closed; v0.2.0 published.

Parallel tracks: PRs 3.1 to 3.5 (canvas and structure) and 3.6 to 3.8 (export) touch different crates and can run at the same time on separate branches.

## PR 3.1: shapes and ruler

| | |
|---|---|
| Branch | `feat/shapes` |
| Hours | 30-50 |
| Rows | IN-07, IN-09 |

Steps: Draw tab Shapes gallery with at least 15 presets (line, arrow, double arrow, rectangle, rounded rectangle, ellipse, triangle, right triangle, diamond, pentagon, hexagon, parallelogram, trapezoid, star, cloud, axes 2D); stroke color, width, fill; resize and rotate handles; hold Shift for constrained proportions; ruler overlay that ink snaps to, rotated with two fingers or the mouse wheel; Add Pen on the pen gallery (a pen is kind, color, and width; up to 24 saved through `craft-settings`, parsed leniently so a damaged settings file loses only the bad pens; docs/architecture.md section 10.6). Inside the estimate.

Commits:

1. `feat(canvas): add shape model and geometry for 16 presets`
2. `feat(ui): add shapes gallery to the draw tab`
3. `feat(canvas): resize and rotate shapes with handles`
4. `feat(canvas): add ruler that ink snaps to`
5. `test(canvas): shape geometry and hit testing`

PR title: `feat(canvas): add shapes and the ruler`.

## PR 3.2: ink to shape

| | |
|---|---|
| Branch | `feat/ink-to-shape` |
| Hours | 32-56 |
| Rows | IN-08, IN-13 |

Steps: classify a finished stroke as line, rectangle, ellipse, triangle, or arrow with a geometric recognizer (corner detection and fit error); replace the stroke with a shape when confidence passes a threshold; toggle on the Draw tab; undo restores the ink. Approach (docs/architecture.md section 10.5, from the related WordCraft app's `crates/engine/src/cmd/ink_shape.rs`): bounded, dependency-free fits in order line, arrow (shaft plus two barbs), ellipse (RMS of the normalized radius), then a polygon of up to 8 corners from an iterative Ramer-Douglas-Peucker pass with crowded and near-straight corners merged; strokes under a minimum size stay ink. Improve on it with rotated rectangles and ellipses and a double arrow. A port of its code needs an ATTRIBUTION.md row. Ink Replay (scheduled by the maintainer, 2026-10-15; docs/architecture.md section 10.6): Draw tab button replays the page's strokes in drawing order, each drawn up to a fraction of its length at about 400 units per second with a short gap between strokes; a transport bar with play, pause, rewind, and skip; replay is view-only and needs no format change, since stroke order in the canvas block is drawing order.

Commits:

1. `feat(canvas): recognize basic shapes from strokes`
2. `feat(ui): add ink to shape toggle`
3. `test(canvas): recognizer accuracy on sample strokes`
4. `feat(ui): replay ink in drawing order`

PR title: `feat(canvas): convert ink to shapes and replay ink`.

## PR 3.3: selection, z-order, snapping, insert space

| | |
|---|---|
| Branch | `feat/selection-tools` |
| Hours | 34-56 |
| Rows | CV-07 to CV-10 |

Commits:

1. `feat(canvas): add lasso and rectangle selection across object kinds` (a stroke is picked when at least 60 percent of its sampled points are inside the loop, as in WordCraft's lasso; docs/architecture.md section 10.4)
2. `feat(engine): move, delete, and recolor selections`
3. `feat(engine): add bring forward and send backward`
4. `feat(canvas): add grid snap and alignment guides`
5. `feat(canvas): add insert space tool`
6. `test(canvas): selection and z-order`

PR title: `feat(canvas): add selection tools, z-order, and snapping`.

## PR 3.4: table editing and spell check

| | |
|---|---|
| Branch | `feat/tables-spell` |
| Hours | 30-50 |
| Rows | TX-07, TX-12 |

Steps: Insert > Table grid picker; contextual Table tab (insert and delete rows and columns, header row, alignment); Tab moves between cells; writes GFM tables with column padding kept tidy. Spell check through the OS spell checker where available (Windows spell checking API needs `unsafe` bindings, so use a safe wrapper crate or a pure-Rust checker with an openly licensed dictionary; decide in the PR and record in docs/architecture.md).

Commits:

1. `feat(ui): add table insert picker`
2. `feat(engine): add table row and column commands`
3. `feat(ui): add contextual table tab`
4. `docs: record spell check approach`
5. `feat(ui): underline misspellings and suggest fixes`

PR title: `feat(ui): edit tables and check spelling`.

## PR 3.5: trash, version history, templates

| | |
|---|---|
| Branch | `feat/trash-history` |
| Hours | 30-50 |
| Rows | ST-09, ST-10, ST-11 |

Commits:

1. `feat(storage): keep page snapshots in notecraft history`
2. `feat(ui): add page versions pane with restore`
3. `feat(ui): add notebook recycle bin with restore and purge`
4. `feat(engine): add page templates from a templates section`

PR title: `feat(storage): add trash, page versions, and templates`.

## PR 3.6: PDF export engine

| | |
|---|---|
| Branch | `feat/pdf-export` |
| Hours | 40-70 |
| Rows | EX-02 |

Steps: follow docs/printing.md sections 3 and 4. Pure-Rust PDF writer (evaluate `krilla` and `printpdf` for license, font subsetting, and wasm builds; record the choice); text as real text with subset fonts; ink as variable-width paths; highlighter with blend mode; pagination with line-aware cuts; link annotations. Golden tests: text extraction and rasterized comparison.

Commits:

1. `docs: record pdf writer choice`
2. `feat(engine): lay out pages for pdf with line-aware cuts`
3. `feat(engine): write text, ink, shapes, and images to pdf`
4. `feat(engine): add pdf link annotations`
5. `feat(ui): add export to pdf`
6. `test(engine): golden pdf output`

PR title: `feat(engine): export pages and sections to PDF`.

## PR 3.7: printing through craft-print

| | |
|---|---|
| Branch | `feat/print` |
| Hours | 20-34 |
| Rows | EX-01 |

Steps: File > Print opens the `craft-ui` print settings dialog with the options in docs/printing.md section 2; preview from the PDF; send through `craft-print`. The Printing workflow runs on this PR.

Commits:

1. `build(deps): add craft-print`
2. `feat(app): print pages through craft-print`
3. `feat(ui): add print dialog with preview`
4. `test(app): print to file on every os`

PR title: `feat(app): print with preview`.

## PR 3.8: HTML export, notebook zip, PDF printouts

| | |
|---|---|
| Branch | `feat/html-zip-printout` |
| Hours | 40-70 |
| Rows | EX-03, EX-04, AT-06 |

Steps: self-contained HTML (inline CSS, images as data URIs, ink as inline SVG); notebook zip (the folder as is, minus `.notecraft/index.sqlite`); Insert > File Printout renders PDF pages to images on the canvas (evaluate a pure-Rust PDF rasterizer; if none is good enough, ship the printout as page-one preview only and keep the row P).

Commits:

1. `feat(engine): export pages to self-contained html`
2. `feat(engine): export a notebook as zip`
3. `docs: record pdf rasterizer choice`
4. `feat(canvas): insert pdf pages as a file printout`
5. `test(engine): html and zip export`

PR title: `feat(engine): add HTML export, notebook zip, and PDF printouts`.

## PR 3.9: URL scheme, multiple windows, Quick Access Toolbar

| | |
|---|---|
| Branch | `feat/url-windows-qat` |
| Hours | 24-40 |
| Rows | LK-04, CV-12, SH-16 |

Commits:

1. `feat(app): handle notecraft urls and copy link to page`
2. `build(packaging): register the notecraft url scheme in installers`
3. `feat(app): open a second window on the same session`
4. `feat(ui): add customizable quick access toolbar`

PR title: `feat(app): add notecraft links, new window, and the quick access toolbar`.

## PR 3.10: touch and palm rejection

| | |
|---|---|
| Branch | `feat/touch` |
| Hours | 20-36 |
| Rows | IN-10 |

Commits:

1. `feat(core): add touch gestures for pan and zoom`
2. `feat(canvas): ignore touch while a pen is in range`
3. `test(core): gesture recognition`

PR title: `feat(canvas): add touch input and palm rejection`.

## PR 3.11: localization plumbing

| | |
|---|---|
| Branch | `feat/i18n` |
| Hours | 30-50 |
| Rows | SH-21 |

Steps: string table in `locales/en-US.ftl` (Fluent, `fluent-bundle`), every UI string moved there, `cargo xtask text` exempts `locales/` (already in AGENTS.md section 2); a pseudo-locale for testing text expansion.

Commits:

1. `feat(ui): add fluent string table with en-US`
2. `refactor(ui): move all ui strings to the string table`
3. `feat(ui): add pseudo-locale for layout testing`

PR title: `feat(ui): move UI strings into a string table`.

## PR 3.12: performance

| | |
|---|---|
| Branch | `perf/targets` |
| Hours | 30-50 |
| Rows | CV-11 |

Steps: measure every SCRD section 11 target on a mid-range laptop per OS (cold start under 2 s, sidebar under 500 ms at 1,000 pages, 60 FPS at 500 objects, under 200 MB memory, MSI under 50 MB); fix the misses (mesh caching for ink, texture atlases, lazy page parsing); add criterion benches and record numbers in docs/architecture.md.

Commits:

1. `perf(canvas): cache tessellated ink meshes`
2. `perf(ui): reduce per-frame allocations in the canvas`
3. `perf: record performance targets and results`

PR title: `perf: meet the SCRD performance targets`.

## PR 3.14: command line render, export, print

| | |
|---|---|
| Branch | `feat/cli-render` off `phase/3-polish-parity` |
| Hours | 12-20 |
| Rows | none |

Steps: `notecraft-cli render <page> --format pdf|html|png [--out <file>]`, `notecraft-cli export <notebook> --format zip|html|pdf [--out <path>]`, and `notecraft-cli print <page> [--printer <name>] [--copies <n>]` (through craft-print) call the same engine paths as the File tab; no rendering code is duplicated. PNG output reuses the canvas painter's software path. Depends on PR 3.6, 3.7, and 3.8.

Commits:

1. `feat(cli): add render to pdf, html, and png`
2. `feat(cli): add export to zip, html, and pdf`
3. `feat(cli): add print through craft-print`

PR title: `feat(cli): add render, export, and print`.

This PR is part of the command line decision of 2026-10-15 (docs/cli.md). It can land at any point after PR 3.8 and before PR 3.13.

## PR 3.15: MCP server

| | |
|---|---|
| Branch | `feat/cli-mcp` off `phase/3-polish-parity` |
| Hours | 24-40 |
| Rows | none |

Steps: `notecraft-cli mcp [--notebook <path>] [--allow-destructive]` serves the tools, resources, and safety rules in docs/mcp.md over stdio, headless, on the same command registry and engine as the other subcommands. Destructive tools stay off unless `--allow-destructive` is given. Every tool call is bounded (input size, result size) and never panics. The bridge to a running app (`mcp --connect <port>`) stays post-parity because it needs the full control channel (docs/post-parity.md). Depends on PR 1.16, 2.11, and 3.14.

Commits:

1. `feat(mcp): add the stdio transport and tool dispatch`
2. `feat(mcp): expose commands, pages, and search as tools and resources`
3. `feat(mcp): enforce the safety rules and add protocol tests`

PR title: `feat(cli): add an MCP server over stdio`.

This PR is part of the command line decision of 2026-10-15 (docs/cli.md). It can land at any point after PR 3.14 and before PR 3.13.

## PR 3.13: close Phase 3 and ship the beta

Follow 00-conventions.md section 6; release v0.9.0.

## Beta to 1.0 hardening

Not new features. Between v0.9.0 and v1.0.0, work only from docs/gaps.md: beta bug reports, crash reports, parity rows still at P, accessibility fixes, docs completeness. Use `fix/` and `docs/` branches off `main` (no phase branch). Release candidates `1.0.0-rc.n` from the `release` branch until a candidate has had two weeks with no new P1 bugs.

| | |
|---|---|
| Hours | 80-160 |
| Ends with | v1.0.0 |

## Estimate

| PR | Hours |
|---|---|
| 3.1 shapes and ruler | 30-50 |
| 3.2 ink to shape, ink replay | 32-56 |
| 3.3 selection tools | 34-56 |
| 3.4 tables and spell check | 30-50 |
| 3.5 trash, history, templates | 30-50 |
| 3.6 PDF export | 40-70 |
| 3.7 print | 20-34 |
| 3.8 HTML, zip, printouts | 40-70 |
| 3.9 links, windows, QAT | 24-40 |
| 3.10 touch | 20-36 |
| 3.11 localization plumbing | 30-50 |
| 3.12 performance | 30-50 |
| 3.13 close and beta | 8-14 |
| 3.14 command line render, export, print | 12-20 |
| 3.15 MCP server | 24-40 |
| Total | 404-686 |

Beta to 1.0 hardening (section above) adds 80-160 hours outside the phase.

## Exit criteria

- Every row listed at the top scored D (AT-06 may stay P, see PR 3.8).
- The beta is followed by the hardening work above, ending in v1.0.0.
- Overall parity about 70% (docs/target-app-parity.md).
- v0.9.0 published.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Control channel, MCP, CLI, and plugins moved to docs/post-parity.md (not OneNote features). |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-15 | 0 | Added PR 3.14 (command line render, export, print) and PR 3.15 (MCP server) after the maintainer decision of 2026-10-15; Phase 3 estimate 392-666 hours. |
| 2026-10-15 | 0 | Re-estimated at the Phase 0 close: no actual Phase 0 hours were recorded and Phase 0 moved no scope into this phase, so the estimate is unchanged. |
| 2026-10-15 | 1 | Notes from the WordCraft ink survey: Add Pen in PR 3.1, the recognizer approach in PR 3.2, the lasso pick rule in PR 3.3. Hours unchanged. |
| 2026-10-15 | 1 | Ink Replay scheduled in PR 3.2 (maintainer decision, 2026-10-15), adding 12-20 hours and row IN-13; Phase 3 estimate 404-686 hours. Add Pen confirmed inside PR 3.1's estimate. |
