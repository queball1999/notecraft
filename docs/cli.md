# notecraft-cli reference

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-15. Change: the desktop app opens a notebook or page passed at launch.

`notecraft-cli` is the command-line companion to the desktop app. It runs the same engine without a window, so it is good for scripts, CI, and agents.

Status: the `notecraft-cli` binary and `--version` land in Phase -1 (the release workflow packages and runs the CLI). Everything else here is post-parity optional (docs/post-parity.md); OneNote has no command line. The Phase column below says what the build plan delivers.

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
| `notecraft-cli open <path>` | Open a notebook or page in the desktop app (starts it if needed) | post-parity |
| `notecraft-cli commands` | List every command id, label, and status | post-parity |
| `notecraft-cli run --page <page> --cmd <id> [--args <json>]` | Run a command against a page and save | post-parity |
| `notecraft-cli render <page> --format pdf\|html\|png [--out <file>]` | Render a page | post-parity |
| `notecraft-cli export <notebook> --format zip\|html\|pdf [--out <path>]` | Export a whole notebook | post-parity |
| `notecraft-cli print <page> [--printer <name>] [--copies <n>]` | Print through craft-print | post-parity |
| `notecraft-cli search <query> [--tag <t>] [--limit <n>]` | Full-text search | post-parity |
| `notecraft-cli index [--rebuild]` | Update or rebuild `index.sqlite` | post-parity |
| `notecraft-cli check` | Validate a notebook: unparseable pages, broken links, missing attachments | post-parity |
| `notecraft-cli mcp [--notebook <path>] [--allow-destructive]` | MCP server over stdio | post-parity |
| `notecraft-cli mcp --connect <port>` | MCP bridge to a running app | post-parity |

`<page>` accepts a page id, a notebook-relative path, or a page link target.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error |
| 2 | Bad arguments |
| 3 | Not found (notebook, page, command) |
| 4 | Command is "Coming Soon" |
| 5 | Conflict (file changed on disk) |
| 6 | Limit exceeded (input too large) |

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
