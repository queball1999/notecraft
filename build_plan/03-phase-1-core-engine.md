# Phase 1: core engine

> Stage: pre-alpha (Phase 1 on main; v0.0.2 alpha next). Last reviewed: 2026-10-18. Last updated: 2026-10-19. Change: bold text is drawn with the OS bold face (feat/bold-face); PR 1.9 follow-up: page style saved in the page frontmatter; IN-02 pressure decision under PR 1.10 (Windows only; macOS and Linux deferred); earlier: phase branch merged into main (#55); earlier: pictures and files in the canvas block (feat/canvas-pictures); textured pens and the pencil notes (IN-01 scored D); earlier: PR 1.15 close-out notes (hands-on checklist, version question); PR 1.13 follow-up: collapse and expand subpages, subpages travel with a moved or deleted parent; earlier: PR 1.5 sits on top of PR 1.9 and PR 1.13: container runs are a view over the single marker reader (docs/markdown.md section 3.3); PR 1.1 landed; deviations noted under PR 1.1; PR 1.10 landed on feat/ink-pen; deviations and the per-OS pressure notes added; PR 1.3 done; deviations noted under PR 1.3; page text read from disk when a page is shown (FM-01 scored D); also: PR 1.4 done; deviations noted under PR 1.4; PR 1.10 follow-ups (anti-aliased ink edges, marked current pen, WordCraft survey notes); Ink Replay scheduled in PR 3.2 (IN-13); PR 1.11 follow-ups (eraser sizes, grazing point eraser); PR 1.11 done; deviations noted under PR 1.11; PR 1.13 done: deviations and open questions noted under PR 1.13; Ink Replay scheduled in PR 3.2 (IN-13). PR 1.5: text editing in containers; PR 1.14 done; notes under PR 1.14. PR 1.6 notes added; PR 1.12 landed (pictures and attachments, AT-01 to AT-05 scored P); PR 1.12 follow-up: SVG pictures, resize handles, Ctrl+V images; PR 1.5 sits on top of PR 1.9 and PR 1.13: container runs are a view over the single marker reader (docs/markdown.md section 3.3); PR 1.9 follow-ups landed (container positions, repair, warning bar and Convert); PR 1.16 done (command line on notebooks).

Goal: real notebooks on disk. Open a folder, read and write `.md` and `.notecraft` pages with minimal diffs, type in note containers, draw ink, embed images and files, undo and redo, auto-save, and survive crashes and outside edits. Ends with the v0.0.2 alpha (maintainer decision, 2026-10-18).

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
- `IndexWorker` is a plain thread with an mpsc channel (`try_result`, `wait`). The navigation now reads it (see the wiring notes below); ST-07 is scored D.
- Bench (criterion, `cargo bench -p notecraft-storage --bench sidebar`, 1,000 pages in 20 sections): open and list from the index about 27 ms; a refresh with nothing changed about 1 s and a cold build about 30 s on the development machine, a remote session on a nearly full disk where each file open took roughly 30 ms. The cold build is limited by per-file I/O, not SQLite, and runs in the background.
- Navigation wiring (feat/nav-from-index, 2026-10-17, gaps rank 15, ST-07 now D):
  - `open_folder_for_navigation` (engine `open.rs`) builds the tree from `Index::sidebar` when `.notecraft/index.sqlite` exists, has sections, and matches the notebook id; otherwise the folder reader is the fallback. It opens the file without `quick_check` or repair, so the UI thread never waits on a bad database; the worker repairs it.
  - The worker is `io/index_sync.rs` in `SessionIo`, one thread per open notebook, started on first sight, on watcher events outside `.notecraft`, and again when a result is stale. It uses `refresh_or_rebuild` (new, shared with `IndexWorker`), reads frontmatter titles for added and changed pages, and returns the tree. `Session::apply_index_refresh` swaps it in only when it differs and no structure edit read the tree again meanwhile (per-notebook tree generation).
  - Schema version 2 adds `pages.fm_title`. `title` stays the file name; `PageRow.title` is the title to show (frontmatter title when set). A refresh clears `fm_title` for a page whose content hash changed, and the worker fills it again.
  - Structure edits still read the folder (the folder is the truth, so the tree is exact at once); the remembered frontmatter titles are applied on top, and the watcher events from the edit trigger the index refresh.
  - Not done: the scan warnings (skipped folders, bad settings files) are shown only when the folder reader runs, not when the tree comes from the index; a rename dialog still edits the file name, not the frontmatter title; no hand check on a huge notebook in the live app.
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
- Bold is drawn with the OS bold face when one is found (see the PR 1.6 notes); overstriking is the fallback when there is none, and for monospace code.
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

### PR 1.5 notes: deviations and open questions

Built on `feat/container-editing` (2026-10-16). The design is in docs/architecture.md section 11. What differs from the steps above:

- Not split into 1.5a and 1.5b; the PR is about 4,000 lines with tests. Commits: model, markdown container runs, canvas text logic, engine commands, UI widget, UI tests, docs. The plan's commits 2 and 3 (widget, then IME and keyboard) are one commit because the widget cannot be shown without the keyboard.
- Container model: `notecraft_model::Container` holds id, optional position (none means flow layout), and width, clamped. Text lives in the page markdown (docs/markdown.md section 3.3 markers), read by `notecraft_markdown::split_containers`. A click on empty canvas appends a marker line to the page text and records a position in the session (`SetContainer` undo op); the `.notecraft` canvas block that stores positions is PR 1.9, so positions were not saved at first (now they are, see PR 1.9 notes). Containers made this way are in a `.md` page with markers; promotion to `.notecraft` is PR 1.9.
- Merged onto PR 1.9 and PR 1.13 (one container model): the marker reader is `notecraft_markdown::container` (strict markers: a marker comment without an `id=` is an ordinary comment; ids up to 64 characters; fenced code and the canvas block are skipped; at most 10,000 markers; fresh ids are the highest `cN` plus one). `split_containers` is now a thin view over it that adds the content span, the block range, and the id a save will give a bad marker (taken from `fix_container_ids`, so the editor and the save path always agree). The editor also stops at the start of the `notecraft-canvas` block: the block is never laid out, never edited, and a new container is added before it. The PR 1.5 dirty set was dropped in favor of the PR 1.7 save state (`mark_page_dirty` is called by the `ReplaceText` undo op). Container positions are now saved in the canvas block (PR 1.9 follow-up, below).
- Command ids: `text.set_caret`, `text.move`, `text.insert`, `text.delete`, `text.enter`, `text.tab`, `container.new`, `container.move`, `container.resize`; the existing Home tab ids `edit.cut`, `edit.copy`, `edit.paste`, `edit.paste_text_only`, `edit.select_all`, and `edit.delete` became Ready. Copy and Cut return the markdown text for the front end to put on the clipboard (the shell reads the clipboard through egui events); Paste takes the clipboard text as an argument. Up, Down, Home, and End follow wrapped rows, which need the layout, so the canvas turns them into `text.set_caret` calls.
- Edit granularity: edits are splices at byte offsets through `Session::edit_page_text`, finer than the block replacement PR 1.4 allowed; `Editor` is not needed for typing. It remains the tool for PR 1.6 style changes.
- Decision on the typing pause (the open question from PR 1.8): no pause timer. Word-sized steps from the pure rule were enough in tests, and a clock would make undo depend on typing speed. Revisit after hand testing.
- Live markdown rendering is block level: the caret's block (or all blocks a selection touches) shows raw markdown, the rest stays styled. Syntax visible only on the active line, as the plan says, is approximated by the active block, because inline runs have no spans.
- Space+drag pan: the canvas now holds keyboard focus while typing, so Space+drag pans whenever no text field has focus, and a Space typed in the text still inserts a space.
- Shortcuts: while the canvas is in typing mode, shortcuts other than cut, copy, paste, and delete still run (Ctrl+Z, zoom, tabs); the text keys reach the text as key and clipboard events.
- Not done: formatting buttons (PR 1.6), saving (PR 1.7), find (Phase 2), drag and drop of text. Containers do not yet overlap-protect or snap. No IME was tried on a real input method (the tests drive egui's IME events); add it to the real hardware checks in PR 1.15.
- Open questions: (1) should an empty container survive a click away? It is removed now, as in OneNote. (2) Should New Container be on the ribbon? OneNote has none, so it is click only. (3) The page extent grows with containers but not with an off-screen drag; confirm by hand.

### PR 1.9 follow-ups (feat/save-container-positions)

- Positions: `notecraft_storage::write_canvas` writes ink and the `containers` map in one pass with minimal diffs; `CanvasContainer` carries id, x, y, and an optional width. Container move, resize, and their undo steps mark the page dirty; a `.md` page with a positioned container is promoted to `.notecraft` on save. Convert to markdown drops the positions with the ink (the warning says so).
- Repair: `page.repair_canvas` (`Session::repair_current_canvas`) replaces a block that did not parse with a fresh one keeping what could be read. It is offered only for an unreadable block, not for a newer format, an oversized block, or too many strokes. The original is already under `.notecraft/history/<page>/canvas-unreadable.notecraft`; the page becomes editable and unsaved, and its undo steps are dropped.
- UI: the bar under the ribbon shows Repair for an unreadable block (`bars::Bar::CanvasUnreadable`); the File Info backstage page shows the data-loss warning and a Convert to Markdown button (`page.convert_to_markdown` with `confirm`), only for a `.notecraft` page that is not read-only or in conflict (`Session::can_convert_to_markdown`). Decision: the plan said the Convert button goes in the bar; it is on File Info instead, since a healthy page should not show a standing bar.
- Not done: images, files, and shapes in the block (Phase 2 to 3). Hand check: drag a container, quit, reopen; hand-break a block and press Repair.

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

Notes (as built):
- Commits landed as markdown format edits (inline and block marks, lists, tasks, indent, alignment, state at the caret), engine commands, canvas drawing of the new run styles, enabling the buttons, and tests; the five listed commits were regrouped because the markdown and engine halves needed each other. Tables already drew read-only from PR 1.5 and gained a layout test here.
- Format commands: `format.bold`, `format.italic`, `format.strikethrough`, `format.underline`, `format.subscript`, `format.superscript`, `format.highlight`, `format.font_color`, `format.font_size`, `format.font_family`, `format.clear`, `format.bullets`, `format.numbering`, `format.indent`, `format.outdent`, `format.align_left`, `format.align_center`, `format.align_right`, `style.apply`, and `tag.todo` (Ctrl+1). Each is one trimmed splice through the PR 1.5 guard (one undo step); toggles on the selection or the word at the caret.
- Markup beyond CommonMark and GFM (docs/markdown.md section 3.2): `<u>`, `<sub>`, `<sup>`, `==yellow==`, one-property `<span style>` for color, size, and family, and a `<!-- notecraft:align=... -->` comment line above a paragraph. All are plain text a reader without NoteCraft still sees.
- Formatting is bounded to the text before the `notecraft-canvas` block, and never reaches the frontmatter or a container marker; container marker lines come only from `notecraft_markdown::container` through `split_containers`.
- Bold face (feat/bold-face): Bold text uses a real bold face: the OS UI font's bold (Segoe UI Bold `segoeuib.ttf` on Windows, the system font at weight 700 on macOS, the file fontconfig names for `sans-serif:bold` on Linux; a variable UI font is instanced at its weight axis), registered as the egui family `notecraft:bold` with the regular fonts behind it. Without one, bold is overstruck as before. Layout and drawing use the same galley, so caret, selection, and wrapping agree with the bold widths. Bold monospace code and bold in the raw-edit block (plain markdown source) are unchanged. Not done: bold for `font-family` spans (the canvas does not draw the family of a span yet), and a hands-on look on macOS and Linux.
- Not done: per-character formatting inside the raw-edit block, Format Painter and Clear Formatting beyond `format.clear` (Phase 2), table editing (Phase 3).
- Open questions: (1) alignment as a comment line is invisible in other editors; an HTML `<p align>` would show but break plain markdown. (2) Heading 1 to 6 and Normal replace the line prefix; mixed selections change every line they touch.

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

### PR 1.7 notes: deviations and open questions

Built on `feat/autosave` (2026-10-16), on top of `wip/phase1-base`. What differs from the steps above:

- Commits: the plan listed five. The storage pieces (recovery journal, folder watcher, and a page reader thread) are one commit; dirty tracking, timed auto-save, and the outside edit logic are one engine commit, because the engine ties them together; the bars are one UI commit; the crash and outside-edit simulations live in the engine tests (`io::tests`) next to the code they exercise, since the session is where the pieces meet. Auto-save and the session logic are in `notecraft-engine` (`session/saving.rs`, `io.rs`), not `notecraft-storage`: storage stays a set of file tools (journal, watcher, reader), and the engine owns the session state. The design is in docs/architecture.md sections 3.2 to 3.4.
- Dirty tracking API for PR 1.5: `Session::mark_page_dirty(page_id)`. Edits through `commit`, `undo`, and `redo` (page text and strokes) call it already.
- Only page text is written. Strokes mark a page dirty but add no bytes until PR 1.9 saves the canvas block.
- Pages whose notice is read-only are never saved or journaled. A page with invalid UTF-8 is editable and saves with the replacement characters (the notice said so when it was shown).
- Conflict bar (maintainer-visible decision): Keep my version and Use outside version. A side-by-side compare is left out: OneNote has no such view, and the architecture doc's "compare" was a placeholder. A save never writes over a file that changed since NoteCraft last read or wrote it; it turns into the same conflict. While a conflict is open the page is not saved and stays in the journal.
- Outside reload drops the page's undo steps and logs a line, with no toast (maintainer decision, 2026-10-15).
- Saves run on the UI thread; page reads and the watcher run on worker threads. Moving saves off the UI thread is possible later; one atomic write of a small file is fast, and doing it inline keeps the quit path simple.
- The journal is refreshed every 2 s while text changes (the plan gave no interval). It is a JSON file per page, not an append-only log, so a crash loses at most 2 s of edits and replay is a straight text restore.
- `notify` 8 (CC0-1.0) is a new dependency of notecraft-storage, native only.
- Quit: `AppShell::flush_unsaved` runs on a close request and from the app's `on_exit`. craft-quit has no save hook of its own; it already runs before eframe closes, so the close request path covers the prompt case.

Open questions:

- Should a deleted page file (outside delete while open) show a bar too? Today it is only logged and the page stays open; the next save recreates the file.
- Should Restore also offer a per-page choice? Today it restores every offered page.

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

### PR 1.9 notes: what was built and the calls made

Built on `feat/notecraft-format` (2026-10-16). Ink saving (IN-01, IN-02) and the container and canvas readers are done; positions for the other canvas objects are not.

- Markdown crate: `read_containers`, `fix_container_ids` (an invalid, `c0`, or duplicate id gets a fresh `cN`; only the id bytes change), `remove_container_markers`. Markers inside code fences and frontmatter do not count; at most 10,000 are read.
- Storage: `canvas_block` finds the last top-level `notecraft-canvas` fence, reads ink with a streaming lenient reader (unknown fields skipped, bad numbers fall back, coordinates clamped to 1,000,000, ids cut to 64 characters, strokes over 20,000 points simplified with Ramer-Douglas-Peucker in `notecraft_core::simplify`, nesting over 32 refused) and writes only the `"ink"` value, so every other key and all spacing stay byte for byte. A page with no block and no ink stays as it is; the first stroke appends a block. Identical strokes (after rounding to 2 places) leave the text untouched.
- Failure policy: a block that does not parse, is over 48 MiB, has a newer `format`, or has over 100,000 strokes opens read-only with a notice (`CanvasProblem`), text shown in flow layout, and a copy goes to `.notecraft/history/<page path>/canvas-unreadable.notecraft`. The writer refuses (never rewrites) such a block, and refuses to append a block to a page that ends inside an unclosed code fence.
- `.md` pages: the spec keeps a markdown page plain, so it has no canvas block. The sensible call: the first stroke on a `.md` page promotes it (`change_page_kind`: atomic write of the `.notecraft` file, then the `.md` file is removed, and a quoted file name in `.section.toml` is rewritten). The page id changes, so the session re-keys everything it holds and clears the undo history.
- Engine: `load_current_page` reads `.notecraft` pages (whole file text in the session, strokes into the ink layer); `Session::save_page(id)` writes text and strokes in one atomic write; `PageSource::visible_text` hides the block from the layout; `page.convert_to_markdown { confirm }` (destructive, not undoable) returns the warning until `confirm` is true, then drops the markers and the block.
- One save path (merged with PR 1.7, #39): `Session::save_page` keeps #39's guards (dirty, read-only, open conflict, disk-hash check before writing, journal cleanup) and its outcomes, and adds the ink. The text it writes is `file_text`: the page text with the strokes in the canvas block. A `.md` page with strokes is written as `.notecraft` and the `.md` file removed; the outcome is `Promoted { new_id }`, and the dirty mark, disk hash, conflict, pending recovery, queued reads, selection, page list, text, notice, style, and strokes all follow the new id (`Session::rekey_page`; the page's undo steps are dropped). `SessionIo` (timer, focus loss, page switch, quit) therefore saves ink with no changes. The recovery journal holds the whole file text, canvas block included, so a crash keeps the strokes: `restore_recovery` reads the block back (`load_canvas_from_journal`) and the next save writes it, promoting a `.md` page if needed. An outside change to a `.notecraft` page with unsaved strokes is a conflict, not an overwrite; taking the outside version reloads its strokes.

Open questions: a read-only page whose block did not parse cannot be repaired or saved from the app yet (the copy under history is the way back); positions of containers, shapes, images, and files in the block wait for the PRs that draw them; there is no UI button or warning bar for Convert yet.

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

Decision (maintainer, 2026-10-19): IN-02 is scored on Windows only. Pressure on macOS and Linux is deferred until eframe adopts a winit release that reports tablet pressure (docs/gaps.md, Deferred). A research pass found XInput2 through `x11rb` as the only safe path today (about 25 hours, X11 only); macOS and Wayland would need `unsafe` code or a separate wrapper crate.

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

### PR 1.11 notes: deviations and open questions

Built on `feat/ink-erase` (2026-10-16), on top of PR 1.8 and PR 1.10. The design is in docs/architecture.md sections 10.3 and 10.4 ("as built" paragraphs). What differs from the steps above:

- Commits: the engine, canvas geometry, and command work for all three tools landed in one commit (they share the tool state, the ids, and the undo case table), then the front end, then tests and docs. The four commits above were not kept apart.
- New command `ink.erase { points, radius, mode?, join? }` (Ready, undoable). `draw.highlighter` and `draw.eraser { mode? }` are Ready and only pick the tool; the canvas sends `ink.erase` once per frame of an eraser drag. Both erasers are the same command, selected by `mode` (`stroke` or `point`), so a script can erase without the pointer.
- No new undo op: a point-eraser split is `DeleteStroke` plus `InsertStroke` ops at the stroke's paint position, in one transaction. The "one undo step per drag" rule uses `merge_into_previous()` but only while the step just before is an erase (`History::erase_chain`); the first sample of a drag sends `join: false`. Without that guard a join right after a pen stroke or an undo would have merged into the wrong step (tested in `a_new_drag_and_other_steps_start_a_new_step`).
- The highlighter has its own style (default yellow, 8 pt) next to the pen's; `draw.color` and `draw.thickness` change whichever is the tool. The Draw tab menus mark the current highlighter, eraser mode, and thickness (`ribbon::ActiveTool`).
- Eraser sizes (maintainer decision, 2026-10-16): the Eraser menu lists Stroke Eraser plus Small, Medium, and Large point erasers, as OneNote's does. `draw.eraser` takes `mode` and `size` (a size alone means the point eraser; the size is kept across modes); the circle is 5, 10, or 20 screen pixels and the stroke eraser keeps its 6 px reach. The menu marks the current entry.
- Point eraser reach (maintainer decision, 2026-10-16): it cuts wherever its circle touches a stroke's outline, including a graze between two stored points. `split_stroke` solves the circle against each segment exactly, adding the drawn half width (nominal width and pressure, interpolated along the segment) to the radius; cut ends sit where the circle meets the outline. Tests: `a_graze_between_points_cuts`, `the_width_at_that_point_counts`, `the_point_eraser_cuts_a_graze_between_stored_points`.
- The highlighter blends with the paper at 0.5 into an opaque mesh. That covers rule lines under the stroke and does not let strokes darken each other; true multiply is the paint callback in section 10.3, step 3. IN-04 stays P for that and for saving.
- Shared build folder note for agents: several worktrees building into one `CARGO_TARGET_DIR` can clobber each other's workspace crates (same package id), so this PR was built in `shared-target/ink-erase`.

Decisions that closed the open questions (2026-10-16): eraser sizes and the grazing point eraser are built as above; true multiply for the highlighter stays a recorded gap (docs/gaps.md rank 21), and the maintainer decides where it goes.

### Textured pens and the pencil notes (2026-10-18, feat/textured-pens)

The last Draw tab leftover of PR 1.10 and PR 1.11 (IN-01), built on top of the Phase 1 close-out. Decisions and what differs from what the notes above assumed:

- Format: a stroke records `"texture": "<id>"` in the canvas block (docs/markdown.md section 3.4), only for a pen stroke with a texture. The ids are `pencil`, `rainbow`, `galaxy`, `lava`, `ocean`, `gold`, `silver`, `bronze`, and `rose_gold`. `notecraft_core::InkTexture` is the type; a missing key, an unknown id (any case is read), or a value that is not text reads as a plain color stroke, and a highlighter never has one (`Stroke::bounded`). The stroke's `color` stays meaningful: it is the color older readers and unknown textures draw, and for a textured pen it starts as a color that stands for the texture (`InkTexture::swatch`). No format version bump, because the key is ignored by readers that do not know it.
- Ocean was added to the list as a water-colored pen of the same family (the exact set OneNote offers should be checked in the hands-on pass; unused pens are cheap to drop); the looks are described in our own words in `notecraft_canvas::ink::texture`: a rainbow whose hue turns once per repeat with a soft gloss, a night sky of violet and teal clouds with stars, a dark crust cut by glowing veins, blue-green water with pale wave bands, four metals with a bright band on the upper side and a slow shimmer, and a grainy pencil.
- Rendering stays in the canvas crate for the images and the vertex coordinates (L4, no egui) and in `notecraft-ui-egui` for the upload. Each texture is a small deterministic RGBA image built from integer hashes and wrapped value noise (no image assets, so nothing new in ATTRIBUTION.md); the egui layer uploads it once on first use (`canvas/textures.rs`, `TextureOptions::LINEAR_REPEAT`). A stroke is still one mesh: `tessellate` now records, for every vertex, how far along the smoothed centerline it lies (`StrokeMesh::along`, with caps running on past the ends) and where it lies across (`across`, -1 to 1; ribbon edges are exactly +1 and -1, fringe vertices copy their rim vertex). The image coordinate is `along / period` for a stroke texture (the period grows with the stroke width so the image keeps its proportions, with a minimum so a thin pen does not cycle too fast) and the page position over a 96 unit tile for the pencil. The paint cost is the plain stroke's: no per-pixel work on the CPU and no extra pass.
- The pencil's grain is a page-pinned tile tinted by the pen color through the vertex color; the other textures draw in white so the image shows true. The fringe still fades the edge (its alpha multiplies the image). A sharp bend double-draws the inside of a join disc, which can darken the pencil's grain there slightly; accepted.
- The stroke in progress is chunked as before. A textured chunk starts its image where the one before it ended, using the length of the points before it as the arc length, so the preview matches the saved stroke to within the difference between the polyline and its smoothed curve (under one percent for dense points). The saved stroke replaces the preview on the next frame.
- Pen list and commands: `PenStyle` gains `texture`. `draw.pen { texture? }` and `draw.tool_add { texture? }` take a texture id (`plain` or empty for none); `draw.tool_select` brings the texture along; `draw.pen` output reports `texture` for a textured pen. A color given to a pen whose look is its own (anything but the pencil) turns it plain in that color (`PenStyle::with_color`), as do `draw.color` and the Eyedropper; the pencil keeps its texture and takes the color. The settings keep `texture` per pen in `config.toml` (an older file has none; an unknown id reads as a plain pen in the saved color). The point eraser's runs keep the texture.
- UI: the Add Pen dropdown lists Pen, Highlighter, Pencil, and the eight textured pens, marked when the pen in use is that kind (`ribbon::menus::is_current_pen`); the gallery draws the pencil with a pencil icon (`ToolShape::Pencil`), names textured pens by their look ("Gold Pen") and the pencil by its color ("Black Pencil"), and raises the one in use. The menu of a textured pen shows a preview drawn with the same mesh as the page (`canvas::paint_pen_sample`), Thickness, and Delete Tool, without the color palette; the pencil's menu keeps the palette.
- Hours: this was the "about 16-30 hours" of the Draw tab leftover in gaps rank 21; it was not part of any PR's range, so no phase total changes.
- Open question: whether OneNote's textured pens keep their look when the pen color is changed. We assume not (the color list is hidden for them); revisit after the hands-on pass if a real OneNote shows otherwise.

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

Done (2026-10-16), with these deviations from the steps above:

- Commits were regrouped: storage copy; markdown images as runs; engine insert commands; canvas picture decoding; canvas drawing; UI insert, drop, paste, and open; app wiring; docs. Names follow the layers (pictures are decoded in `notecraft-canvas::picture`, drawn in `notecraft-ui-egui`).
- Naming: `<6 hex of an FNV-1a content hash>-<sanitized name>`; a taken name gets ` 2`, ` 3`; the same bytes under the same name are reused; the name is reserved with `create_new`, so nothing is ever overwritten. Links are percent-encoded (`attachments/3f9a2c-My%20file.pdf`). Files over 100 MiB are linked in place by absolute path; the `notecraft: linked` canvas-block marker waits for PR 1.9 (the canvas block).
- An insert appends one paragraph (`![name](link)` or `[name](link)`) at the end of the page text through `Session::insert_attachment`, which uses the PR 1.8 transaction (one undo step, label "Insert picture" or "Insert file"). Undo takes the paragraph out and leaves the copied file in `attachments/`, so Redo works; unused attachments are cleaned up in a later task. Placement at the click point needs the canvas block (PR 1.9).
- The text change is minimal: `Editor::insert` at the end of the page, in the file's own line ending. `Run` gained `image: bool` so the parser keeps `![alt](target)` as a run (it was flattened to a link before) and the canonical writer prints it back.
- New commands: `attachment.open { src }` (new id; resolves the link inside the notebook, refuses programs and scripts such as exe, bat, sh, and returns the path for the app to open); `insert.picture`, `insert.file_attachment`, and `edit.paste_picture` are Ready with an optional `path`. The shell asks the picker (`pick_picture`, `pick_attachment`, `clipboard_picture`) when the path is missing, as the Phase 0 pattern does. The OS dialogs, the `open` crate, and `arboard` live in the app (L7); the engine never calls the OS.
- Bounds: picture file 32 MiB, 16,384 px on a side, 36 million pixels, texture shrunk to 2,048 px on its long side, 256 pictures per page, one decode per frame. A missing, unsupported, damaged, or too-large picture draws a placeholder with the reason.
- Not built: SVG (needs `resvg`), resize handles, selecting and moving pictures (both need the object model of PR 1.5 and 1.9), and Ctrl+V routing of images (waits for text paste in PR 1.5). Drag and drop works anywhere on the window, not at the drop point. Pictures linked from the web are not loaded.
- Follow-up (2026-10-17, branch `feat/pictures-svg-paste`): SVG pictures are drawn with `resvg` (default features off, so no text, system fonts, or linked images; `crates/notecraft-canvas/src/svg.rs`): file cap 4 MiB, a fixed bitmap of at most 2,048 px on the long side, and parse plus draw on a helper thread with a 3 second timeout (a hostile SVG shows a placeholder; the thread ends by itself). Resize handles: a click selects a picture, its four corner handles keep the proportions, and releasing sends one `picture.resize { src, width, nth? }` command (one undo step, label "Resize picture"; width 0 returns to the natural size). The width is stored as `<!-- notecraft:width=N -->` right after the image (docs/markdown.md section 2.4), a few bytes of diff that other readers ignore; `Run` gained `image_width`. `nth` counts pictures with the same link in page order; while the caret is inside a container shown as raw text its pictures are not drawn, so with duplicate links the count can differ (rare, left). Ctrl+V: the window layer sends a paste event only for text and swallows the key press otherwise, so the shell treats a V key release with the command key and no text paste before it as an image paste (`edit.paste_picture`, silent when the clipboard has no picture). Still open: placing a picture at the click point (needs canvas block positions), a real-OS check, moving pictures.
- Open questions: should Undo also delete a copied attachment that nothing refers to? Should SVG wait for Phase 3 with the other vector work?

### PR 1.9 and PR 1.12 follow-up: pictures and files in the canvas block (feat/canvas-pictures, 2026-10-18)

- Format (docs/markdown.md section 3.4): `images` and `files` lists of `{ id, src, nth?, x, y }`. Call made: the markdown (`![alt](src)` or a one-link paragraph) stays in the page text, usually at the end of the last container, and an entry ties to it by `src` and `nth` (which occurrence of that link, in reading order), so no marker comment is added to the text. The width stays the `<!-- notecraft:width=N -->` comment; the entries hold only the corner, so the spec example lost `w` and `h`. An entry whose link left the text is dropped on the next save (deleting the text deletes the object).
- Storage: `CanvasObject`, `write_canvas_all` (the older writers keep the objects they find), bounded reader (10,000 per kind, links up to 2,048 characters, ids unique, unusable entries skipped), fuzz check and hostile shapes added. Model: `notecraft_model::PlacedObject`. Markdown: `objects::object_slots` lists pictures and files in reading order; the canvas lays out in the same order, which is what makes `nth` agree.
- Engine: `Op::SetObjects` (whole list before and after, so the inverse is exact). `insert.picture`, `insert.file_attachment`, and `edit.paste_picture` take optional `x` and `y` (both or neither; text change and place are one undo step). New commands `picture.move` and `attachment.move { src, x, y, nth? }`: an object that flowed in its container gets a place when moved. Bug fixed on the way: an insert into a `.notecraft` page used to append after the canvas block, which hid the picture; it now goes before the block.
- UI: a placed picture or file is drawn at its corner, takes no room in its container, and extends the scrollable page. Dragging a picture (anywhere on it) or a file previews live and sends one move command on release. Insert rule (lead review, matches OneNote): with the caret in a note, `insert.picture`, `insert.file_attachment`, and `edit.paste_picture` (Ctrl+V of an image) put the markdown inline at the caret offset in that note, replacing a selection like typed text, as one undo step with the caret left after the object (it reuses the PR 1.5 `Ctx` guard, so marker lines and the canvas block are never touched; undo puts the caret back at the insertion point). A file attachment gets blank lines around it so it is a paragraph of its own and still draws as a file object; a picture goes inline in the paragraph. The object has no place of its own. Only when the last click was on empty canvas (no caret in a note) does the shell pass that click point and the object gets a free place; each further insert steps the point 24 units down and right. A file dropped on the page lands at the drop point (each further file steps 24 units). With no click yet, inserts flow with the text.
- Not done: shapes (IN-07, Phase 3); `z` order of placed objects (they paint in layout order); moving a placed object back into the text flow; a real-OS check of drag and drop.
- Calls made on the open questions (lead review): inserting with the caret in a note puts the object inline, not beside the note (done, see UI above). Dragging an object that sits inside a note gives it a place and leaves its markdown where it is in the note text (kept as built). Closed (lead decision): inline insert uses the caret offset, not the end of the page text. With no caret and no click, an insert still appends to the page text.

### PR 1.9 follow-up: page style in the page frontmatter (fix/save-page-style, 2026-10-18)

- Rule lines, rule line color, page color, and paper size are saved as keys of the `notecraft:` frontmatter map (`paper`, `paper_spacing`, `paper_size`, `rule_color`, `color`; docs/markdown.md section 2.5). `Editor::set_frontmatter_map` in `notecraft-markdown` is the new minimal-diff map-key rewrite (touched keys only, a block or map added only when needed, a map with no keys removed); `session/page_style.rs` maps `PageStyle` to the keys and back.
- Read: `load_canvas` and `load_canvas_from_journal` also read the style from the loaded text. Write: `Session::file_text` writes the keys before the canvas block, so the one save path, the recovery journal, and the outside-change check all see them. `SetPageStyle` marks the page dirty, so undo and redo save too. New pages made with Always Create Pages with Rule Lines are created with the `paper` key. Sample pages stay session-only.
- Not done: a `notecraft:` written as a flow map is neither read nor rewritten; page style is not part of a page's version history yet.

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
- A page title is its file name (docs/markdown.md section 2.3), so renaming renames the file. A frontmatter title that differs from the file name is read by the index refresh and shown in the page list (see the PR 1.2 notes); renaming still renames the file.
- Moving a page to another section also copies the attachments its text links to (found by scanning the text for `attachments/<name>`); the original stays for pages that still use it. The attachment store itself (hash prefixes, linked files) is PR 1.12. Subpage collapsing (feat/subpage-collapse, 2026-10-17): `page.collapse_subpages` and `page.expand_subpages` (undoable; a chevron on a parent row and a page menu item) keep `collapsed = true` on the parent's entry in `.section.toml`; the page list hides the rows below a collapsed parent, and collapsing while a subpage is shown selects the parent. A parent moves with its subpages (`page.move`, Move Up and Down, drag, and to another section), keeping their inner levels, in one undo step; the moved parent is lowered if it would sit more than one level under the page before it. Deleting a collapsed parent trashes its subpages with it (one trash slot per page, one undo step); deleting an expanded parent trashes only that page and promotes the subpages so levels stay valid. Sample (mock) notebooks still move a single page. The chevron row is indented 14 px more than rows without subpages.
- A section moved into a group keeps its name; a taken name gets a number. Moving checks the 8 level nesting limit and refuses a group moved into itself.
- UI (feat(ui) commit): context menus on notebook, group, section (tab and pane), and page rows; double-click or the Rename item renames in place (Enter commits, Esc cancels); drag a page to reorder it or onto a section; drag a section or group to reorder it, into a group, or onto the notebook for the top level; Move Up and Move Down in the page menu do what a drag does for the keyboard. The drag logic is tested through the position math and the commands it emits (`drop_index`, `move_page`, `move_section`); the headless harness does not drive a pointer drag.
- Section and notebook colors: a Color submenu with eight colors (the six defaults, teal, and gray) writes `color` to `.section.toml` or `notebook.toml`. A custom color picker is not built.
- Sample (mock) notebooks cannot be edited: the commands fail with "this is a sample notebook; open or create a notebook folder first" (shown as a toast). Open or create a notebook folder first. `page.move` still reorders and moves pages in memory there.
- The index (PR 1.2) needs no new code for structure edits: a rename or move is a removal and an addition for `Index::refresh`, which reads the folder. Tests check that the index agrees with a fresh scan after each kind of edit.
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

### PR 1.14 notes: what was built and the calls made

Built on `test/fuzz-parsers` (2026-10-16). FM-08 is scored P, not D.

- `fuzz/` is a separate crate (its own `[workspace]`, listed in the root `exclude`), so stable `cargo xtask ci` and the wasm check never build it. Seven targets: `markdown`, `frontmatter`, `containers`, `canvas_block`, `notebook_toml`, `section_toml`, `recovery_journal` (the plan named four; the container markers, the notebook file, and the journal are parsers too). `fuzz/seed.sh` fills `fuzz/corpus/` from `testdata/`.
- The checks live once in `fuzz/checks/*.rs` and are included by path from both the fuzz targets and two stable test files (`notecraft-markdown/tests/fuzz_props.rs`, `notecraft-storage/tests/fuzz_props.rs`). The tests use a seeded generator to mutate the testdata (flips, splices, truncation, hostile tokens), 3,000 cases per parser, about 30 seconds in a debug build. This is the only fuzzing that runs in CI.
- Not done: running each target for 30 minutes. The dev box has only the stable toolchain and no cargo-fuzz, so the targets were not built here. There is no scheduled workflow (the plan allows a manual run, documented in CONTRIBUTING.md under Fuzz the parsers).

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

Done (branch `feat/cli-notebook-commands`). Deviations and notes:

- Global options are taken out of the arguments wherever they stand (`--cmd` and `--page` values are skipped), so `commands --json` still works. `--log-level` is validated but nothing logs yet. `--quiet` drops summaries and successful command lines but keeps problems and the `text` output.
- `<page>` resolution is local to the CLI (`notebook.rs`): page id, notebook-relative path (with or without extension), path relative to the linking page's section (check only), file name, then the same ignoring case. Frontmatter aliases and Unicode normalization (docs/markdown.md section 5.3 steps 4 and 5) are not applied; Phase 2 should move resolution into a shared crate. Several matches are exit 2 for `text` and `run`, and a warning for a link in `check`.
- `text` uses a new `Document::plain_text_up_to` (the 16 KiB cap of `plain_text` is kept for the UI), with container markers removed through the markdown crate's own marker edits.
- `check` problem kinds: `unreadable_page`, `bad_text`, `page_too_large`, `bad_canvas`, `broken_link`, `missing_attachment`, `bad_settings`. Attachment paths come from `Session::attachment_path`. Page links in code are ignored. Exit 1 on any problem.
- `run --page` opens the notebook through `notebook_from_tree`, selects the page with `nav.page.select`, runs the commands, then `Session::save_dirty`. A failing command stops the run and nothing is saved. A conflict (file changed on disk since it was read) exits 5 and writes nothing. Without `--page`, `--notebook` is an error (exit 2) and `--page` with `--sample` is exit 2.

## PR 1.15: close Phase 1 and ship the alpha

Close-out docs done on chore/close-phase-1 (2026-10-18): rows re-scored, alpha gate Evidence filled, and the hands-on checklist for the maintainer is in docs/roadmap.md (numbered steps, each naming the rows it clears). The phase branch merged into `main` on 2026-10-18 (#55); #53 and #54 first brought the 0.0.1 release commit into it, and #54 had to be a merge commit because #53 was squashed. What is left of this PR: the maintainer runs that checklist, fixes what fails, then releases v0.0.2 (maintainer decision, 2026-10-18; this plan used to say v0.1.0). Original text: follow 00-conventions.md section 6, then release the alpha (docs/releasing.md). Fill every Evidence cell in the docs/roadmap.md alpha gate table first; do not release with an empty cell.

Also do the hands-on checks deferred from Phase 0 (maintainer decision, 2026-10-15; docs/gaps.md rank 25, 2-4 hours): launch the window and follow system dark mode and the OS UI font on macOS (SH-01, TH-06); follow system dark mode on Linux (TH-06); ribbon key tips on a physical keyboard (SH-05); Narrator on Windows and VoiceOver on macOS (SH-15); shortcuts using Cmd on a Mac (SH-14); the real OS file dialogs on all three systems (SH-12). Record each result in the row's evidence and fix what fails before tagging v0.0.2.

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
- v0.0.2 published.

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
| 2026-10-16 | 1 | PR 1.11 deviations and the eraser design as built. Hours unchanged. |
| 2026-10-16 | 1 | PR 1.11 follow-ups: eraser sizes and grazing point eraser decided and built; open questions closed. |
| 2026-10-16 | 1 | PR 1.13 done: deviations (trash layout, close as notebook delete, commands, sample notebooks read-only) and open questions noted under PR 1.13. |
| 2026-10-16 | 1 | PR 1.14 done: seven fuzz targets, shared checks run as stable mutation tests, 30-minute runs left as a manual step; notes under PR 1.14. |
| 2026-10-15 | 1 | PR 1.4 done: deviations (Editor API, canonical serializer scope, minimal save path in storage, corpus location) noted under PR 1.4. |
| 2026-10-15 | 1 | PR 1.2 done: deviations (native only, mtime and size then hash, titles from file names, worker not yet wired) noted under PR 1.2. |
| 2026-10-16 | 1 | PR 1.12 done: deviations (markdown image runs, picture bounds, link-in-place threshold, open command safety, clipboard path) noted under PR 1.12. |
| 2026-10-17 | 1 | PR 1.2 follow-up: navigation wired to the index (feat/nav-from-index); notes under PR 1.2. Hours unchanged. |
| 2026-10-17 | 1 | PR 1.16 done: CLI info, text, check, and run --page; notes under PR 1.16. |
| 2026-10-18 | 1 | PR 1.15 close-out docs: Phase 1 rows re-scored, alpha gate evidence filled, hands-on checklist in docs/roadmap.md; the release itself (tag, version) is not done; version question (v0.1.0 or v0.0.2) is open. |
| 2026-10-18 | 1 | Textured pens and the pencil notes added after PR 1.11 (format key, procedural textures, vertex coordinates, pen list, UI). Hours unchanged. |
| 2026-10-18 | 1 | Pictures and file attachments get places in the canvas block (placing at the click or drop point, dragging, saving); notes under PR 1.9 and PR 1.12 follow-up. |
| 2026-10-18 | 1 | Phase branch merged into main (#55) after #53 and #54; PR 1.15 note updated. |
| 2026-10-18 | 1 | Page style (rule lines, rule line color, page color, paper size) saved in the page frontmatter; notes under PR 1.9 follow-ups. |
| 2026-10-18 | 1 | PR 1.6 follow-up: bold text drawn with the OS bold face (feat/bold-face); the "Not done: a bold face" notes removed. |
| 2026-10-19 | 1 | PR 1.10 notes: IN-02 scored on Windows only; macOS and Linux pressure deferred until winit (maintainer decision). |

True multiply (2026-10-17, feat/highlighter-multiply): built as the `egui_wgpu` callback in `crates/notecraft-ui-egui/src/canvas/multiply.rs`. Each stroke is drawn with a MIN blend into a scratch layer (a stroke never darkens itself), multiplied into an accumulation layer (crossing strokes darken), and the accumulation layer is multiplied into the frame after the page text and images (rule lines and text show through). The app calls `install_multiply` at startup; the opaque pre-blend remains the fallback when it is not installed, on an sRGB surface, or with more than 128 highlighter strokes in view. New dependency: `egui-wgpu` 0.36 (MIT OR Apache-2.0, same release as eframe), ui-egui only; no `unsafe`. Also in this branch: Always Create Pages with Rule Lines (`draw.always_rule_lines`) and the Eyedropper reading text and paper colors.
