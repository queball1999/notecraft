# Contributing to NoteCraft

Thanks for helping. Read [AGENTS.md](AGENTS.md) first. Its rules apply to everyone, human or agent.

## Set up

1. Install Rust 1.95 or newer with `rustup`, plus `rustup component add rustfmt clippy` and `rustup target add wasm32-unknown-unknown`. CI uses current stable; the workspace `rust-version` is 1.95.
2. On Linux, install the system packages egui and wgpu need:
   ```sh
   sudo apt-get install pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev \
     libxi-dev libgtk-3-dev libfontconfig1-dev libfreetype-dev mesa-vulkan-drivers
   ```
3. Clone the repo and run `cargo xtask ci`. If it passes, you are ready.

## Run the checks

`cargo xtask ci` runs every gate CI runs, in order:

| Step | What it checks |
|---|---|
| `cargo fmt --all -- --check` | formatting |
| `cargo clippy --workspace --all-targets -- -D warnings` | lints, including the never-crash lints |
| `cargo test --workspace` | unit and integration tests |
| `cargo xtask text` | plain ASCII, US English text (see AGENTS.md section 2) |
| `cargo xtask assets` | every bundled asset has an ATTRIBUTION.md row |
| `cargo xtask layers` | crates only depend on lower layers |
| `cargo xtask wasm` | the engine crates build for `wasm32-unknown-unknown` |

Run single steps with the commands in the left column while you work.

## Fuzz the parsers

The page parsers (markdown blocks, frontmatter, container markers, the canvas block, `notebook.toml`, `.section.toml`, and the recovery journal) have `cargo fuzz` targets in `fuzz/`. That crate is outside the workspace and needs a nightly toolchain, so `cargo xtask ci` never builds it. The same checks run on stable inside `cargo test` as bounded, seeded mutation tests (`crates/notecraft-markdown/tests/fuzz_props.rs` and `crates/notecraft-storage/tests/fuzz_props.rs`).

Manual run (there is no scheduled workflow; run each target for 30 minutes before a release):

```
cargo install cargo-fuzz
bash fuzz/seed.sh
cd fuzz
cargo +nightly fuzz run markdown -- -max_total_time=1800
```

Targets: `markdown`, `frontmatter`, `containers`, `canvas_block`, `notebook_toml`, `section_toml`, `recovery_journal`. A crash writes its input to `fuzz/artifacts/`; turn it into a regression test in the crate that owns the parser, then fix the parser. `fuzz/corpus/` and `fuzz/artifacts/` are not committed.

## Layering

Crates are stacked in layers L0 to L7 (table in AGENTS.md section 5). A crate may depend only on crates in lower layers. Nothing below L6 may depend on egui, eframe, winit, or rfd. If you need something from a higher layer, pass it in as a trait or a callback instead.

## Add a command

Every user-visible action is a command.

1. Pick a stable id in `area.verb` form, for example `page.new` or `ink.set_color`. Ids never change once released.
2. Add the command in `crates/notecraft-engine/src/cmd/<area>.rs` and register it in the registry.
3. Give it a label, a description, and an optional default shortcut.
4. Wire it to a ribbon button or menu in `crates/notecraft-ui-egui`. If the engine side is not ready, the button shows a "Coming Soon" badge and a toast.
5. Add a test that runs the command through the registry.

The control channel used by UI tests picks it up automatically.

Only add commands for features OneNote has (AGENTS.md section 1). Ideas for features OneNote does not have go in docs/post-parity.md.

## Themes

Theme palettes live in `crates/notecraft-ui-egui/src/themes/`. Colors are original values chosen to evoke each OneNote era, not sampled from screenshots. See [docs/themes.md](docs/themes.md). A theme PR needs before and after screenshots of the ribbon, sidebar, and canvas in both light and dark mode.

## Assets

Only original or openly licensed assets (MIT, Apache-2.0, CC0, OFL). Add a row to [ATTRIBUTION.md](ATTRIBUTION.md) for every file you bundle. Never copy icons, fonts, or artwork from Microsoft or any other vendor.

## Branches, commits, and PRs

The full rules, with examples, are in [build_plan/00-conventions.md](build_plan/00-conventions.md). In short:

- Branch off the current phase branch: `feat/<slug>`, `fix/<slug>`, `docs/<slug>`, or `chore/<slug>`.
- Commit with Conventional Commits: `feat(canvas): add zoom to fit`. Imperative mood, subject at most 72 characters.
- One logical change per PR. Fill in the PR template. Attach screenshots for any UI change.
- Update the progress docs when your change lands (AGENTS.md section 8).

## Reporting bugs

Use the bug report template. Attach the log file:

- Windows: `%APPDATA%\NoteCraft\logs\notecraft.log`
- macOS: `~/Library/Application Support/NoteCraft/logs/notecraft.log`
- Linux: `$XDG_CONFIG_HOME/notecraft/logs/notecraft.log`

Logs never contain note content, so they are safe to share.

## Code of conduct

See [.github/CODE_OF_CONDUCT.md](.github/CODE_OF_CONDUCT.md).
