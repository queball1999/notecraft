# Changelog

All notable changes to NoteCraft are listed here. Versions follow [Semantic Versioning](https://semver.org/). Pre-1.0 releases may change anything.

## 0.0.2 - 2026-10-19

Alpha release at the end of Phase 1. NoteCraft now opens, edits, and saves real notebooks on disk in its markdown-first format.

### Added

- Notebook folders read from disk, with a rebuildable index that keeps the navigation fast and current; frontmatter titles shown in the page list.
- Create, rename, move, color, and delete notebooks, section groups, sections, and pages; deleted items go to the notebook trash; collapse and expand subpages.
- Markdown read and minimal-diff write: saving changes only the bytes you edited. `.notecraft` pages with a canvas block for ink, container positions, pictures, and files; a `.md` page is promoted when it gets canvas content, and can be converted back.
- Text editing in containers (caret, selection, IME, clipboard, move, resize) and formatting commands (bold, italic, strikethrough, underline, sub and superscript, highlight, color, size, family, headings, lists, indent, tasks, quote, code, alignment).
- Ink: pen, highlighter with a true multiply blend, pencil and textured pens, stroke and point erasers, smoothing, an editable pen list, the Eyedropper, and pen pressure on Windows.
- Pictures (PNG, JPEG, WebP, SVG) and file attachments: insert, paste, drag and drop, place freely, resize, and open.
- Undo and redo for typing, ink, page style, and notebook changes.
- Auto-save every 30 seconds and on focus loss, crash recovery, and a conflict bar when a page changes on disk.
- Page style (rule lines, rule line color, page color, paper size) saved in the page frontmatter.
- What's New dialog on the first launch after an update.
- `notecraft-cli` `info`, `text`, `check`, and `run --page` on notebooks.
- Fuzz targets and property tests for every page parser.

### Changed

- Bold text is drawn with the OS bold face when one is found, instead of overstriking.

### Fixed

- Linux: when the only Vulkan adapter is a software renderer (llvmpipe), NoteCraft uses the GL backend instead of aborting at startup in winit. `WGPU_BACKEND` still wins.

### Known limitations

- Builds are not code signed (Windows) or notarized (macOS); see issue #5.
- Pen pressure is only read on Windows; macOS and Linux use speed-based width.
- Several checks that need a person at a real machine (real pen, IME, OS dialogs, screen readers, macOS and Linux dark mode) are still open; see the hands-on checklist in docs/roadmap.md.

## 0.0.1 - 2026-10-15

Internal test release at the end of Phase 0. It proves the release pipeline end to end with the real interface. The app shows mock notebooks only: it does not read or write notes yet (that is Phase 1).

### Added

- Main window on wgpu with saved size and position, logging, and `--version`.
- Command registry: every button and shortcut runs a command with a stable id; commands that are not built yet show a "Coming Soon" tooltip and toast.
- Four theme presets after OneNote 2016, 2019, 2021, and Microsoft 365, each in light and dark, following the system dark mode by default, with the OS UI font.
- Ribbon with the File, Home, Insert, Draw, History, Review, View, and Help tabs and original icons; collapse (Ctrl+F1), key tips, Ribbon Display Options, and the simplified ribbon.
- Font and size boxes listing the installed fonts, Office-style color menus with Recent Colors and a Colors dialog, and Styles and Tags galleries.
- Notebook list, colored section tabs, and page list with subpages, placed as each preset places them.
- Canvas with pan, zoom, rule lines and grids, dotted paper, the ruled margin line, paper sizes, and a Rule Lines menu of paper thumbnails.
- Start page with recent notebooks and pages, open dialogs for notebooks, `.md`, and `.notecraft` files, and opening a path passed at launch.
- Status bar with page and zoom, full page view (F11), and the OneNote default shortcut map.
- Keyboard navigation, focus rings, and screen reader names for every control.
- File backstage About page with version and build details, licenses, and a Copy button for bug reports.
- `notecraft-cli` with `commands` and `run`, and a minimal local control channel for UI tests (`notecraft --control <port>`).
