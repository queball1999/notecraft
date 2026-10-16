# MCP server

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-15. Change: the headless server joins the build plan (PR 3.15).

NoteCraft includes a Model Context Protocol (MCP) server so AI agents can read and edit notebooks. It is a thin layer over the command registry: every tool either reads the model or runs commands by id, so anything a user can do from the ribbon, an agent can do too.

Status: the headless server (`notecraft-cli mcp` over stdio) is in the build plan as PR 3.15, by maintainer decision of 2026-10-15, because the related craft apps ship one. The bridge to a running app (`--connect`) stays post-parity (docs/post-parity.md) because it needs the full control channel.

## 1. Transports

| Mode | Start it with | What it controls |
|---|---|---|
| Headless over stdio | `notecraft-cli mcp --notebook <path>` | A private session on the notebook folder, no window |
| Bridge to the running app | `notecraft-cli mcp --connect <port>` | The open desktop window, through the control channel (docs/control-protocol.md) |

Example client configuration:

```json
{
  "mcpServers": {
    "notecraft": {
      "command": "notecraft-cli",
      "args": ["mcp", "--notebook", "C:/Users/me/Notes/Work"]
    }
  }
}
```

## 2. Tools

| Tool | Arguments | Returns |
|---|---|---|
| `list_notebooks` | none | open notebooks with ids and paths |
| `list_pages` | `notebook`, optional `section` | page tree with ids, titles, paths |
| `read_page` | `page` (id or path), optional `format` (`markdown` or `json`) | page markdown, or blocks plus canvas objects |
| `create_note` | `section`, `title`, optional `markdown` | new page id |
| `edit_note` | `page`, `edits`: list of `{container, op, text, at}` with `op` in `insert`, `replace`, `append` | updated page revision |
| `draw_ink` | `page`, `strokes`: list of `{points, color, width, tool}` | stroke ids |
| `embed_attachment` | `page`, `path`, `x`, `y` | object id |
| `render_canvas` | `page`, optional `format` (`png` or `pdf`), `scale` | image or PDF as base64 |
| `search_notes` | `query`, optional `scope`, `tags`, `limit` | ranked hits with snippets |
| `get_links` | `page` | outgoing page links and their targets |
| `get_backlinks` | `page` | linked and unlinked mentions |
| `list_commands` | none | every command id, label, argument schema, status |
| `execute_command` | `id`, `args` | command result |

Page references accept a page id (`pg_...`), a notebook-relative path, or a page link target, resolved with the same rules as page links (docs/markdown.md section 5.3).

## 3. Resources

Each page is also an MCP resource at `notecraft://page/<page-id>` with MIME type `text/markdown`, so clients can attach pages as context without a tool call.

## 4. Safety rules

- Destructive commands (delete page, delete section, empty trash, overwrite on conflict) are refused unless the server was started with `--allow-destructive`. Deleted pages still go to the trash.
- Every edit is an undoable transaction labeled `MCP: <tool>`. In bridge mode the user sees it in the undo history and can revert it.
- The server never reads files outside the notebook folders it was given, and never follows attachment links that point outside them.
- Logs record tool names and timing, never page content.
- Input limits match the file parsers (docs/markdown.md section 9). Oversized requests get an error, not a crash.

## 5. Errors

Errors use MCP's standard error shape with a stable `code` string in `data`: `not_found`, `ambiguous`, `invalid_args`, `coming_soon`, `destructive_refused`, `conflict`, `limit_exceeded`, `internal`.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Transports, tools, resources, safety rules, errors. Same day: marked post-parity optional; graph tool replaced by a links tool. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-15 | 0 | The headless stdio server moved into the build plan (PR 3.15); the `--connect` bridge stays post-parity. |
