# Architecture

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-15. Change: ink design (PR 1.10) added to section 4; PR 1.3: notecraft-markdown parses blocks with source spans; the canvas renders pages from session page text (the seam for PR 1.1); page text read from disk when a page is shown (FM-01 scored D); PR 1.10 follow-ups (anti-aliased ink edges, marked current pen, WordCraft survey notes); Ink Replay scheduled in PR 3.2 (IN-13); also: PR 1.4: markdown write (span-replacement Editor, canonical serializer) and atomic page writes in storage.

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

### 3.3 Outside edits

The watcher (`notify`) reports a changed page. If NoteCraft has no unsaved edits to it, NoteCraft reloads it. If it does, NoteCraft keeps both: the user's version stays open, and a bar offers to compare, keep mine, or take theirs. Nothing is overwritten silently.

## 4. Commands

- Ids are `area.verb` strings, for example `page.new`, `ink.set_color`, `view.zoom_in`. They are stable once released.
- Each command declares: id, label, description, argument schema (serde), whether it is undoable, whether it is destructive, and its default shortcut.
- A command not implemented yet is registered with `status = ComingSoon`. The UI shows the badge and a toast; the control channel returns a clear "not implemented" error.
- Every action being a command is what keeps the CLI and MCP server (and post-parity options such as a command palette) cheap to add later without touching features.
- Implemented in `crates/notecraft-engine/src/cmd/` (PR 0.2): `ids.rs` declares every id, `builtin.rs` holds the specs, and `Registry::run(&mut Session, id, args)` checks the JSON arguments and returns errors with stable codes (`not_found`, `coming_soon`, `invalid_args`, `internal`). The egui shell's `AppShell::dispatch` turns an error into a toast.
- The canvas (PR 0.7) splits view state the same way: the zoom and each page's style (rule lines, page color, paper size) live in the session and change only through commands, so Ctrl+wheel and pinch dispatch `view.zoom` like the ribbon does. The scroll position is front-end state in `notecraft-ui-egui/src/canvas`. The page-to-screen math is `notecraft_canvas::Viewport`, with no egui types.
- File commands (PR 0.8) take an optional `path`. A button sends them without one, and the shell asks its `FilePicker` (the OS dialogs through `rfd` in the desktop app, a fake in tests) and runs the command again with the chosen path; canceling runs nothing. The control channel always passes a path. The recent list lives in the session and the app saves it in `config.toml`.
- Shortcuts (PR 0.9) are data in the engine: `cmd::shortcuts::default_map()` turns each command's `default_shortcut` (plus a few bindings with arguments, such as Ctrl+2 to Ctrl+9 for note tags) into key chords, most modifiers first. The egui shell matches them each frame, with Ctrl as egui's command modifier (Cmd on macOS), and dispatches the commands; nothing is matched while a text field has focus. Tests reject duplicate chords and bindings with invalid arguments.
- Ink (PR 1.10) follows the same split. The front end turns egui events into `notecraft_core::PointerSample`s (page units, time, optional pressure, source kind) and feeds a `notecraft_canvas::ink::StrokeBuilder`, which smooths with a one-euro filter and gives a stroke a speed-based width when the device reports no pressure. When the pen lifts, the canvas sends `ink.add_stroke { points }`; the session keeps one `InkLayer` (strokes in paint order plus a grid index) per page id, and the active tool and pen (`draw.type`, `draw.pen`, `draw.color`, `draw.thickness`). Drawing is a renderer-agnostic triangle list (`tessellate`) that the egui layer turns into a `Mesh`. The `Stroke` type serializes to the canvas block's `ink` entries (docs/markdown.md section 3.4); writing it to the file is PR 1.9.

- Page text (PR 1.3) enters the session through `Session::set_page_source(page_id, text)`, which is not a command: `Session::load_current_page` calls it after every command when the page shown comes from a notebook folder and has not been read yet (read through `notecraft_storage::read_page_file`, bounded at 64 MiB; a cut or unreadable page gets a `PageNotice` the canvas draws under the title). The canvas parses the text of the page shown with `notecraft_markdown::parse` (frontmatter, then blocks that keep their source byte spans), caches the result per page and text revision, and draws the blocks in flow layout from theme tokens. The markdown crate has no egui types; `canvas/flow.rs` is the only place blocks meet the painter.

- Page writing (PR 1.4): `notecraft_markdown::Editor` collects span replacements against the page text and `finish()` splices them, so bytes outside the edited spans never change; `canonical` serializes new blocks. `notecraft_storage::page_file::write_page_text` writes the result through `craft-atomic` and skips the write when the bytes are unchanged. Auto-save and the conflict check come with PR 1.7.

## 5. Undo and redo

Each command produces a transaction: a list of reversible model operations. Undo history is per page, capped at 500 steps or 64 MiB. Cross-page commands (rename with link updates, moves) produce a notebook-level transaction that undoes as one step.

## 6. Never crash

- Lints: every `lib.rs` denies `unwrap_used`, `expect_used`, `panic`, `unimplemented`, `todo`, `unreachable`. `unsafe` is forbidden workspace-wide, so OS integrations (pen pressure, spell check, camera) go through safe wrapper crates.
- Parsers are lenient and bounded (limits in docs/markdown.md section 9). Fuzz targets for the markdown, frontmatter, canvas, and toml parsers land in Phase 1.
- The command guard wraps each command in `std::panic::catch_unwind`. If a dependency panics anyway, the guard rolls back the transaction, logs the error, and shows a toast. The document stays as it was before the command.
- Today (PR 0.2) the guard rolls back by restoring a snapshot of the session taken before the command, which is cheap while the session holds no notes. Phase 1 replaces the snapshot with transaction rollback (section 5). A command that returns an error is rolled back the same way.
- All writes use `craft-atomic`. A crash mid-save leaves the old file intact.

## 7. Threads

| Thread | Does |
|---|---|
| UI (main) | egui frame, input, command dispatch |
| IO worker | file reads and writes, index updates, reindexing |
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

### 10.3 Highlighter (PR 1.11 design, not built)

OneNote's highlighter multiplies over the page. egui has no multiply blend in its painter, and a translucent mesh darkens wherever its own triangles overlap. The plan:

1. Draw highlighter strokes in a pass of their own, behind the text containers and above the paper, so dark text stays dark over the color (the same result as multiply for dark text on a light page). WordCraft also draws its highlighter behind text and its pens in front.
2. Draw each highlighter stroke as an opaque mesh in the color pre-blended with the paper color at the highlighter's alpha (about 0.5), so triangle overlap inside a stroke cannot show and the AA fringe stays valid. The pre-blend uses the theme's paper color at paint time, so it follows dark themes.
3. If overlapping highlighter strokes must darken each other (true multiply), or the highlighter must tint images, add an `egui_wgpu` paint callback with a multiply blend state later; the stroke type and file format do not change.
4. The stored stroke keeps `tool: "highlighter"`, the picked color, and its width; the blend is a rendering choice (docs/printing.md specifies multiply at 50% opacity for PDF).

### 10.4 Erasers, lasso, and undo (PR 1.11, 3.3 design)

- Hit testing is `InkLayer::hit_test` and `topmost_at` over the `GridIndex` (distance from the point to each stroke segment, widened by half the stroke width plus a pointer reach in screen pixels). This is already better than a linear scan; keep it for both erasers.
- Stroke eraser: one drag is one undo step. The first stroke it removes starts a step and later ones join it (WordCraft does this with a `join_next_undo` call; PR 1.8's transactions should take a "merge with the previous step" flag for the same effect).
- Point eraser: remove the points under the eraser circle and split the stroke into the runs that remain, each keeping color, width, and pressure; runs shorter than two points are dropped.
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
| 2026-10-15 | 1 | Section 4: page writing is span replacements (`Editor`) plus `page_file::write_page_text` over craft-atomic (PR 1.4). |
| 2026-10-15 | 1 | Section 4: page text is read from disk by `Session::load_current_page` (storage `read_page_file`), with a `PageNotice` for cut or unreadable pages. |
| 2026-10-15 | 1 | Section 10: ink and drawing design (anti-aliased edges, highlighter, erasers, lasso, Ink to Shape, custom pens), after a survey of the related WordCraft app. |
