# Parity checklist

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-16. Change: PR 1.11 follow-ups (eraser sizes, grazing point eraser); PR 1.11: IN-04, IN-05, and IN-06 scored P; stale duplicate FM-02, ST-07, and ST-08 rows removed; ST-01 scored D (PR 1.1); SH-06 evidence updated; summary regenerated; PR 1.10: IN-01 and IN-02 scored P, IN-03 scored D; summary regenerated; PR 1.3: FM-01 and FM-04 scored P (markdown read, frontmatter read); page text from disk: FM-01 scored D; page text read from disk when a page is shown (FM-01 scored D); PR 1.2 landed (rebuildable notebook index); page text read from disk when a page is shown (FM-01 scored D). PR 1.8: ST-08 scored P (undo and redo); also: PR 1.4: FM-02 scored P (minimal-diff write library and atomic page writes); PR 1.10 follow-ups (anti-aliased ink edges, marked current pen, WordCraft survey notes); Ink Replay scheduled in PR 3.2 (IN-13).


Every OneNote feature NoteCraft must have, one row each, scored:

- D (done): works end to end, has a test, and is reachable as a command.
- P (partial): some of it works, or it works without a test; the Evidence column says what is missing.
- M (missing): not started, or only a "Coming Soon" badge.

Only features OneNote has are listed. Optional features OneNote does not have are in [post-parity.md](post-parity.md) and get rows only when someone picks them up.

`cargo xtask parity` regenerates the summary block from the rows below; `cargo xtask parity --check` (part of `cargo xtask ci`) fails if the block is stale. Do not edit the summary by hand.

Row format, which the xtask parses: `| ID | Area | Feature | Phase | Score | Evidence |`. Ids are permanent once committed. Unused ids: SH-10, TX-08, AT-07 (their features moved to post-parity.md or were merged into LK-08 before the first commit).

<!-- parity:summary:start -->
| Area | D | P | M | Total | Done % |
|---|---:|---:|---:|---:|---:|
| Scaffolding | 10 | 0 | 0 | 10 | 100% |
| Shell | 14 | 0 | 6 | 20 | 70% |
| Themes | 6 | 0 | 0 | 6 | 100% |
| Canvas | 3 | 0 | 9 | 12 | 25% |
| Ink | 1 | 5 | 7 | 13 | 7% |
| Text | 0 | 0 | 12 | 12 | 0% |
| Structure | 1 | 2 | 9 | 12 | 8% |
| Attachments | 0 | 0 | 8 | 8 | 0% |
| Format | 1 | 2 | 6 | 9 | 11% |
| Links | 0 | 0 | 8 | 8 | 0% |
| Export | 0 | 0 | 4 | 4 | 0% |
| Total | 36 | 9 | 69 | 114 | 31% |
<!-- parity:summary:end -->

## Scaffolding (Phase -1)

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| SC-01 | Scaffolding | Cargo workspace with all crates and `xtask` | -1 | D | PR -1.1: workspace, 7 crates, 2 apps, xtask; build, clippy, test pass |
| SC-02 | Scaffolding | `cargo xtask ci` runs every gate | -1 | D | PR -1.4: cargo xtask ci runs fmt, clippy, test, text, assets, layers, wasm, parity |
| SC-03 | Scaffolding | `cargo xtask layers` | -1 | D | PR -1.3: cargo xtask layers; unit tests plus a failing upward-dependency check |
| SC-04 | Scaffolding | `cargo xtask assets` | -1 | D | PR -1.3: cargo xtask assets; unit tests plus a failing unattributed-file check |
| SC-05 | Scaffolding | `cargo xtask text` | -1 | D | PR -1.3: cargo xtask text; unit tests plus a failing em dash check; CI shell step kept |
| SC-06 | Scaffolding | `cargo xtask parity` and `--check` | -1 | D | PR -1.4: cargo xtask parity and --check; also checks build plan hour totals |
| SC-07 | Scaffolding | `cargo xtask version set` (release contract) | -1 | D | PR -1.5: cargo xtask version set; validates semver and refreshes Cargo.lock for --locked builds |
| SC-08 | Scaffolding | Packaging scripts and the release workflow | -1 | D | PR -1.5 and PR #1: packaging scripts plus NoteCraft's own release workflow; dry run 0.0.0-test.2 built 20 artifacts on free runners |
| SC-09 | Scaffolding | Logging to file with rotation and 16 MiB cap | -1 | D | PR -1.6: stderr and file logging, rotation to .1 and .2, 16 MiB cap, RUST_LOG, NOTECRAFT_NO_PREFS; unit tests |
| SC-10 | Scaffolding | Branch protection and labels on GitHub | -1 | D | Phase -1 close: main and release protection, phase/* ruleset, labels, Discussions, release environment |

## Shell

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| SH-01 | Shell | Window opens on wgpu (DX12, Metal, Vulkan) | 0 | D | PR 0.1: window opens on wgpu (Windows DX12 verified); geometry saved through craft-settings. The maintainer ran NoteCraft on Linux (2026-10-15) and reported it works the same as on Windows. Scored D on the Windows and Linux evidence by maintainer decision, 2026-10-15; the macOS launch check is in the deferred hands-on checks gap (docs/gaps.md, before v0.1.0) |
| SH-02 | Shell | Ribbon with File, Home, Insert, Draw, History, Review, View tabs | 0 | D | PR 0.5a and 0.5b: data-driven ribbon with original icons drawn in code; File backstage and all seven OneNote tabs with their groups; egui_kittest tests click every tab in every preset. PR 0.5d: Font and Font Size are editable boxes showing the current value; the font box lists the installed families (a background scan reads only each file's name table; 88 families on Windows 11) and filters as you type; the size box takes any size from 1 to 1638. PR 0.5e: color menus in Office's layout (Automatic, Theme Colors, Standard Colors, Recent Colors saved in settings, More Colors) with a Colors dialog (Standard and Custom pages, New over Current, OK and Cancel); split color buttons show and apply the last color; Text Highlight Color offers fifteen colors with No Color last. PR 0.5f: Styles and Tags are in-ribbon galleries (a bordered box with two entries drawn in their own style, up and down arrows that scroll two at a time, and a More arrow for the full list; one dropdown each in the simplified ribbon), and the Help tab (Help, Support, Feedback, Show Training) follows View; egui_kittest tests click entries, arrows, and More |
| SH-03 | Shell | Every ribbon button bound to a command id | 0 | D | PR 0.2, 0.5a, 0.5b: every button, menu item, and backstage entry on all tabs runs a registered command with arguments its schema accepts (unit test); clicks dispatch through the registry (egui_kittest tests) |
| SH-04 | Shell | "Coming Soon" tooltip and toast for unimplemented commands | 0 | D | PR 0.2 and 0.5: clicking a Coming Soon button shows "Coming Soon: <label>" for 3 s and its tooltip says Coming Soon (egui_kittest tests). The per-button badge was dropped at the maintainer's request and stays dropped (decision, 2026-10-15) |
| SH-05 | Shell | Collapsible ribbon (Ctrl+F1) and key tips | 0 | D | PR 0.5b: view.ribbon.toggle (Ctrl+F1, chevron, double-click a tab), peek while collapsed, collapsed state saved; key tips on tabs and buttons (Alt, F10, Alt plus letter, Esc); egui_kittest tests. PR 0.5c: Ribbon Display Options menu on the chevron (Classic or Simplified Ribbon; Full-screen mode, Show tabs only, Always show Ribbon) through view.ribbon.layout and view.ribbon.show, both saved (the old collapsed key still read); the simplified ribbon is one row of icon buttons with a More Options menu for what does not fit; full-screen mode shows the ribbon while the pointer is at the top; checked in the running app on Windows 11. A lone Alt tap sent as OS keyboard input (keybd_event) showed the key tips on Windows 11; not yet tried by hand on a physical keyboard. Scored D on the Windows evidence by maintainer decision, 2026-10-15; the physical keyboard check is in the deferred hands-on checks gap (docs/gaps.md, before v0.1.0) |
| SH-06 | Shell | Navigation: notebook list | 0 | D | PR 0.6: notebook pane with color swatches, expandable notebooks and section groups; clicks run nav.notebook.select and nav.section.select (egui_kittest and engine tests). Shows the mock notebooks at start; opening a real folder replaces them (ST-01, PR 1.1) |
| SH-07 | Shell | Section tabs with colors | 0 | D | PR 0.6: colored section tabs across the page, softened in dark mode, "+" tab (section.new, Coming Soon), overflow menu; clicks run nav.section.select (egui_kittest tests) |
| SH-08 | Shell | Page list with subpage indent | 0 | D | PR 0.6: page list with Add Page (page.new, Coming Soon), subpages indented two levels, current page highlighted; right of the page in 2016 and 2019, left in 2021 and Microsoft 365; clicks run nav.page.select (egui_kittest tests) |
| SH-09 | Shell | Status bar (zoom, page info) | 0 | D | PR 0.9 (#15): Page n of m and the zoom, which opens a zoom menu through the zoom commands; test status_bar_shows_the_page_and_zoom_menu. Save state joins with auto-save (FM-05) |
| SH-11 | Shell | File backstage with recent notebooks and pages | 0 | D | PR 0.8 (#14): start page and the backstage Open page list the last 10 notebooks and pages (saved in config.toml); a row reopens, Clear Recent empties; file.recent.clear Ready; tests recent_rows_reopen_without_the_picker, backstage_open_page_lists_recents_and_opens_files. About page (feat/about-page): the About entry shows the version, commit, build date, profile, target, app ID, copyright, licenses, third-party notices, links, settings and log folders, a Copy button (file.copy_about_info), and the Microsoft trademark notice; file.about and file.copy_about_info Ready; tests in about.rs (file_about_is_ready_and_opens_the_page, page_shows_version_notice_and_license, copy_button_puts_the_details_on_the_clipboard, about_controls_are_named_and_pressable) |
| SH-12 | Shell | Open dialog for notebooks, `.md`, and `.notecraft` | 0 | D | PR 0.8 (#14): file.new_notebook, file.open_notebook, file.open Ready; a path-less command asks the picker (OS dialogs through rfd in the app; a fake in tests); only .md and .notecraft accepted; Phase 0 shows the mock notebooks. Scored D on the harness tests (fake picker) by maintainer decision, 2026-10-15; trying the real OS folder and file dialogs on Windows, macOS, and Linux is in the deferred hands-on checks gap (docs/gaps.md, before v0.1.0) |
| SH-13 | Shell | Full page view (F11) | 0 | D | PR 0.9 (#15): view.full_page (F11) and view.normal Ready; only the page shows, with a Normal View button; F11, Esc, or the button leaves; test full_page_view_shows_only_the_page |
| SH-14 | Shell | OneNote default shortcut map | 0 | D | PR 0.9 (#15): cmd::shortcuts (every default shortcut plus Ctrl+2 to Ctrl+9 tags), dispatched by the shell, Ctrl as Cmd on macOS, idle while a text field has focus; tests reject duplicates and invalid bindings; shortcuts_run_their_commands. The Cmd-on-Mac part is in the deferred hands-on checks gap (docs/gaps.md, before v0.1.0); the row stays D on the Windows evidence (maintainer decision, 2026-10-15) |
| SH-15 | Shell | Keyboard navigation and screen reader labels (AccessKit) | 0 | D | PR 0.10 (#16): every actionable AccessKit node named (panel splitters added); Tab order ribbon, navigation, section tabs, page, status bar; accent focus ring on every control including the canvas and the splitters; buttons are push buttons (UIA Invoke) and navigation rows are selectable list items (UIA SelectionItem), checked in the running app; canvas scroll bars named. A UI Automation dump of the running app on Windows 11 (Home tab) listed 54 controls: 8 ribbon tabs as TabItem with SelectionItem, 40 buttons with Invoke, and the Font and Font Size boxes as Edit with Value; none actionable without a name; tests every_control_has_a_screen_reader_name, every_control_can_be_pressed_by_a_screen_reader, buttons_are_push_buttons_and_rows_report_selection, tab_moves_through_ribbon_navigation_page_and_status_bar. Scored D on the Windows UI Automation evidence by maintainer decision, 2026-10-15; runs with Narrator (Windows) and VoiceOver (macOS) are in the deferred hands-on checks gap (docs/gaps.md, before v0.1.0) |
| SH-16 | Shell | Quick Access Toolbar | 3 | M | PR 1.8 put Undo and Redo buttons at the right of the tab strip (they disable when empty); the customizable toolbar is still to do |
| SH-17 | Shell | Docked window (Ctrl+Alt+D) | 4 | M | |
| SH-18 | Shell | Browser version | 4 | M | |
| SH-19 | Shell | Translated UI (first locales) | 4 | M | |
| SH-20 | Shell | Right-to-left layout | 4 | M | |
| SH-21 | Shell | All UI strings in a string table (en-US) | 3 | M | |

## Themes

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| TH-01 | Themes | OneNote 2016 preset, light and dark | 0 | D | PR 0.4: light and dark palettes match docs/themes.md section 4 and pass WCAG AA (unit tests); selected with view.theme.set; screenshots on Windows 11. The accent and tab strip first show on the ribbon (PR 0.5) |
| TH-02 | Themes | OneNote 2019 preset, light and dark | 0 | D | PR 0.4: light and dark palettes match docs/themes.md section 4 and pass WCAG AA (unit tests); selected with view.theme.set; screenshots on Windows 11. The accent and tab strip first show on the ribbon (PR 0.5) |
| TH-03 | Themes | OneNote 2021 preset, light and dark | 0 | D | PR 0.4: light and dark palettes match docs/themes.md section 4 and pass WCAG AA (unit tests); selected with view.theme.set; screenshots on Windows 11. The accent and tab strip first show on the ribbon (PR 0.5) |
| TH-04 | Themes | Microsoft 365 preset, light and dark | 0 | D | PR 0.4: light and dark palettes match docs/themes.md section 4 and pass WCAG AA (unit tests); selected with view.theme.set; screenshots on Windows 11. The accent and tab strip first show on the ribbon (PR 0.5) |
| TH-05 | Themes | Switch theme and mode live, persisted in settings | 0 | D | PR 0.3 and 0.4: view.theme.set, view.mode.toggle, view.mode.set switch between four distinct presets and three modes live (egui_kittest test); theme.preset and theme.mode saved with craft-settings (unit tests). Ribbon buttons for them come in PR 0.5 |
| TH-06 | Themes | Follow system dark mode | 0 | D | PR 0.3: system is the default mode and follows the OS live (egui_kittest test switching the OS theme); verified on Windows 11 (dark); launched on Linux by the maintainer (2026-10-15), dark mode following not separately reported. All four presets in light and dark were switched through the control channel and screenshotted on Windows 11 with no errors in the log. Scored D on the Windows evidence by maintainer decision, 2026-10-15; the macOS dark mode check and the Linux dark mode follow check are in the deferred hands-on checks gap (docs/gaps.md, before v0.1.0) |

## Canvas

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| CV-01 | Canvas | Pan (scroll, middle drag, space drag) | 0 | D | PR 0.7 (#13): canvas widget scrolls with the wheel and scroll bars, pans with middle-drag and Space+drag; Viewport property tests; app_shell tests middle_drag_and_space_drag_pan_the_canvas, wheel_scrolls_the_canvas |
| CV-02 | Canvas | Zoom 10% to 400%, Ctrl+wheel, pinch | 0 | D | PR 0.7 (#13): view.zoom_in, view.zoom_out, view.zoom, view.zoom_actual, view.zoom_page_width Ready; Ctrl+wheel and pinch zoom about the pointer through view.zoom; tests zoom_buttons_zoom_the_canvas, ctrl_wheel_and_pinch_zoom_about_the_pointer (pinch tested with synthetic zoom events only) |
| CV-03 | Canvas | Rule lines and grid (blank, lined, grid, dotted) | 0 | D | PR 0.7 (#13): view.rule_lines (none, 4 ruled, 4 grid sizes), view.page_color, view.paper_size Ready and drawn for the visible area only; per page in the session. Ruled paper has a thin red margin line 1 inch from the left edge, as in OneNote (theme token canvas_margin_rule). The View > Rule Lines menu is a gallery of thumbnail cards, as in OneNote, each drawn by the canvas paper code at a reduced scale. Dotted (a quarter inch dot grid, capped at 20000 dots) is a kept extra beyond OneNote, see docs/post-parity.md |
| CV-04 | Canvas | Click anywhere to create a note container | 1 | M | |
| CV-05 | Canvas | Move and resize containers | 1 | M | |
| CV-06 | Canvas | Page grows with content | 1 | M | |
| CV-07 | Canvas | Lasso and rectangle select | 3 | M | |
| CV-08 | Canvas | Z-order commands | 3 | M | |
| CV-09 | Canvas | Snap to grid and alignment guides | 3 | M | |
| CV-10 | Canvas | Insert Space tool | 3 | M | |
| CV-11 | Canvas | 60 FPS with 500 objects at 100% zoom | 3 | M | |
| CV-12 | Canvas | New window on the same notebook | 3 | M | |

## Ink

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| IN-01 | Ink | Pen with color and width | 1 | P | PR 1.10: draw.pen, draw.color, draw.thickness, and ink.add_stroke are Ready; Draw tab pen gallery; tests `canvas::ink_tests::dragging_with_the_pen_makes_a_stroke`, `a_tap_makes_a_dot`, `the_stroke_is_painted_as_a_mesh`, `draw::tests::the_pen_takes_a_color_and_a_width`. Edges are anti-aliased (`tessellate::tests::the_fringe_wraps_the_outline_and_adds_no_area`, `a_join_fringes_only_its_outer_wedge`, `canvas::ink::tests::a_fringed_mesh_fades_over_one_pixel_around_the_edge`) and the pen menus mark the pen in use (`ribbon::menus::tests::the_menu_marks_the_pen_in_use`). Missing: strokes are not saved to the page file yet (PR 1.9) |
| IN-02 | Ink | Pressure from pen tablets | 1 | P | PR 1.10: touch force becomes stroke pressure (tests `a_touch_force_becomes_pressure`, `touch_force_becomes_pen_pressure_and_hides_the_emulated_mouse`); speed-based width when there is none (`speed_sets_width_when_there_is_no_pressure`). Missing: no real pen or tablet was tried; from the winit source only Windows reports pressure, macOS and Linux need a wrapper crate (build_plan/03 PR 1.10 notes) |
| IN-03 | Ink | Stroke smoothing | 1 | D | PR 1.10: one-euro filter; tests `filter::tests::jitter_at_rest_is_cut_by_more_than_half`, `a_fast_line_is_followed_closely`, `capture::tests::smoothing_cuts_jitter_on_a_slow_line`, `a_quick_flick_is_not_flattened`. Feel on a real pen is part of the deferred hands-on checks (gaps rank 25) |
| IN-04 | Ink | Highlighter | 1 | P | PR 1.11: draw.highlighter is Ready (color and width, default yellow 8 pt); strokes keep `tool: "highlighter"` and are undoable; drawn in a pass of their own above the paper and under the page text as opaque meshes pre-blended with the paper color at 0.5 (no fringe artifacts); the menus mark the current highlighter. Tests `draw::tests::the_highlighter_has_its_own_style_and_marks_its_strokes`, `canvas::ink_tests::dragging_with_the_highlighter_makes_a_highlighter_stroke`, `canvas::ink::tests::a_highlighter_color_is_blended_with_the_paper_and_opaque`. Missing: true multiply (overlapping strokes darken, rule lines show through) is a later paint callback; strokes are not saved yet (PR 1.9) |
| IN-05 | Ink | Stroke eraser | 1 | P | PR 1.11: draw.eraser { mode: stroke } and ink.erase are Ready; reach is 6 screen pixels past the stroke edge through `InkLayer::hit_test`, the path of a fast drag is sampled so no stroke is skipped, and one drag is one undo step (later erases join the first with merge-into-previous). Tests `draw::tests::the_stroke_eraser_removes_whole_strokes_in_one_step`, `one_drag_is_one_undo_step`, `a_new_drag_and_other_steps_start_a_new_step`, `canvas::ink_tests::the_stroke_eraser_removes_what_it_crosses_in_one_undo_step`, `undo::tests::every_undoable_command_undoes_and_redoes_exactly` (ink.erase row). Missing: strokes are not saved yet (PR 1.9) |
| IN-06 | Ink | Point eraser | 1 | P | PR 1.11: draw.eraser { mode: point } cuts the points inside a 10 pixel circle (plus half the stroke width) and splits the stroke into the runs that remain, each keeping tool, color, width, and pressure; runs under two points are dropped; long segments are subdivided where the eraser crosses them; undo restores the stroke in its paint position. Tests `ink::erase::tests` (5), `draw::tests::the_point_eraser_splits_a_stroke_and_undo_puts_it_back`, `canvas::ink_tests::the_point_eraser_splits_a_stroke`. Sizes: Small, Medium, Large (5, 10, 20 px); a graze between stored points cuts (`ink::erase::tests::a_graze_between_points_cuts`). Missing: strokes are not saved yet (PR 1.9) |
| IN-07 | Ink | Shapes gallery (15 or more) | 3 | M | |
| IN-08 | Ink | Ink to shape | 3 | M | |
| IN-09 | Ink | Ruler | 3 | M | |
| IN-10 | Ink | Touch input and palm rejection | 3 | M | |
| IN-11 | Ink | Ink to text (handwriting recognition) | 4 | M | |
| IN-12 | Ink | Search handwritten ink | 4 | M | |
| IN-13 | Ink | Ink replay | 3 | M | |

## Text

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| TX-01 | Text | Type plain text in a container | 1 | M | |
| TX-02 | Text | Bold, italic, strikethrough | 1 | M | |
| TX-03 | Text | Headings and styles | 1 | M | |
| TX-04 | Text | Bulleted and numbered lists, indent | 1 | M | |
| TX-05 | Text | To Do checkboxes (task lists) | 1 | M | |
| TX-06 | Text | Tables (render) | 1 | M | |
| TX-07 | Text | Table editing tools | 3 | M | |
| TX-09 | Text | Equations | 2 | M | |
| TX-10 | Text | Text highlight color | 2 | M | |
| TX-11 | Text | Find on page | 2 | M | |
| TX-12 | Text | Spell check | 3 | M | |
| TX-13 | Text | Insert date and time stamps | 2 | M | |

## Structure

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| ST-01 | Structure | Open a folder as a notebook | 1 | D | PR 1.1: notecraft-storage scans a folder into groups, sections, and pages (notebook.toml and .section.toml read leniently, ordering, depth 8, warning at 50,000 files); file.open_notebook shows the real tree in the navigation. Tests: storage unit tests, basic_notebook (testdata/notebooks/basic), engine file command tests, opening_a_real_folder_shows_its_real_sections_and_pages |
| ST-02 | Structure | Create, rename, delete notebooks | 1 | M | |
| ST-03 | Structure | Create, rename, delete, color sections | 1 | M | |
| ST-04 | Structure | Section groups | 1 | M | |
| ST-05 | Structure | Create, rename, delete pages; subpages | 1 | M | |
| ST-06 | Structure | Drag to reorder and move pages and sections | 1 | M | |
| ST-07 | Structure | Navigation loads 1,000 pages in under 500 ms | 1 | P | PR 1.2: notecraft-storage keeps a rebuildable `.notecraft/index.sqlite` (sections, pages, attachments; links, tags, and FTS tables created empty), refreshes it by modified time, size, and hash, and rebuilds it when it is corrupt or from a newer schema. Listing 1,000 pages from the index takes about 27 ms (criterion bench `sidebar`). The navigation does not read the index yet; wiring it in is gaps rank 15 |
| ST-08 | Structure | Undo and redo | 1 | P | PR 1.8: transactions of reversible ops with per-page stacks (500 steps or 64 MiB) and a notebook stack; edit.undo and edit.redo Ready (Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z) with Undo and Redo buttons on the tab strip that disable when empty; ink.add_stroke, view.rule_lines, view.page_color, and view.paper_size undo and redo exactly; merge-into-previous (for the eraser, PR 1.11), typing coalescing, and notebook-level moves are built and tested against the engine API. Tests: engine undo::tests (38), undo_and_redo_buttons_follow_the_history, undo_and_redo_shortcuts_run_the_commands. Missing: text commands (PR 1.5), structure commands (PR 1.13), and the eraser (PR 1.11) are the callers; the History tab in the shell is OneNote's (page versions, unread), so no undo list is shown |
| ST-09 | Structure | Notebook Recycle Bin with restore | 3 | M | |
| ST-10 | Structure | Page versions | 3 | M | |
| ST-11 | Structure | Page templates | 3 | M | |
| ST-12 | Structure | Password-protected sections | 4 | M | |

## Attachments

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| AT-01 | Attachments | Insert pictures (PNG, JPEG, WebP, SVG) | 1 | M | |
| AT-02 | Attachments | Insert any file as an attachment icon | 1 | M | |
| AT-03 | Attachments | Drag and drop onto the page | 1 | M | |
| AT-04 | Attachments | Paste picture from clipboard | 1 | M | |
| AT-05 | Attachments | Open attachment in default app | 1 | M | |
| AT-06 | Attachments | File printout (PDF pages as images) | 3 | M | |
| AT-08 | Attachments | Audio and video recording | 4 | M | |
| AT-09 | Attachments | Copy text from picture (OCR) | 4 | M | |

## Format

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| FM-01 | Format | Read `.md` pages | 1 | D | PR 1.3: notecraft-markdown parses a page into blocks with exact source spans (golden tests on 6 sample pages, hostile-input tests) and the canvas renders them in flow layout (harness test); opening a folder reads the shown page from disk through notecraft-storage (64 MiB cap, lenient UTF-8, read-only when cut short, short error on the canvas when unreadable). Tests: page_file storage unit tests, session page_load_tests, opened_pages_show_their_text_from_disk, an_unreadable_page_shows_a_short_error_on_the_canvas |
| FM-02 | Format | Write `.md` pages with minimal diffs | 1 | P | PR 1.4: edits are span replacements on the original text (blocks, list items, table cells, frontmatter keys); canonical serializer for new blocks; atomic page writes in notecraft-storage; property tests over 73 corpus pages and CRLF copies (byte-identical round trip, edits touch only their span); saving from the editor waits for PR 1.5 and 1.7 |
| FM-03 | Format | Read and write `.notecraft` pages (containers, canvas block) | 1 | M | |
| FM-04 | Format | Frontmatter read and write | 1 | P | PR 1.3: bounded YAML-subset frontmatter reader (scalars, lists, one level of maps, 64 KiB cap, unknown keys kept as raw text; unit and hostile-input tests); PR 1.4 added the write side (set and remove keys rewrite only their own lines); reading and saving real page files waits for PR 1.1 and 1.7 |
| FM-05 | Format | Auto-save every 30 s and on focus loss | 1 | M | |
| FM-06 | Format | Crash recovery of unsaved edits | 1 | M | |
| FM-07 | Format | Outside edit detection and conflict bar | 1 | M | |
| FM-08 | Format | Fuzzed parsers (no panic on any input) | 1 | M | |
| FM-09 | Format | Import OneNote `.one` sections | 4 | M | |

## Links

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| LK-01 | Links | Page links typed as `[[Page]]` | 2 | M | |
| LK-02 | Links | Link resolution, page picker, create missing page | 2 | M | |
| LK-03 | Links | Links update on rename and move | 2 | M | |
| LK-04 | Links | Copy link to page (`notecraft://`) | 3 | M | |
| LK-05 | Links | Search with scopes (page to all notebooks) | 2 | M | |
| LK-06 | Links | Note tags with icons, gallery, shortcuts | 2 | M | |
| LK-07 | Links | Find Tags summary pane and summary page | 2 | M | |
| LK-08 | Links | Search attachments by name and type | 2 | M | |

## Export

| ID | Area | Feature | Phase | Score | Evidence |
|---|---|---|---|---|---|
| EX-01 | Export | Print with preview (craft-print) | 3 | M | |
| EX-02 | Export | Export page or section to PDF | 3 | M | |
| EX-03 | Export | Export to single-file HTML | 3 | M | |
| EX-04 | Export | Export notebook as zip | 3 | M | |

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Re-scoped the same day to OneNote features only: graph, automation, and plugin rows removed (docs/post-parity.md); Phase 4 parity rows added. 113 rows, all M. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | SC-01 scored D (workspace and crate skeletons). |
| 2026-10-14 | -1 | SC-02 scored P (cargo xtask ci with fmt, clippy, test). |
| 2026-10-14 | -1 | SC-03, SC-04, SC-05 scored D (text, assets, layers gates). |
| 2026-10-14 | -1 | SC-02 and SC-06 scored D; summary now generated by cargo xtask parity. |
| 2026-10-14 | -1 | SC-07 scored D and SC-08 scored P (release contract). |
| 2026-10-14 | -1 | SC-09 scored D (logging). |
| 2026-10-14 | -1 | SC-08 and SC-10 scored D; Phase -1 closed. |
| 2026-10-14 | -1 | SH-01 scored P (window opens; Windows verified). |
| 2026-10-14 | 0 | SH-03 and SH-04 scored P (command registry and Coming Soon toast). |
| 2026-10-14 | 0 | TH-05 and TH-06 scored P (theme tokens, commands, persistence, system mode). |
| 2026-10-14 | 0 | TH-01 to TH-05 scored D (four presets, PR 0.4). |
| 2026-10-14 | 0 | SH-02 scored P; SH-03 and SH-04 evidence updated (ribbon File and Home tabs, PR 0.5a). |
| 2026-10-14 | 0 | SH-02 and SH-03 scored D; SH-05 scored P; SH-04 evidence updated (PR 0.5b). |
| 2026-10-14 | 0 | SH-06, SH-07, SH-08 scored D (navigation on mock notebooks, PR 0.6). |
| 2026-10-14 | 0 | CV-01 and CV-02 scored D, CV-03 scored P (canvas pan, zoom, paper, PR 0.7). |
| 2026-10-14 | 0 | SH-11 scored D, SH-12 scored P (start page, recents, open dialogs, PR 0.8). |
| 2026-10-14 | 0 | SH-09, SH-13, SH-14 scored D (status bar, full page view, shortcut map, PR 0.9). |
| 2026-10-14 | 0 | SH-15 scored P (screen reader names, keyboard order, focus ring, PR 0.10). |
| 2026-10-15 | 0 | SH-15 evidence: buttons are push buttons, navigation rows selectable list items, splitter focus ring, named scroll bars; checked in the running app (PR 0.10). |
| 2026-10-15 | 0 | SH-05 evidence: Ribbon Display Options, simplified ribbon, full-screen mode (PR 0.5c); Alt key tips seen with OS keyboard input. |
| 2026-10-15 | 0 | SH-02 evidence: font and size boxes with installed fonts (PR 0.5d). |
| 2026-10-15 | 0 | SH-02 evidence: Office-style color menus, Recent Colors, and the Colors dialog (PR 0.5e). |
| 2026-10-15 | 0 | SH-02 evidence: Styles and Tags galleries and the Help tab (PR 0.5f). |
| 2026-10-15 | 0 | SH-11 evidence: the About page (feat/about-page). |
| 2026-10-15 | 0 | CV-03 scored D: ruled paper draws the red margin line; dotted paper kept as an extra (docs/post-parity.md). |
| 2026-10-15 | 0 | CV-03 evidence: the Rule Lines menu shows thumbnail cards of each paper style. |
| 2026-10-15 | 0 | Phase 0 close (maintainer decisions, 2026-10-15): SH-01, SH-05, SH-12, SH-15, and TH-06 scored D on the Windows evidence (SH-01 also on the maintainer's Linux run), and SH-14 stays D, with their macOS, Linux dark mode, physical keyboard, screen reader, and real OS dialog runs moved to one deferred hands-on checks gap in docs/gaps.md; SH-04 reworded to the Coming Soon tooltip and toast and scored D (the per-button badge stays dropped); SH-12 scored D on the harness tests. |
| 2026-10-15 | 1 | ST-01 scored D and SH-06 evidence updated: the folder reader and the real navigation tree (PR 1.1). |
| 2026-10-15 | 1 | IN-01 and IN-02 scored P and IN-03 scored D (PR 1.10, ink engine and pen): pen, smoothing, and stroke capture work and are tested with synthetic events; saving strokes waits for PR 1.9 and no real pen has been tried. |
| 2026-10-15 | 1 | Page text from disk: FM-01 scored D (the shown page of an opened folder is read and rendered). |
| 2026-10-15 | 1 | PR 1.3: FM-01 scored P (page text parses into blocks and renders in flow layout; folder reading waits for PR 1.1); FM-04 scored P (frontmatter read; write waits for PR 1.4). |
| 2026-10-15 | 1 | ST-07 scored P (PR 1.2): rebuildable `index.sqlite`, incremental refresh, 1,000-page sidebar in about 27 ms; the navigation is not wired to it yet. |
| 2026-10-15 | 1 | ST-08 scored P (PR 1.8, undo and redo): ink strokes and page setup undo exactly, text and notebook moves are built against the engine API; SH-16 evidence notes the Undo and Redo buttons. |
| 2026-10-15 | 1 | PR 1.4: FM-02 scored P (span-replacement write, canonical serializer, atomic page write; editor wiring waits for PR 1.5 and 1.7); FM-04 evidence updated for the frontmatter write. |
| 2026-10-15 | 1 | IN-01 evidence: anti-aliased stroke edges and the marked current pen (PR 1.10 follow-ups). Scores unchanged. |
| 2026-10-15 | 1 | Added IN-13 ink replay (Ink Replay scheduled in PR 3.2 (maintainer decision, 2026-10-15)). |
| 2026-10-16 | 1 | IN-04, IN-05, and IN-06 scored P (PR 1.11, highlighter and erasers); removed the stale M duplicates of FM-02, ST-07, and ST-08. |
| 2026-10-16 | 1 | IN-05 and IN-06 evidence: eraser sizes and grazing cuts (PR 1.11 follow-ups). Scores unchanged. |
