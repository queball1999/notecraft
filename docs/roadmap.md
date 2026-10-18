# Roadmap: focus, alpha plan, milestones

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-18. Last updated: 2026-10-18. Change: PR 1.15 close-out: alpha gate evidence filled, hands-on checklist for the maintainer, open questions (version); PR 1.5 sits on top of PR 1.9 and PR 1.13: container runs are a view over the single marker reader (docs/markdown.md section 3.3); PR 1.7 landed (auto-save, crash recovery, outside edits); PR 1.1 landed (notebook folder reader); navigation alpha gate row updated; page text read from disk when a page is shown (FM-01 scored D); PR 1.2 landed (rebuildable notebook index); page text read from disk when a page is shown (FM-01 scored D). PR 1.8 landed (undo and redo); also: PR 1.4: markdown write library landed; PR 1.10 follow-ups (anti-aliased ink edges, marked current pen, WordCraft survey notes); Ink Replay scheduled in PR 3.2 (IN-13); PR 1.11 landed (highlighter and erasers); PR 1.13 landed (structure editing); PR 1.12 landed (pictures and attachments, AT-01 to AT-05 scored P).

The stage summary and progress log are in [ROADMAP.md](../ROADMAP.md). This doc says what we are doing now, what the alpha needs, and the milestones after it. The step-by-step execution plan is in [build_plan/](../build_plan/README.md). Optional features OneNote does not have are in [post-parity.md](post-parity.md) and are not on this roadmap.

## Current focus

Phase 1, core engine ([build_plan/03](../build_plan/03-phase-1-core-engine.md)): real notebooks, markdown read and write, text editing, ink, pictures, undo, and the v0.1.0 alpha. Every PR of the phase (1.1 to 1.14, 1.16) and the follow-ups (navigation from the index, container positions, SVG pictures and picture resize, highlighter multiply, subpage collapse) are built; PR 1.15 is docs done and waiting for the hands-on checklist below and the version decision. Branch `phase/1-core-engine`, created from `main` after the Phase 0 PR merges.

Phase 0 (UI shell) closed on 2026-10-15: the whole OneNote interface, clickable, on mock data (PR 0.1 to 0.11, 0.13, and the ribbon follow-ups 0.5c to 0.5f). Hands-on checks that agents cannot run (macOS launch and dark mode, Linux dark mode, physical keyboard key tips, Narrator and VoiceOver, Cmd shortcuts on a Mac, the real OS file dialogs) moved to one gap, docs/gaps.md rank 25, to do before the v0.1.0 alpha (maintainer decision, 2026-10-15). Internal release 0.0.1 is cut right after the Phase 0 PR merges into `main`.

Phase -1 closed on 2026-10-14 with every exit criterion met. The Release workflow dry run (0.0.0-test.2) built all 20 artifacts on free runners with NoteCraft's own release workflow. Signing secrets are not set yet; add them to the `release` environment before the first real release.

## Phases and estimates

Hours are summed from every PR in the build plan. They assume one experienced Rust developer working with agent help, and include tests, docs, and review. Calendar weeks assume about 30 focused hours a week.

| Phase | Theme | Hours | Calendar | Plan |
|---|---|---|---|---|
| -1 | Scaffolding and CI/CD | 55-98 | 2-4 weeks | [01](../build_plan/01-phase-minus-1-scaffolding.md) |
| 0 | UI shell | 284-474 | 9-16 weeks | [02](../build_plan/02-phase-0-ui-shell.md) |
| 1 | Core engine | 456-776 | 15-26 weeks | [03](../build_plan/03-phase-1-core-engine.md) |
| 2 | Page links, search, note tags | 134-230 | 4-8 weeks | [04](../build_plan/04-phase-2-links-search-tags.md) |
| 3 | Polish and parity | 404-686 | 13-23 weeks | [05](../build_plan/05-phase-3-polish-parity.md) |
| Hardening | Beta to 1.0 | 80-160 | 3-6 weeks | [05](../build_plan/05-phase-3-polish-parity.md#beta-to-10-hardening) |
| To 1.0 | | 1,413-2,424 | 47-80 weeks | |
| 4 | Remaining parity (post-1.0) | 644-1,228 | 22-41 weeks | [06](../build_plan/06-phase-4-advanced.md) |
| Full parity | | 2,057-3,652 | 68-121 weeks | |

### What changed in the 2026-10-14 re-assessment

- Before: dimension hours were top-down guesses carried over from the SCRD (about 1,030-1,800 hours, with no phase totals).
- Now: every PR has a range, phases are sums of PRs, dimensions are sums of PRs mapped to them.
- Scope removed (not OneNote features, now in post-parity.md): backlinks, graph view, command palette, full CLI, MCP, full control channel, plugins, code highlighting, footnotes, embeds. About 300-530 hours.
- Scope added (OneNote features that were missing): Find Tags summary, date and time stamps, audio and video recording, password-protected sections, OCR, ink search.
- Raised after review of risk: ribbon icon set, live markdown editor, minimal-diff writer, pen pressure without `unsafe`, `.one` import, video capture.

## Alpha gate

v0.1.0 ships when every row below works end to end on Windows, macOS, and Linux. Evidence is a test name or a PR link with screenshots. SCRD section 5 is updated to match.

| Workflow | Must work | Phase | Checklist rows | Evidence |
|---|---|---|---|---|
| UI shell | Full ribbon, every OneNote tab, every button clickable; unimplemented ones show "Coming Soon" | 0 | SH-02 to SH-05 | SH-02 to SH-05 scored D. Tests: every_tab_draws_in_every_preset, every_ribbon_button_maps_to_a_registered_command, coming_soon_command_shows_a_toast_for_three_seconds, key_tips_reach_tabs_and_buttons (PR 0.5a to 0.5f, #9, #10, #17 to #19, #21). Key tips seen on Windows 11 with OS keyboard input; the physical keyboard check is in gaps rank 25. A Windows UI Automation dump of the Home tab listed 54 controls (8 tabs, 40 invokable buttons, 2 value boxes), all named |
| Canvas | Pan and zoom on an empty page, rule lines and grid | 0 | CV-01 to CV-03 | CV-01 to CV-03 scored D (PR 0.7, #13; margin line and dotted paper #23; Rule Lines thumbnails #25). Tests: middle_drag_and_space_drag_pan_the_canvas, wheel_scrolls_the_canvas, zoom_buttons_zoom_the_canvas, ctrl_wheel_and_pinch_zoom_about_the_pointer, Viewport property tests. Pinch tested with synthetic zoom events only |
| Notebook navigation | Notebook list, section tabs, page list from a real folder | 0, 1 | SH-06 to SH-08, ST-01 | Phase 0 part: SH-06 to SH-08 scored D on mock notebooks (PR 0.6, #11; tests section_groups_collapse_in_the_notebook_pane, page_list_side_follows_the_preset, add_page_and_new_section_are_coming_soon). ST-01 scored D (PR 1.1): opening a folder shows its real sections and pages. ST-07 scored D: the navigation is built from the notebook index (about 27 ms for 1,000 pages, criterion bench `sidebar`), with the folder reader as the fallback (tests the_index_gives_the_same_tree_as_the_folder_reader, a_corrupt_index_falls_back_to_the_folder, a_collapsed_page_stays_collapsed_in_the_index_tree). ST-03 to ST-05 scored D (PR 1.13). Dragging pages and sections (ST-06) waits for the hands-on pass (steps 2 and 3) Tests: opening_a_real_folder_shows_its_real_sections_and_pages, an_empty_folder_opens_as_an_empty_notebook, a_folder_with_a_bad_settings_file_opens_with_a_toast, and the storage tests over testdata/notebooks/basic |
| Theme switching | All four presets switch live | 0 | TH-01 to TH-05 | TH-01 to TH-05 scored D (PR 0.3 and 0.4, #7, #8). Tests: palettes_match_the_themes_doc, presets_are_distinct, theme_commands_switch_the_theme_live, view_tab_switches_the_theme_with_no_toast, themes_switch_and_screenshot_over_tcp; ignored real-window test every_theme_screenshots_through_the_control_channel (PR 0.11, #22). WCAG AA contrast checked over all eight variants. All four presets in light and dark were switched through the control channel and screenshotted on Windows 11 with no errors in the log |
| Markdown read | Open a `.md` file, render it on the canvas | 1 | FM-01 | FM-01 scored D: the shown page of an opened folder is read from disk and rendered. Tests: opened_pages_show_their_text_from_disk, an_unreadable_page_shows_a_short_error_on_the_canvas |
| Markdown write | Type in a container, save to `.md` | 1 | TX-01, FM-02 | FM-02 scored D; TX-01 scored P, waits for the hands-on pass (steps 5 and 6: a real keyboard and IME). Typing: text.insert, text.delete, text.enter, and the clipboard commands undo as word-sized steps (engine cmd::text_tests, ui canvas::editing_tests). Writing: edits are span replacements saved with only their own bytes changed (property tests over 73 corpus pages and CRLF copies; session saving_tests; `session::save::tests::ink_is_saved_with_a_minimal_diff_and_reloads`). Formatting TX-02 to TX-06 scored D (PR 1.6) |
| Open dialog | Open a notebook folder, `.md`, or `.notecraft` | 0 | SH-12 | SH-12 scored D on the harness tests (PR 0.8, #14): file.new_notebook, file.open_notebook, and file.open are Ready and tested with a fake picker (maintainer decision, 2026-10-15). Trying the real OS dialogs on Windows, macOS, and Linux is in gaps rank 25. Phase 0 opens the mock notebooks; Phase 1 loads real ones |
| Auto-save | Every 30 seconds and on focus loss | 1 | FM-05 scored D (PR 1.7). Tests: io::tests::pages_load_on_the_worker_and_save_on_the_timer, losing_focus_and_switching_pages_save, flush_on_quit_saves_and_leaves_no_journal; crash recovery FM-06 and outside edits FM-07 scored D (a_crash_leaves_a_journal_that_the_next_run_offers_back, the_conflict_bar_offers_both_versions_and_runs_commands). Killing the real process and the 30-second timer on a real window wait for the hands-on pass (step 11) |
| Dark mode | Toggle light and dark | 0 | TH-01 to TH-05 | view.mode.toggle and view.mode.set (PR 0.3, #7); the View tab Dark Mode button; TH-05 and TH-06 scored D (tests theme_commands_switch_the_theme_live, installs_the_theme_from_the_session). System dark mode followed live on Windows 11; macOS and Linux follow checks in gaps rank 25; launched on Linux by the maintainer, reported working like Windows |
| Recent files | File backstage lists the last 10 | 0 | SH-11 | SH-11 scored D (PR 0.8, #14; About page #24). Tests: recent_rows_reopen_without_the_picker, backstage_open_page_lists_recents_and_opens_files |

### Gate status (2026-10-18)

Every row works in the code and is covered by tests that run in `cargo xtask ci` (the dev box is Windows; the CI workflow runs on Linux, and only the printing workflow runs on all three systems, so macOS and Linux behavior is covered by the per-OS steps below). The rows below that say "waits for the hands-on pass" are the ones whose last step needs a person at a real machine; no row is left empty. The pass is the numbered list in the next section.

## Hands-on checklist for the maintainer

One pass, about 6 to 10 hours with the three systems (the first 11 steps on Windows, then the per-OS steps on macOS and Linux). Agents cannot run these: they need a real pointer, keyboard, pen, clipboard, file manager, or screen reader. Use a copy of `testdata/notebooks/basic` for the notebook steps. After each step, write the result in the evidence cell of the rows it names (docs/parity-checklist.md) and score the row D when it passes; fix what fails before tagging. When the list is done, delete gaps rank 25 and the other hands-on parts of ranks 17, 21, and 22.

| Step | Do this | Rows it clears |
|---:|---|---|
| 1 | Open a notebook with about 1,000 pages (make one with a script). The navigation should list it at once; edit a page in another editor and see the page list follow. | ST-01, ST-07 |
| 2 | Drag pages in the page list: reorder, drop onto another section tab, drop a parent whose subpages are collapsed, drop on the trash edge. Undo each. | ST-06 |
| 3 | Drag section tabs to reorder them, drag a section into and out of a group in the notebook pane, drag a group. Undo each. | ST-06 |
| 4 | Double-click empty canvas to make a container, type, drag its top bar, drag its right edge, drag one far right and down so the page grows past the paper, close the notebook, reopen it: the positions must reload. | CV-04, CV-05, CV-06 |
| 5 | Type with a real keyboard: letters, Enter, Backspace, Delete, arrows, Home and End, word moves, Shift selection, Ctrl+B, Ctrl+I, Ctrl+U, Tab and Shift+Tab in lists, Ctrl+Z and Ctrl+Y. On macOS use the Cmd versions. | TX-01, SH-14 |
| 6 | Type with an IME (Japanese or Chinese on Windows, macOS, and Linux with ibus or fcitx): the composition shows at the caret and commits as text; cancel with Escape. | TX-01 |
| 7 | Insert Pictures with the real file dialog (PNG, JPEG, WebP, SVG). Click a picture, drag a corner handle to resize it, close and reopen the page: the width must stay. | AT-01, SH-12 |
| 8 | Insert File Attachment, then click it: the file must open in the OS default app. Drag a picture and a file from the file manager onto the window. Copy an image (a screenshot) and press Ctrl+V; also try Paste as Picture. | AT-02, AT-03, AT-04, AT-05 |
| 9 | Draw with a real pen or tablet on Windows (pressure should change the width; smoothing should feel right; on macOS and Linux note that pressure is expected to be missing, gaps rank 21). Use the highlighter over text and rule lines and check it multiplies (crossing strokes darken); use the stroke eraser, the point eraser, and the eraser end of the pen. | IN-01, IN-02, IN-03, IN-04, IN-05, IN-06 |
| 10 | Subpages: make a page a subpage, collapse and expand it with the chevron and the page menu, move and delete a collapsed parent, close and reopen the notebook: the collapsed state must stay (also after the index refreshes). | ST-05, ST-06 |
| 11 | Auto-save and recovery: type, wait 30 seconds, check the file in a text editor; type again, end the process from the task manager, start the app: the recovery bar must offer the text. Edit the file in a text editor while the app has unsaved edits: the conflict bar must offer both versions. | FM-02, FM-05, FM-06, FM-07 |
| 12 | macOS: launch the window, follow system dark mode and the OS UI font (switch the system mode while the app runs). | SH-01, TH-06 |
| 13 | Linux: follow system dark mode while the app runs. | TH-06 |
| 14 | macOS: shortcuts that use Cmd (copy, paste, undo, save, ribbon shortcuts). | SH-14 |
| 15 | Windows, macOS, and Linux: ribbon key tips on a physical keyboard (Alt on Windows and Linux, the key tip key on macOS). | SH-05 |
| 16 | Narrator on Windows, VoiceOver on macOS: move through the ribbon, the page list, and the canvas and listen to the names. | SH-15 |
| 17 | The real OS folder and file dialogs on Windows, macOS, and Linux: New Notebook, Open Notebook, Open, Insert Picture, Insert File. | SH-12 |
| 18 | Pinch zoom on a real touchpad; type into the font and size boxes by hand. | CV-02, SH-02 |
| 19 | Run the 30-minute fuzz runs for each target (`cargo +nightly fuzz run <target> -- -max_total_time=1800` from `fuzz/` after `bash fuzz/seed.sh`, see CONTRIBUTING.md; targets: markdown, frontmatter, containers, canvas_block, notebook_toml, section_toml, recovery_journal). | FM-08 |
| 20 | Record the results, re-score the rows, run `cargo xtask parity`, and decide the version question below. | all |

Rows that stay P after this pass for a reason that is not a hands-on check: IN-01 (textured pens and a pencil, gaps rank 21), FM-03 (images, files, and shapes in the canvas block, Phase 2 to 3), ST-02 (moving a closed notebook folder to the OS trash).

## Open questions

- Version: build_plan/03 PR 1.15 and docs/releasing.md name v0.1.0 for the alpha, but the maintainer has talked about releasing v0.0.2 at the end of Phase 1. Not decided here; no tag was made and no version number was changed.
- Whether ST-02 (notebook delete moves the folder to the OS trash) and IN-01 (textured pens, pencil) must be D before the alpha, or may ship as P. They are not hands-on checks, so the pass above cannot clear them.

## Milestones

| Milestone | Version | Contents | Ends with |
|---|---|---|---|
| M-1 | none | Scaffolding | Phase -1 exit criteria |
| M0 | 0.0.1 (internal test release) | UI shell on mock data | Phase 0 done; first `release` branch build |
| Alpha | 0.1.0 | Alpha gate plus ink, pictures, attachments, undo | Phase 1 done |
| M2 | 0.2.0 | Page links, search, note tags, equations | Phase 2 done |
| Beta | 0.9.0 | Shapes, selection tools, tables, recycle bin, page versions, print, PDF and HTML export | Phase 3 done; parity about 70% |
| 1.0 | 1.0.0 | Beta plus fixes, performance targets (SCRD section 11) | parity about 90% |
| Full parity | 1.x | Handwriting, ink search, `.one` import, recording, password sections, OCR, docked window, web, translations | Phase 4 done |
| Post-parity | later | Optional features from post-parity.md, by demand | maintainer decision |

## How this doc is maintained

- Change "Current focus" when a phase starts.
- Fill the Evidence column of the alpha gate as rows land.
- At the end of each phase, replace estimates with actual hours for finished work, re-estimate the remaining phases, and record the change in the revision history.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Re-scoped the same day to OneNote parity; estimates re-assessed bottom-up (1,279-2,202 hours to 1.0, 1,923-3,430 to full parity); command palette removed from the alpha gate. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | Current focus moved to Phase 0; Phase -1 close recorded. |
| 2026-10-15 | 0 | Phase 0 estimate raised to 278-464 hours for ribbon parity follow-ups (PR 0.5c to 0.5f); totals 1,343-2,306 to 1.0 and 1,987-3,534 to full parity. |
| 2026-10-15 | 0 | Command line PRs added (0.13, 1.16, 2.11, 3.14, 3.15) after the maintainer decision of 2026-10-15; totals 1,399-2,400 hours to 1.0 and 2,043-3,628 to full parity. |
| 2026-10-15 | 0 | Phase 0 closed (SH-12 now D on the harness tests; the Linux launch was checked by the maintainer). Alpha gate evidence filled for the Phase 0 rows. Current focus moved to Phase 1. No actual hours were recorded during Phase 0 (no time log exists), so Phases 1 to 4 keep their estimates, except PR 1.15 (+2-4 hours for the hands-on checks deferred from Phase 0, gaps rank 25): Phase 1 is 456-776 hours, 1,401-2,404 to 1.0, 2,045-3,632 to full parity (maintainer decisions, 2026-10-15). |
| 2026-10-15 | 1 | PR 1.1 landed: current focus moves to PR 1.2 and PR 1.3; the navigation alpha gate row has its ST-01 evidence. |
| 2026-10-15 | 1 | PR 1.2 landed (notebook index, ST-07 scored P): current focus moves to PR 1.4. |
| 2026-10-15 | 1 | PR 1.8 landed (undo and redo): current focus text and alpha gate unchanged; the remaining callers of the undo API are PR 1.5, 1.11, and 1.13. |
| 2026-10-15 | 1 | PR 1.4 landed: minimal-diff markdown write (library and atomic page writes); typing into containers and auto-save wire it to the UI. |
| 2026-10-15 | 1 | Ink Replay scheduled in PR 3.2 (maintainer decision, 2026-10-15): Phase 3 404-686 hours, 1,413-2,424 to v1.0, 2,057-3,652 to full parity; calendar ranges unchanged. |
| 2026-10-16 | 1 | PR 1.7 landed: auto-save, crash recovery, and outside edit handling; typing into containers (PR 1.5) and saving strokes (PR 1.9) are what remain before pages round trip fully. |
| 2026-10-16 | 1 | PR 1.11 landed (highlighter and erasers): the remaining callers of the undo API are PR 1.5 and 1.13. |
| 2026-10-16 | 1 | PR 1.13 landed (structure editing): current focus text and alpha gate unchanged; the remaining callers of the undo API are PR 1.5 and 1.11. |
| 2026-10-16 | 1 | PR 1.12 landed (pictures and attachments): current focus text unchanged; the remaining Phase 1 work is PR 1.5, 1.6, 1.7, 1.9, 1.11, 1.13, 1.14. |
| 2026-10-18 | 1 | PR 1.15 close-out: alpha gate Evidence filled for Markdown write and Auto-save, Notebook navigation updated; hands-on checklist for the maintainer and open questions (the v0.1.0 or v0.0.2 version question) added. Hours unchanged. |
