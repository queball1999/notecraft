# Phase 0: UI shell

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: PR 0.10 recorded: keyboard navigation and screen reader labels.

Goal: the whole OneNote interface, fully clickable, on mock data. Every ribbon button exists and is bound to a command id. Buttons whose engine work is not done show a "Coming Soon" badge and toast. Themes switch live. The canvas pans and zooms. Nothing reads or writes real notes yet (that is Phase 1).

| | |
|---|---|
| Phase branch | `phase/0-ui-shell` |
| Hours | see "Estimate" at the end |
| Checklist rows | SH-01 to SH-09, SH-11 to SH-15, TH-01 to TH-06, CV-01 to CV-03 |
| Gaps | ranks 6 to 15 |

## Entry criteria

- Phase -1 exit criteria met; `phase/0-ui-shell` created from `main`.

## Layout target

```text
+-----------------------------------------------------------------------------+
| [QAT]  NoteCraft - Meeting notes - Work                         [_] [ ] [X] |
+-----------------------------------------------------------------------------+
| File | Home | Insert | Draw | History | Review | View                       |
+-----------------------------------------------------------------------------+
|  ribbon groups: [Clipboard] [Basic Text] [Styles] [Tags] [Email] ...        |
+-----------------+-----------------------------------------------------------+
| Notebooks     v | [Work] [Personal] [Projects] [+]       <- section tabs    |
|  My Notebook    +------------------------------------+----------------------+
|  Work Notebook  |  Meeting notes                     | Pages         [+]    |
|                 |  Tuesday, October 14, 2026 9:30 AM |  Meeting notes       |
|                 |                                    |    Action items      |
|                 |   (canvas: lined paper, pan/zoom)  |  Reading list        |
|                 |                                    |                      |
+-----------------+------------------------------------+----------------------+
| Zoom 100% | Page 1 of 3 | Saved                                             |
+-----------------------------------------------------------------------------+
```

The page list sits on the right of the canvas in the 2016 and 2019 presets, and next to the section list on the left in the 2021 and Microsoft 365 presets, matching where each era put it. This is a layout option driven by the theme, not separate code paths.

## PR 0.1: egui app scaffold

| | |
|---|---|
| Branch | `feat/app-scaffold` off `phase/0-ui-shell` |
| Hours | 8-14 |
| Rows | SH-01 |

Files: `apps/notecraft/src/{main.rs,app.rs}`, `crates/notecraft-ui-egui/src/{lib.rs,app_shell.rs}`, workspace deps `egui`, `eframe` (wgpu feature), `craft-ui`, `craft-quit` (macOS), `craft-settings`.

Steps:

1. Add `egui`, `eframe` (features `wgpu`, `accesskit`), and the pinned craft-libs `rev` to `[workspace.dependencies]` (docs/craft-libs.md section 1).
2. `notecraft-ui-egui::AppShell` implements `eframe::App` and owns empty panels (top, left, right, bottom, central).
3. `apps/notecraft` builds `NativeOptions` with the wgpu renderer, window title "NoteCraft", minimum size 800 x 600, restores window size and position from `craft-settings`.
4. macOS: hook `craft-quit` for the app menu and Cmd+Q.
5. UI test: launch `AppShell` headless with `egui_kittest` (or what WordCraft uses), render one frame, assert no panic.

Commits:

1. `build(deps): add egui, eframe, and pinned craft-libs`
2. `feat(ui): add app shell with empty panels`
3. `feat(app): open main window on wgpu and restore geometry`
4. `feat(app): wire craft-quit on macos`
5. `test(ui): render one frame of the app shell headless`

PR title: `feat(app): open the main window with an empty shell`. Screenshot required (empty window on each OS).

Acceptance: window opens on Windows (DX12), macOS (Metal), Linux (Vulkan); `cargo xtask layers` still passes (egui only in L6 and L7).

## PR 0.2: command registry

| | |
|---|---|
| Branch | `feat/command-registry` off `phase/0-ui-shell` |
| Hours | 12-20 |
| Rows | SH-03, SH-04 |

Files: `crates/notecraft-engine/src/cmd/{mod.rs,registry.rs,guard.rs,ids.rs}`, `crates/notecraft-engine/src/session.rs`, `crates/notecraft-ui-egui/src/toast.rs`, `apps/notecraft-cli/src/commands.rs`.

Steps:

1. `CommandId` newtype over `&'static str`; `ids.rs` lists every id as a constant, grouped by area, with a doc comment each. Start with the full list for every ribbon button in PR 0.5 (about 150 ids), most with `Status::ComingSoon`.
2. `CommandSpec { id, label, description, args_schema, undoable, destructive, default_shortcut, status }`.
3. `Registry::run(&mut Session, id, args) -> Result<CommandOutput, CommandError>`; unknown id and Coming Soon are errors, not panics.
4. `guard.rs`: wrap execution in `catch_unwind`; on panic, log, return `CommandError::Internal`, leave the session as it was.
5. Toast widget in `notecraft-ui-egui`: "Coming Soon: <label>" for 3 seconds.
6. Tests: every id is unique, matches `^[a-z]+(\.[a-z_]+)+$`, and has a label; a test command that panics is caught by the guard.

Commits:

1. `feat(engine): add command spec, ids, and registry`
2. `feat(engine): add panic guard around command execution`
3. `feat(engine): register all phase 0 ribbon command ids`
4. `feat(ui): add toast for coming soon commands`
5. `test(engine): cover registry ids and panic guard`

PR title: `feat(engine): add the command registry and coming soon flow`.

## PR 0.3: theme system and tokens

| | |
|---|---|
| Branch | `feat/theme-tokens` off `phase/0-ui-shell` |
| Hours | 16-28 |
| Rows | TH-05, TH-06 |

Files: `crates/notecraft-ui-egui/src/themes/{mod.rs,tokens.rs,apply.rs}`.

Steps:

1. `ThemeTokens` struct with every token in docs/themes.md section 3. `Preset` enum and `Mode` enum.
2. `apply.rs`: map tokens onto `egui::Style` and `Visuals`, using `craft-ui` theme helpers where they exist.
3. Settings `theme.preset` and `theme.mode` through `craft-settings`; `system` mode follows the OS dark setting (eframe exposes it).
4. Commands `view.theme.set`, `view.mode.toggle`, `view.mode.set` (now Status::Ready).
5. WCAG AA contrast test helper (docs/themes.md section 6).

Commits:

1. `feat(ui): add theme tokens, presets, and modes`
2. `feat(ui): apply theme tokens to egui style`
3. `feat(ui): persist theme and follow system dark mode`
4. `feat(engine): implement theme commands`
5. `test(ui): add contrast check helper`

PR title: `feat(ui): add the theme token system`.

## PR 0.4: four theme presets

| | |
|---|---|
| Branch | `feat/theme-presets` off `phase/0-ui-shell` |
| Hours | 24-40 |
| Rows | TH-01 to TH-04 |

Files: `crates/notecraft-ui-egui/src/themes/{onenote2016.rs,onenote2019.rs,onenote2021.rs,m365.rs}`; possibly `assets/fonts/` and ATTRIBUTION.md.

Steps:

1. Encode the palettes from docs/themes.md section 4. Tune by eye against the running shell; record any change in docs/themes.md.
2. Font decision: use the OS UI font (docs/themes.md section 2). Decide whether to bundle a fallback from storytold/craft-fonts. If yes: add it, add the ATTRIBUTION.md row, and add `craft-fonts-ref` to `release.yml` and `freebsd.yml` like DeckCraft. Record the decision in docs/craft-libs.md.
3. Contrast test runs over all eight variants.
4. Screenshots of all eight variants go in the PR.

Commits:

1. `feat(ui): add onenote 2016 preset`
2. `feat(ui): add onenote 2019 preset`
3. `feat(ui): add onenote 2021 preset`
4. `feat(ui): add microsoft 365 preset`
5. `feat(ui): load os ui font with bundled fallback` (if bundling)
6. `test(ui): check contrast for all eight theme variants`
7. `docs: record theme tuning and font decision`

PR title: `feat(ui): add the four OneNote theme presets`.

## PR 0.5: ribbon

| | |
|---|---|
| Branch | `feat/ribbon` off `phase/0-ui-shell` |
| Hours | 60-100 |
| Rows | SH-02, SH-05 |

Files: `crates/notecraft-ui-egui/src/ribbon/{mod.rs,tab.rs,group.rs,button.rs,spec.rs}`, `crates/notecraft-ui-egui/src/icons/` (original vector icons drawn in code), ATTRIBUTION.md rows for any icon files.

Steps:

1. Ribbon is data: `spec.rs` declares tabs, groups, and buttons (label, icon id, command id, size large or small, dropdown items). The ribbon widget renders the spec.
2. Tabs and groups (mirror OneNote 2016 to 2021 in names and order; no tabs OneNote does not have):
   - File: backstage view with New, Open, Info, Print, Export, Settings, About.
   - Home: Clipboard, Basic Text, Styles, Tags, Email (email shows Coming Soon), Meetings (Coming Soon).
   - Insert: Insert Space, Tables, Files (attachment, printout), Images (pictures, screenshot), Links, Recording (Coming Soon), Time Stamp, Pages (templates), Symbols (equation, symbol).
   - Draw: Tools (select, lasso, panning hand, pens gallery, highlighter, eraser), Shapes, Edit (insert space, delete, arrange, rotate), Convert (ink to shape, ink to text).
   - History: Unread, Authors (Coming Soon), Page Versions, Notebook Recycle Bin.
   - Review: Spelling, Language, Section (password, Coming Soon), Notes.
   - View: Views (normal, full page, dock), Page Setup (paper color, rule lines, hide title), Zoom, Window (new window), Theme (preset, dark mode).
3. Icons: original, simple vector drawings using `egui::Painter`, one function per icon, 16 and 32 px. Never trace or copy Microsoft icons.
4. Collapse and expand with Ctrl+F1 and a chevron; double-click a tab toggles collapse.
5. Coming Soon badge: small pill in the top right of a button, color `badge_bg`.
6. Keyboard access: Alt shows key tips (letters) on tabs and buttons.

Commits (one per tab keeps review manageable):

1. `feat(ui): add data-driven ribbon widget`
2. `feat(ui): add original vector icon set for the ribbon`
3. `feat(ui): add file backstage view`
4. `feat(ui): add home tab`
5. `feat(ui): add insert tab`
6. `feat(ui): add draw tab`
7. `feat(ui): add history, review, and view tabs`
8. `feat(ui): add ribbon collapse and key tips`
9. `test(ui): every ribbon button maps to a registered command`

PR title: `feat(ui): add the ribbon with every tab and button`. Screenshots: each tab in two presets.

If the diff passes 1,000 lines, split into 0.5a (widget, icons, File, Home) and 0.5b (other tabs).

Split applied: PR 0.5a (PR #9, about 1,800 lines) landed commits 1 to 4 and 9 for the File and Home tabs, with the icons and the widget in one commit each. PR 0.5b covers commits 5 to 8 (Insert, Draw, History, Review, and View tabs with their icons, collapse, and key tips) and extends the command-mapping test to the new tabs. The 60-100 hour estimate covers both. Step 5 (the Coming Soon badge) was dropped at the maintainer's request: with almost every command Coming Soon, a pill on each button was noise. The tooltip and the toast say Coming Soon instead. PR 0.5b (PR #10) landed the other five tabs, ribbon collapse (Ctrl+F1, the chevron, double-click a tab; a collapsed ribbon shows a tab's groups over the page when the tab is clicked, and the collapsed state is saved), and key tips (Alt or F10, then a tab letter, then a button code; Alt plus a tab letter works too). view.ribbon.toggle is Ready.

## PR 0.6: sidebar, section tabs, page list

| | |
|---|---|
| Branch | `feat/navigation` off `phase/0-ui-shell` |
| Hours | 24-40 |
| Rows | SH-06, SH-07, SH-08 |

Files: `crates/notecraft-ui-egui/src/nav/{notebooks.rs,sections.rs,pages.rs,mock.rs}`, `crates/notecraft-model/src/{notebook.rs,section.rs,page.rs}` (types only).

Steps:

1. Model types in `notecraft-model`: `Notebook`, `SectionNode` (section or group), `PageRef` (title, level, id). No I/O.
2. `mock.rs`: two notebooks, five sections (one group), twenty pages with subpages.
3. Notebook pane: collapsible list with color swatches.
4. Section tabs: colored tabs across the top of the canvas, overflow menu, "+" tab.
5. Page list: titles with indent for subpages, "+ Add page", selection highlight.
6. Clicking changes the session's current page (mock). Commands `nav.page.select`, `nav.section.select`, `nav.notebook.select`.
7. Placement follows the theme (see Layout target).

Commits:

1. `feat(model): add notebook, section, and page reference types`
2. `feat(ui): add mock notebook data`
3. `feat(ui): add notebook pane`
4. `feat(ui): add colored section tabs`
5. `feat(ui): add page list with subpage indent`
6. `feat(engine): add navigation commands`

PR title: `feat(ui): add notebook, section, and page navigation`.

## PR 0.7: canvas pan, zoom, paper

| | |
|---|---|
| Branch | `feat/canvas-viewport` off `phase/0-ui-shell` |
| Hours | 24-40 |
| Rows | CV-01, CV-02, CV-03 |

Files: `crates/notecraft-canvas/src/{viewport.rs,paper.rs}`, `crates/notecraft-ui-egui/src/canvas/{mod.rs,widget.rs}`.

Steps:

1. `Viewport` in `notecraft-canvas` (no egui): offset, zoom (0.1 to 4.0), screen-to-page and page-to-screen transforms, zoom about a point. Property tests for round trips.
2. Canvas widget: scroll to pan, middle-drag and Space+drag to pan, Ctrl+wheel and pinch to zoom about the pointer, scroll bars.
3. Paper: blank, lined (rule spacing 0.25 inch), grid (0.25 inch), dotted; color from `canvas_rule`. Drawn only for the visible area.
4. Page title area and date line (mock data).
5. Commands: `view.zoom_in`, `view.zoom_out`, `view.zoom_100`, `view.zoom_fit_width`, `view.paper.set`.

Commits:

1. `feat(canvas): add viewport transforms and zoom limits`
2. `test(canvas): property-test viewport round trips`
3. `feat(ui): add canvas widget with pan and zoom`
4. `feat(ui): draw paper styles for the visible area`
5. `feat(ui): draw page title and date area`
6. `feat(engine): add zoom and paper commands`

PR title: `feat(canvas): add pan, zoom, and paper styles`.

PR 0.7 (PR #13) landed with these changes to the plan. The command ids are the ones the View tab has used since PR 0.5b: `view.zoom_in`, `view.zoom_out`, `view.zoom`, `view.zoom_actual`, `view.zoom_page_width`, `view.rule_lines`, `view.page_color`, and `view.paper_size` (not `view.zoom_100`, `view.zoom_fit_width`, or `view.paper.set`). Paper styles follow OneNote's Rule Lines menu (none, narrow, college, standard, and wide ruled; small, medium, large, and very large grid); dotted was left out because OneNote does not appear to have it, pending the maintainer's confirmation. The rule color is `canvas_rule`. The engine commit came before the UI commits, because the widget reads the zoom from the session, and the paper styles got their own `feat(canvas)` commit. Zoom is session state set only by commands, so Ctrl+wheel and pinch dispatch `view.zoom`; the scroll position stays in the front end.

## PR 0.8: start page, recent files, file picker

| | |
|---|---|
| Branch | `feat/start-page` off `phase/0-ui-shell` |
| Hours | 12-20 |
| Rows | SH-11, SH-12 |

Steps:

1. Start page shown when nothing is open: New notebook, Open notebook, Open file, recent list (last 10 notebooks and pages, from `craft-settings`).
2. File picker through `rfd` (only in L6 and L7): folders for notebooks, `.md` and `.notecraft` for files. Phase 0 records the choice in recents and shows the mock notebook; Phase 1 loads it for real.
3. Commands: `file.new_notebook`, `file.open_notebook`, `file.open`, `file.recent.clear`.

Commits:

1. `feat(ui): add start page`
2. `feat(ui): add recent files list backed by settings`
3. `feat(ui): add open dialogs for notebooks and pages`

PR title: `feat(ui): add the start page, recents, and file picker`.

PR 0.8 (PR #14) landed with these changes to the plan. `file.open_notebook` and `file.recent.clear` are new ids; `file.open` now opens page files only, and Ctrl+O moved to `file.open_notebook` (the shortcut is bound in PR 0.9). The recent list lives in the engine session (`notecraft-engine/src/recent.rs`) and the app saves it in `config.toml`; the mock notebooks moved into `notecraft-engine/src/mock.rs` so the open commands can show them. `rfd` is used only in the desktop app crate (L7), behind a `FilePicker` trait in the UI crate, so tests run with a fake picker. The File backstage's New and Open entries show pages with their own buttons (and the recent list on Open). At launch the app reopens the newest recent notebook or page, as OneNote reopens what was open; with nothing recent it shows the start page. Commits: the engine commands first, then one UI commit for the start page and backstage pages, one app commit for the dialogs and saved recents, and a small fix giving the page buttons a visible frame.

## PR 0.9: status bar, full page view, shortcut map

| | |
|---|---|
| Branch | `feat/status-shortcuts` off `phase/0-ui-shell` |
| Hours | 12-20 |
| Rows | SH-09, SH-13, SH-14 |

Steps:

1. Status bar: zoom percent (click for zoom menu), page n of m, save state.
2. Full page view (F11): hide ribbon and navigation; Esc or F11 to leave.
3. Shortcut map in `notecraft-engine/src/cmd/shortcuts.rs`: OneNote defaults where they do not clash with the OS (Ctrl+N new page, Ctrl+T new section, Ctrl+E search, Ctrl+1 to Ctrl+9 tags, Ctrl+Alt+D dock, Ctrl+F1 ribbon, Alt+Shift+D date, Alt+Shift+T time). macOS maps Ctrl to Cmd.
4. Test: no two commands share a shortcut in the same context.

Commits:

1. `feat(ui): add status bar`
2. `feat(ui): add full page view`
3. `feat(engine): add onenote default shortcut map`
4. `test(engine): reject duplicate shortcuts`

PR title: `feat(ui): add status bar, full page view, and shortcuts`.

PR 0.9 (PR #15) landed with these changes to the plan. The status bar shows the page position and the zoom; save state waits for auto-save (FM-05), since Phase 0 has nothing to save. Ctrl+E is bound to a new Coming Soon command, `search.all`, until Phase 2 builds search. The shortcut map is built from each command's `default_shortcut` plus a short table of bindings with arguments (Ctrl+2 to Ctrl+9 note tags), so there is one place to declare a shortcut. Commit order: the engine map and its duplicate test are one commit, then the shell dispatch (Ctrl+F1 moved into the map, and egui's own Ctrl+=, Ctrl+-, and Ctrl+0 UI scaling turned off), the full page view, and the status bar.

## PR 0.10: accessibility

| | |
|---|---|
| Branch | `feat/accessibility` off `phase/0-ui-shell` |
| Hours | 10-16 |
| Rows | SH-15 |

Steps: AccessKit labels on every ribbon button, tab, and navigation item; logical Tab order through ribbon, navigation, canvas; visible focus ring with `accent`; test with Narrator (Windows) and VoiceOver (macOS) and record the results in the PR.

Commits:

1. `feat(ui): label every control for screen readers`
2. `feat(ui): add keyboard focus order and focus ring`

PR title: `feat(ui): add keyboard navigation and screen reader labels`.

PR 0.10 (PR #16) landed both commits. An audit of the AccessKit tree found the ribbon, menus, navigation, backstage, and start page already named; the panel resize handles were the only unnamed focus stops and now carry the Splitter role and a name. The status bar's contents are drawn after the page so it comes last in the Tab order without changing the layout. Ribbon, navigation, and egui buttons already drew an accent focus ring; the canvas and the start page's recent rows now do too. The Narrator and VoiceOver runs could not be done from an agent session and stay open in gaps rank 14, so SH-15 is P.

## PR 0.5c to 0.5f: ribbon parity follow-ups

A review against OneNote (Microsoft 365 screenshots) and against WordCraft's ribbon, the related craft app with the same egui stack, found four gaps in the PR 0.5 ribbon. Each is its own PR, stacked in this order. Rows: SH-02 and SH-05 (and TX-10 for the highlight palette, whose command stays Coming Soon until Phase 1).

### PR 0.5c: ribbon display options and the simplified ribbon

| | |
|---|---|
| Branch | `feat/ribbon-display` |
| Hours | 20-32 |
| Rows | SH-05 |

Steps: the chevron at the ribbon's lower right opens OneNote's Ribbon Display Options menu: Ribbon Layout (Classic Ribbon, Simplified Ribbon) and Show Ribbon (Full-screen mode, Show tabs only, Always show Ribbon), each a command (`view.ribbon.layout`, `view.ribbon.show`) with the current choice checked. Simplified Ribbon is one row of icon buttons built from the same spec as the classic ribbon, groups separated by a divider, with a "More Options" overflow menu for what does not fit. Show tabs only is today's collapsed state (peek on a tab click); Full-screen mode hides tabs and ribbon until the pointer reaches the top edge. Ctrl+F1 switches between Always show and Show tabs only, as in OneNote. The layout and show mode persist in settings (the existing `ribbon.collapsed` key reads as Show tabs only).

Commits:

1. `feat(engine): add ribbon layout and show mode commands`
2. `feat(ui): add the ribbon display options menu`
3. `feat(ui): add the simplified ribbon`

### PR 0.5d: font and size boxes

| | |
|---|---|
| Branch | `feat/ribbon-font-boxes` |
| Hours | 12-20 |
| Rows | SH-02 |

Steps: replace the Font and Size dropdown buttons with editable combo boxes that show the current value, as in OneNote and WordCraft (`widgets::font_combo`, `combo`). The font box lists the installed families (craft-ui/OS fonts; the fixed list is the fallback), each in its own face, building only visible rows; typing filters (exact, then prefix, then contains), Up and Down move, Enter applies, Esc restores; text matching no family is applied as typed. The size box takes any typed size (1 to 1638 pt, kept as typed). Both run `format.font.family` and `format.font.size`.

Commits:

1. `feat(ui): add an editable combo box for the ribbon`
2. `feat(ui): use font and size boxes on the home tab`

### PR 0.5e: color pickers

| | |
|---|---|
| Branch | `feat/ribbon-colors` |
| Hours | 16-28 |
| Rows | SH-02, TX-10 |

Steps: color menus follow Office's layout: Automatic (or No Color), Theme Colors (ten hues and five tint and shade rows, original values), Standard Colors (one row), Recent Colors (the last ten picked, saved in settings), and More Colors, which opens a Colors dialog (a standard swatch tab and a custom tab with a hue and saturation field, lightness, RGB, HSL, hex, and alpha, plus a new over current preview) instead of expanding inside the menu. Split color buttons draw a bar in the last color used under the icon, and clicking the button applies it. Text Highlight Color offers OneNote's fifteen highlighter colors with No Color last. Alpha stays (OneNote has none; listed in docs/post-parity.md).

Commits:

1. `feat(ui): lay out color menus with theme, standard, and recent colors`
2. `feat(ui): add the colors dialog`
3. `feat(ui): show the last color on split color buttons`

### PR 0.5f: style and tag galleries, Help tab

| | |
|---|---|
| Branch | `feat/ribbon-galleries` |
| Hours | 16-24 |
| Rows | SH-02 |

Steps: Styles and Tags become in-ribbon galleries as in OneNote: a bordered box showing two entries drawn in their own style (Heading 1, Heading 2; To Do, Important with their icons and shortcuts), up and down arrows to scroll, and a More arrow that opens the full list. Add the Help tab (Help, Support, Feedback, Show Training) with each command Coming Soon or opening the docs.

Commits:

1. `feat(ui): add ribbon galleries for styles and tags`
2. `feat(ui): add the help tab`

## PR 0.11: minimal control channel for UI tests

| | |
|---|---|
| Branch | `feat/control-minimal` off `phase/0-ui-shell` |
| Hours | 8-14 |
| Rows | none (enables testing; full protocol is Phase 3) |

Steps: `notecraft --control <port>` with token auth, `auth`, `command.run`, `screenshot`, `wait.idle` only (docs/control-protocol.md). An integration test starts the app with `--control`, switches all four themes, and saves screenshots as CI artifacts.

Commits:

1. `feat(app): add minimal control channel with token auth`
2. `test(app): screenshot every theme through the control channel`

PR title: `feat(app): add a minimal control channel for UI tests`.

## PR 0.12: close Phase 0

Follow 00-conventions.md section 6. Also:

- Fill the alpha gate Evidence column in docs/roadmap.md for the Phase 0 rows.
- Cut internal test release 0.0.1 (docs/releasing.md section 2) to prove the pipeline end to end with real UI.
- Re-estimate Phases 1 to 4 from actual Phase 0 hours.

## Estimate

| PR | Hours |
|---|---|
| 0.1 app scaffold | 8-14 |
| 0.2 command registry | 12-20 |
| 0.3 theme tokens | 16-28 |
| 0.4 presets | 24-40 |
| 0.5 ribbon | 60-100 |
| 0.5c ribbon display options | 20-32 |
| 0.5d font boxes | 12-20 |
| 0.5e color pickers | 16-28 |
| 0.5f galleries and Help tab | 16-24 |
| 0.6 navigation | 24-40 |
| 0.7 canvas viewport | 24-40 |
| 0.8 start page | 12-20 |
| 0.9 status and shortcuts | 12-20 |
| 0.10 accessibility | 10-16 |
| 0.11 control minimal | 8-14 |
| 0.12 close and 0.0.1 | 4-8 |
| Total | 278-464 |

## Exit criteria

- SH-01 to SH-09 and SH-11 to SH-15, TH-01 to TH-06, CV-01 to CV-03 scored D.
- Internal release 0.0.1 built from `release`.
- Every ribbon button runs a command or shows Coming Soon; none does nothing.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Command palette and the Graph and Plugins tabs moved to docs/post-parity.md. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | 0 | PR 0.5 split into 0.5a (widget, icons, File, Home) and 0.5b (other tabs, collapse, key tips). |
| 2026-10-14 | 0 | PR 0.5b recorded: the other five tabs, collapse, and key tips (PR #10). |
| 2026-10-14 | 0 | PR 0.7 recorded: canvas pan, zoom, and paper (PR #13). |
| 2026-10-14 | 0 | PR 0.8 recorded: start page, recents, and file picker (PR #14). |
| 2026-10-14 | 0 | PR 0.9 recorded: status bar, full page view, and shortcuts (PR #15). |
| 2026-10-14 | 0 | PR 0.10 recorded: keyboard navigation and screen reader labels (PR #16). |
| 2026-10-15 | 0 | Added PR 0.5c to 0.5f (ribbon display options and simplified ribbon, font boxes, color pickers, galleries and Help tab) after a review against OneNote and WordCraft. |
