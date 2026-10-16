# Changelog

All notable changes to NoteCraft are listed here. Versions follow [Semantic Versioning](https://semver.org/). Pre-1.0 releases may change anything.

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
