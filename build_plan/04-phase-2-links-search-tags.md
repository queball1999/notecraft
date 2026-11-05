# Phase 2: page links, search, and note tags

> Stage: pre-alpha (Phase 1 on main; v0.0.2 alpha next). Last reviewed: 2026-10-15. Last updated: 2026-11-05. Change: PR 2.5 landed notes; earlier: PR 2.2 notes (link resolution and index built); earlier: PR 2.8 notes (highlight colors, date and time stamps); earlier: PR 2.9 notes (find on page built); earlier: entry criteria name v0.0.2, the Phase 1 alpha version; earlier: stage line: Phase 1 merged into main (#55); earlier: reviewed and re-estimated at the Phase 0 close; no change; PR 2.1 landed.

Goal: the OneNote features for finding and connecting notes. Links to other pages (OneNote lets you type `[[Page name]]`), links that keep working when pages are renamed or moved, search across notebooks, note tags with the Find Tags summary, equations, highlight, date and time stamps, and find on page. Ends with v0.2.0.

Backlinks, the graph view, the command palette, and other features OneNote does not have are not in this plan. They are listed in docs/post-parity.md.

| | |
|---|---|
| Phase branch | `phase/2-links-search-tags` |
| Hours | see "Estimate" |
| Checklist rows | LK-01 to LK-03, LK-05 to LK-08, TX-09 to TX-11, TX-13 |
| Spec | docs/markdown.md sections 4, 5, and 8 |

## Entry criteria

- Phase 1 closed; v0.0.2 published (the Phase 1 alpha; maintainer decision, 2026-10-18).
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

Notes (landed): resolution is a pure type in `notecraft-markdown` (`LinkIndex`, `LinkPage`, `Resolved`); the canvas builds it from the open notebook's tree, so it needs no index and follows typing. `unicode-normalization` (MIT OR Apache-2.0, pure Rust, already in the lockfile through the math stack) does the NFC step. The index gets schema version 6: `pages.links_hash` (like `text_hash`, so a changed page is found by hash) and a `page_aliases` table; the old `links` rows were never written, so the migration drops them. Storage keeps the rows (`index/links.rs`); the engine finds the links and aliases (`link_index.rs`) and resolves them in a second pass over all pages, run when pages were added, removed, or changed. An own save is still not refreshed unless the page's links, their byte positions, or its aliases changed (`own_writes`). Deviations and decisions: (1) step 4 also matches paths ignoring case, not only base names; (2) clicking a link follows it; Shift+click and a click in the block that holds the caret edit instead, since that block shows raw text; (3) a missing page is created from the last part of the target; (4) the picker excludes the page being edited and lists at most 50 pages; it is session state (`Session::link_picker`), opened from `text.insert` (any insert that leaves `[[` before the caret) or `link.picker.open`; (5) `link.follow` is flagged undoable because it can add a page; it has a row in the structure undo cases. Not done: the command line `check` still has its own link check (apps/notecraft-cli/src/check.rs) and should move to `LinkIndex`; `notecraft://` URLs are LK-04.

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

Notes (landed): the index stores stripped page text in an FTS5 table kept in step with the page files; titles rank above body words. Attachment search takes `type:` words (extension, major type, or mime type) next to name words. An attachment hit opens the first page that links to it, and does nothing when no page does.

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

Notes (landed): Branch `feat/note-tags` was built on the phase branch. Commit list: `feat(markdown): parse note tags`, `test(markdown): note tag round trip`, `feat(storage): store note tags in the index and query them by scope`, `feat(engine): fill the note tag index from the refresh thread`, `feat(engine): add note tag commands and the tag gallery`, `feat(ui): draw original note tag icons`, `feat(ui): add tags gallery, tag shortcuts, and customize tags dialog` (the plan's commit 4 and the gallery commit were merged as the dialog and gallery share the app shell wiring; the command and index work are extra commits). Decisions: (1) Tags attach to the paragraph, heading, or list item that holds them; the scanner works on source text and skips code, HTML, links, and page links; the exact grammar is in docs/markdown.md section 4.2. (2) The existing command ids `tag.apply` (toggle, argument `tag`), `tag.remove`, and `tag.customize` were kept instead of renamed; `tag.custom_add`, `tag.custom_edit`, and `tag.custom_delete` are new. The Ctrl+1 to Ctrl+9 and Ctrl+0 bindings were already in the shortcut table and clash with nothing. (3) To Do stays the task list item through `tag.todo`/`tag.apply todo`. (4) Highlight-style tags (Remember for later, Definition, Highlight) paint the text and draw no icon. (5) Only custom tags can be renamed, deleted, or recolored; the default tags are fixed (checklist note). (6) Slugs for the tags the spec did not fix: website, password, critical, project-a, project-b, movie, book, music, source, blog, discuss-a, discuss-b, discuss-manager, email, meeting, callback, todo-1, todo-2, client. (7) The index tags table got schema version 7 (renumbered from 6 at the merge, after PR 2.2 took 6) (holder span, text, checked, and `pages.tags_hash`); `update_tag_index` in `crates/notecraft-engine/src/tags_index.rs` runs after `update_text_index` in the refresh thread, and `Index::tagged_items(TagScope, tags)` is the query PR 2.6 and PR 2.11 build on (storage had no TextScope to reuse for tags: a `TagScope` of notebook, folder (section or group), or page). Known limit: the canonical block serializer cannot tell `\#word` from a tag.

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

Notes (landed): PR 1.6 had already built `format.highlight` with a color argument, the split-button palette, and the canvas drawing, so this PR finished TX-10 by switching the written form to `<mark style="background-color: #RRGGBB">` (the old `background:` spelling still reads), making the style reading lenient and bounded (`!important`, shorthand words, `rgb()`; docs/markdown.md section 3.2), and adding the round trip and one-undo-step tests. TX-13 added `insert.date`, `insert.time`, and `insert.date_time` (their specs, shortcuts, and Insert tab buttons already existed as Coming Soon). Decisions: the format is US only (`11/5/2026`, `3:07 PM`) because reading the OS locale's patterns needs a locale database, and no pure-Rust, wasm-safe source exists that is worth a new dependency; the engine reads time through an injectable `Clock` (`notecraft_engine::stamp`, `Session::set_clock`), which defaults to UTC and returns nothing on wasm32 (the command then does nothing); the desktop app installs a local-time clock using `chrono` (default features off, `clock` only; three new small crates: chrono, iana-time-zone, iana-time-zone-haiku) because the OS time zone is not readable without it. The shortcuts clash with nothing in the default map (test `the_shortcuts_are_bound_once`). On Linux, desktops that switch keyboard layouts with Alt+Shift may take the chords first. The commit split is merged into one commit because the highlight work was a small remainder.

## PR 2.9: find on page

| | |
|---|---|
| Branch | `feat/find-on-page` |
| Hours | 8-14 |
| Rows | TX-11 |

Commits:

1. `feat(ui): add find on page with match highlighting`

PR title: `feat(ui): add find on page`.

Notes (built 2026-10-24):

- The matching is engine-level (`notecraft-engine/src/find.rs`): a case-insensitive search over the page's text units (one per paragraph, heading, table cell, or code block, in container order), bounded to 256 query characters, 10,000 matches, and 100,000 units. The canvas lays out the same units and a test checks that both lists are equal, so a match counted by a command is a match drawn on screen.
- Commands: `edit.find` (opens the bar; was Coming Soon, `Ctrl+F`), `edit.find.set_query`, `edit.find.next` (`F3`), `edit.find.previous` (`Shift+F3`), `edit.find.close`. Enter and Shift+Enter in the bar, and Esc to close, are handled by the bar and dispatch the same commands. The bar state (query, current match, scroll and focus counters) lives in the session so it survives page changes.
- Opening the bar drops the text caret, so no block is shown as raw markdown while searching. A block the user then clicks into shows raw and its matches are not highlighted until the caret leaves it (known limit).
- Find on page has no ribbon button in OneNote, so none was added. Ctrl+E (search across notebooks) is PR 2.4.

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
| 2026-10-15 | 0 | Re-estimated at the Phase 0 close: no actual Phase 0 hours were recorded and Phase 0 moved no scope into this phase, so the estimate is unchanged. |
| 2026-10-18 | 1 | Stage line: Phase 1 merged into main (#55); the v0.0.2 alpha is next. |
| 2026-10-19 | 1 | Entry criteria: v0.0.2 (not v0.1.0) is the Phase 1 alpha that must be published first. |
| 2026-10-24 | 2 | PR 2.9 built: find on page (TX-11 scored D); notes added to the PR section. |
| 2026-10-24 | 2 | PR 2.1 landed. Decisions: a link run is a `Run` with `page_link: Some(target)` and the shown text in `text`; a link whose display text equals its target is written back as `[[T]]`; a link never spans styled runs or a line break; the reader marks real links in a same-length copy of the source before inline parsing, so an escaped `[[x]]` stays plain text. Follow-up: the escaped pipe separates only in table cells |
| 2026-11-05 | 2 | PR 2.8 landed: Notes (landed) added to PR 2.8. |
| 2026-11-05 | 2 | PR 2.2 built: link resolution, link styles, click to follow, the page picker, and the link index (LK-01 and LK-02 scored D); notes added to the PR section. |
| 2026-11-05 | 2 | PR 2.5 landed: notes added (note tags, index, gallery, dialog). |
