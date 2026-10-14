# Control channel protocol

> Stage: pre-alpha (scaffolding). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: project renamed to NoteCraft; earlier changes are in the revision history.

The control channel lets tests and scripts drive a running NoteCraft window: run commands, simulate input, read state, and take screenshots. It follows the same pattern as the other craft apps.

Status: a minimal version (`auth`, `command.run`, `screenshot`, `wait.idle`) lands in Phase 0 for UI tests (build_plan/02-phase-0-ui-shell.md, PR 0.11). It is test infrastructure, not a user feature. The rest of this protocol is post-parity optional (docs/post-parity.md).

## 1. Starting it

```sh
notecraft --control 47830
```

- Listens on `127.0.0.1` only, never on other interfaces.
- On start, writes a random 32-byte token (hex) to `control.token` in the settings folder, readable only by the current user. Clients send it in the first message.
- Off unless `--control` is given. There is no setting to turn it on permanently.

## 2. Framing

Newline-delimited JSON over TCP, one object per line, UTF-8, at most 16 MiB per line. Requests and responses follow JSON-RPC 2.0:

```json
{"jsonrpc":"2.0","id":1,"method":"auth","params":{"token":"9f2c..."}}
{"jsonrpc":"2.0","id":1,"result":{"ok":true,"version":"0.3.0","protocol":1}}
```

Any request before a successful `auth` closes the connection.

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

JSON-RPC error codes, with NoteCraft's string code in `data.code` (same set as docs/mcp.md section 5). `-32001` means not authenticated.

## 5. Versioning

`protocol` starts at 1. New methods and new optional params do not change it. Removing or changing a method does, and the old version keeps working for at least one minor release.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Transport, token auth, framing, methods, events, errors. Same day: full protocol marked post-parity; minimal Phase 0 subset kept. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
