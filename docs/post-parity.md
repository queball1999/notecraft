# Post-parity optional features

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-15. Change: Full CLI and MCP server moved into the build plan; dotted paper recorded as a kept extra.


The build plan covers OneNote parity only. Everything in this doc is something OneNote does not do. None of it is scheduled. Each item is optional and is considered only after full parity (v1.0.0 and the parity items in Phase 4) is reached, and only if users ask for it.

The architecture keeps these possible without rework: every action is already a command with a stable id, the file format is plain markdown, and the index already stores links (needed for OneNote-style link updates on rename).

## Candidates

| Feature | What it would add | Depends on | Hours (rough) |
|---|---|---|---|
| Backlinks pane | List of pages that link to the current page, with context | link index (Phase 2) | 14-24 |
| Unlinked mentions | Plain-text mentions of the page title, with one-click linking | backlinks pane | 10-18 |
| Graph view | Force-directed graph of pages and links, with a Graph ribbon tab | link index | 44-74 |
| Graph filters and local graph | Filter by section, tag, date; neighbors of one page | graph view | 14-24 |
| Heading links and embeds | `[[Page#Heading]]`, `![[Page]]`, `![[image.png]]` | page links (Phase 2) | 10-18 |
| Page tag pane | Frontmatter `tags:` and inline `#tags` listed with counts | note tags (Phase 2) | 8-14 |
| Code syntax highlighting | Colored fenced code blocks | none | 10-16 |
| Footnotes | `[^1]` footnotes | none | 6-10 |
| Command palette | Ctrl+Shift+P search over every command | command registry | 10-16 |
| Color alpha | Transparency (A) in the custom color editor; OneNote colors are opaque. Kept by maintainer decision (PR 0.5e) | none | 0 (built) |
| Dotted paper | Dot grid in Rule Lines (`view.rule_lines` style `dotted`, `paper: dotted`); OneNote has rule lines and grids only. Kept by maintainer decision (2026-10-15) | none | 0 (built) |
| Full control channel | Input simulation, state queries, events (docs/control-protocol.md) | minimal channel (Phase 0) | 16-28 |
| MCP bridge | MCP tools driving the running window | full control channel | 10-18 |
| Plugins | Manifest, lifecycle, macro commands, Plugins ribbon tab | command registry | 50-90 |
| WASM plugins | Sandboxed plugins with a host API | plugins | 50-90 |
| Total | | | 252-440 |

The full CLI and the MCP server moved into the build plan on 2026-10-15 (maintainer decision; PR 0.13, 1.16, 2.11, 3.14, and 3.15), so they are no longer candidates here. The MCP bridge and the full control channel stay.

Hour ranges come from the PRs these features had in the build plan before it was narrowed to OneNote parity. Re-estimate before starting any of them.

## How an item gets picked up

1. Full parity is reached (docs/roadmap.md).
2. The maintainer picks an item and writes a short scope note.
3. The item gets checklist rows in a new "Post-parity" area of docs/parity-checklist.md and a new build plan file (`build_plan/07-...`), following 00-conventions.md.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Backlinks, graph, embeds, tag pane, code highlighting, footnotes, command palette, control channel, MCP, CLI, and plugins moved here from the build plan. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-15 | 0 | Color alpha in the custom color editor recorded as a kept extra (PR 0.5e). |
| 2026-10-15 | 0 | Full CLI and MCP server rows removed: both are in the build plan now (maintainer decision, 2026-10-15). MCP bridge and full control channel kept. |
| 2026-10-15 | 0 | Dotted paper recorded as a kept extra (maintainer decision, 2026-10-15). |
