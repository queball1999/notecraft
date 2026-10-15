# SCRD: System Combined Resource Document

Target: NoteCraft, a one-to-one remake of OneNote with a markdown-first, folder-based file format
Prepared: 2026-10-13. Revised: 2026-10-15 (command line and MCP server added to the build plan; estimates updated)
Stack: Rust + egui (ArtCraft pattern)
Platforms: Windows, macOS, Linux (FreeBSD built by CI; browser in Phase 4)

---

## 1. Executive summary

NoteCraft is a free, open-source, clean-room note-taking application that remakes the freeform canvas experience of Microsoft OneNote (2020-era look and feel), backed by a markdown-first, folder-based file format that is easy to manage and edit in any text editor. Built in pure Rust with egui on the GPU, it targets macOS, Windows, and Linux.

Each page is a freeform canvas where users can:

- write text in note containers placed anywhere, stored as plain markdown files in a local folder;
- draw freely with pen, highlighter, eraser, and shapes;
- embed pictures and files of any type;
- link pages with `[[Page]]` links, search across notebooks, and tag notes with OneNote's note tags;
- switch between OneNote theme presets (2016, 2019, 2021, Microsoft 365), each light or dark.

The development philosophy follows the ArtCraft pattern: build the UI first as a fully interactive shell, mark features that are not wired up with "Coming Soon" badges, and wire up engine logic behind the finished interface. Every action is a command with a stable id.

NoteCraft is a related but independent project. It follows the patterns of the ArtCraft apps ([WordCraft](https://github.com/storytold/wordcraft), [DeckCraft](https://github.com/storytold/deckcraft), [GridCraft](https://github.com/storytold/gridcraft)) and uses their shared libraries (craft-libs) and CI workflows (craft-common-actions), but it is developed separately and is not an official ArtCraft app.

Scope rule: the build plan covers OneNote features only. The one exception is the command line and its MCP server, which follow the related craft apps (maintainer decision, 2026-10-15; [docs/cli.md](docs/cli.md)). Other features OneNote does not have (backlinks, graph view, command palette, plugins) are optional post-parity ideas in [docs/post-parity.md](docs/post-parity.md).

---

## 2. Technical metadata

| Property | Value |
|---|---|
| Product | NoteCraft |
| Language | Rust (2021 edition), MSRV 1.95 (required by craft-ui) |
| UI framework | egui + eframe + wgpu, with craft-ui components |
| Graphics backend | wgpu (DirectX 12 on Windows, Metal on macOS, Vulkan on Linux) |
| Windowing | winit (through eframe) |
| Document format | Markdown: `.md` (text-only pages) and `.notecraft` (markdown plus a canvas block), in a local folder (docs/markdown.md) |
| Attachment storage | Files in each section's `attachments/` folder, referenced by markdown links |
| Index | SQLite (`.notecraft/index.sqlite`), a rebuildable cache for structure, links, tags, and search |
| Rendering | Custom canvas renderer (ink, text, shapes, images, attachments) |
| Markdown engine | comrak, wrapped by `notecraft-markdown` |
| Page links | Custom parser for OneNote-style `[[Page]]` links |
| Crash-safe writes | craft-atomic |
| Settings | craft-settings (`config.toml`) |
| Printing | craft-print (wraps winprint on Windows, CUPS on macOS and Linux) |
| CLI | `notecraft-cli` (commands and run in Phase 0; the rest through Phase 3, see docs/cli.md) |
| License | MIT OR Apache-2.0 |
| First version | 0.1.0 (alpha) |

---

## 3. Architecture overview

NoteCraft follows the ArtCraft layered architecture: engine crates are UI-agnostic, and the egui front end is a separate, swappable layer. A crate may only depend on lower layers; `cargo xtask layers` enforces it.

```text
+-----------------------------------------------------------------------------+
| L7  apps                                                                    |
|     notecraft (desktop), notecraft-cli (--version), notecraft-web (Phase 4) |
+-----------------------------------------------------------------------------+
| L6  notecraft-ui-egui: OneNote-style ribbon, File backstage, navigation,    |
|     canvas widget, themes (2016, 2019, 2021, M365), dialogs                 |
+-----------------------------------------------------------------------------+
| L5  notecraft-engine: session, command registry, undo/redo, clipboard,      |
|     PDF and HTML export                                                     |
+-----------------------------------------------------------------------------+
| L4  notecraft-canvas: ink engine, shapes, ink to shape, layout, hit         |
|     testing, zoom and pan                                                   |
+-----------------------------------------------------------------------------+
| L3  notecraft-markdown: parse, minimal-diff write, page links, note         |
|     tags, equations                                                         |
+-----------------------------------------------------------------------------+
| L2  notecraft-model: notebook, section group, section, page, containers,    |
|     ink, shapes, images, file objects                                       |
+-----------------------------------------------------------------------------+
| L1  notecraft-storage: folder scan, page I/O, index.sqlite, watcher,        |
|     auto-save, attachments                                                  |
+-----------------------------------------------------------------------------+
| L0  notecraft-core: geometry, color, units, strokes, input events           |
+-----------------------------------------------------------------------------+
```

Details: [docs/architecture.md](docs/architecture.md).

---

## 4. Feature specification

All features start as "Coming Soon". The phase column says when each lands; checklist ids are in [docs/parity-checklist.md](docs/parity-checklist.md).

### 4.1 Canvas

| Feature | Phase | Notes |
|---|---|---|
| Pan and zoom (10% to 400%) | 0 | wgpu-backed canvas |
| Rule lines and grid | 0 | blank, lined (with the red margin line, as in OneNote), grid, dotted (a kept extra beyond OneNote) |
| Note containers anywhere on the page | 1 | click to create, drag to move, resize width |
| Page grows with content | 1 | |
| Lasso and rectangle select | 3 | across text, ink, shapes, images |
| Arrange (bring forward, send back) | 3 | |
| Snap to grid, alignment guides | 3 | |
| Insert Space | 3 | |
| New Window | 3 | |

### 4.2 Ink and drawing

| Feature | Phase | Notes |
|---|---|---|
| Pen (color, width, pressure) | 1 | mouse and pen tablets |
| Highlighter | 1 | |
| Stroke and point eraser | 1 | |
| Shapes gallery (16 presets), ruler | 3 | |
| Ink to shape | 3 | |
| Touch and palm rejection | 3 | |
| Ink to text (handwriting recognition) | 4 | on-device |
| Search handwritten ink | 4 | |

### 4.3 Text and markdown

Pages are plain markdown (CommonMark plus GFM). The canvas shows rendered text with live editing.

| Feature | Phase | Notes |
|---|---|---|
| Typing, bold, italic, strikethrough, underline | 1 | |
| Headings and styles, lists, indent | 1 | |
| To Do checkboxes | 1 | GFM task lists |
| Tables | 1 (render), 3 (tools) | GFM tables |
| Frontmatter (title, dates, page id, paper, color) | 1 | bounded YAML subset |
| Equations | 2 | LaTeX math |
| Text highlight | 2 | `==text==` |
| Date and time stamps | 2 | Alt+Shift+D, T, F |
| Find on page | 2 | |
| Spell check | 3 | |

### 4.4 Pictures, attachments, media

| Feature | Phase | Notes |
|---|---|---|
| Pictures (PNG, JPEG, WebP, SVG) | 1 | resize with handles |
| Any file as an attachment icon | 1 | opens in the default app |
| Drag and drop, paste pictures | 1 | |
| File printout (PDF pages as images) | 3 | |
| Audio and video recording | 4 | open codecs, linked notes |
| Copy text from picture (OCR) | 4 | |

### 4.5 Notebook structure

```text
Notebook (folder)
 `-- Section group (folder of sections, optional)
      `-- Section (folder)
           `-- Page (.md or .notecraft file), with optional subpages
```

| Feature | Phase | Notes |
|---|---|---|
| Open a folder as a notebook | 1 | |
| Create, rename, delete notebooks, sections, groups, pages | 1 | undoable |
| Section colors, subpages, drag to reorder and move | 1 | |
| Undo and redo | 1 | |
| Notebook Recycle Bin | 3 | `.notecraft/trash/` |
| Page Versions | 3 | `.notecraft/history/` |
| Page templates | 3 | |
| Password-protected sections | 4 | encrypted at rest |

### 4.6 Ribbon and chrome (OneNote 2020 look)

| Feature | Phase | Notes |
|---|---|---|
| Tabs: File (backstage), Home, Insert, Draw, History, Review, View | 0 | OneNote's tabs and groups, original icons |
| Collapsible ribbon, key tips | 0 | Ctrl+F1, Alt |
| Navigation: notebooks, section tabs, page list | 0 | placement follows the theme preset |
| Status bar | 0 | zoom, page info, save state |
| Full Page View | 0 | F11 |
| OneNote shortcut map | 0 | |
| Accessibility | 0 | keyboard navigation, screen reader labels |
| Quick Access Toolbar | 3 | |
| Docked window | 4 | |
| Localized UI, right-to-left | 3 (string table), 4 | |

### 4.7 Themes

Four presets, each light and dark. Colors are original values chosen to evoke each era (OneNote's identity color is purple); they are never sampled from Microsoft artwork. Full token tables: [docs/themes.md](docs/themes.md).

| Theme | Character |
|---|---|
| OneNote 2016 | flat purple ribbon, light gray navigation, square corners |
| OneNote 2019 | deeper purple, cleaner spacing |
| OneNote 2021 | light ribbon, purple accent on the selected tab, muted colors |
| Microsoft 365 | soft surfaces, rounded corners, accent on selection only |

Each theme defines ribbon, navigation, and canvas colors; button style; corner radius; spacing; and shadow style. NoteCraft uses the OS UI font (Segoe UI on Windows) and bundles an openly licensed fallback only if needed.

### 4.8 Page links, search, note tags

| Feature | Phase | Notes |
|---|---|---|
| `[[Page]]` links, page picker, create missing page | 2 | as in OneNote |
| Links update on rename and move | 2 | |
| Copy link to page | 3 | `notecraft://` URL |
| Search with scopes (page to all notebooks) | 2 | SQLite FTS5 |
| Note tags (To Do, Important, Question, and more), Customize Tags | 2 | stored as inline tags |
| Find Tags summary and summary page | 2 | |

### 4.9 File format

| Feature | Phase | Notes |
|---|---|---|
| Read and write `.md` with minimal diffs | 1 | outside edits keep their formatting |
| Read and write `.notecraft` | 1 | markdown with container markers and one `notecraft-canvas` block |
| Auto-save (every 30 s and on focus loss) | 1 | |
| Crash recovery, outside edit detection | 1 | |
| Export to PDF, HTML, notebook zip | 3 | |
| Import OneNote `.one` | 4 | best effort, from Microsoft's published spec |

Full spec: [docs/markdown.md](docs/markdown.md).

### 4.10 Automation

Every action is a command with a stable id, used by the ribbon, shortcuts, and a minimal control channel for UI tests (Phase 0). The command line and its MCP server ([docs/cli.md](docs/cli.md), [docs/mcp.md](docs/mcp.md)) are not OneNote features but are in the build plan by maintainer decision (2026-10-15). A command palette, a full control channel, and plugins are not OneNote features; their specs ([docs/control-protocol.md](docs/control-protocol.md)) are kept for a possible post-parity effort ([docs/post-parity.md](docs/post-parity.md)).

---

## 5. Alpha gate (v0.1.0)

| Area | Must work |
|---|---|
| UI shell | Full ribbon, all OneNote tabs visible, every button clickable (no-ops show a "Coming Soon" toast) |
| Canvas | Pan and zoom on an empty page, rule lines and grid |
| Navigation | Notebook list, section tabs, and page list from a real folder |
| Theme switching | Switching between the 2016, 2019, 2021, and M365 presets updates colors |
| Markdown read | Open a `.md` file and render it on the canvas |
| Markdown write | Type plain text in a container and save to `.md` |
| Open dialog | Open a notebook folder, `.md`, or `.notecraft` |
| Auto-save | Save the current page every 30 seconds |
| Dark mode | Toggle dark and light |
| Recent files | File backstage shows the last 10 opened files |

Everything else shows a "Coming Soon" badge. Evidence per row: [docs/roadmap.md](docs/roadmap.md#alpha-gate).

---

## 6. Development phases

Hours are summed bottom-up from every PR in [build_plan/](build_plan/README.md); each range includes tests, docs, and review. Calendar weeks assume about 30 focused hours a week.

| Phase | Theme | Hours | Calendar | Ends with |
|---|---|---|---|---|
| -1 | Scaffolding and CI/CD | 55-98 | 2-4 weeks | CI green, release pipeline proven |
| 0 | UI shell | 284-474 | 9-16 weeks | internal 0.0.1 |
| 1 | Core engine | 454-772 | 15-26 weeks | v0.1.0 alpha |
| 2 | Page links, search, note tags | 134-230 | 4-8 weeks | v0.2.0 |
| 3 | Polish and parity | 392-666 | 13-23 weeks | v0.9.0 beta |
| Hardening | Beta to 1.0 | 80-160 | 3-6 weeks | v1.0.0 |
| 4 | Remaining parity | 644-1,228 | 22-41 weeks | full OneNote parity |
| Total to 1.0 | | 1,399-2,400 | 47-80 weeks | |
| Total to full parity | | 2,043-3,628 | 68-121 weeks | |

### Phase -1: scaffolding and CI/CD

Done first. Cargo workspace and crate skeletons; `cargo xtask` with `ci`, `text`, `assets`, `layers`, `wasm`, `parity`, and `version set`; packaging scripts for the shared release workflow; logging; GitHub settings. CI/CD mirrors the related craft apps: `ci.yml`, `printing.yml`, `freebsd.yml`, `windows-arm64.yml`, `packaging-lint.yml`, and `release.yml`, which call the reusable workflows in storytold/craft-common-actions. Releases are built by pushing to the `release` branch. Issue templates, PR template, code of conduct, and Dependabot (cargo only) are NoteCraft additions.

### Phase 0: UI shell

egui app on wgpu; command registry with "Coming Soon" flow; theme tokens and the four presets; the ribbon with every OneNote tab and original icons; navigation on mock data; canvas pan, zoom, and rule lines; File backstage with recents and the open dialog; status bar, full page view, shortcut map; accessibility; minimal control channel for UI tests.

### Phase 1: core engine

Notebook folder reader and index; markdown read and minimal-diff write; live text editing in note containers; formatting commands; auto-save, crash recovery, outside edits; undo and redo; `.notecraft` format; ink pen, highlighter, erasers; pictures and attachments; structure editing; fuzzed parsers. Ends with the alpha.

### Phase 2: page links, search, note tags

`[[Page]]` links with resolution and rename updates; search with scopes; note tags and Find Tags; equations; highlight; date and time stamps; find on page.

### Phase 3: polish and parity

Shapes and ruler; ink to shape; selection, arrange, snap, Insert Space; table tools and spell check; recycle bin, page versions, templates; PDF export; printing through craft-print; HTML and zip export, file printouts; copy link to page, new window, Quick Access Toolbar; touch; string table; performance. Ends with the beta, then hardening to 1.0.

### Phase 4: remaining parity (post-1.0)

Handwriting recognition and ink search; `.one` import; sync research; browser build; docked window; right-to-left and translations; OCR; audio and video recording; password-protected sections.

---

## 7. Dependencies (planned)

| Crate | Purpose |
|---|---|
| `egui`, `eframe` | UI framework |
| `wgpu` | GPU rendering (through eframe) |
| craft-libs: `craft-ui`, `craft-settings`, `craft-atomic`, `craft-print`, `craft-quit`, `craft-cms` (optional) | shared ArtCraft components (docs/craft-libs.md) |
| `comrak` | markdown parsing |
| `rusqlite` (bundled) | index database |
| `notify` | file watching |
| `serde`, `serde_json`, `toml` | serialization |
| `kurbo` | curve geometry for ink and shapes |
| `image`, `resvg` | picture decoding |
| `rfd` | file dialogs (L6 and L7 only) |
| `tracing`, `tracing-subscriber` | logging |
| `clap` | `notecraft-cli` arguments |
| Equation renderer | chosen in Phase 2 (PR 2.7), pure Rust, permissive license |
| PDF writer | chosen in Phase 3 (PR 3.6), pure Rust, permissive license |

No crate that needs `unsafe` in NoteCraft's own code; OS integrations go through safe wrapper crates.

---

## 8. Project structure

```text
notecraft/
|-- Cargo.toml, Cargo.lock                (Phase -1)
|-- README.md, ROADMAP.md, SCRD.md, AGENTS.md, CLAUDE.md, CONTRIBUTING.md
|-- ATTRIBUTION.md, NOTICE, LICENSE-MIT, LICENSE-APACHE
|-- rustfmt.toml, clippy.toml, .editorconfig, .gitattributes, .gitignore, .shellcheckrc
|-- .cargo/config.toml                    (xtask alias)
|-- .github/
|   |-- workflows/  ci, printing, freebsd, windows-arm64, packaging-lint, release
|   |-- ISSUE_TEMPLATE/  bug_report, feature_request, config_change, plugin_suggestion, question, config.yml
|   |-- pull_request_template.md
|   |-- CODE_OF_CONDUCT.md
|   `-- dependabot.yml
|-- apps/
|   |-- notecraft/            desktop app (main, app, logging, control channel, build.rs)
|   |-- notecraft-cli/        --version
|   `-- notecraft-web/        Phase 4
|-- crates/
|   |-- notecraft-core/       L0
|   |-- notecraft-storage/    L1
|   |-- notecraft-model/      L2
|   |-- notecraft-markdown/   L3
|   |-- notecraft-canvas/     L4
|   |-- notecraft-engine/     L5 (cmd/, session, undo, export/)
|   `-- notecraft-ui-egui/    L6 (ribbon/, nav/, canvas/, themes/, icons/)
|-- xtask/                ci, text, assets, layers, wasm, parity, version
|-- packaging/            env.sh, macos/, windows/, linux/, freebsd/
|-- assets/app-icon/      original icon, hicolor sizes
|-- docs/                 progress docs and specs
|-- build_plan/           execution plan
`-- testdata/             original sample notebooks and markdown corpus
```

---

## 9. Parity assessment strategy

Parity is measured against OneNote only, in [docs/target-app-parity.md](docs/target-app-parity.md) and row by row in [docs/parity-checklist.md](docs/parity-checklist.md).

| Area | Alpha | Beta | 1.0 |
|---|---:|---:|---:|
| Canvas and freeform layout | 15% | 70% | 90% |
| Ink and drawing | 10% | 65% | 85% |
| Text and formatting | 30% | 80% | 95% |
| Notebook structure | 40% | 85% | 90% |
| Ribbon and chrome | 50% | 85% | 90% |
| Themes | 60% | 90% | 98% |
| Attachments and media | 15% | 70% | 75% |
| Page links, search, note tags | 0% | 65% | 85% |
| Print and export | 0% | 70% | 90% |
| Overall | about 25% | about 70% | about 90% |

Phase 4 takes each area to full parity.

---

## 10. Priority matrix

| Tag | Meaning |
|---|---|
| P0 | must work for v0.1.0 alpha |
| P1 | must work for v0.9.0 beta |
| P2 | should work for v1.0.0 |
| P3 | Phase 4, completes full parity |

| Feature | Priority | Phase |
|---|---|---|
| CI/CD, templates, contributing guide, agent docs, Dependabot, code of conduct | P0 | -1 |
| Cargo workspace scaffold | P0 | -1 |
| Ribbon (all OneNote tabs) | P0 | 0 |
| Canvas pan and zoom | P0 | 0 |
| Theme switching, four presets, dark mode | P0 | 0 |
| Navigation (notebook, section, page) | P0 | 0 |
| Open dialog, recent files | P0 | 0 |
| Markdown read and write | P0 | 1 |
| Auto-save | P0 | 1 |
| Pen, highlighter, eraser | P1 | 1 |
| Pictures and attachments | P1 | 1 |
| Undo and redo | P1 | 1 |
| Page links | P1 | 2 |
| Search | P1 | 2 |
| Note tags and Find Tags | P1 | 2 |
| Equations | P1 | 2 |
| Shapes, ink to shape, selection tools | P1 | 3 |
| Tables tools, spell check | P1 | 3 |
| Print and PDF export | P1 | 3 |
| Recycle bin, page versions, templates | P2 | 3 |
| HTML export, file printouts | P2 | 3 |
| Quick Access Toolbar, new window, copy link to page | P2 | 3 |
| Handwriting recognition, ink search | P3 | 4 |
| OneNote `.one` import | P3 | 4 |
| Audio and video recording | P3 | 4 |
| Password-protected sections | P3 | 4 |
| OCR, docked window, translations, RTL, browser build | P3 | 4 |
| Backlinks, graph view, command palette, plugins, MCP bridge, full control channel | not scheduled | post-parity (optional) |

---

## 11. Non-functional requirements

| Requirement | Target |
|---|---|
| Startup time | under 2 seconds (cold start) |
| Large notebook (1,000+ pages) | navigation loads in under 500 ms |
| Canvas with 500 objects | 60 FPS at 100% zoom |
| Memory (typical session) | under 200 MB |
| Installer size | under 50 MB (Windows MSI) |
| Log file | 16 MiB cap |
| Crash recovery | unsaved edits recovered on next launch |
| Offline-first | no network required for any feature |
| Accessibility | keyboard navigation, screen reader labels |

---

## 12. Logging and diagnostics

NoteCraft follows the ArtCraft logging convention:

- The desktop app logs to stderr and to `logs/notecraft.log` in the settings folder:
  - Windows: `%APPDATA%\NoteCraft\logs\`
  - macOS: `~/Library/Application Support/NoteCraft/logs/`
  - Linux: `$XDG_CONFIG_HOME/notecraft/logs/`
- Each launch rotates the previous log (`notecraft.1.log`, `notecraft.2.log`).
- The log file caps at 16 MiB.
- Default level: `info` for NoteCraft crates, `warn` for dependencies.
- `RUST_LOG` overrides levels.
- `NOTECRAFT_NO_PREFS` disables file logging and settings writes (for agent test runs).
- Note content is never logged.

---

## 13. License and credits

NoteCraft is dual-licensed under MIT OR Apache-2.0, at the user's option.

Microsoft, OneNote, and Microsoft 365 are trademarks of Microsoft Corporation. NoteCraft is an independent, open-source project and is not affiliated with, sponsored by, or endorsed by Microsoft Corporation. These names are used only to describe the workflow NoteCraft is compatible with.

---

## 14. References

| Reference | Description |
|---|---|
| [WordCraft](https://github.com/storytold/wordcraft) | Related ArtCraft app (Word remake): UI-first pattern, command registry, printing workflow |
| [GridCraft](https://github.com/storytold/gridcraft) | Related ArtCraft app (Excel remake): layered architecture, packaging scripts |
| [DeckCraft](https://github.com/storytold/deckcraft) | Related ArtCraft app (PowerPoint remake): shapes, drawing, canvas |
| [craft-libs](https://github.com/storytold/craft-libs) | Shared crates (craft-ui, craft-print, craft-atomic, craft-settings, craft-quit, craft-cms) |
| [craft-common-actions](https://github.com/storytold/craft-common-actions) | Shared reusable CI/CD workflows |
| [egui](https://github.com/emilk/egui) | Immediate mode GUI for Rust |
| [wgpu](https://github.com/gfx-rs/wgpu) | Graphics API for Rust |
| [comrak](https://github.com/kivikakk/comrak) | CommonMark and GFM parser |
| [Microsoft OneNote](https://www.onenote.com) | The application NoteCraft remakes |

---

This SCRD is a living document. Features marked "Coming Soon" are updated as implementation progresses. See [ROADMAP.md](ROADMAP.md) for stage estimates and the progress log.
