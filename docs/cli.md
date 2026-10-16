# notecraft-cli reference

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-15. Change: the CLI follows the related craft apps; `commands` and `run` landed (PR 0.13); the other subcommands are scheduled in Phases 1 to 3; the desktop app's arguments are documented.

`notecraft-cli` is the command-line companion to the desktop app. It runs the same engine without a window, so it is good for scripts, CI, and agents.

Status: the `notecraft-cli` binary and `--version` landed in Phase -1. OneNote has no command line, but the CLI now follows the related craft apps (WordCraft's wordcraft-cli has convert, info, text, inspect, render, run, commands, and mcp): a deliberate exception to the OneNote-only scope rule, for the CLI and its MCP server only (maintainer decision, 2026-10-15). `commands` and `run` landed in Phase 0 (PR 0.13); the rest arrive with the engine work they use, as the Phase column says. Only `mcp --connect` stays post-parity (docs/post-parity.md).

## Desktop app arguments

The desktop app takes a notebook or page to open, as OneNote opens a file handed to it by the OS (file associations and "Open with"):

```sh
notecraft [--help] [--version] [<notebook folder or page file>]
```

- A folder opens as a notebook (`file.open_notebook`); a `.md` or `.notecraft` file opens as a page (`file.open`). A relative path resolves against the current directory.
- With no path, or if the path cannot be opened (missing, wrong file type, or not valid UTF-8), the app starts anyway, shows an error toast for a bad path, and reopens the last recent item.
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
| `notecraft-cli info` | Notebook, section, and page counts and format version | Phase 1, PR 1.16 |
| `notecraft-cli text <page>` | Print a page's plain text | Phase 1, PR 1.16 |
| `notecraft-cli check` | Validate a notebook: unparseable pages, broken links, missing attachments | Phase 1, PR 1.16 |
| `notecraft-cli run --page <page> --cmd <id> ...` | Run commands against a page of a notebook and save it | Phase 1, PR 1.16 |
| `notecraft-cli search <query> [--tag <t>] [--limit <n>]` | Full-text search | Phase 2, PR 2.11 |
| `notecraft-cli index [--rebuild]` | Update or rebuild `index.sqlite` | Phase 2, PR 2.11 |
| `notecraft-cli render <page> --format pdf\|html\|png [--out <file>]` | Render a page | Phase 3, PR 3.14 |
| `notecraft-cli export <notebook> --format zip\|html\|pdf [--out <path>]` | Export a whole notebook | Phase 3, PR 3.14 |
| `notecraft-cli print <page> [--printer <name>] [--copies <n>]` | Print through craft-print | Phase 3, PR 3.14 |
| `notecraft-cli mcp [--notebook <path>] [--allow-destructive]` | MCP server over stdio, headless | Phase 3, PR 3.15 |
| `notecraft-cli mcp --connect <port>` | MCP bridge to a running app (needs the full control channel) | post-parity |

### `commands`

`notecraft-cli commands` prints one line per registered command, sorted by id: id, status (`ready` or `coming_soon`), label, and arguments. With `--json` it prints one JSON array; each entry has `id`, `label`, `description`, `status`, `shortcut`, `undoable`, and `args` (each with `name`, `type`, `required`, `description`).

### `run`

`notecraft-cli run [--sample] --cmd 'id={json}' [--cmd ...]` runs each command in order through the same registry the desktop app uses, on an empty session. `--sample` loads the built-in sample notebooks instead. Each `--cmd` is a command id, optionally followed by `=` and a JSON value for its arguments; `--cmd` is repeatable and a bare id passes no arguments. Each command prints one JSON line: `{"id":..., "ok":true, "value":...}` on success, or `{"id":..., "ok":false, "error":..., "message":...}` on failure. The run stops at the first command that fails and exits with that failure's code. Limits: each `--cmd` is at most 16 KB and a run takes at most 1000 of them (exit code 6 beyond that). From Phase 1 (PR 1.16), `--page <page>` runs the commands against a page of the `--notebook` and saves it.

```sh
notecraft-cli run --sample --cmd 'view.zoom={"percent":150}' --cmd view.zoom_in
```

`<page>` accepts a page id, a notebook-relative path, or a page link target.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error |
| 2 | Bad arguments |
| 3 | Not found (notebook, page, command) |
| 4 | Command is "Coming Soon" |
| 5 | Conflict (file changed on disk); not used yet, arrives with PR 1.16 |
| 6 | Limit exceeded (input too large) |

In use today: 0, 1 (internal error), 2 (bad arguments, including a command given invalid arguments), 3 (unknown command), 4, and 6.

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
