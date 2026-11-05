# notecraft-cli reference

> Stage: pre-alpha (Phase 2 closed; v0.2.0 next). Last reviewed: 2026-11-05. Last updated: 2026-11-05. Change: Phase 2 close review (stage line, review date); earlier: `search` and `index` landed (PR 2.11) and `check` resolves page links with the app's LinkIndex; earlier: opening a page file shows it (fix/open-page-file): the notebook it belongs to, else a loose page view; a missing path is an error with an offer to drop the recent entry; earlier: PR 1.16 landed (info, text, check, run --page); the CLI follows the related craft apps; `commands` and `run` landed (PR 0.13); the other subcommands are scheduled in Phases 1 to 3; the desktop app's arguments are documented.

`notecraft-cli` is the command-line companion to the desktop app. It runs the same engine without a window, so it is good for scripts, CI, and agents.

Status: the `notecraft-cli` binary and `--version` landed in Phase -1; `info`, `text`, `check`, and `run --page` landed in PR 1.16. OneNote has no command line, but the CLI now follows the related craft apps (WordCraft's wordcraft-cli has convert, info, text, inspect, render, run, commands, and mcp): a deliberate exception to the OneNote-only scope rule, for the CLI and its MCP server only (maintainer decision, 2026-10-15). `commands` and `run` landed in Phase 0 (PR 0.13); the rest arrive with the engine work they use, as the Phase column says. Only `mcp --connect` stays post-parity (docs/post-parity.md).

## Desktop app arguments

The desktop app takes a notebook or page to open, as OneNote opens a file handed to it by the OS (file associations and "Open with"):

```sh
notecraft [--help] [--version] [<notebook folder or page file>]
```

- A folder opens as a notebook (`file.open_notebook`). A `.md` or `.notecraft` file opens as a page (`file.open`): the app looks for the notebook it belongs to (the nearest folder above the file that has a `.notecraft/` folder), opens that notebook (or switches to it if it is open already), and shows the page. A file outside any notebook opens on its own as a loose page: it is shown and editable and saves in place, with no notebook around it, and no `.notecraft/` folder is created next to it. A relative path resolves against the current directory.
- With no path, or if the path cannot be opened (missing, wrong file type, or not valid UTF-8), the app starts anyway, shows an error toast for a bad path, and reopens the last recent item. A recent item whose notebook or file no longer exists shows a "Not Found" message with a choice to remove it from the recent list (`file.recent.remove`); sample notebooks are never shown in its place.
- `--help` and `--version` print to stdout and exit 0. An unknown option or a second path prints the error and usage to stderr and exits 2. `--` ends the options.

## Global options

| Option | Meaning |
|---|---|
| `--notebook <path>` | Notebook folder to use (default: current directory if it is a notebook) |
| `--json` | Machine-readable output |
| `--quiet` | Errors only |
| `--log-level <level>` | `error`, `warn`, `info`, `debug`, `trace` (default `warn`) |
| `--version` | Print version and build info |

## Subcommands

| Command | What it does | Phase |
|---|---|---|
| `notecraft [<notebook folder or page file>]` | Open a notebook or page in the desktop app at launch (the desktop binary, not the CLI) | Phase 0, PR 0.13 |
| `notecraft-cli commands [--json]` | List every command id, label, status, and arguments | Phase 0, PR 0.13 (landed) |
| `notecraft-cli run [--sample] --cmd 'id={json}' ...` | Run commands headlessly on an empty session (or the sample notebooks) | Phase 0, PR 0.13 (landed) |
| `notecraft-cli info` | Notebook, section, and page counts and format version | Phase 1, PR 1.16 (landed) |
| `notecraft-cli text <page>` | Print a page's plain text | Phase 1, PR 1.16 (landed) |
| `notecraft-cli check` | Validate a notebook: unparseable pages, broken links, missing attachments | Phase 1, PR 1.16 (landed) |
| `notecraft-cli run --page <page> --cmd <id> ...` | Run commands against a page of a notebook and save it | Phase 1, PR 1.16 (landed) |
| `notecraft-cli search <query> [--tag <t>]... [--limit <n>] [--scope <s> --in <path>] [<folder>]` | Ranked full-text search, or tagged items | Phase 2, PR 2.11 (landed) |
| `notecraft-cli index [--rebuild] [<folder>]` | Update or rebuild `index.sqlite` | Phase 2, PR 2.11 (landed) |
| `notecraft-cli render <page> --format pdf\|html\|png [--out <file>]` | Render a page | Phase 3, PR 3.14 |
| `notecraft-cli export <notebook> --format zip\|html\|pdf [--out <path>]` | Export a whole notebook | Phase 3, PR 3.14 |
| `notecraft-cli print <page> [--printer <name>] [--copies <n>]` | Print through craft-print | Phase 3, PR 3.14 |
| `notecraft-cli mcp [--notebook <path>] [--allow-destructive]` | MCP server over stdio, headless | Phase 3, PR 3.15 |
| `notecraft-cli mcp --connect <port>` | MCP bridge to a running app (needs the full control channel) | post-parity |

### `commands`

`notecraft-cli commands` prints one line per registered command, sorted by id: id, status (`ready` or `coming_soon`), label, and arguments. With `--json` it prints one JSON array; each entry has `id`, `label`, `description`, `status`, `shortcut`, `undoable`, and `args` (each with `name`, `type`, `required`, `description`).

### `run`

`notecraft-cli run [--sample] --cmd 'id={json}' [--cmd ...]` runs each command in order through the same registry the desktop app uses, on an empty session. `--sample` loads the built-in sample notebooks instead. Each `--cmd` is a command id, optionally followed by `=` and a JSON value for its arguments; `--cmd` is repeatable and a bare id passes no arguments. Each command prints one JSON line: `{"id":..., "ok":true, "value":...}` on success, or `{"id":..., "ok":false, "error":..., "message":...}` on failure. The run stops at the first command that fails and exits with that failure's code. Limits: each `--cmd` is at most 16 KB and a run takes at most 1000 of them (exit code 6 beyond that). With `--page <page>` (PR 1.16) the commands run against that page of the `--notebook` instead, and the page is saved afterwards; nothing is saved if a command fails, and exit code 5 means the file changed on disk. `info` prints the notebook name, id, path, format version, and section, group, and page counts; `text` prints the page without markdown syntax; `check` prints one line per problem (`path: kind: message`) and ends with a summary unless `--quiet`.

```sh
notecraft-cli run --sample --cmd 'view.zoom={"percent":150}' --cmd view.zoom_in
```

### `search`

`notecraft-cli search <query> [--tag <t>]... [--limit <n>] [--scope <s> --in <path>] [<notebook folder>]` calls the same `notecraft_engine::search::search` the desktop app uses, on the notebook's `.notecraft/index.sqlite`, which it brings up to date first (the same refresh as `index`). The notebook folder is the trailing argument or `--notebook`, not both.

- Text output is two lines per hit: `<page path>  <title>`, then the snippet indented by four spaces. Hits are best match first. No hits prints `No matches.` (exit 0).
- `--limit <n>` is 1 to 500 (the engine's `MAX_RESULTS`), default 20. When more matched, `truncated` is true in JSON and a note goes to stderr.
- `--tag <t>` (repeatable, `#` optional, case-insensitive) keeps only pages that carry any of the note tags. Each JSON hit then has a `tags` list. Attachment hits are left out when `--tag` is used. The engine caps a search at 500 hits before the tag filter runs.
- With `--tag` and an empty query (leave the query out when no notebook folder is given, or pass `""`), it lists the tagged items instead, in notebook order: `<path>  <title>  #<tag> [x] <text>`.
- `--scope notebook` (default), `section`, `group`, or `page`, with `--in <path>`: a section folder path for `section` and `group` (`group` searches the section group that holds that section), a page id, path, or link target for `page`. A missing section is exit 3.
- `--json` prints `{"ok", "query", "tags", "scope", "count", "truncated", "hits": [{"kind", "path", "page_id", "title", "section", "rank", "modified", "snippet", "highlights": [[start, end]], "tags"?}]}`. The tagged-item form has `items` instead of `hits`: `{"path", "page_id", "title", "section", "tag", "text", "checked", "modified"}`.

### `index`

`notecraft-cli index [--rebuild] [<notebook folder>]` runs the full index refresh plus the text, link, and tag passes the app runs. With `--rebuild` the index file is deleted first and everything is read again. It prints one line (`Updated|Rebuilt the index of <name>: N pages, N links (N unresolved), N tagged items in N tags.`); with `--json`: `{"ok", "rebuilt", "pages", "indexed_text", "links", "unresolved_links", "tagged_items", "tags"}`. The index is only a cache: deleting it loses nothing.

### `check` and page links

`check` resolves `[[links]]` with `notecraft_markdown::LinkIndex`, the resolver the app uses (path, then path from the linking page's folder, base name, case and Unicode folding, frontmatter aliases). A link that matches nothing is a `broken_link` problem (exit 1); a link that matches several pages is only a warning naming the candidates.

`<page>` accepts a page id, a notebook-relative path, or a page link target.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error |
| 2 | Bad arguments |
| 3 | Not found (notebook, page, command) |
| 4 | Command is "Coming Soon" |
| 5 | Conflict (file changed on disk); nothing is written |
| 6 | Limit exceeded (input too large) |

In use today: all of them. 3 also covers a missing notebook or page; 2 also covers a page name that matches more than one page; `check` exits 1 when it finds problems.

## Examples

```sh
notecraft-cli --notebook ~/Notes/Work render "Meeting notes" --format pdf --out meeting.pdf
notecraft-cli --notebook ~/Notes/Work search "budget" --tag q3 --json
notecraft-cli --notebook ~/Notes/Work run --page "Meeting notes" --cmd page.add_tag --args '{"tag":"done"}'
```

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Global options, subcommands, exit codes. Same day: everything but `--version` marked post-parity; graph subcommand dropped. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | The release workflow, not the shared one, packages the CLI. |
| 2026-10-15 | 0 | Documented the desktop app's arguments: `notecraft [<notebook folder or page file>]`, `--help`, `--version`. |
| 2026-10-15 | 0 | The CLI now follows the related craft apps (maintainer decision). `commands` and `run` landed (PR 0.13); `run --sample`, the `--cmd 'id={json}'` format, and the exit codes in use documented; the other subcommands scheduled in PR 1.16, 2.11, 3.14, and 3.15; only `mcp --connect` stays post-parity. |
| 2026-10-17 | 1 | PR 1.16 landed: global options, `info`, `text`, `check`, and `run --page`; exit code 5 in use. |
| 2026-10-24 | 1 | Desktop app arguments: a page file opens in its notebook, or as a loose page when it has none (fix/open-page-file). |
| 2026-11-05 | 2 | PR 2.11 landed: `search` (ranked hits, `--tag`, `--limit`, `--scope`, `--json`) and `index [--rebuild]`; `check` resolves page links with `LinkIndex` (ambiguous is a warning, missing is a problem). No MCP server exists yet (PR 3.15), so search is not exposed there. |
| 2026-11-05 | 2 | Phase 2 close: reviewed (Last reviewed bumped), stage line set to Phase 2 closed. |
