# Phase 2: page links, search, and note tags

> Stage: pre-alpha (Phase 2 closed; v0.0.3 next). Last reviewed: 2026-11-05. Last updated: 2026-11-05. Change: Phase 2 ships as v0.0.3, not v0.2.0 (maintainer decision); earlier: Phase 2 close review and close notes in PR 2.10; earlier: page ids and times written on first save of a `.notecraft` page (feat/page-ids, LK-03 Missing narrowed); earlier: Customize Tags edits the default tags (rename, icon, colors), hides and resets them, and orders the list that sets Ctrl+1 to Ctrl+9 (LK-06); earlier: PR 2.6 decisions recorded; earlier: PR 2.3 notes (links through renames and moves); earlier: PR 2.6 Find Tags summary notes; earlier: PR 2.11 notes; earlier: PR 2.7 live equation preview notes; earlier: PR 2.5 landed notes; earlier: PR 2.2 notes (link resolution and index built); earlier: PR 2.8 notes (highlight colors, date and time stamps); earlier: PR 2.9 notes (find on page built); earlier: entry criteria name v0.0.2, the Phase 1 alpha version; earlier: stage line: Phase 1 merged into main (#55); earlier: reviewed and re-estimated at the Phase 0 close; no change; PR 2.1 landed.

Goal: the OneNote features for finding and connecting notes. Links to other pages (OneNote lets you type `[[Page name]]`), links that keep working when pages are renamed or moved, search across notebooks, note tags with the Find Tags summary, equations, highlight, date and time stamps, and find on page. Ends with v0.0.3.

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

Notes (landed): the planner is `notecraft-engine/src/link_rename.rs` (pure; shared by the app and the refresh thread), fed by `Session::link_fix`. It covers every link in the notebook, so the outgoing links of a moved page are kept too, and a rename that creates or removes a name clash changes the form of links to the page they meant. Edits are `FsOp::PatchText` (one per link) at the front of the rename's transaction, so patches use the old paths and one Ctrl+Z restores names and links; the rename never fails because of a stale page: if a patch cannot apply, the step is made without link edits and a toast says links were not updated. Open pages are patched in memory (dirty, saved by the normal path); closed pages on disk with a byte check. Read-only pages and pages in conflict are skipped and reported. Shown text: display text kept, plain-name links follow the new name, other links keep what they showed. Setting `links.update_on_rename` (command plus `[links]` in config.toml). Outside renames: schema 8 adds `pages.fm_id`; the refresh thread pairs a vanished page with the single new page that has the same `notecraft.id` and the session offers the fix only when links would break (counted from open text and files, so a rename NoteCraft already fixed offers nothing, while one made with the setting off does). Deviations and limits: (1) Ids are written by feat/page-ids (see the note below this paragraph). (2) A rename seen in two separate refreshes (gone first, new page later) is not paired. (3) Planning reads every page file that is not open (links must be found), on the calling thread; large notebooks may pause briefly on a rename. (4) Page delete is not covered. (5) `link.follow` now creates `Page` in `Section` for `[[Section/Page]]` when that section exists (the 2.2 limit), else in the current section.

Notes (landed, feat/page-ids, finishes the id part of LK-03): `Session::save_page` stamps a `.notecraft` page (and a `.md` page that is being promoted) after the conflict check: `notecraft.id` (`pg_` plus 12 Crockford base32 characters, from `RandomState`, the system time, and a counter in `stamp::IdSource`; no new dependency), `created`, and `modified` (RFC 3339 to the minute; `Clock` gained `with_offset_minutes`, default 0 so the UTC clock writes `Z`). The edit is the minimal-diff frontmatter edit, all or nothing: a frontmatter that cannot be edited is left alone. Decisions: (1) the stamps are added at write time only, so unsaved text, the recovery journal, and conflict checks never see them, and the disk hash is taken over the stamped text, so the watcher echo of an own write is recognized; (2) `modified` changes only when the stamped text differs from the file, so a save that finds the same content writes nothing; (3) after the write the session adopts the stamped text and `follow_text_change` moves the caret and the recorded `ReplaceText` offsets by the bytes added at the top of the file (this also fixes the same shift for page style lines); (4) a plain `.md` page is never stamped, so it has no id and its outside renames are not offered (by spec); (5) pages are created as `.md`, so there is no `.notecraft` creation path to stamp; (6) the refresh pairs nothing when an id is on two or more pages, before or after (a copied page); (7) the browser build has no clock and no id source, so it saves as before. Not done: the desktop app still installs no local-time clock, so times are UTC (`Z`); a hand check in the app.

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

Notes (landed, follow-up `feat/customize-default-tags`): finishes the part of LK-06 PR 2.5 left out. Decisions: (1) Every tag can be modified in the dialog (display name, icon, font color, highlight color). A default tag keeps its slug, so page text never changes. `TagSet::edit` works on any tag; `tag.edit` is the command (`tag.custom_edit` stays for custom tags). (2) Settings (`[tags]` in config.toml) hold `custom`, `overrides` (one `[[overrides]]` per modified default, only the fields that differ from the built-in value; a color of `none` means cleared), `order` (slugs, only when it is not the built-in order), and `hidden` (default slugs). A later change to a built-in default still applies to every field not overridden. Every damaged entry is dropped on its own: an unknown slug, a bad icon, a name that is empty or already used, a slug in `order` that does not exist; the good fields of a partly damaged override still apply. Slugs in `order` that the file lacks (a tag added later) go to the end. (3) Delete on a default tag runs `tag.hide`: it leaves the gallery, the dialog list, and Ctrl+number, but the session still knows it, so tagged text keeps its icon, `tag.apply` still accepts its id, and the Tags Summary still shows its name. Custom tags are deleted (`tag.custom_delete`). Hidden tags come back only through Reset Tags (`tag.reset_all`, with a confirm in the dialog): it restores every default tag (look, place, visibility) and keeps custom tags after them (a custom tag whose name a default now has gets " 2" added). There is no per-tag unhide. (4) Names are unique across the list and the hidden tags, so a name freed by a rename can be reused but a hidden default name cannot until Reset Tags. `tag.reset` on a default tag fails if its built-in name is now taken. (5) Move Up and Move Down (`tag.move { tag, direction }`) swap with the neighbor; the order of the list is the order of the gallery and the Home > Tags list. (6) Ctrl+1 to Ctrl+9 are now nine bindings of one command, `tag.apply_nth { number }`, which looks up the nth tag of the list when it runs; `tag.todo` lost its own Ctrl+1 binding (it is still the To Do Tag button). With fewer than n tags the key does nothing. (7) The ribbon gallery and its lists are built from the session tags each frame (`Look::tags`); the spec keeps three placeholder items so galleries still have a dropdown for the simplified ribbon, key tips, and tests. Tag names in the gallery are now the tag names (for example "Remember for later", not "Remember for Later"). (8) Tag list commands change settings, not the page, so they are not undo steps (same as the custom tag commands in PR 2.5). The Tags Summary pane, if open, is read again after a rename or reset so it shows the new names; the index holds ids only, so nothing is reindexed. Command ids added: `tag.edit`, `tag.hide`, `tag.move`, `tag.reset`, `tag.reset_all`, `tag.apply_nth`. Known limit: a hand check of the dialog on each OS has not been done.

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

Notes (landed): Branch `feat/find-tags`. Commits: `feat(engine): add the Tags Summary query and commands`, `feat(ui): add the Tags Summary pane` (docs and tests in the same commits or the last one). Decisions: (1) `crates/notecraft-engine/src/tag_summary.rs` holds the UI-agnostic query `tag_summary(roots, context, scope, group, only_unchecked, tag_name)` (same inputs as search, same `SearchScope` list, mapped onto `TagScope`; a section outside a group has the notebook as its group), grouping, and `summary_page_text`; the CLI can call them. `tags_index::refresh_tag_index(root)` brings a folder's tag index up to date without the app. (2) Commands: `tag.find` (now Ready; optional `scope`, `group`, `only_unchecked`), `tag.summary.set_scope`, `tag.summary.set_group`, `tag.summary.set_unchecked`, `tag.summary.refresh`, `tag.summary.open_item`, `tag.summary.create_page` (undoable), `tag.summary.close`. The pane is a right side pane; opening it closes the search pane and the other way round. The default scope is This Notebook. (3) The index keeps one modified time per page, so the Date grouping is the page modified day, newest first (OneNote groups by when each tag was applied, which a text file cannot record). (4) Clicking an item selects its page and sets the caret at the paragraph; if the page text is loaded the paragraph is found again by tag and text, so an index that is a little behind the page still lands right. The caret is stored for the page and read back once the page has been read. (5) Show only unchecked items hides checked To Do items and keeps other tags. (6) Create Summary Page uses `Session::create_page_with` (the page creation path of `page.new`, with starting text), so the page and its text are one undo step. The page is named Tags Summary, numbered when taken (Tags Summary 2). It lists `## group` headings and list items that keep the tag (`- [ ] text`, `- [x] text`, `- #idea text`) followed by `[[shortest form]]` from `LinkIndex::shortest_form`; items from another notebook get the page title as plain text, because links resolve inside one notebook. Item text is escaped so it does not turn into tags or links. (7) The summary is read from the index, so it is as fresh as the index (Refresh Results reads it again; unsaved edits are not in it).

Maintainer decisions (2026-11-05): the Date grouping by page modified date is accepted as a known difference from OneNote (no per-tag date is stored, and the file format does not change); links back to pages in other notebooks wait for `notecraft://` (LK-04, Phase 3).

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

Notes (landed): live preview (`feat/equation-preview`). While the caret (not a selection) is inside a formula, the block still shows its source, and a small card with the typeset formula is drawn right below the source block, with the text under it pushed down so nothing is covered. `notecraft_markdown::math::formula_at(text, offset)` finds the formula: it parses only the blank-line-bounded paragraph around the caret (at most 64 KiB, else no preview), takes the math runs in order, and matches them back to bytes, so code spans, code blocks, and escaped dollar signs never count; the caret counts from just after the opening dollar sign to just after the closing one. The preview uses the same typesetting path and limits as the page (4 KiB, depth 32, helper thread with a time limit) but its own one-entry cache keyed by the exact source and size (`PictureCache::preview`), so typing one character at a time never fills the 512-entry page cache or leaks textures; a source that does not typeset shows "Cannot show this formula: <reason>" in the card. The card sits below the whole source block, not below the formula's own line, because the source is one text galley and a card between its lines is not possible without splitting it; it goes above the block only when the page would end past 8192 units. Still open for TX-09: a hands-on look at the real window (the tests use stand-in textures).

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

Notes (landed): Branch `feat/cli-search`. The two plan commits were merged into one engine and CLI change set. Decisions: (1) `notecraft_engine::search::index_notebook(root, rebuild)` runs the refresh plus the text, link, and tag passes and returns an `IndexSummary`; `refresh_search_index` now calls it, so it also fills the tag table. `delete_database_files` in storage became public for the rebuild. (2) `search` refreshes the index first, so it never needs a separate `index` run; if the refresh fails but an index exists, it warns and searches what is there. (3) `--tag` matches any of the given tags and filters the engine's ranked pages (the engine caps at 500 hits before the filter); with no query it lists the items from `Index::tagged_items`. (4) `--scope` needs `--in`; `group` takes a section path, as in the app. (5) The notebook folder is a trailing argument or `--notebook`, not both. (6) The CLI has no MCP server yet (PR 3.15), so search is exposed there when it is built. (7) Follow-up from PR 2.2 done: `check` builds a `LinkIndex` (with frontmatter aliases) and reads links with `page_links_of`; its own link scanner was removed; ambiguous links stay warnings. Known limit: the engine caps a search at 500 hits before the tag filter runs.

## PR 2.10: close Phase 2

Follow 00-conventions.md section 6; release v0.0.3.

Notes (closed 2026-11-05): every PR of the phase (2.1 to 2.9, 2.11) and the follow-ups (equation live preview, editable default tags, page ids written on first save) are merged into `phase/2-links-search-tags`, together with the file safety branches (fix/dirty-follows-structure, fix/page-kind-safety, fix/open-page-file, fix/save-feedback, fix/encoding-safety, fix/read-only-edits, fix/journal-hardening, docs/file-safety-review). Exit criteria: TX-10, TX-11, TX-13, LK-01, LK-02, LK-05, and LK-08 are D; TX-09, LK-03, LK-06, and LK-07 are P with only a hand check left (docs/roadmap.md hands-on checklist steps 21 to 27, docs/gaps.md rank 26). Maintainer decision (2026-11-05): close the phase and release v0.0.3 before the hand checks. No PR moved to a later phase.

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
| 2.10 close and v0.0.3 | 4-8 |
| 2.11 command line search | 4-8 |
| Total | 134-230 |

## Exit criteria

- Every row listed at the top scored D.
- v0.0.3 published.

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
| 2026-11-05 | 2 | PR 2.7 follow-up (feat/equation-preview): Notes (landed) added to PR 2.7 (live preview while a formula is edited). |
| 2026-11-05 | 2 | PR 2.11 landed: Notes (landed) added to PR 2.11 (command line search and index). |
| 2026-11-05 | 2 | PR 2.6 built (feat/find-tags): Notes (landed) added (Tags Summary pane, query, summary page). |
| 2026-11-05 | 2 | PR 2.3 built: notes added (links through renames and moves, LK-03 scored P). |
| 2026-11-05 | 2 | PR 2.6: maintainer decisions on Date grouping and cross-notebook links recorded. |
| 2026-11-05 | 2 | PR 2.5 follow-up (feat/customize-default-tags): Notes (landed) added (default tags editable, hide and reset, ordering, tag.apply_nth). |
| 2026-11-05 | 2 | PR 2.3 notes: page ids and times written on first save (feat/page-ids). |
| 2026-11-05 | 2 | Phase 2 close: reviewed (Last reviewed bumped), stage line set to Phase 2 closed. |
| 2026-11-05 | 2 | Version: Phase 2 ends with v0.0.3, not v0.2.0 (maintainer decision, 2026-11-05); every mention updated. |
