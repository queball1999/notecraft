# Architecture

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-17. Change: OneNote import of every OneNote file type (`.one`, `.onex`, `.onetoc2`, `.onepkg`) moved into Phase 3 with full fidelity (PR 3.16, 3.17; maintainer decision, 2026-10-16); PR 1.9 landed (.notecraft pages and saving ink; one save path with auto-save); PR 1.7: dirty tracking, auto-save, recovery journal, and outside edit handling (sections 3.2 to 3.4, 7); ink design (PR 1.10) added to section 4; PR 1.3: notecraft-markdown parses blocks with source spans; the canvas renders pages from session page text (the seam for PR 1.1); page text read from disk when a page is shown (FM-01 scored D); PR 1.2 landed (rebuildable notebook index); page text read from disk when a page is shown (FM-01 scored D). PR 1.8: sections 5 and 6 describe undo as built; also: PR 1.4: markdown write (span-replacement Editor, canonical serializer) and atomic page writes in storage; PR 1.10 follow-ups (anti-aliased ink edges, marked current pen, WordCraft survey notes); Ink Replay scheduled in PR 3.2 (IN-13); PR 1.11 follow-ups (eraser sizes, grazing point eraser); PR 1.11: sections 10.3 and 10.4 describe the highlighter and erasers as built; section 5.1: structure editing and how undo works on disk (PR 1.13); Ink Replay scheduled in PR 3.2 (IN-13). PR 1.5: text editing in containers; Draw tab parity (feat/draw-tab-parity): OneNote Draw groups, editable pen list saved in settings, pen menu with palette and Eyedropper, Format Background, smooth curves for ink, Rule Lines thumbnail margin fix; attachment storage note (PR 1.12).


How NoteCraft is built. Until Phase -1 lands, this describes the target; each phase updates it to match the code.

## 1. Layers

NoteCraft follows the ArtCraft layered pattern. A crate may depend only on crates in lower layers. `cargo xtask layers` checks this on every CI run, and fails if anything below L6 depends on `egui`, `eframe`, `winit`, or `rfd`.

```text
+--------------------------------------------------------------------------+
| L7  apps          notecraft (desktop)  notecraft-cli  notecraft-web (P4) |
+--------------------------------------------------------------------------+
| L6  front end     notecraft-ui-egui                                      |
+--------------------------------------------------------------------------+
| L5  engine        notecraft-engine  (session, commands, undo,            |
|                   clipboard, export)                                     |
+--------------------------------------------------------------------------+
| L4  canvas        notecraft-canvas  (ink, shapes, layout, hit            |
|                   testing, zoom and pan)                                 |
+--------------------------------------------------------------------------+
| L3  markdown      notecraft-markdown (parse, render, page links,         |
|                   note tags, equations)                                  |
+--------------------------------------------------------------------------+
| L2  model         notecraft-model   (notebook tree, page, canvas         |
|                   objects)                                               |
+--------------------------------------------------------------------------+
| L1  storage       notecraft-storage (files, index.sqlite, watch,         |
|                   auto-save, attachments)                                |
+--------------------------------------------------------------------------+
| L0  core          notecraft-core    (geometry, color, units,             |
|                   strokes, input events)                                 |
+--------------------------------------------------------------------------+
```

Plus `xtask` (build tooling, not shipped) and `testdata/`. The MCP server (PR 3.15, docs/mcp.md) would be a new L6 crate `notecraft-mcp` beside the UI, started by `notecraft-cli mcp`.

## 2. Crates

| Crate | Layer | Owns | Key dependencies |
|---|---|---|---|
| `notecraft-core` | L0 | `Point`, `Vec2`, `Rect`, `Color`, `Stroke`, `InputEvent`, units (px96) | `serde` |
| `notecraft-storage` | L1 | notebook folder scan, page file read and write, `index.sqlite`, file watcher, auto-save timer, attachment copy | `rusqlite` (bundled), `notify`, `craft-atomic`, `craft-settings` |
| `notecraft-model` | L2 | `Notebook`, `Section`, `Page`, `Container`, `InkStroke`, `Shape`, `ImageObj`, `FileObj` | `notecraft-core` |
| `notecraft-onenote` | L2 | (Phase 3) read OneNote `.one`, `.onex`, `.onetoc2`, and `.onepkg` from bytes into `notecraft-model`; no file I/O | `notecraft-core`, `notecraft-model`, a pure-Rust CAB reader |
| `notecraft-markdown` | L3 | markdown to blocks, blocks to markdown with minimal diffs, page link parse and resolve, note tags, equation layout | `comrak` |
| `notecraft-canvas` | L4 | stroke smoothing, eraser, shapes, ink to shape, container layout, hit testing, viewport | `kurbo` |
| `notecraft-engine` | L5 | `Session`, command registry, undo/redo, clipboard model, PDF and HTML export | the crates above |
| `notecraft-ui-egui` | L6 | ribbon, File backstage, navigation, canvas widget, themes, dialogs | `egui`, `eframe`, `craft-ui` |
| `notecraft` | L7 | desktop app: window, logging, minimal control channel (tests), printing, macOS quit | `notecraft-ui-egui`, `craft-print`, `craft-quit` |
| `notecraft-cli` | L7 | `--version` only before full parity (the release workflow packages and runs it) | `notecraft-engine`, `clap` |
| `notecraft-web` | L7 | browser build (Phase 4) | `notecraft-ui-egui`, `eframe` web |

## 3. Data flow

### 3.1 Opening a notebook

```text
user picks folder
  -> notecraft-storage: scan folders, read notebook.toml and .section.toml
  -> notecraft-storage: open index.sqlite, diff mtimes and hashes, queue changed pages
  -> notecraft-model: build Notebook tree (navigation can draw now)
  -> background worker: parse queued pages (notecraft-markdown), update links, tags, search index
  -> notecraft-storage: start file watcher
```

Navigation must appear in under 500 ms for 1,000 pages (SCRD section 11). Page bodies are parsed lazily, except in the background reindex.

The index (PR 1.2) is `notecraft_storage::index::Index`, native only (SQLite compiles from C, so the module is cfg-ed out on wasm32 and `cargo xtask wasm` stays green). `Index::open_or_rebuild` deletes and recreates a file that is not a database, fails `PRAGMA quick_check`, or has a newer `user_version`. `Index::refresh(root)` scans the folder, stats every page, and reads and hashes only pages whose modified time or size changed, in one transaction; it reports added, changed, and removed page ids so the Phase 2 reparse knows what to visit. `IndexWorker` runs open and refresh on a named thread and hands back a `WorkerReport` through a channel. The sidebar reads sections and pages straight from the tables.

### 3.2 Editing

```text
ribbon click, shortcut, gesture, or control channel (tests)
  -> command id + args
  -> notecraft-engine: registry looks up the command, runs it inside the command guard
  -> command mutates notecraft-model through an undoable transaction
  -> engine marks the page dirty and emits change events
  -> notecraft-ui-egui repaints; auto-save timer (30 s, and on focus loss) flushes dirty pages
  -> notecraft-markdown serializes with minimal diffs; notecraft-storage writes via craft-atomic
```

Dirty tracking (PR 1.7) is a set of page ids in the session. The call an editing path makes is `Session::mark_page_dirty(page_id)`; it ignores pages with no file (the sample notebooks). Anything that goes through `Session::commit`, `undo`, or `redo` already calls it for page text (`ReplaceText`) and strokes (`InsertStroke`, `DeleteStroke`), so PR 1.5 only has to call `edit_page_text` or `commit`; a path that changes page content some other way calls `mark_page_dirty` itself. `Session::save_page(id)` and `save_dirty()` write the page text (strokes join in PR 1.9) with `write_page_text` and return a `SaveOutcome`. Rules: a page whose `PageNotice` is read-only is never written; a page whose file changed since NoteCraft last read or wrote it is not written over (the file is read first and compared by hash; a difference becomes a conflict, section 3.3); a failed write leaves the page dirty.

`notecraft_engine::io::SessionIo` is the clock and the threads. The front end calls `pump(session, now, focused)` once per frame and `flush(session)` on quit. It saves every 30 s (`AUTOSAVE_EVERY`), when focus is lost, and when another page is shown; it keeps the recovery journal fresh every 2 s; and it owns the watcher and the page reader. `AppShell::with_io` turns it on (the desktop app does; tests do not, so they read pages inline and never write). The quit path is the close request seen in `AppShell::logic` plus the app's `on_exit`, both calling `flush_unsaved`; on macOS, `on_exit` ends the process after the flush (craft-quit). Saves run on the UI thread (a save is one small atomic write); reads do not.
### 3.3 Outside edits

The watcher (`notify`, `notecraft_storage::watch::FolderWatcher`) reports changed paths after the folder has been quiet for 300 ms. NoteCraft's own saves show up too; the session tells them apart by comparing a hash of the file text with the hash of what it last read or wrote, so they change nothing. `Session::note_outside_change(path)` maps a path to a page and queues a read; the read happens on the reader thread (`notecraft_storage::reader::PageReader`), and `Session::finish_read` applies it.

- A page with no unsaved edits reloads. Its undo steps refer to the old text, so they are dropped; this is logged, with no toast (maintainer decision, 2026-10-15).
- A page with unsaved edits keeps the user's text. The outside version is held (`Session::current_conflict`) and a bar over the page offers Keep my version (`file.keep_my_version`: the next save replaces the file) or Use outside version (`file.use_outside_version`: the text is replaced, undo steps dropped). The page is not saved while the question is open, and its text stays in the recovery journal. If the outside version equals the user's text there is nothing to ask. Nothing is overwritten silently. A side-by-side Compare is not built: OneNote has none.
- A page that is shown but not yet read, or that failed to read, just reads again when it is next shown.
- Page loading uses the same reader thread: with `Session::set_background_reads(true)` (which `SessionIo` sets), `load_current_page` queues a `ReadRequest` instead of reading inline, and the page appears when the answer arrives. With it off (tests, the command line), reads are inline.

### 3.4 Crash recovery

`notecraft_storage::Journal` keeps one JSON file per unsaved page in `recovery/` inside the settings folder, named by a hash of the page file's path, written with craft-atomic. `SessionIo` refreshes an entry every 2 s while the page text changes and deletes it when the page is saved, replaced by the outside version, or discarded; on quit it also journals any page that could not be saved (for example one in conflict). When a notebook folder is opened, entries for pages in it are compared with the files; an entry that equals its file is deleted, any other is offered (`Session::pending_recovery`) on a bar with Restore (`file.restore_unsaved`: the text comes back as unsaved changes) and Discard (`file.discard_unsaved`). Entries for notebooks that are not open wait. The journal reads at most 2,000 entries and skips any that do not parse.

## 4. Commands

- Ids are `area.verb` strings, for example `page.new`, `ink.set_color`, `view.zoom_in`. They are stable once released.
- Each command declares: id, label, description, argument schema (serde), whether it is undoable, whether it is destructive, and its default shortcut.
- A command not implemented yet is registered with `status = ComingSoon`. The UI shows the badge and a toast; the control channel returns a clear "not implemented" error.
- Every action being a command is what keeps the CLI and MCP server (and post-parity options such as a command palette) cheap to add later without touching features.
- Implemented in `crates/notecraft-engine/src/cmd/` (PR 0.2): `ids.rs` declares every id, `builtin.rs` holds the specs, and `Registry::run(&mut Session, id, args)` checks the JSON arguments and returns errors with stable codes (`not_found`, `coming_soon`, `invalid_args`, `internal`). The egui shell's `AppShell::dispatch` turns an error into a toast.
- The canvas (PR 0.7) splits view state the same way: the zoom and each page's style (rule lines, page color, paper size) live in the session and change only through commands, so Ctrl+wheel and pinch dispatch `view.zoom` like the ribbon does. The scroll position is front-end state in `notecraft-ui-egui/src/canvas`. The page-to-screen math is `notecraft_canvas::Viewport`, with no egui types.
- File commands (PR 0.8) take an optional `path`. A button sends them without one, and the shell asks its `FilePicker` (the OS dialogs through `rfd` in the desktop app, a fake in tests) and runs the command again with the chosen path; canceling runs nothing. The control channel always passes a path. The recent list lives in the session and the app saves it in `config.toml`.
- Shortcuts (PR 0.9) are data in the engine: `cmd::shortcuts::default_map()` turns each command's `default_shortcut` (plus a few bindings with arguments, such as Ctrl+2 to Ctrl+9 for note tags) into key chords, most modifiers first. The egui shell matches them each frame, with Ctrl as egui's command modifier (Cmd on macOS), and dispatches the commands; nothing is matched while a text field has focus. Tests reject duplicate chords and bindings with invalid arguments.
- Ink saving (PR 1.9) uses the one save path, `Session::save_page(id)` (auto-save, dirty tracking, the outside-edit check, and the recovery journal from PR 1.7 apply to ink as to text): it writes the page text and its strokes together, and the journal holds that same whole-file text. A `.notecraft` page is held as its whole file text, and only the canvas block's `"ink"` value is rewritten (`notecraft_storage::write_ink`). A `.md` page with a stroke is promoted to `.notecraft` (the page id changes and the session re-keys it); `page.convert_to_markdown` goes back. `PageSource::visible_text` hides the canvas block from the layout.
- Ink (PR 1.10) follows the same split. The front end turns egui events into `notecraft_core::PointerSample`s (page units, time, optional pressure, source kind) and feeds a `notecraft_canvas::ink::StrokeBuilder`, which smooths with a one-euro filter and gives a stroke a speed-based width when the device reports no pressure. When the pen lifts, the canvas sends `ink.add_stroke { points }`; the session keeps one `InkLayer` (strokes in paint order plus a grid index) per page id, and the active tool and pen (`draw.type`, `draw.pen`, `draw.color`, `draw.thickness`). Drawing is a renderer-agnostic triangle list (`tessellate`) that the egui layer turns into a `Mesh`. The `Stroke` type serializes to the canvas block's `ink` entries (docs/markdown.md section 3.4); PR 1.9 writes it to the file. The Draw tab keeps a list of pens and highlighters in `InkState` (`draw.tool_select`, `draw.tool_add`, `draw.tool_delete`); the app saves it in `config.toml` under `[draw]`. The mesh runs a Hermite curve through the stored points (`smooth_centerline`; the saved points are not changed) and the stroke in progress is tessellated in finished chunks so a frame only works on the newest points.

- Page text (PR 1.3) enters the session through `Session::set_page_source(page_id, text)`, which is not a command: `Session::load_current_page` calls it after every command when the page shown comes from a notebook folder and has not been read yet (read through `notecraft_storage::read_page_file`, bounded at 64 MiB; a cut or unreadable page gets a `PageNotice` the canvas draws under the title). The canvas parses the text of the page shown with `notecraft_markdown::parse` (frontmatter, then blocks that keep their source byte spans), caches the result per page and text revision, and draws the blocks in flow layout from theme tokens. The markdown crate has no egui types; `canvas/flow.rs` is the only place blocks meet the painter.

- Page writing (PR 1.4): `notecraft_markdown::Editor` collects span replacements against the page text and `finish()` splices them, so bytes outside the edited spans never change; `canonical` serializes new blocks. `notecraft_storage::page_file::write_page_text` writes the result through `craft-atomic` and skips the write when the bytes are unchanged. Auto-save, the recovery journal, and the conflict check landed in PR 1.7 (sections 3.2 to 3.4).

## 5. Undo and redo

A change to the document is a `Transaction` (`notecraft_engine::undo`): a labeled list of reversible `Op`s. `Session::commit` applies the ops in order, then records the transaction. If an op does not fit the document as it is (a stale stroke index, a text range that no longer matches), the ones already applied are reversed and nothing is recorded. Undo applies the inverses of a step in reverse order; redo applies the step again. Today's ops are `InsertStroke`, `DeleteStroke`, `ReplaceText`, `SetPageStyle`, `MovePage`, and `Fs` (the file operations of section 5.1); each later PR adds the ops its commands need.

- Stacks: history is per page, capped at 500 steps or 64 MiB (an estimate of each op's size), oldest dropped first and the newest always kept. A second stack holds notebook-level transactions (`Scope::Notebook`: moves, renames with link updates) that touch several pages and undo as one step. Starting a new step clears the redo steps of its own stack only.
- Order: Undo takes the newest step among the shown page's stack and the notebook stack, so steps undo in the order they happened, from any page. Redo mirrors it. A step that can no longer be reversed (the file was reloaded under it) is dropped.
- Merge: a transaction marked `merge_into_previous()` joins the previous step of its stack, so the two undo together. The stroke eraser (PR 1.11) uses it: the first stroke a drag erases starts a step and the rest join it, so one drag is one undo step. With no previous step it starts one.
- Typing: `Session::edit_page_text` is the engine call text editing (PR 1.5) makes. With `typing` set, consecutive insertions in one place coalesce into one step up to a word boundary (a word and the space after it), runs of Backspace or Delete coalesce, and a run is capped at 512 bytes. Another step, a moved caret, or an undo starts a new step.
- Commands: `edit.undo` and `edit.redo` (Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z) are not themselves steps. The Undo and Redo buttons at the right of the tab strip run them and are disabled while their stack is empty (`Session::can_undo`, `can_redo`). The History tab keeps OneNote's contents (unread notes, authors, page versions, recycle bin), so the stack is not listed there; `History::undo_steps` and `redo_steps` return the labeled steps for any view that wants them.
- Opening a different set of notebooks clears the history.

### 5.1 Structure editing (PR 1.13)

Creating, renaming, moving, coloring, and deleting notebooks, section groups, sections, and pages are ordinary transactions (`Scope::Notebook`) made of `FsOp`s (`notecraft_engine::undo::FsOp`), so each edit is one undo step that includes its effects on the files. The folder is the truth, and the tree in memory follows it.

- Ops: `Create { rel, content, slot }` (a folder, a file with text, or a copy of a file), `Move { from, to }` (a rename or a move of a file or folder), `WriteText { rel, before, after }` (a settings file: `.section.toml` for page order and levels, section order, and colors; `notebook.toml` for the display name, color, and section order), and `Unlist` / `Relist` (closing a notebook and opening it again). Paths are relative to the notebook folder, checked to hold only plain components, so an op cannot leave the notebook.
- Inverses are exact: `Move` swaps its paths; `WriteText` swaps `before` and `after` (the old file text is read when the edit is planned, `None` when the file did not exist); `Create` is undone by moving the made entry into its trash `slot`, and a redo moves it back from there, so what was created comes back byte for byte. `Unlist` and `Relist` swap.
- Order of work: the plan builds the ops against the tree as it is, then `Session::commit` applies them one by one. If one is refused (a target exists, a source is gone, a path leaves the notebook), the ones already applied are reversed with their inverses and nothing is recorded. Every file step checks first and never overwrites: a rename or move onto an existing name fails, and the commands choose a free name (`Plan 2`, compared ignoring case) before they try. A rename that only changes letter case goes through a temporary name, so volumes that ignore case still accept it. The settings files are written with `craft-atomic`.
- Failures and consistency: an op that changes the disk marks its notebook dirty. After a step is applied, rolled back, undone, or redone, `Session::resync_dirty` reads the dirty notebooks from disk again and replaces their trees, then fixes the selection. So even if an inverse fails half way (someone else changed the files meanwhile), the tree shown matches the files, the broken step is dropped from the history (the same rule as other stale steps), and nothing is lost: files are only ever moved, never deleted.
- Ids are paths (`<notebook id>/<path>`), so a rename or move changes them. `Move` calls `Session::rekey_ids`, which renames everything stored per id: page text, styles, notices, ink, the selection, and the history (page stacks and the ops inside them), so a text edit made before a rename still undoes after it, and undoing the rename gives the old ids back.
- Delete goes to the trash folder: `.notecraft/trash/<seconds>-<n>/<name>` with an `origin.toml` beside it (original path, kind, index, level, time). It is a `Move` plus a `WriteText`, so Undo is the exact inverse. OneNote's Recycle Bin, its list and restore UI, and the 30 day purge are Phase 3 (PR 3.5); nothing in PR 1.13 reads the trash back except Undo and Redo. Closing a notebook (`file.close_notebook`) is the notebook-level delete: its folder is never touched.
- Moving a page to another section moves its file and copies the files in the old section's `attachments/` folder that its text links to (the link text stays valid because it is relative); the originals stay for other pages. Moving inside a section only rewrites the order. A section or group moves as a folder, with everything in it. Nesting stays within 8 levels, and a group cannot move into itself.
- A notebook with no folder (the sample notebooks) cannot be edited; the commands say so. `page.move` is the exception: it moves pages in memory there, as Phase 0 did.
- New pages are `.md` files with no text, named `Untitled Page` (numbered when taken); sections and groups get a color from the default list and a `.section.toml`, and a new section also gets one `Untitled Page` in the same step; the first write to `notebook.toml` puts in the notebook's current id and name, so ids stay stable. `file.new_notebook` creates the folder and `notebook.toml` and opens it; like Open it is not an undo step.

## 6. Never crash

- Lints: every `lib.rs` denies `unwrap_used`, `expect_used`, `panic`, `unimplemented`, `todo`, `unreachable`. `unsafe` is forbidden workspace-wide, so OS integrations (pen pressure, spell check, camera) go through safe wrapper crates.
- Parsers are lenient and bounded (limits in docs/markdown.md section 9). Fuzz targets for the markdown, frontmatter, canvas, and toml parsers live in `fuzz/` (nightly, outside the workspace), and the same checks run in `cargo test`.
- The command guard wraps each command in `std::panic::catch_unwind`. If a dependency panics anyway, the guard rolls back the transaction, logs the error, and shows a toast. The document stays as it was before the command.
- The guard rolls back by restoring a snapshot of the session taken before the command. The snapshot includes the undo history, so a command that panics or fails after recording a step leaves both the document and the undo and redo stacks as they were. The snapshot stays cheap because ink layers and history steps are shared (`Arc`), not copied; page text is a string per page, so a very large notebook is a reason to revisit this (a transaction-level rollback is the alternative, since every change is already a reversible `Op`). A command that returns an error is rolled back the same way.
- All writes use `craft-atomic`. A crash mid-save leaves the old file intact.

## 7. Threads

| Thread | Does |
|---|---|
| UI (main) | egui frame, input, command dispatch |
| IO worker | page file reads (PR 1.7), index updates, reindexing |
| Watcher | file system events, debounced and forwarded to the IO worker |
| Control listener (tests only, `--control`) | accepts connections, forwards commands to the UI thread through a channel |

Commands always run on the thread that owns the session (the UI thread in the desktop app). Other threads send requests and wait for replies on a channel.

## 8. Settings and logging

- Settings live in `config.toml`, read and written through `craft-settings`, in the platform settings folder (`%APPDATA%\NoteCraft`, `~/Library/Application Support/NoteCraft`, `$XDG_CONFIG_HOME/notecraft`).
- Logs follow SCRD section 12: `logs/notecraft.log`, rotated each launch, 16 MiB cap, `RUST_LOG` override, `NOTECRAFT_NO_PREFS=1` disables file logging and settings writes. Note content is never logged. Implemented in `apps/notecraft/src/logging.rs` (PR -1.6); the settings folder is computed there until `craft-settings` arrives in Phase 0.

## 9. Quality gates

`cargo xtask ci` (also the CI gate, see `.github/workflows/ci.yml`). All eight gates are implemented (PRs -1.2 to -1.4).

| Step | Tool |
|---|---|
| Format | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Test | `cargo test --workspace` |
| Text | `cargo xtask text` (ASCII only, AGENTS.md section 2) |
| Assets | `cargo xtask assets` (ATTRIBUTION.md rows) |
| Layers | `cargo xtask layers` |
| WASM | `cargo xtask wasm` (L0 to L5 build for `wasm32-unknown-unknown`) |
| Parity | `cargo xtask parity --check` (checklist summary and build plan hour totals are current) |

Other workflows mirror the related craft apps: `printing.yml` (craft-print on all three OSes), `freebsd.yml` and `windows-arm64.yml` (nightly and on the `release` branch), `packaging-lint.yml`, and `release.yml` (see docs/releasing.md).

## 10. Ink and drawing design

How ink works today (PR 1.10) and how the next ink PRs build on it. The WordCraft survey of 2026-10-15 informed this section: WordCraft is a related craft app with the same patterns, and its Draw tab was read for ideas (no code was copied; if a later PR ports code, it adds an ATTRIBUTION.md row first).

### 10.1 Pipeline

```
egui events -> PointerSample (core, L0) -> StrokeBuilder (canvas, L4: one-euro filter,
  speed-based width when no pressure) -> ink.add_stroke -> InkLayer per page (strokes in
  paint order + GridIndex) -> tessellate (canvas: triangle list + Edge tags) -> egui Mesh
  (ui-egui: color, zoom, AA fringe)
```

Engine crates never see egui. The tessellator returns page-unit positions, indices, and one `Edge` tag per vertex; the egui side picks the color and the screen-space fringe.

### 10.2 Anti-aliased edges

Meshes are drawn with a one-pixel fringe so edges fade instead of stair-stepping, at any zoom and display scale.

- Tessellation (`ink/tessellate.rs`) adds a strip of quads along every outline chain: both ribbon sides, both caps, and, at a sharp turn, only the wedge of the join disc that shows past the ribbon (the rest of the disc lies inside the body and gets no fringe). Each outline vertex is tagged `Edge::Rim { dir, radius }` (outward direction and distance from the centerline) and gets an `Edge::Outer(dir)` twin at the same place. Everything else is `Edge::Inside`.
- The egui side (`canvas/ink.rs`, `append`) moves a rim vertex up to half a device pixel inward (never past the centerline, so lines under one pixel wide do not invert) and an outer vertex half a pixel outward with transparent color. The result is a one-pixel ramp centered on the true edge. Half a device pixel is `0.5 / pixels_per_point` egui points; the page-to-screen map only scales and shifts, so directions carry over unchanged.
- The fringe is built in page units, so cached meshes do not depend on zoom or display scale; only the cap segment count does.
- It is correct for opaque strokes only: the fringe quads overlap the body and must hit the same color. A translucent stroke asks for no fringe (`TessParams::fringe`), and the mesh cache keys on it.

Why not egui's own feathering: epaint feathers filled convex paths and stroked polylines, but a variable-width ribbon with round joins is neither one convex path nor a constant-width polyline, so the stroke would need its outline polygon (a self-overlapping union) first. Tagging the mesh keeps the engine free of egui and the cost to one extra vertex per outline vertex.

### 10.3 Highlighter (PR 1.11)

OneNote's highlighter multiplies over the page. egui has no multiply blend in its painter, and a translucent mesh darkens wherever its own triangles overlap. The plan:

1. Draw highlighter strokes in a pass of their own, behind the text containers and above the paper, so dark text stays dark over the color (the same result as multiply for dark text on a light page). WordCraft also draws its highlighter behind text and its pens in front.
2. Draw each highlighter stroke as an opaque mesh in the color pre-blended with the paper color at the highlighter's alpha (about 0.5), so triangle overlap inside a stroke cannot show and the AA fringe stays valid. The pre-blend uses the theme's paper color at paint time, so it follows dark themes.
3. If overlapping highlighter strokes must darken each other (true multiply), or the highlighter must tint images, add an `egui_wgpu` paint callback with a multiply blend state later; the stroke type and file format do not change.
4. The stored stroke keeps `tool: "highlighter"`, the picked color, and its width; the blend is a rendering choice (docs/printing.md specifies multiply at 50% opacity for PDF).

As built (PR 1.11): `draw.highlighter { color?, width? }` picks the tool (default yellow, 8 pt; it has its own style, so the pen is untouched, and `draw.color` and `draw.thickness` change whichever of the two is the tool). The canvas captures it like the pen and sends `ink.add_stroke`, which stamps `tool: highlighter`. `canvas/widget.rs` runs three steps each frame: capture, then the highlighter pass (`InkUi::paint_highlights`, after the paper and before the title and text), then the pen pass (`paint_pens`, after the text). The pass blends each stroke's color with `paper::paper_color` (the theme page color under the page's own color) at 0.5, or at the stroke's own alpha when the file stored one, and always asks for the fringe. Known gap: because the mesh is opaque, the rule lines under a highlighter stroke are covered, where true multiply would show them; the paint callback in step 3 fixes that without a format change.

### 10.4 Erasers, lasso, and undo (erasers: PR 1.11; lasso: PR 3.3 design)

- Hit testing is `InkLayer::hit_test` and `topmost_at` over the `GridIndex` (distance from the point to each stroke segment, widened by half the stroke width plus a pointer reach in screen pixels). This is already better than a linear scan; keep it for both erasers.
- Stroke eraser: one drag is one undo step. The first stroke it removes starts a step and later ones join it (WordCraft does this with a `join_next_undo` call; PR 1.8's transactions should take a "merge with the previous step" flag for the same effect).
- Point eraser: remove the points under the eraser circle and split the stroke into the runs that remain, each keeping color, width, and pressure; runs shorter than two points are dropped.
- As built (PR 1.11): one command, `ink.erase { points, radius, mode?, join? }` (undoable), does both erasers; `draw.eraser { mode? }` only picks the tool (`stroke` or `point`, the last one kept). The canvas sends one `ink.erase` per frame of a drag with the path the pointer took (continuing from the last frame's end), and the radius in page units is the reach in screen pixels divided by zoom: 6 px for the stroke eraser (past the stroke edge), and 5, 10, or 20 px for the Small, Medium, and Large point erasers (`draw.eraser { size }`). `notecraft-canvas::ink::erase` samples the path (spacing at most half the radius, at most 512 centers) so a fast drag cannot step over a stroke, then either collects the touched stroke ids (`strokes_touched`, over `InkLayer::hit_test`) or cuts them (`split_stroke`: the circle is solved against each segment with the drawn half width, which varies with pressure, added to the radius, so a graze between points cuts; cut ends sit where the circle meets the outline, runs under two points are dropped, pressure is interpolated). The engine turns that into `DeleteStroke` and `InsertStroke` ops, working from the topmost stroke down so paint-order indexes stay right; a split needed no new op. New runs get fresh ids and sit where the stroke was. The first batch of a drag (`join: false`) is its own undo step; later batches (`join: true`) use `Transaction::merge_into_previous()` only while the step just before is an erase of the same drag (`History::erase_chain`, cleared by any other recorded step, undo, or redo), so a stray join can never swallow an earlier pen stroke. Touching nothing records nothing.
- Lasso (PR 3.3): a stroke is selected when at least 60 percent of its sampled points fall inside the loop polygon (WordCraft's rule, which beats "bounding box inside" and "any point inside" for loose loops); thin long loops are down-sampled to a point cap before testing. Selected ink moves, recolors, and deletes as one undo step each.

### 10.5 Ink to Shape (PR 3.2 design)

WordCraft's `crates/engine/src/cmd/ink_shape.rs` is a bounded, dependency-free geometric recognizer, which is the approach the plan calls for: a stroke is a line when it stays within 6 percent of its chord, an ellipse when the RMS of the normalized radius is low, otherwise a polygon of at most 8 corners found with an iterative Ramer-Douglas-Peucker pass plus merging of crowded and near-straight corners, with arrows recognized as a shaft plus two barbs. NoteCraft ports the idea to `notecraft-canvas` (L4) over `InkPoint` and adds what OneNote needs on top: a confidence threshold with a Draw tab toggle, rotated rectangles and ellipses (WordCraft snaps to upright ones), a double arrow, and the shape model of PR 3.1 as the output. A port of its code carries an ATTRIBUTION.md row (MIT OR Apache-2.0, source files named); tests use deterministic wobble like its `around` helper.

### 10.6 Other adoptable pieces

| Piece | Design |
|---|---|
| Custom pens (Add Pen) | A pen is `{ kind, color, width }`. The gallery keeps up to 24, saved through `craft-settings`; a hand-edited settings file loses the bad pens and never the rest (lenient deserializer). PR 3.1, inside its estimate. |
| Ink Replay | Strokes carry their drawing order (paint order already is that); replay draws each stroke up to a fraction of its length at about 400 units per second with a short gap between strokes, with a transport bar. Needs no format change. OneNote has it; docs/target-app-parity.md lists it as not planned for 1.x, so it needs a maintainer decision before it gets a PR. |
| Caps | WordCraft caps a stroke at 5,000 points and a page at 2,000 strokes. NoteCraft keeps 20,000 points and 100,000 strokes (docs/markdown.md section 9) and simplifies with Ramer-Douglas-Peucker on import. |
| Live preview | WordCraft paints the live stroke as a constant-width polyline and the committed one later in a different path. NoteCraft paints both with the same tessellator, so a stroke does not change look on pen-up. |

## 11. Text editing design (PR 1.5)

- The page source text is the document. The caret and selection (`TextCursor`, byte offsets on character boundaries) live in the session, so typing, selecting, and clicking are all commands: `text.set_caret`, `text.move`, `text.insert`, `text.delete`, `text.enter`, `text.tab`, `edit.cut`, `edit.copy`, `edit.paste`, `edit.paste_text_only`, `edit.select_all`, `edit.delete`, and `container.new`, `container.move`, `container.resize`. The command handlers are in `notecraft-engine` (`cmd/text.rs`); the pure text logic (clusters, words, lines, Enter and Tab rules, the raw region) is in `notecraft-canvas::text`; egui only maps events to commands (`canvas/keymap.rs`, `canvas/editing.rs`).
- Every edit is one splice through `Session::edit_page_text`, or a `Transaction` when it also changes a container (new, remove empty, move, resize). That gives one undo path, word-sized typing steps, the dirty flag for auto-save (`Session::is_page_dirty`, `dirty_pages`, `mark_page_saved`), and a splice on the original bytes, so saving touches only what changed. A replacement of a whole block is not needed: a splice is smaller than the block spans of PR 1.4.
- Live rendering: the lines of the caret or selection, grown to whole blocks, are laid out as raw markdown; every other block stays styled. A click on a styled block lays the block out raw off screen to find the offset (`flow::probe`), so the caret lands where the click was.
- IME: the composition (`Ime::Preedit`) is kept in the canvas state, drawn underlined at the caret inside the raw galley, and never written to the page. `Ime::Commit` is one `text.insert`. The IME candidate window is placed from the caret rectangle.
- Guards: the caret never sits in the frontmatter or on a container marker line; an edit that would cut a marker in half or leave one off the start of a line is refused. A page with a read-only `PageNotice` or a disk page that was never read is not editable.
- Containers are marker lines (docs/markdown.md section 3.3) appended to the page; geometry is kept in the session (`SetContainer` op) until PR 1.9 stores it in the canvas block. A container the caret leaves with no text is removed, as in OneNote.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Target layers, crates, data flow, commands, undo, never-crash design, threads, gates. Same day: re-scoped to OneNote parity (no notecraft-mcp crate, no graph, palette, or plugin loader). |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | Section 9 marks which ci steps exist. |
| 2026-10-14 | -1 | Section 9: text, assets, and layers gates implemented. |
| 2026-10-14 | -1 | Section 9: every cargo xtask ci gate implemented. |
| 2026-10-14 | -1 | Section 8: logging implemented in apps/notecraft/src/logging.rs. |
| 2026-10-14 | 0 | Sections 4 and 6: command registry and panic guard implemented (PR 0.2). |
| 2026-10-14 | 0 | Section 4: canvas zoom and page style are session state; scroll position is front-end state (PR 0.7). |
| 2026-10-14 | 0 | Section 4: file commands without a path ask the front end picker; recents in the session (PR 0.8). |
| 2026-10-14 | 0 | Section 4: the default shortcut map lives in the engine; the shell matches and dispatches it (PR 0.9). |
| 2026-10-15 | 0 | CLI and MCP server are in the build plan, not post-parity. |
| 2026-10-15 | 1 | Section 4: ink capture, strokes, and pen commands (PR 1.10). |
| 2026-10-15 | 1 | Section 4: page text enters through `Session::set_page_source` (one page id and its source text; the folder reader calls it in PR 1.1); the canvas parses with `notecraft_markdown::parse`, caches the result per page and text revision, and lays blocks out in flow layout (PR 1.3). |
| 2026-10-15 | 1 | Section 4: page text is read from disk by `Session::load_current_page` (storage `read_page_file`), with a `PageNotice` for cut or unreadable pages. |
| 2026-10-15 | 1 | Index design (PR 1.2): rebuild on corruption, incremental refresh, background worker, native only. |
| 2026-10-16 | 1 | Section 4: ink is saved by `Session::save_page` into the canvas block (PR 1.9), and a markdown page with ink is promoted to `.notecraft`. |
| 2026-10-15 | 1 | Section 4: page writing is span replacements (`Editor`) plus `page_file::write_page_text` over craft-atomic (PR 1.4). |
| 2026-10-15 | 1 | Sections 5 and 6: transactions, stacks, merge, typing coalescing, notebook-level steps, and the guard restoring the history (PR 1.8). |
| 2026-10-15 | 1 | Section 10: ink and drawing design (anti-aliased edges, highlighter, erasers, lasso, Ink to Shape, custom pens), after a survey of the related WordCraft app. |
| 2026-10-16 | 1 | Sections 3.2 to 3.4 and 7: dirty tracking, timed and event saves, the recovery journal, the folder watcher, and page reads on a worker thread (PR 1.7). |
| 2026-10-16 | 1 | Sections 10.3 and 10.4: the highlighter pass, the pre-blended opaque meshes, `ink.erase`, the eraser reach, point-eraser splitting, and the drag-as-one-step rule as built (PR 1.11). |
| 2026-10-16 | 1 | PR 1.11 follow-ups: eraser sizes and exact point-eraser cutting as built. |
| 2026-10-16 | 1 | Section 5.1: structure editing as file operations in notebook transactions, undo and failure handling, trash decision (PR 1.13). |
| 2026-10-16 | 1 | PR 1.5: new section 11 records the text editing design (commands, one undo path, raw region, IME, guards). |
| 2026-10-16 | 1 | Ink section: the pen list in InkState and settings, Hermite curves in the mesh, chunked preview (Draw tab parity). |
| 2026-10-16 | 1 | Attachment storage (PR 1.12): attachments are copied under hash-prefixed names with percent-encoded links; the picture and open commands live in the engine, the OS calls in the app. |
| 2026-10-16 | 1 | Section 2: planned `notecraft-onenote` crate (L2) for OneNote import (PR 3.16). |
