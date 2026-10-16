# Control channel protocol

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-15. Change: the minimal subset (`auth`, `command.run`, `screenshot`, `wait.idle`) is implemented (PR 0.11); earlier changes are in the revision history.

The control channel lets tests and scripts drive a running NoteCraft window: run commands, simulate input, read state, and take screenshots. It follows the same pattern as the other craft apps.

Status: a minimal version (`auth`, `command.run`, `screenshot`, `wait.idle`) is implemented for UI tests (build_plan/02-phase-0-ui-shell.md, PR 0.11). It is test infrastructure, not a user feature. The rest of this protocol is post-parity optional (docs/post-parity.md).

Implemented now, in `apps/notecraft/src/control/`:

- The four methods above. `command.run` goes through the same dispatcher as ribbon buttons and shortcuts (`AppShell::run_command`). A file command sent without a `path` asks the OS picker, as a button does, so tests should always pass a `path`; if the picker is canceled the result is `{"canceled":true}`.
- `screenshot` captures the next rendered frame and returns `{"png": "<base64>"}`. The optional `region` parameter is not implemented; crop on the client.
- `wait.idle` answers `{"ok":true}` at the end of the first frame with no immediate repaint pending. `timeout_ms` defaults to 5000 and is capped at 60000. On timeout the error is `-32002` with `data.code` `limit_exceeded`. It is not a promise that nothing changes later (a delayed repaint, such as a toast timer, does not count as pending).
- At most 8 clients at once. A client that does not finish its first line within 10 seconds is dropped.
- `command.list`, `state.get`, `input.*`, and `subscribe` are not implemented; an unknown method gets `-32601`.

## 1. Starting it

```sh
notecraft --control 47830
```

- Listens on `127.0.0.1` only, never on other interfaces.
- On start, writes a random 32-byte token (64 hex characters, from the OS random source) to `control.token` in the settings folder (the folder that holds `config.toml`, so `NOTECRAFT_CONFIG_DIR` applies). Clients send it in the first message. The file is deleted on a normal exit.
- Reading the token is the only access control, so on Unix the file is created with mode 0600. On Windows no ACL is changed: the file inherits the permissions of the settings folder (`%APPDATA%\NoteCraft`), which only the current user, administrators, and SYSTEM can read by default. The token is compared in constant time.
- With no settings folder (`NOTECRAFT_NO_PREFS` set, or an empty `NOTECRAFT_CONFIG_DIR`) there is nowhere to put the token, so `--control` exits with an error instead of listening.
- The port is 1 to 65535; a bad port is a usage error (exit code 2).
- Off unless `--control` is given. There is no setting to turn it on permanently.

## 2. Framing

Newline-delimited JSON over TCP, one object per line, UTF-8, at most 16 MiB per line. Requests and responses follow JSON-RPC 2.0:

```json
{"jsonrpc":"2.0","id":1,"method":"auth","params":{"token":"9f2c..."}}
{"jsonrpc":"2.0","id":1,"result":{"ok":true,"version":"0.3.0","protocol":1}}
```

Any request before a successful `auth` gets error `-32001` and the connection is closed; so does a wrong token or a line that is not valid JSON. A line over 16 MiB gets an error (`data.code` `limit_exceeded`) and the connection is closed; the line is never buffered whole.

## 3. Methods

| Method | Params | Result |
|---|---|---|
| `auth` | `token` | `ok`, app `version`, `protocol` number |
| `command.list` | none | all commands with id, label, args schema, status |
| `command.run` | `id`, `args` | the command's result, or an error |
| `state.get` | `path`, for example `"session.page"` or `"ui.theme"` | JSON value |
| `input.pointer` | `kind` (`down`, `move`, `up`), `x`, `y` (window px), `button`, optional `pressure`, `pointer` (`mouse`, `pen`, `touch`) | `ok` |
| `input.key` | `key`, `modifiers`, `kind` (`down`, `up`, `press`) | `ok` |
| `input.text` | `text` | `ok` |
| `screenshot` | optional `region` | PNG as base64 |
| `wait.idle` | optional `timeout_ms` | `ok` once no repaint is pending |
| `subscribe` | `events` list, for example `["page.changed","command.ran"]` | `ok`; events then arrive as notifications |

Events are JSON-RPC notifications (no `id`):

```json
{"jsonrpc":"2.0","method":"event","params":{"type":"page.changed","page":"pg_01J9Z4B2M7XR"}}
```

## 4. Errors

JSON-RPC error codes, with NoteCraft's string code in `data.code` (same set as docs/mcp.md section 5). `-32001` means not authenticated. Standard JSON-RPC codes apply (`-32700` bad JSON, `-32600` bad request, `-32601` unknown method, `-32602` bad params, `-32603` internal). A command that does not run (`not_found`, `coming_soon`, `internal`, `invalid_args`) keeps its string code in `data.code`; `invalid_args` and `internal` use `-32602` and `-32603`, the rest use `-32000`.

## 5. Versioning

`protocol` starts at 1. New methods and new optional params do not change it. Removing or changing a method does, and the old version keeps working for at least one minor release.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Transport, token auth, framing, methods, events, errors. Same day: full protocol marked post-parity; minimal Phase 0 subset kept. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-15 | 0 | PR 0.11: minimal subset implemented (`auth`, `command.run`, `screenshot` without `region`, `wait.idle`); documented the token file protections, limits, and error codes. |
