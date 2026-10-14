# NoteCraft

NoteCraft is a free, open-source note-taking app with the freeform canvas and ink of OneNote (2020-era look), backed by a markdown-first, folder-based file format you can open and edit in any text editor. It is written in pure Rust with egui and wgpu, and it runs on Windows, macOS, and Linux.

NoteCraft is a related but independent project. It follows the patterns of the ArtCraft apps ([WordCraft](https://github.com/storytold/wordcraft), [DeckCraft](https://github.com/storytold/deckcraft), [GridCraft](https://github.com/storytold/gridcraft)) and uses their shared libraries (craft-libs) and CI workflows (craft-common-actions), but it is developed separately and is not an official ArtCraft app.

> Status: pre-alpha. No code has landed yet. The repo holds the spec, the docs, and the build plan. See [ROADMAP.md](ROADMAP.md) for where things stand.

## What it will do

- A freeform canvas per page: place text, ink, images, and attachments anywhere, then pan and zoom.
- Pen, highlighter, eraser, shapes, and text boxes, with pressure-sensitive ink.
- Notebooks, section groups, sections, and pages, in the familiar OneNote layout.
- Plain `.md` files on disk, plus a richer `.notecraft` file that stores canvas layout and ink next to the markdown.
- Page links (`[[Page Name]]`), search across notebooks, and OneNote's note tags with Find Tags.
- Four theme presets (2016, 2019, 2021, Microsoft 365), each in light and dark.
- Print, PDF, and HTML export; recycle bin and page versions; and, later, handwriting recognition, recording, and `.one` import.

Features that are not wired up yet show a "Coming Soon" badge in the app. The goal is OneNote parity first; extras OneNote does not have are listed in [docs/post-parity.md](docs/post-parity.md) for later.

## Notes are just files

A notebook is a folder. A section is a subfolder. A page is one file: a plain `.md` file for text-only pages, or a `.notecraft` file (still markdown, with a canvas block at the end) for pages with ink or free layout. You can sync the folder with any tool, edit pages in any editor, and keep them under version control. See [docs/markdown.md](docs/markdown.md) for the full format.

```text
My Notebook/
|-- .notecraft/              <- notebook settings, search index, history, trash
|-- Work/
|   |-- .section.toml
|   |-- Meeting notes.notecraft
|   |-- Reading list.md
|   `-- attachments/
`-- Projects/           <- section group
    |-- Alpha/
    `-- Beta/
```

## Building

Requires Rust 1.95 or newer.

```sh
cargo run -p notecraft          # desktop app
cargo run -p notecraft-cli -- --version
cargo xtask ci              # everything CI runs
```

These commands work once Phase -1 of the [build plan](build_plan/README.md) lands.

## Documentation

| Doc | What it covers |
|---|---|
| [SCRD.md](SCRD.md) | The full system spec |
| [ROADMAP.md](ROADMAP.md) | Stage summary and progress log |
| [AGENTS.md](AGENTS.md) | Binding rules for contributors and agents |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to set up, build, test, and send a PR |
| [docs/](docs/) | Parity, format, architecture, automation, release docs |
| [build_plan/](build_plan/README.md) | Phase-by-phase execution plan |

## License

NoteCraft is dual-licensed under either of

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option. Unless you explicitly state otherwise, any contribution you submit for inclusion in NoteCraft, as defined in the Apache-2.0 license, is dual licensed as above, without any additional terms or conditions.

Microsoft, OneNote, and Microsoft 365 are trademarks of Microsoft Corporation. NoteCraft is an independent project and is not affiliated with, sponsored by, or endorsed by Microsoft Corporation. These names are used only to describe the workflow NoteCraft is compatible with.
