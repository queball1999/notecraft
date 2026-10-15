# Phase 2: page links, search, and note tags

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: project renamed to NoteCraft; earlier changes are in the revision history.

Goal: the OneNote features for finding and connecting notes. Links to other pages (OneNote lets you type `[[Page name]]`), links that keep working when pages are renamed or moved, search across notebooks, note tags with the Find Tags summary, equations, highlight, date and time stamps, and find on page. Ends with v0.2.0.

Backlinks, the graph view, the command palette, and other features OneNote does not have are not in this plan. They are listed in docs/post-parity.md.

| | |
|---|---|
| Phase branch | `phase/2-links-search-tags` |
| Hours | see "Estimate" |
| Checklist rows | LK-01 to LK-03, LK-05 to LK-08, TX-09 to TX-11, TX-13 |
| Spec | docs/markdown.md sections 4, 5, and 8 |

## Entry criteria

- Phase 1 closed; v0.1.0 published.
- Alpha feedback triaged into docs/gaps.md (bugs from alpha users rank above new Phase 2 work).

## PR 2.1: page link parser

| | |
|---|---|
| Branch | `feat/page-link-parse` |
| Hours | 10-16 |
| Rows | LK-01 |

Steps: recognize `[[Page]]`, `[[Page|shown text]]`, and `[[Section/Page]]` (docs/markdown.md section 5.1) with source spans; skip code spans and code blocks; handle `\|` in tables; cap link length at 1,024 bytes; render as links on the canvas.

Commits:

1. `feat(markdown): parse page links with spans`
2. `feat(ui): render page links on the canvas`
3. `test(markdown): page link forms, escapes, and code exclusion`

PR title: `feat(markdown): parse page links`.

## PR 2.2: link resolution and link index

| | |
|---|---|
| Branch | `feat/page-link-resolve` |
| Hours | 18-30 |
| Rows | LK-02 |

Steps: resolution order from docs/markdown.md section 5.3; ambiguous and unresolved link styles; `links` table filled on save and on watcher events; click to follow; typing `[[` opens a page picker (as in OneNote); clicking an unresolved link creates the page in the current section (as in OneNote).

Commits:

1. `feat(markdown): resolve page link targets`
2. `feat(storage): index links on save and on outside changes`
3. `feat(ui): style ambiguous and unresolved links`
4. `feat(ui): add page picker on typing double brackets`
5. `feat(engine): follow links and create missing pages`
6. `test(markdown): resolution order and ambiguity`

PR title: `feat(markdown): resolve page links and index them`.

## PR 2.3: keep links working through renames and moves

| | |
|---|---|
| Branch | `feat/link-rename` |
| Hours | 16-28 |
| Rows | LK-03 |

Steps: `page.rename` and move commands rewrite every inbound link span (shortest unique form) in one notebook-level transaction; setting `links.update_on_rename`; outside renames detected by page id, with an offer to fix links.

Commits:

1. `feat(engine): rewrite inbound links on rename and move`
2. `feat(storage): detect outside renames by page id`
3. `test(engine): link updates across sections with undo`

PR title: `feat(engine): keep page links working through renames and moves`.

## PR 2.4: search

| | |
|---|---|
| Branch | `feat/search` |
| Hours | 24-40 |
| Rows | LK-05, LK-08 |

Steps: FTS5 table with page text (markdown stripped), titles weighted higher; search box (Ctrl+E) with OneNote's scope dropdown (this page, this section, this section group, this notebook, all notebooks); results pane with snippets and highlight, sortable by section, title, or date modified; attachment search by name and type.

Commits:

1. `feat(storage): maintain fts5 index of page text`
2. `feat(engine): add search command with scopes and ranking`
3. `feat(ui): add search box and results pane`
4. `feat(engine): search attachments by name and type`
5. `test(storage): search ranking and scopes`

PR title: `feat(engine): add search across notebooks`.

## PR 2.5: note tags

| | |
|---|---|
| Branch | `feat/note-tags` |
| Hours | 14-24 |
| Rows | LK-06 |

Steps: OneNote's note tags (To Do, Important, Question, Remember for later, Definition, Highlight, Contact, Address, Phone number, Idea, and the rest of the default gallery) stored as inline tags (docs/markdown.md section 4); original icon drawings; Home tab Tags gallery; Ctrl+1 to Ctrl+9 shortcuts; Customize Tags dialog (name, icon, font color, highlight color), saved in settings.

Commits:

1. `feat(markdown): parse note tags`
2. `feat(ui): draw original note tag icons`
3. `feat(ui): add tags gallery and tag shortcuts`
4. `feat(ui): add customize tags dialog`
5. `test(markdown): note tag round trip`

PR title: `feat(ui): add OneNote note tags`.

## PR 2.6: Find Tags summary

| | |
|---|---|
| Branch | `feat/find-tags` |
| Hours | 10-16 |
| Rows | LK-07 |

Steps: Home > Find Tags opens the Tags Summary pane, grouped by tag name, section, title, or date, with the same scopes as search; "Create Summary Page" writes a new page that lists the tagged items with links back.

Commits:

1. `feat(engine): query tagged items by scope`
2. `feat(ui): add tags summary pane`
3. `feat(engine): create tag summary page`

PR title: `feat(ui): add Find Tags and the tags summary page`.

## PR 2.7: equations

| | |
|---|---|
| Branch | `feat/equations` |
| Hours | 20-36 |
| Rows | TX-09 |

Steps: Insert > Equation, stored as LaTeX math (`$...$` and `$$...$$`). Evaluate pure-Rust math layout crates at the start (license MIT, Apache-2.0, or compatible; builds for wasm32); record the choice in docs/architecture.md; source shown when the caret is inside; bounded input (4 KiB per formula, depth 32).

Commits:

1. `docs: record equation renderer choice`
2. `feat(markdown): parse inline and block math`
3. `feat(ui): render equations and add insert equation`
4. `test(markdown): hostile math input`

PR title: `feat(markdown): add equations`.

## PR 2.8: highlight and date and time stamps

| | |
|---|---|
| Branch | `feat/highlight-timestamp` |
| Hours | 6-10 |
| Rows | TX-10, TX-13 |

Steps: Home > Text Highlight Color (`==text==`; colors other than yellow stored as `<mark style>` HTML, which NoteCraft already preserves); Insert > Date, Time, Date and Time (Alt+Shift+D, Alt+Shift+T, Alt+Shift+F) in US format by default, following the OS locale.

Commits:

1. `feat(markdown): add text highlight`
2. `feat(engine): insert date and time stamps`

PR title: `feat(engine): add highlight and date and time stamps`.

## PR 2.9: find on page

| | |
|---|---|
| Branch | `feat/find-on-page` |
| Hours | 8-14 |
| Rows | TX-11 |

Commits:

1. `feat(ui): add find on page with match highlighting`

PR title: `feat(ui): add find on page`.

## PR 2.11: command line search

| | |
|---|---|
| Branch | `feat/cli-search` off `phase/2-links-search-tags` |
| Hours | 4-8 |
| Rows | none |

Steps: `notecraft-cli search <query> [--tag <t>] [--limit <n>]` prints ranked hits (page, snippet) from the same engine search the desktop app uses, as text or with `--json`. `notecraft-cli index [--rebuild]` updates or rebuilds `index.sqlite`. Depends on PR 2.4 and 2.5.

Commits:

1. `feat(cli): add search with tag and limit filters`
2. `feat(cli): add index and index --rebuild`

PR title: `feat(cli): add search and index`.

This PR is part of the command line decision of 2026-10-15 (docs/cli.md). It can land at any point after PR 2.5 and before PR 2.10.

## PR 2.10: close Phase 2

Follow 00-conventions.md section 6; release v0.2.0.

## Estimate

| PR | Hours |
|---|---|
| 2.1 page link parser | 10-16 |
| 2.2 resolution and index | 18-30 |
| 2.3 rename updates | 16-28 |
| 2.4 search | 24-40 |
| 2.5 note tags | 14-24 |
| 2.6 Find Tags summary | 10-16 |
| 2.7 equations | 20-36 |
| 2.8 highlight and timestamps | 6-10 |
| 2.9 find on page | 8-14 |
| 2.10 close and v0.2.0 | 4-8 |
| 2.11 command line search | 4-8 |
| Total | 134-230 |

## Exit criteria

- Every row listed at the top scored D.
- v0.2.0 published.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. OneNote-only scope: page links, search, note tags, Find Tags, equations, highlight, timestamps, find on page. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-15 | 0 | Added PR 2.11 (command line search and index) after the maintainer decision of 2026-10-15; Phase 2 estimate 134-230 hours. |
