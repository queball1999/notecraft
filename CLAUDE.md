# CLAUDE.md

Context for Claude when working in this repo. [AGENTS.md](AGENTS.md) is binding and wins over anything here.

## The project in one paragraph

NoteCraft is a clean-room remake of the OneNote experience (freeform canvas, ink, ribbon, four theme presets) on top of NoteCraft's own markdown-first, folder-based file format. Pure Rust, egui, wgpu. Windows, macOS, Linux. Dual-licensed MIT OR Apache-2.0. Related to, but independent of, the ArtCraft apps (WordCraft, DeckCraft, GridCraft): same patterns, shared craft-libs and CI workflows, separate project. Say "related craft apps", never "part of the ArtCraft family". NoteCraft's own identity is separate: app ID `io.github.queball1999.notecraft`, repo `github.com/Queball1999/notecraft`, manufacturer "The NoteCraft Contributors". Never use the `ai.storyteller` namespace, ArtCraft websites, or ArtCraft company names for NoteCraft; links to the shared craft-libs, craft-common-actions, craftrules, craft-fonts, and the related apps are fine.

## Hard rules to keep in mind

- Plain ASCII, US English, everywhere: no em dashes, en dashes, emoji, typographic arrows, curly quotes, or box-drawing characters. This includes chat replies, commit messages, and PR bodies. (AGENTS.md section 2.)
- Compare NoteCraft only to OneNote. Never name or compare against any other note-taking app.
- Build OneNote features only until full parity. Anything OneNote lacks goes in docs/post-parity.md, not in the build plan.
- Never crash: no `unsafe`, no `unwrap`, `expect`, or `panic!` outside tests. Parsers are lenient and bounded.
- Every user action is a command with a stable id.
- Respect the L0 to L7 layering. Engine crates never touch egui.
- Use craft-libs (`craft-atomic`, `craft-settings`, `craft-ui`, `craft-print`, `craft-quit`, `craft-cms`) instead of writing your own. Never depend on `winprint` directly.
- Do not commit, push, or open PRs unless the maintainer asks in the current session.

## Where to look

| Question | File |
|---|---|
| What are we building? | [SCRD.md](SCRD.md) |
| What is done, what is next? | [ROADMAP.md](ROADMAP.md), [docs/roadmap.md](docs/roadmap.md), [docs/gaps.md](docs/gaps.md) |
| How is it put together? | [docs/architecture.md](docs/architecture.md) |
| What exactly do I do for this phase? | [build_plan/](build_plan/README.md) |
| What does the file format look like? | [docs/markdown.md](docs/markdown.md) |

## Working style

- Pick work from the top of `docs/gaps.md` or from the current phase file in `build_plan/`.
- After a change lands, update the progress docs as AGENTS.md section 8 describes.
- Run `cargo xtask ci` before saying a change is done. Report failures as they are.

## Attribution (only when asked to commit or open a PR)

- End commit messages with: `Co-Authored-By: Claude <noreply@anthropic.com>` (use the model name given by the session).
- End PR descriptions with the line `Generated with [Claude Code](https://claude.com/claude-code)`, with no emoji.
