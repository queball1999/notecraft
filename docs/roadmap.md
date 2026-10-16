# Roadmap: focus, alpha plan, milestones

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-15. Change: Phase 0 closed; current focus is Phase 1; alpha gate evidence filled for the Phase 0 rows.

The stage summary and progress log are in [ROADMAP.md](../ROADMAP.md). This doc says what we are doing now, what the alpha needs, and the milestones after it. The step-by-step execution plan is in [build_plan/](../build_plan/README.md). Optional features OneNote does not have are in [post-parity.md](post-parity.md) and are not on this roadmap.

## Current focus

Phase 1, core engine ([build_plan/03](../build_plan/03-phase-1-core-engine.md)): real notebooks, markdown read and write, text editing, ink, pictures, undo, and the v0.1.0 alpha. Next up: PR 1.1 (notebook folder reader) and PR 1.3 (markdown read). Branch `phase/1-core-engine`, created from `main` after the Phase 0 PR merges.

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
| 3 | Polish and parity | 392-666 | 13-23 weeks | [05](../build_plan/05-phase-3-polish-parity.md) |
| Hardening | Beta to 1.0 | 80-160 | 3-6 weeks | [05](../build_plan/05-phase-3-polish-parity.md#beta-to-10-hardening) |
| To 1.0 | | 1,401-2,404 | 47-80 weeks | |
| 4 | Remaining parity (post-1.0) | 644-1,228 | 22-41 weeks | [06](../build_plan/06-phase-4-advanced.md) |
| Full parity | | 2,045-3,632 | 68-121 weeks | |

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
| Notebook navigation | Notebook list, section tabs, page list from a real folder | 0, 1 | SH-06 to SH-08, ST-01 | Phase 0 part: SH-06 to SH-08 scored D on mock notebooks (PR 0.6, #11; tests section_groups_collapse_in_the_notebook_pane, page_list_side_follows_the_preset, add_page_and_new_section_are_coming_soon). To be filled in Phase 1 for ST-01 (real folder) |
| Theme switching | All four presets switch live | 0 | TH-01 to TH-05 | TH-01 to TH-05 scored D (PR 0.3 and 0.4, #7, #8). Tests: palettes_match_the_themes_doc, presets_are_distinct, theme_commands_switch_the_theme_live, view_tab_switches_the_theme_with_no_toast, themes_switch_and_screenshot_over_tcp; ignored real-window test every_theme_screenshots_through_the_control_channel (PR 0.11, #22). WCAG AA contrast checked over all eight variants. All four presets in light and dark were switched through the control channel and screenshotted on Windows 11 with no errors in the log |
| Markdown read | Open a `.md` file, render it on the canvas | 1 | FM-01 | |
| Markdown write | Type in a container, save to `.md` | 1 | TX-01, FM-02 | |
| Open dialog | Open a notebook folder, `.md`, or `.notecraft` | 0 | SH-12 | SH-12 scored D on the harness tests (PR 0.8, #14): file.new_notebook, file.open_notebook, and file.open are Ready and tested with a fake picker (maintainer decision, 2026-10-15). Trying the real OS dialogs on Windows, macOS, and Linux is in gaps rank 25. Phase 0 opens the mock notebooks; Phase 1 loads real ones |
| Auto-save | Every 30 seconds and on focus loss | 1 | FM-05 | |
| Dark mode | Toggle light and dark | 0 | TH-01 to TH-05 | view.mode.toggle and view.mode.set (PR 0.3, #7); the View tab Dark Mode button; TH-05 and TH-06 scored D (tests theme_commands_switch_the_theme_live, installs_the_theme_from_the_session). System dark mode followed live on Windows 11; macOS and Linux follow checks in gaps rank 25; launched on Linux by the maintainer, reported working like Windows |
| Recent files | File backstage lists the last 10 | 0 | SH-11 | SH-11 scored D (PR 0.8, #14; About page #24). Tests: recent_rows_reopen_without_the_picker, backstage_open_page_lists_recents_and_opens_files |

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
