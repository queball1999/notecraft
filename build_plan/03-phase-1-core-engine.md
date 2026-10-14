# Phase 1: core engine

> Stage: pre-alpha (scaffolding). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: project renamed to NoteCraft; earlier changes are in the revision history.

Goal: real notebooks on disk. Open a folder, read and write `.md` and `.notecraft` pages with minimal diffs, type in note containers, draw ink, embed images and files, undo and redo, auto-save, and survive crashes and outside edits. Ends with the v0.1.0 alpha.

| | |
|---|---|
| Phase branch | `phase/1-core-engine` |
| Hours | see "Estimate" |
| Checklist rows | ST-01 to ST-08, FM-01 to FM-08, TX-01 to TX-06, CV-04 to CV-06, IN-01 to IN-06, AT-01 to AT-05 |
| Gaps | ranks 16 to 25 |
| Format spec | docs/markdown.md (read it first) |

## Entry criteria

- Phase 0 closed; 0.0.1 internal release built.
- docs/markdown.md reviewed and accepted by the maintainer (it is the contract for this phase).

## PR 1.1: notebook folder reader

| | |
|---|---|
| Branch | `feat/notebook-reader` |
| Hours | 16-28 |
| Rows | ST-01 |

Files: `crates/notecraft-storage/src/{notebook_dir.rs,section_toml.rs,notebook_toml.rs,paths.rs}`, `testdata/notebooks/basic/` (original sample notes), ATTRIBUTION.md row for `testdata/notebooks/`.

Steps:

1. Scan a folder into a tree per docs/markdown.md section 2: notebook, groups, sections, pages; read `.notecraft/notebook.toml` and `.section.toml` with lenient `toml` parsing (unknown keys kept, bad values defaulted).
2. Apply ordering rules (listed first, unlisted after by name).
3. Enforce limits: depth 8, file count warning at 50,000.
4. File name sanitizing helpers for all three OSes (section 2.3), with tests for reserved Windows names (`CON`, `NUL`, ...).
5. Replace the Phase 0 mock in the navigation with the real tree.

Commits:

1. `feat(storage): scan notebook folders into a section and page tree`
2. `feat(storage): read notebook.toml and .section.toml leniently`
3. `feat(storage): add cross-platform page file name rules`
4. `test(storage): add sample notebook and tree tests`
5. `feat(ui): load navigation from a real notebook folder`

PR title: `feat(storage): open a folder as a notebook`.

## PR 1.2: index.sqlite

| | |
|---|---|
| Branch | `feat/index` |
| Hours | 18-30 |
| Rows | ST-07 |

Steps: `rusqlite` with the `bundled` feature; schema from docs/markdown.md section 8 (pages, sections, attachments; links, tags, FTS come in Phase 2 but create the tables now); migrations with a `user_version` pragma; incremental refresh by mtime and hash; background worker thread; rebuild on corruption (delete and rescan, never crash). Benchmark: 1,000-page fixture lists in under 500 ms (criterion bench, recorded in the PR).

Commits:

1. `feat(storage): add index schema and migrations`
2. `feat(storage): refresh index incrementally from file changes`
3. `feat(storage): rebuild index on corruption`
4. `perf(storage): bench sidebar load for 1000 pages`

PR title: `feat(storage): add the rebuildable notebook index`.

## PR 1.3: markdown read

| | |
|---|---|
| Branch | `feat/markdown-read` |
| Hours | 20-34 |
| Rows | FM-01, FM-04 |

Steps: `notecraft-markdown` wraps `comrak` (GFM extensions on); parse into NoteCraft blocks (heading, paragraph, list, task list, table, code, quote, rule, html) that keep source byte spans; bounded YAML subset parser for frontmatter (scalars, lists, one level of maps; 64 KiB cap; unknown keys preserved as raw text); render blocks to the canvas in flow layout.

Commits:

1. `feat(markdown): parse markdown into blocks with source spans`
2. `feat(markdown): add bounded frontmatter parser`
3. `feat(ui): render markdown blocks on the canvas in flow layout`
4. `test(markdown): golden tests for gfm sample pages`

PR title: `feat(markdown): read and render markdown pages`.

## PR 1.4: minimal-diff markdown write

| | |
|---|---|
| Branch | `feat/markdown-write` |
| Hours | 40-70 |
| Rows | FM-02 |

Steps: edits are applied as span replacements on the original text, never by re-serializing the whole AST; untouched bytes (spacing, line endings, list markers, trailing whitespace) are preserved; a fresh page is serialized in a canonical style (ATX headings, `-` bullets, LF endings unless the file already uses CRLF); all writes through `craft-atomic`. Round-trip property test: parse then write with no edits gives identical bytes, for every file in `testdata/markdown/` (collect 50 or more varied, original sample files).

Commits:

1. `feat(markdown): apply edits as source span replacements`
2. `feat(markdown): serialize new blocks in canonical style`
3. `feat(storage): write pages through craft-atomic`
4. `test(markdown): byte-identical round trip on sample corpus`

PR title: `feat(markdown): write pages with minimal diffs`.

## PR 1.5: text editing in containers

| | |
|---|---|
| Branch | `feat/container-editing` |
| Hours | 80-140 |
| Rows | TX-01, CV-04, CV-05, CV-06 |

The biggest and riskiest PR of the phase. Split into 1.5a (editor widget) and 1.5b (containers on the canvas) if it passes 1,000 lines.

Steps:

1. Container model in `notecraft-model`: id, position, width, blocks.
2. Editor widget in `notecraft-ui-egui`: live-rendered editing (markdown styling shown as you type, syntax visible only on the active line), caret, selection, IME, clipboard text, word and line movement, Home and End.
3. Click on empty canvas creates a container at that point; drag by the top bar to move; drag the right edge to resize width; page extents grow.
4. Edits become engine commands (`text.insert`, `text.delete`, `container.new`, `container.move`, `container.resize`).

Commits:

1. `feat(model): add note containers`
2. `feat(ui): add live markdown text editor widget`
3. `feat(ui): support ime, selection, and keyboard movement`
4. `feat(canvas): create, move, and resize containers`
5. `feat(engine): add text and container commands`
6. `test(ui): cover typing, selection, and container moves`

PR title: `feat(canvas): type anywhere in note containers`.

## PR 1.6: formatting commands

| | |
|---|---|
| Branch | `feat/formatting` |
| Hours | 24-40 |
| Rows | TX-02 to TX-06 |

Steps: Home tab Basic Text and Styles buttons go live: bold, italic, strikethrough, headings 1 to 6, normal text, bulleted and numbered lists, indent and outdent (Tab and Shift+Tab), task list toggle (Ctrl+1), quote, code; tables render read-only (editing tools in Phase 3). Each is a command that rewrites the markdown span.

Commits:

1. `feat(engine): add inline formatting commands`
2. `feat(engine): add heading and style commands`
3. `feat(engine): add list, indent, and task commands`
4. `feat(ui): render gfm tables on the canvas`
5. `feat(ui): enable home tab formatting buttons`

PR title: `feat(engine): add text formatting commands`.

## PR 1.7: auto-save, crash recovery, outside edits

| | |
|---|---|
| Branch | `feat/autosave` |
| Hours | 24-40 |
| Rows | FM-05, FM-06, FM-07 |

Steps: dirty tracking per page; save every 30 seconds and on focus loss, page switch, and quit; recovery journal of unsaved edits in the settings folder, replayed on next launch with a prompt; `notify` watcher with 300 ms debounce; outside change with no local edits reloads, with local edits shows the conflict bar (docs/architecture.md section 3.3).

Commits:

1. `feat(storage): add dirty tracking and timed auto-save`
2. `feat(storage): add crash recovery journal`
3. `feat(storage): watch notebook folders for outside changes`
4. `feat(ui): add conflict bar for outside edits`
5. `test(storage): simulate crash and outside edit`

PR title: `feat(storage): auto-save and recover safely`.

## PR 1.8: undo and redo

| | |
|---|---|
| Branch | `feat/undo` |
| Hours | 20-34 |
| Rows | ST-08 |

Steps: transactions of reversible operations (docs/architecture.md section 5); per-page history (500 steps or 64 MiB); typing coalesces into word-sized steps; notebook-level transactions for moves; Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z; History tab shows the stack.

Commits:

1. `feat(engine): add transactions and undo stack`
2. `feat(engine): coalesce typing into word steps`
3. `feat(engine): add notebook-level transactions`
4. `test(engine): undo and redo every phase 1 command`

PR title: `feat(engine): add undo and redo`.

## PR 1.9: `.notecraft` pages

| | |
|---|---|
| Branch | `feat/notecraft-format` |
| Hours | 30-50 |
| Rows | FM-03 |

Steps: container markers and the canvas block per docs/markdown.md sections 3.3 to 3.4; promotion of `.md` to `.notecraft`; lenient canvas parsing with snapshot-on-failure; limits from section 9; `page.convert_to_markdown` command with warning.

Commits:

1. `feat(markdown): read and write container markers`
2. `feat(storage): read and write the notecraft-canvas block`
3. `feat(storage): promote md pages to notecraft on first canvas edit`
4. `feat(engine): add convert to markdown command`
5. `test(storage): hostile and truncated canvas blocks`

PR title: `feat(storage): add the .notecraft page format`.

## PR 1.10: ink engine and pen

| | |
|---|---|
| Branch | `feat/ink-pen` |
| Hours | 60-100 |
| Rows | IN-01, IN-02, IN-03 |

Steps: input normalization in `notecraft-core` (mouse, pen with pressure where the OS exposes it, touch); stroke capture at full rate; smoothing (one-euro filter or similar) in `notecraft-canvas`; variable-width tessellation to egui meshes; pen gallery (colors and widths) on the Draw tab; stroke storage in the canvas block; spatial index (grid or R-tree) for hit testing. Prototype pressure on Windows Ink, macOS, and Linux (libinput via winit) first and record what each OS gives.

Commits:

1. `feat(core): normalize mouse, pen, and touch input`
2. `feat(canvas): capture and smooth ink strokes`
3. `feat(canvas): tessellate variable-width strokes`
4. `feat(canvas): add spatial index for strokes`
5. `feat(ui): add pen gallery to the draw tab`
6. `test(canvas): stroke smoothing and tessellation`

PR title: `feat(canvas): draw with the pen`.

## PR 1.11: highlighter and erasers

| | |
|---|---|
| Branch | `feat/ink-erase` |
| Hours | 24-40 |
| Rows | IN-04, IN-05, IN-06 |

Commits:

1. `feat(canvas): add highlighter strokes with multiply blend`
2. `feat(canvas): add stroke eraser`
3. `feat(canvas): add point eraser that splits strokes`
4. `test(canvas): eraser hit testing and stroke splitting`

PR title: `feat(canvas): add highlighter and erasers`.

## PR 1.12: attachments

| | |
|---|---|
| Branch | `feat/attachments` |
| Hours | 40-70 |
| Rows | AT-01 to AT-05 |

Steps: copy into `attachments/` with hash prefix (docs/markdown.md section 2.4); images decoded with `image` (PNG, JPEG, WebP) and `resvg` (SVG), downscaled textures for zoomed-out views; file objects with an original generic icon per type family; drag and drop from the OS; paste images from the clipboard; open in default app (`open` crate or OS call in L7); resize images with handles; linked mode for files over the size limit.

Commits:

1. `feat(storage): copy attachments with hash-prefixed names`
2. `feat(canvas): place and resize images`
3. `feat(canvas): add file attachment objects with type icons`
4. `feat(ui): drop files and paste images onto the canvas`
5. `feat(app): open attachments in the default app`
6. `test(storage): attachment naming and linked files`

PR title: `feat(canvas): embed images and files`.

## PR 1.13: structure editing

| | |
|---|---|
| Branch | `feat/structure-editing` |
| Hours | 30-50 |
| Rows | ST-02 to ST-06 |

Steps: create, rename, delete (to trash folder, restore UI comes in Phase 3) for notebooks, groups, sections, pages; subpage level changes; drag to reorder and move between sections (moves files and attachments); section colors. All as undoable commands.

Commits:

1. `feat(engine): create, rename, and delete pages and sections`
2. `feat(engine): move pages and sections between folders`
3. `feat(ui): drag to reorder and move in navigation`
4. `feat(ui): set section and notebook colors`
5. `test(engine): structure commands with undo`

PR title: `feat(engine): edit notebook structure`.

## PR 1.14: fuzzing

| | |
|---|---|
| Branch | `test/fuzz-parsers` |
| Hours | 10-16 |
| Rows | FM-08 |

Steps: `cargo fuzz` targets for markdown blocks, frontmatter, canvas block, and `.section.toml`; seed corpora from testdata; run each for 30 minutes before merge; add a weekly scheduled workflow only if the related craft apps have one (otherwise document a manual run in CONTRIBUTING.md).

Commits:

1. `test(markdown): add fuzz targets for markdown and frontmatter`
2. `test(storage): add fuzz targets for canvas block and toml`

PR title: `test: fuzz every page parser`.

## PR 1.15: close Phase 1 and ship the alpha

Follow 00-conventions.md section 6, then release v0.1.0 (docs/releasing.md). Fill every Evidence cell in the docs/roadmap.md alpha gate table first; do not release with an empty cell.

## Estimate

| PR | Hours |
|---|---|
| 1.1 folder reader | 16-28 |
| 1.2 index | 18-30 |
| 1.3 markdown read | 20-34 |
| 1.4 minimal-diff write | 40-70 |
| 1.5 container editing | 80-140 |
| 1.6 formatting | 24-40 |
| 1.7 auto-save and recovery | 24-40 |
| 1.8 undo and redo | 20-34 |
| 1.9 `.notecraft` format | 30-50 |
| 1.10 ink pen | 60-100 |
| 1.11 highlighter and erasers | 24-40 |
| 1.12 attachments | 40-70 |
| 1.13 structure editing | 30-50 |
| 1.14 fuzzing | 10-16 |
| 1.15 close and alpha | 8-14 |
| Total | 444-756 |

## Exit criteria

- Every row listed at the top scored D.
- Alpha gate table complete with evidence.
- v0.1.0 published.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
