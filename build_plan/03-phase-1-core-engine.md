# Phase 1: core engine

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-16. Change: PR 1.13 done: deviations and open questions noted under PR 1.13; PR 1.1 landed; deviations noted under PR 1.1; PR 1.10 landed on feat/ink-pen; deviations and the per-OS pressure notes added; PR 1.3 done; deviations noted under PR 1.3; page text read from disk when a page is shown (FM-01 scored D); also: PR 1.4 done; deviations noted under PR 1.4; PR 1.10 follow-ups (anti-aliased ink edges, marked current pen, WordCraft survey notes); Ink Replay scheduled in PR 3.2 (IN-13).

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

Landed 2026-10-15 with these deviations from the plan:

- The storage crate (L1) sits below the model crate (L2), so `notecraft-storage` returns its own tree types (`NotebookTree`, `TreeNode`, `SectionEntry`, `PageEntry`) and the engine converts them to model types in the new `notecraft-engine::open` module. Extra files: `notebook_dir.rs` also holds the ordering and limits; the plan's four files all exist.
- The fifth commit also holds the engine side (`Session::open_notebook`, `file.open_notebook` scanning the folder), because the navigation reads the session, not the storage crate.
- `file.open_notebook` with a path that does not exist keeps the Phase 0 behavior (record it, show the mock notebooks if nothing is open), so stale recent entries and the existing harness tests with made-up paths still work. A path that exists but is not a readable folder is `invalid_args`. `file.new_notebook` and `file.open` still show the mock; creating notebook files is PR 1.13.
- Ids: a section or group id is `<notebook id>/<folder path>`, a page id is `<notebook id>/<file path>`; the notebook id is `id` from `notebook.toml`, else `dir-` plus a hash of the folder path. `notebook.toml` is only read, never created, in this PR.
- Extra limits beyond the plan: the scan stops at 500,000 entries (warning), settings files over 1 MiB are not read, and `sections` and `pages` lists keep at most 100,000 entries. Symbolic links are not followed. Hidden names and `attachments` folders are skipped.
- A page title is the file name without its extension until PR 1.3 reads frontmatter titles.
- A final `docs:` commit carries the progress doc updates (build_plan/00-conventions.md section 5).

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

Done (2026-10-15), with these deviations from the steps above:

- The index is native only: `rusqlite` sits under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` and the `index` module is cfg-ed out on wasm32, so `cargo xtask wasm` is unchanged.
- Freshness test is modified time plus size, then a content hash (FNV-1a, 64 bits) only for pages that differ; a touched file with the same bytes is not reported as changed. A same-size edit inside one millisecond of the old modified time would be missed; the next change catches it.
- Titles come from the file name until the Phase 2 reparse fills them from frontmatter (storage is L1 and cannot depend on `notecraft-markdown`). `refresh` returns the added, changed, and removed page ids for that reparse.
- The page `ord` column orders the sidebar; `sections.ord` and `pages.ord` are rewritten each refresh, so reordering never needs a rebuild. Attachments are listed to depth 3 under `attachments/` with a mime guess from the extension; the hash column stays null until PR 1.12.
- Corruption handling: `Index::open_or_rebuild` runs `PRAGMA quick_check(1)` and deletes the file and its `-wal`, `-shm`, and `-journal` siblings when open, migration, or the check fails (including a newer `user_version`). `IndexWorker` retries once from scratch if the refresh itself hits a database error.
- `IndexWorker` is a plain thread with an mpsc channel (`try_result`, `wait`). The engine does not call it yet: the navigation still shows the tree from `scan_notebook`. Wiring is gaps rank 15; ST-07 is scored P.
- Bench (criterion, `cargo bench -p notecraft-storage --bench sidebar`, 1,000 pages in 20 sections): open and list from the index about 27 ms; a refresh with nothing changed about 1 s and a cold build about 30 s on the development machine, a remote session on a nearly full disk where each file open took roughly 30 ms. The cold build is limited by per-file I/O, not SQLite, and runs in the background.
- `criterion` (MIT OR Apache-2.0, `default-features = false`) and `rusqlite` (bundled SQLite) are recorded in ATTRIBUTION.md.

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

Done (2026-10-15), with these deviations from the steps above:

- PR 1.1 was built in parallel, so the canvas does not read files. The seam is `Session::set_page_source(page_id, text)` (also `AppShell::set_page_source`); the mock Welcome page carries a sample. FM-01 was scored P until the follow-up "page text from disk" change (branch `feat/page-text-from-disk`) landed: `notecraft-storage::read_page_file` (bounded at 64 MiB, lossy UTF-8) and `Session::load_current_page`, which the command registry calls after every command, read the shown `.md` page of a notebook opened with `file.open_notebook` and call `set_page_source`. A page cut at the cap is read-only; an unreadable page gets a short error line on the canvas (`Session::current_page_notice`). Reads are synchronous until PR 1.7 (auto-save and outside edits). FM-01 is now D.
- Block model for PR 1.4: `Block { span, kind }` with `Span { start, end }` as page byte offsets (end exclusive). Lists keep one `ListItem` span per item and tables one span per cell. The comrak positions needed two fixes (last list item reached past its line; an indented code block started after its indent), covered by the golden tests. Inline runs carry no spans, so PR 1.4 edits at block, item, and cell level.
- Task lists are a `ListKind::Task` list whose items carry `task: Some(checked)`, not a separate block type.
- The frontmatter reader is hand-written (no YAML crate). A value outside the subset (block scalars, deeper maps, flow maps) becomes `Value::Raw`, and every entry keeps its source span so PR 1.4 can rewrite single keys.
- Bold is drawn by overstriking, because egui ships no bold face; a real bold face can replace it with the font work in Phase 3.
- The canvas also sets the page text as the accessibility value of the canvas node (screen readers read it; harness tests use it).
- `comrak` is BSD-2-Clause, with `default-features = false`; it builds for wasm32 (checked by `cargo xtask wasm`). Recorded in ATTRIBUTION.md.

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

Done (2026-10-15), with these deviations from the steps above:

- The write side is an `Editor` over the original text. Callers hand it spans from the parsed `Document` (block, list item, table cell, frontmatter entry) and new markdown; `finish()` splices and returns the page. A replacement that equals the bytes already there is dropped, so putting back a span's own text is a no-op even in a file with mixed line endings. New text takes the file's dominant line ending (LF on a tie).
- Edit granularity matches the PR 1.3 block model: block, list item, table cell, and top-level frontmatter key. Inline runs have no spans, so text editing in containers (PR 1.5) replaces the whole block.
- `canonical.rs` serializes a `Block` tree (and a whole fresh page with frontmatter) in the canonical style: ATX headings, `-` bullets, `1.` numbering, `*`/`**`/`~~` emphasis, backtick fences that grow past backticks in the code, and escapes for text that would read as another block. Existing text is never re-serialized through it.
- `notecraft-storage::page_file` is the minimal save path: `read_page_file` (PR 1.1 page text read: lossy UTF-8, 64 MiB cap) beside `write_page_text` (craft-atomic, skips the write when the bytes are unchanged). L1 does not depend on L3, so it moves text, not blocks. The engine-facing save path, auto-save, and the conflict check stay in PR 1.7.
- The corpus is `testdata/markdown/corpus/` (73 original pages: CRLF, BOM, lone CR, mixed endings, unusual list markers, setext headings, frontmatter shapes, hostile input, and seeded generated pages), kept byte-exact by a `-text` rule in `.gitattributes`. The property tests also run each page with CRLF endings and use a small seeded generator instead of a new crate.
- `craft-atomic` was added to the workspace dependencies at the pinned craft-libs commit.

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

Steps: transactions of reversible operations (docs/architecture.md section 5); per-page history (500 steps or 64 MiB); typing coalesces into word-sized steps; a transaction can be marked to merge into the previous one (the stroke eraser uses it so one drag is one undo step; maintainer decision, 2026-10-15); notebook-level transactions for moves; Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z; History tab shows the stack.

Commits:

1. `feat(engine): add transactions and undo stack`
2. `feat(engine): coalesce typing into word steps`
3. `feat(engine): add notebook-level transactions`
4. `test(engine): undo and redo every phase 1 command`

PR title: `feat(engine): add undo and redo`.

### PR 1.8 notes: deviations and open questions

Built on `feat/undo` (2026-10-15), on top of `feat/markdown-read`. What differs from the steps above:

- The design is in docs/architecture.md section 5. Transactions are `notecraft_engine::undo::Transaction` (scope, label, merge mode, ops); `Session::commit`, `undo`, `redo`, `can_undo`, and `can_redo` are the API. Ops so far: `InsertStroke`, `DeleteStroke`, `ReplaceText`, `SetPageStyle`, `MovePage`. Later PRs add ops (PR 1.13 will want create, rename, and delete).
- Merge into the previous step (maintainer decision, 2026-10-15): `Transaction::merge_into_previous()`. The stroke eraser in PR 1.11 builds its first erase as an ordinary step and each further stroke of the same drag with this flag. Tested directly (`a_merged_transaction_undoes_with_the_previous_one` and neighbors). A merge never crosses stacks, and after an undo it joins the step that is then on top.
- Typing coalescing is built and tested against `Session::edit_page_text(page, at, remove_len, inserted, typing)`, not against a command, because container text editing is PR 1.5. That PR calls it from its commands. The rule: a word and the space after it are one step; Backspace or Delete runs coalesce; a run caps at 512 bytes; a moved caret, another kind of step, or an undo starts a new step. There is no clock: the plan did not ask for a pause timeout, and a pure rule is easier to test. Add one in PR 1.5 if hand testing wants it.
- Notebook-level steps are `Scope::Notebook` transactions with `MovePage` and any other ops. `Session::move_page` is the move API for PR 1.13; it clamps the index and records one step. Rename with link updates (Phase 2) is a notebook transaction of one move plus `ReplaceText` ops, tested as `a_cross_page_command_undoes_as_one_step`.
- Which stack Undo uses: the newest step among the shown page's stack and the notebook stack. A notebook step is therefore undoable from any page, and page and notebook steps undo in the order they happened.
- The command guard still restores a snapshot of the session, and the snapshot now includes the history (entries are `Arc`s, so cloning it copies pointers). A panic or error after a command recorded a step restores the stacks too (`a_rolled_back_command_leaves_the_history_as_it_was`). Replacing the snapshot with transaction rollback is possible now that every change is an op, but is not needed yet; docs/architecture.md section 6 says when to revisit it.
- `view.rule_lines`, `view.page_color`, and `view.paper_size` were marked undoable in Phase 0 but did not record anything; they now record `SetPageStyle`. `InkLayer::insert` (notecraft-canvas) puts a stroke back at its paint-order index.
- The test "undo and redo every Phase 1 command" is a table (`UNDO_CASES`): every Ready, undoable command must have a row, and each row is checked to undo, redo, and chain exactly. A later PR that makes a command Ready and undoable fails the test until it adds a row.
- Shortcuts: Ctrl+Z and Ctrl+Y are the commands' `default_shortcut`; Ctrl+Shift+Z is a second binding (`shortcuts::ALIASES`). Matching tries more modifiers first, so Ctrl+Shift+Z does not fire Undo.
- UI: Phase 0 had no Undo or Redo buttons anywhere (the Quick Access Toolbar, SH-16, is Phase 3). Undo and Redo icon buttons now sit at the right of the tab strip, disabled while their stack is empty. They are not the customizable toolbar.
- History tab: not built. NoteCraft's History tab has OneNote's contents (Next Unread, Mark as Read, Show Unread, Find by Author, Hide Authors, Page Versions, Notebook Recycle Bin, Recent Edits), and OneNote shows no undo list anywhere, so a stack view would be an extra. `History::undo_steps` and `redo_steps` return the labeled steps if a view is ever wanted; a list would go to docs/post-parity.md.

Decisions on the open questions (coordinator, 2026-10-15):

- A typing pause ending a word step is decided in PR 1.5, when real typing exists.
- A notebook-level redo surviving a page edit is fine as built.
- No toast when PR 1.7 drops a stale step after an outside edit; a log line is enough.

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

### PR 1.10 notes: deviations and what each OS gives

Built on `feat/ink-pen` (2026-10-15). What differs from the steps above:

- Stroke storage in the canvas block moved to PR 1.9. Strokes live in the session (`Session::ink()`, one `InkLayer` per page id) and are lost on exit until PR 1.9 writes them. `notecraft_core::Stroke` already serializes to the `ink` entries of docs/markdown.md section 3.4 (`InkPoint` is `[x, y, pressure]`, `InkColor` is `#RRGGBB`), so PR 1.9 only needs a reader and a writer.
- The stroke types are in `notecraft-core` (L0, as AGENTS.md section 5 lists them), not `notecraft-model`; the pointer sample type is there too.
- New command ids: `ink.add_stroke` (the canvas sends it when a pen gesture ends; undoable by spec, undo itself lands in PR 1.8). `draw.type`, `draw.pen`, `draw.color`, and `draw.thickness` are Ready. `draw.highlighter` and `draw.eraser` stay Coming Soon until PR 1.11. `ArgKind::Array` was added for the stroke's points.
- The pen gallery is the existing Draw tab gallery (Phase 0); it now runs real commands, and the pen and thickness menus check the entry that matches the pen in use (`ribbon::ActivePen`).
- Widths are points on the ribbon and page units (px96) in a stroke: 1 pt is 1.33 units.
- Velocity-based width (the docs/gaps.md fallback) is applied at capture: a stroke with no pressure gets a stand-in pressure from speed (1.0 when slow, 0.4 at 1,500 units per second), so it reloads looking the same. The format has no flag saying a pressure was synthesized.
- The command guard snapshots the whole session before each command, strokes included. Layers are shared copies (`Arc`), so commands that do not touch ink stay cheap, but adding a stroke copies that page's layer once. PR 1.8 replaces the snapshot with transactions.
- Mesh edges are anti-aliased with a one-pixel fringe (`Edge` tags from the tessellator, moved by half a device pixel in `canvas/ink.rs`); see docs/architecture.md section 10.2. Translucent strokes get no fringe, which the highlighter design in section 10.3 accounts for.
- Touch and a mouse both draw while the pen is the tool. OneNote's palm rejection and touch-to-pan are IN-10 (Phase 3).

WordCraft survey (2026-10-15): the related WordCraft app draws live ink as a constant-width polyline, does no smoothing, and scans strokes linearly, so the pipeline above is ahead of it. What is worth taking from it is recorded in docs/architecture.md section 10 and under PR 1.11, 3.1, 3.2, and 3.3.

Smoothing: a one-euro filter on position (minimum cutoff 2.5 Hz, beta 0.1, derivative cutoff 1 Hz) and a light one on pressure (6 Hz). The tests assert that jitter at rest drops by more than half and that lag at steady speeds from 100 to 1,500 units per second stays under 2 units. These are numbers on synthetic input; they have not been tuned by hand on a real pen.

What each OS gives for pressure, read from the source of winit 0.30.13, egui-winit 0.36.2, and egui 0.36.2 (the versions in Cargo.lock). **None of this was tested on a real pen or tablet**; the only evidence is source reading and synthetic egui events.

| Platform | What reaches egui | Pressure |
|---|---|---|
| All | egui mouse events (`PointerButton`, `PointerMoved`) carry no pressure. Only `Event::Touch { force }` can. | n/a |
| Windows | winit handles `WM_POINTER` and sends pen and touch input as `Touch` events, walking the frame history so several samples per frame arrive. `force` is the pointer's pressure (1 to 1024) divided by 1024; a pressure outside that range, such as 0, gives `None`. A mouse sends mouse events. | Yes, if the driver reports it through Windows Ink. A pen cannot be told from a touch screen in egui: a touch with force is treated as a pen. |
| macOS | winit 0.30 sends tablet and trackpad input as mouse events. It has a `TouchpadPressure` event for force-touch trackpads, which egui-winit ignores (a TODO in its source). | No. Strokes use speed-based width. |
| Linux, X11 | Touch screens arrive through XInput2 as `Touch` events with `force: None` (marked TODO in winit). A tablet pen appears as a mouse. | No. |
| Linux, Wayland | winit 0.30 handles touch with `force: None` and has no tablet protocol support. | No. |

To get pressure on macOS and Linux, the plan is a safe wrapper crate for each OS (the risk table in docs/gaps.md) or a newer winit that exposes tablet data. Newer winit releases may add it, but that was not checked here. Neither is part of this PR.

## PR 1.11: highlighter and erasers

| | |
|---|---|
| Branch | `feat/ink-erase` |
| Hours | 24-40 |
| Rows | IN-04, IN-05, IN-06 |

Design (docs/architecture.md sections 10.3 and 10.4, from the WordCraft survey):

- Highlighter: a pass of its own behind the text containers, each stroke an opaque mesh in the color pre-blended with the theme's paper color at about 0.5 alpha, so a stroke cannot darken itself and the anti-aliased fringe still applies. True multiply (overlapping strokes darken, images tint) is a later `egui_wgpu` callback and needs no format change. The stored stroke keeps `tool: "highlighter"`.
- Stroke eraser: hit test through `InkLayer::hit_test` with the pointer reach in screen pixels; one drag is one undo step (the first erased stroke starts the step and later ones join it, which needs a merge flag on PR 1.8's transactions).
- Point eraser: cut the points inside the eraser circle and split the stroke into the remaining runs (color, width, and pressure kept; runs under two points dropped).
- The eraser and highlighter entries keep the Draw tab menus; the menu marks the current one the way the pen menu does (`ribbon::ActivePen`).

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

### PR 1.13 notes: deviations and open questions

Built on `feat/structure-editing` (2026-10-16), on top of the Phase 1 base (PR 1.1, 1.2, 1.3, 1.4, 1.8, 1.10). What differs from the steps above:

- The design is in docs/architecture.md section 5.1. Structure edits are `Scope::Notebook` transactions of `FsOp`s (create, move, write a settings file, close or reopen a notebook). The disk changes first, and the tree is read from the folder again after every step, undo, and redo. Undo of a rename or move re-applies the inverse file operation; a step that no longer fits the files is dropped without changing anything, and a half-applied step is reversed.
- Delete goes to `.notecraft/trash/<seconds>-<n>/` with an `origin.toml`, as the plan says ("to trash folder, restore UI comes in Phase 3"). The Recycle Bin list, restore, and the 30 day purge stay in PR 3.5; this PR builds no UI that reads the trash. Notebook "delete" is `file.close_notebook` (OneNote's Close This Notebook): it leaves the list and keeps its folder, because deleting a user's whole folder is not something a menu item should do.
- Commands (all Ready, undoable except `file.new_notebook`): `page.new`, `page.rename`, `page.delete`, `page.move`, `page.set_level`, `page.make_subpage`, `page.promote_subpage`, `section.new`, `section.group_new`, `section.rename`, `section.delete`, `section.move`, `section.set_color`, `notebook.rename`, `notebook.set_color`, `file.close_notebook`, and `file.new_notebook` (now creates the folder and `notebook.toml`). Sections and section groups share the `section.*` commands. `file.notebook_properties` stays Coming Soon (it is a dialog with the folder path; rename and color are separate commands here).
- Renaming a notebook changes its display name in `notebook.toml`; the folder keeps its name, so the notebook id and every page id stay the same.
- A page title is its file name (docs/markdown.md section 2.3), so renaming renames the file. A frontmatter title that differs from the file name is not changed or read yet (titles from frontmatter come with the index worker); the navigation shows the file name.
- Moving a page to another section also copies the attachments its text links to (found by scanning the text for `attachments/<name>`); the original stays for pages that still use it. The attachment store itself (hash prefixes, linked files) is PR 1.12. Subpages do not travel with their parent page when it is dragged away; the moved page lands at level 0. OneNote moves the subpages of a collapsed parent; subpage collapsing is not built, so this waits for it.
- A section moved into a group keeps its name; a taken name gets a number. Moving checks the 8 level nesting limit and refuses a group moved into itself.
- UI (feat(ui) commit): context menus on notebook, group, section (tab and pane), and page rows; double-click or the Rename item renames in place (Enter commits, Esc cancels); drag a page to reorder it or onto a section; drag a section or group to reorder it, into a group, or onto the notebook for the top level; Move Up and Move Down in the page menu do what a drag does for the keyboard. The drag logic is tested through the position math and the commands it emits (`drop_index`, `move_page`, `move_section`); the headless harness does not drive a pointer drag.
- Section and notebook colors: a Color submenu with eight colors (the six defaults, teal, and gray) writes `color` to `.section.toml` or `notebook.toml`. A custom color picker is not built.
- Sample (mock) notebooks cannot be edited: the commands fail with "this is a sample notebook; open or create a notebook folder first" (shown as a toast). Open or create a notebook folder first. `page.move` still reorders and moves pages in memory there.
- The index (PR 1.2) needs no new code: a rename or move is a removal and an addition for `Index::refresh`, which reads the folder. Tests check that the index agrees with a fresh scan after each kind of edit.
- Not done, by plan: Recycle Bin UI (PR 3.5), password-protected sections (PR 3.x), page links and link updates on rename (Phase 2), a custom color dialog.

Decisions on the open questions (coordinator, 2026-10-16):

- A new section starts with one Untitled Page, created in the same undo step and selected, as in OneNote; Add Page stays at the end of the list; closing is the right meaning of "delete notebook", with no OS trash option.

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

## PR 1.16: command line info, text, check, and run on notebooks

| | |
|---|---|
| Branch | `feat/cli-notebooks` off `phase/1-core-engine` |
| Hours | 10-16 |
| Rows | none |

Steps: the global `--notebook <path>` option (default: the current directory if it is a notebook), plus `--json` and `--quiet`. `notecraft-cli info` prints notebook, section, and page counts and the format version. `notecraft-cli text <page>` prints a page's plain text. `notecraft-cli check` validates a notebook (unparseable pages, broken links, missing attachments) and exits non-zero when it finds problems. `notecraft-cli run --page <page> --cmd <id> ...` opens the notebook through the engine, runs the commands against the page, and saves it (exit code 5 if the file changed on disk). `<page>` accepts a page id, a notebook-relative path, or a page link target. Depends on PR 1.1, 1.3, 1.4, and 1.7.

Commits:

1. `feat(cli): add --notebook and the info command`
2. `feat(cli): add text and check`
3. `feat(cli): run commands against a page and save it`

PR title: `feat(cli): add info, text, check, and run on notebooks`.

This PR is part of the command line decision of 2026-10-15 (docs/cli.md). It can land at any point after PR 1.7 and before PR 1.15.

## PR 1.15: close Phase 1 and ship the alpha

Follow 00-conventions.md section 6, then release v0.1.0 (docs/releasing.md). Fill every Evidence cell in the docs/roadmap.md alpha gate table first; do not release with an empty cell.

Also do the hands-on checks deferred from Phase 0 (maintainer decision, 2026-10-15; docs/gaps.md rank 25, 2-4 hours): launch the window and follow system dark mode and the OS UI font on macOS (SH-01, TH-06); follow system dark mode on Linux (TH-06); ribbon key tips on a physical keyboard (SH-05); Narrator on Windows and VoiceOver on macOS (SH-15); shortcuts using Cmd on a Mac (SH-14); the real OS file dialogs on all three systems (SH-12). Record each result in the row's evidence and fix what fails before tagging v0.1.0.

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
| 1.15 close and alpha | 10-18 |
| 1.16 command line on notebooks | 10-16 |
| Total | 456-776 |

Re-estimated at the Phase 0 close (2026-10-15). No actual Phase 0 hours were recorded, so there is nothing to scale from and the PR ranges stay as they were. One change: PR 1.15 gains 2-4 hours for the hands-on checks deferred from Phase 0 (docs/gaps.md rank 25).

## Exit criteria

- Every row listed at the top scored D.
- Alpha gate table complete with evidence.
- v0.1.0 published.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-15 | 0 | Added PR 1.16 (command line info, text, check, and run on notebooks) after the maintainer decision of 2026-10-15; Phase 1 estimate 454-772 hours. |
| 2026-10-15 | 0 | Re-estimated at the Phase 0 close: PR 1.15 raised to 10-18 hours (deferred hands-on checks, gaps rank 25); Phase 1 is 456-776 hours. |
| 2026-10-15 | 1 | PR 1.1 landed; deviations recorded under PR 1.1. |
| 2026-10-15 | 1 | PR 1.10 landed: deviations (stroke storage moved to PR 1.9) and per-OS pressure notes added. |
| 2026-10-15 | 1 | PR 1.3 done: deviations (page text seam instead of the folder reader, block model for PR 1.4, hand-written frontmatter reader) noted under PR 1.3. |
| 2026-10-15 | 1 | PR 1.2 done: deviations (native only, mtime and size then hash, titles from file names, worker not yet wired) noted under PR 1.2. |
| 2026-10-15 | 1 | PR 1.4 done: deviations (Editor API, canonical serializer scope, minimal save path in storage, corpus location) noted under PR 1.4. |
| 2026-10-15 | 1 | PR 1.10 follow-ups from the WordCraft survey: anti-aliased stroke edges and a marked current pen; PR 1.11 gains its highlighter and eraser design. Hours unchanged. |
| 2026-10-15 | 1 | PR 1.8 transactions gain a merge-with-previous flag for the stroke eraser (maintainer decision, 2026-10-15); hours unchanged. |
| 2026-10-16 | 1 | PR 1.13 done: deviations (trash layout, close as notebook delete, commands, sample notebooks read-only) and open questions noted under PR 1.13. |
