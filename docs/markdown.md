# The NoteCraft file format: markdown and page links

> Stage: pre-alpha (Phase 0 closed; Phase 1 next). Last reviewed: 2026-10-15. Last updated: 2026-10-18. Change: pictures and file attachments get positions in the canvas block (`images`, `files`; section 3.4); index schema version 3 adds pages.collapsed; `collapsed` key in section page entries; earlier: PR 1.5 text editing in containers (runs are a view over the section 3.3 markers); PR 1.9 landed (.notecraft pages and saving ink; one save path with auto-save); the folder reader (section 2) is implemented; file name rules in section 2.3 spelled out; reading implemented (PR 1.3); frontmatter limits and entry spans noted. PR 1.2: index refresh and rebuild rules (section 8); writing implemented (PR 1.4); trash layout (`origin.toml` per deleted item) and structure edits writing the order files (PR 1.13). PR 1.6: text formatting (bold, italic, strikethrough, underline, sub and superscript, headings, lists, indent, tasks, quote, code, alignment as commands; TX-02 to TX-06 scored D); Picture display width comment and SVG pictures (section 2.4); PR 1.5 text editing in containers (runs are a view over the section 3.3 markers); container positions written to the canvas block; repair of a block that did not parse (PR 1.9 follow-ups).

This doc is the spec for how NoteCraft stores notebooks on disk and how pages link to each other. The design goal: a notebook is a plain folder of markdown that stays readable and editable in any text editor, while NoteCraft still keeps the freeform canvas, ink, and layout that make it feel like OneNote.

Status: ink saving, container markers, and container positions are implemented (PR 1.9). The folder reader (section 2: notebooks, groups, sections, page order, settings files, folder depth) is implemented in `notecraft-storage` (PR 1.1). Phase 1 builds the rest of the reader and the writer (build_plan/03-phase-1-core-engine.md). Phase 2 builds page links, search, and note tags (build_plan/04-phase-2-links-search-tags.md). Reading is implemented (PR 1.3): `notecraft-markdown` parses a page into blocks that keep their source byte spans, reads the frontmatter subset of section 3.1, and the canvas shows the blocks in flow layout (section 3.5, read-only). Minimal-diff writing is implemented (PR 1.4): edits are span replacements on the original text, new blocks use the canonical style, and pages are written through `craft-atomic`. Typing in containers is implemented (PR 1.5). Not yet: auto-save from the editor (PR 1.7), page links, note tags, and equations (Phase 2). See build_plan/03-phase-1-core-engine.md and build_plan/04-phase-2-links-search-tags.md.

## 1. Design rules

1. The folder is the truth. Every piece of user content lives in files a person can read. The SQLite index is a cache that NoteCraft can delete and rebuild at any time.
2. Text stays text. Page text is markdown in the page file, never hidden inside JSON.
3. Minimal diffs. When NoteCraft saves, it rewrites only the parts of a file that changed. A page edited in another editor keeps its formatting, spacing, and line endings.
4. Lenient reading. Unknown frontmatter keys, unknown HTML, and malformed canvas data are kept or skipped, never fatal. A page NoteCraft cannot fully parse still opens as plain text.
5. Bounded reading. Every parser caps file size, nesting depth, stroke count, and point count (limits in section 9).
6. Crash-safe writing. Every write goes through `craft-atomic` (write to a temp file, flush, rename).

## 2. Folder layout

```text
My Notebook/                      <- notebook (a folder)
|-- .notecraft/                       <- NoteCraft's own data for this notebook
|   |-- notebook.toml             <- id, display name, color, section order
|   |-- index.sqlite              <- rebuildable cache (search, links, tags)
|   |-- history/                  <- local version history snapshots
|   `-- trash/                    <- deleted pages and sections (30 days)
|-- Work/                         <- section (a folder with pages)
|   |-- .section.toml             <- section color, page order, page levels
|   |-- Meeting notes.notecraft       <- page with canvas data (markdown + canvas block)
|   |-- Reading list.md           <- plain markdown page (text only)
|   `-- attachments/              <- files embedded in this section's pages
|       `-- 3f9a2c-budget.xlsx
`-- Projects/                     <- section group (a folder of sections)
    |-- .section.toml             <- kind = "group"
    |-- Alpha/                    <- section
    `-- Beta/                     <- section
```

### 2.1 Notebooks

A notebook is any folder that NoteCraft opens. NoteCraft creates `.notecraft/notebook.toml` on first open:

```toml
format = 1
id = "nb_01J9Z3K8V6Q4"          # stable id, never changes
name = "My Notebook"            # display name; may differ from folder name
color = "#7A4FB5"               # notebook color in the sidebar
sections = ["Work", "Projects"] # display order; folders not listed go at the end
```

### 2.2 Sections and section groups

A folder inside a notebook is:

- a section group if its `.section.toml` says `kind = "group"`, or, when it has no `.section.toml`, if it contains folders but no page files;
- otherwise a section.

`.section.toml` for a section:

```toml
format = 1
kind = "section"
color = "#3C8DBC"
pages = [
  { file = "Meeting notes.notecraft", level = 0 },
  { file = "Action items.md",     level = 1 },  # subpage (OneNote page indent)
]
```

A page entry may carry `collapsed = true`, which folds the page's subpages away in the page list (an unknown or non-boolean value is ignored).

Pages not listed go at the end in name order. Entries for missing files are dropped on the next save. Section groups use the same file with `kind = "group"` and a `sections` list for order. Nesting is capped at 8 levels.

### 2.3 Pages

A page is one file. There are two kinds, and both are markdown:

| Extension | When NoteCraft uses it | What is in it |
|---|---|---|
| `.md` | The page is text only: no ink, no free positioning, no shapes | Optional frontmatter, then markdown |
| `.notecraft` | The page has any canvas data | Frontmatter, markdown with container markers, then one canvas block at the end |

Promotion: when you add ink, a shape, or drag a container off the default flow on a `.md` page, NoteCraft renames it to `.notecraft` with the same base name. Page links resolve by base name without extension (section 5.3), so promotion never breaks a link. NoteCraft never demotes automatically; the `page.convert_to_markdown` command does it on request and warns that canvas data will be dropped.

The file name is the page title, with characters that are illegal on any of Windows, macOS, or Linux (`<>:"/\|?*` and control characters) replaced by `-`, and trailing dots and spaces trimmed. Leading dots and spaces are trimmed too (a leading dot hides a file on macOS and Linux), a Windows device name (`CON`, `NUL`, `COM1`, `LPT1`, and the rest, with or without an extension) gets an underscore after the name (`CON_`), and the name is cut to 255 bytes at a character boundary. A title with nothing usable becomes `Untitled Page`. If two names differ only in case, the second gets a number (`Plan 2`), because Windows and macOS volumes usually ignore case. If the title in frontmatter differs from the file name, the frontmatter title is shown.

### 2.4 Attachments

Embedded files are copied into the section's `attachments/` folder with a short content-hash prefix to avoid collisions: `3f9a2c-budget.xlsx`. Pages reference them with standard markdown links, so any editor shows them:

```markdown
![Whiteboard photo](attachments/a81b07-whiteboard.jpg)
[Q3 budget](attachments/3f9a2c-budget.xlsx)
```

A picture shows at its natural size. When the user resizes it with the corner handles, NoteCraft records the display width, in page units (one unit is one pixel at 100% zoom), in an HTML comment right after the image, on the same line:

```markdown
![Whiteboard photo](attachments/a81b07-whiteboard.jpg)<!-- notecraft:width=320 -->
```

Other markdown readers ignore the comment and show the picture at its natural size. The width is a whole number from 16 to 4096; anything else is left as plain inline HTML. The height always follows the picture's own proportions, so it is never stored. A resize inserts, replaces, or removes just this comment, so the page diff is a few bytes. Pictures can be PNG, JPEG, WebP, or SVG. An SVG is drawn with a pure-Rust renderer under limits (file over 4 MiB, drawing time over 3 seconds, or an unreadable file shows a placeholder); text, fonts, and embedded or linked images inside an SVG are not drawn.

Large files (over 100 MiB by default, configurable) are linked in place instead of copied, using a relative or absolute path, and marked `notecraft: linked` in the canvas block so the UI can warn if the target moves.

## 3. Page file contents

### 3.1 Frontmatter

Optional YAML frontmatter between `---` lines at the top of the file. NoteCraft reads a small, bounded YAML subset (scalars, lists, one level of maps). Unknown keys are preserved verbatim, and so is any value outside the subset (block scalars such as `|`, deeper maps, flow maps): the reader keeps its source text instead of failing.

```yaml
---
title: Meeting notes
created: 2026-10-14T09:30:00-05:00
modified: 2026-10-14T10:05:12-05:00
tags: [work, q3]
aliases: [Q3 sync]
notecraft:
  id: pg_01J9Z4B2M7XR    # stable page id (for notecraft:// links and history)
  paper: lined          # blank | lined | grid | dotted (dotted is a kept extra; lined draws the red margin line)
  color: "#FFF8E1"      # page background color
---
```

NoteCraft fills in `created`, `modified`, and `notecraft.id` the first time it saves a page. For a `.md` page, NoteCraft adds frontmatter only if the page needs it (an id is needed once something links to the page by `notecraft://` URL); otherwise a plain `.md` file stays plain.

### 3.2 Markdown dialect

CommonMark plus these extensions:

| Extension | Syntax | Phase |
|---|---|---|
| GFM tables | pipes and dashes | 1 |
| GFM task lists | `- [ ]` and `- [x]` | 1 |
| GFM strikethrough | `~~text~~` | 1 |
| GFM autolinks | bare URLs | 1 |
| Page links | `[[Page]]` and variants (section 5) | 2 |
| Note tags | `#important` and the other OneNote tags (section 4) | 2 |
| Equations | `$inline$`, `$$block$$` | 2 |
| Highlight | `==text==`; `<mark>` for colors other than yellow | 1 (yellow `==text==`), 2 (other colors) |
| Underline, sub and superscript | `<u>`, `<sub>`, `<sup>` | 1 |
| Font color, size, family | `<span style="color:#RRGGBB">`, `<span style="font-size:12pt">`, `<span style="font-family:Name">` (one property per span) | 1 |
| Paragraph alignment | a `<!-- notecraft:align=center -->` comment line above the paragraph (`left` is the default and writes nothing) | 1 |

Footnotes, embeds, and heading links are not OneNote features and are not planned before full parity (post-parity.md). NoteCraft still preserves them as text if a page contains them.

Raw HTML is kept verbatim and shown as text in edit mode; it is never executed. NoteCraft parses with `comrak` behind its own wrapper in `notecraft-markdown`, so the parser can be swapped later without changing the format.

### 3.3 Note containers

OneNote pages hold note containers: boxes of text you can place anywhere. In a `.notecraft` file, each container is a run of markdown that starts with an HTML comment marker:

```markdown
<!-- notecraft:container id=c1 -->
## Agenda

- Budget review
- Hiring plan

<!-- notecraft:container id=c2 -->
Remember to send the deck to [[Dana Lee]] before Friday.
```

The markers are invisible in any markdown renderer and easy to ignore in a text editor. Text before the first marker belongs to an implicit container `c0`. If someone adds text in another editor, it simply becomes part of whichever container it lands in. A marker with an unknown or duplicate id gets a fresh id on the next save.

### 3.4 The canvas block

The last thing in a `.notecraft` file is one fenced code block with the info string `notecraft-canvas`. It holds positions and everything that is not text. Markdown renderers show it as a code block; NoteCraft hides it.

````markdown
```notecraft-canvas
{
  "format": 1,
  "units": "px96",
  "containers": {
    "c1": { "x": 48, "y": 96, "w": 420 },
    "c2": { "x": 520, "y": 96, "w": 300 }
  },
  "ink": [
    { "id": "s1", "tool": "pen", "color": "#1F3A93", "width": 2.0,
      "points": [[600, 300, 0.42], [604, 302, 0.47], [611, 305, 0.51]] }
  ],
  "shapes": [
    { "id": "sh1", "kind": "arrow", "from": [480, 140], "to": [515, 140],
      "stroke": "#333333", "width": 1.5 }
  ],
  "images": [
    { "id": "i1", "src": "attachments/a81b07-whiteboard.jpg", "x": 48, "y": 420 }
  ],
  "files": [
    { "id": "f1", "src": "attachments/3f9a2c-budget.xlsx", "x": 720, "y": 420 }
  ],
  "z": ["c1", "c2", "i1", "f1", "sh1", "s1"]
}
```
````

Rules:

- Units are CSS pixels at 96 per inch, origin at the top left of the page, y down.
- Ink points are `[x, y, pressure]` with pressure from 0 to 1. Points are rounded to 2 decimal places on save. Strokes over the point cap are simplified (Ramer-Douglas-Peucker) on import.
- Pictures and file attachments that sit at a place of their own are listed in `images` and `files`. The picture or file stays in the page text as its markdown (`![alt](src)` or a paragraph that is one link `[name](src)`), usually at the end of the last container, so a text editor still shows it. An entry ties to that markdown by `src` and `nth`: `nth` (a whole number, 0 when left out) says which occurrence of that link in the page text, counting `![..](src)` images (or file paragraphs) in reading order. The entry holds only the top-left corner `x`, `y` in page units. A picture's width is the width comment of section 2.4 (the height follows its proportions); a file object has a fixed size. A picture or file with no entry flows inside its container as before. An entry needs a `src` and a finite `x` and `y`; one without is skipped, `id` is made up when missing or repeated, and at most 10,000 entries of each kind are read. An entry whose link is no longer in the page text (the user deleted it) is dropped on the next save, so deleting the markdown deletes the object. Other keys inside a changed `images` or `files` list (such as `w` and `h` from other writers) are not kept.
- A `.md` page promotes to `.notecraft` as soon as it has a picture or file entry, as it does for ink and container positions (section 3.5). Convert to markdown drops the entries and keeps the links, so the pictures flow in the text again.
- `z` is the paint order, back to front. Ids missing from `z` paint on top in file order.
- A canvas block that fails to parse is kept byte-for-byte in a `.notecraft/history/` snapshot, and the page opens in flow layout with a warning bar. NoteCraft never deletes the original block until the user saves.
- Only one canvas block per file. Extra blocks are treated as normal code.

### 3.5 Flow layout

A `.md` page, or a `.notecraft` page with no positions, opens in flow layout: containers stack top to bottom at a fixed width, like a document. Moving anything off the flow (a container, a picture, a file attachment, or a stroke of ink) promotes the page and records positions.

## 4. Note tags

OneNote's note tags are stored as inline tags in the text, so they stay readable in any editor:

| OneNote tag | Stored as | Shown as |
|---|---|---|
| To Do | `- [ ]` task list item (preferred) or `#todo` | checkbox |
| Important | `#important` | star icon |
| Question | `#question` | question mark icon |
| Remember for later | `#remember` | highlight |
| Definition | `#definition` | highlight |
| Idea | `#idea` | lightbulb icon |
| Contact, Address, Phone number | `#contact`, `#address`, `#phone` | icons |
| Custom tags (Customize Tags dialog) | `#<name>` | the icon and colors chosen in settings |

Icons are original drawings (ATTRIBUTION.md). A frontmatter `tags:` list is preserved if present but has no UI before full parity (a page tag pane is a post-parity option).

## 5. Page links

### 5.1 Syntax

OneNote lets you type `[[Page name]]` to link to a page (and creates the page if it does not exist). NoteCraft stores exactly that text.

| Form | Meaning |
|---|---|
| `[[Page]]` | link to a page by name |
| `[[Page\|shown text]]` | link with different display text |
| `[[Section/Page]]` | disambiguate by section path |

Inside a table cell the pipe must be escaped as `\|` (GFM rule).

### 5.2 Writing links

Typing `[[` opens a page picker. Choosing a page inserts `[[Name]]` using the shortest form that resolves uniquely. Pasting a `notecraft://` URL inside the same notebook offers to convert it to a page link.

### 5.3 Resolution order

Given `[[target]]`, NoteCraft looks for, in order:

1. a page whose path relative to the notebook root, without extension, equals `target`;
2. a page whose path relative to the current section equals `target`;
3. a page whose base name equals `target` exactly, if exactly one exists;
4. the same, case-insensitive and Unicode-normalized (NFC);
5. a page whose frontmatter `aliases` contains `target`.

If step 3, 4, or 5 finds more than one page, NoteCraft picks the one closest to the current page in the folder tree, marks the link as ambiguous (dotted underline), and lists all candidates on hover. If nothing matches, the link shows as unresolved (dashed); clicking it creates the page in the current section.

### 5.4 Renames and moves

Renaming or moving a page through NoteCraft updates every page link that pointed to it, across the notebook, as one undoable command (`page.rename`). The setting `links.update_on_rename` can turn this off. Renames done outside NoteCraft are detected by the file watcher using the page `notecraft.id`; NoteCraft then offers to fix links.

### 5.5 Links outside the notebook

- `notecraft://open?notebook=<id>&page=<id>` opens a page in any notebook NoteCraft knows, like OneNote's "Copy link to page". Registered as a URL scheme by the installers (Phase 3).
- Normal markdown links and URLs open in the default app or browser.

## 6. Not planned before full parity

Backlinks, unlinked mentions, a graph view, heading links (`[[Page#Heading]]`), and embeds (`![[Page]]`) are not OneNote features. They are listed in [post-parity.md](post-parity.md). The format leaves room for them: the index already records every link, and the extra link forms are plain text that NoteCraft preserves.

## 7. (reserved)

Kept so section numbers stay stable for links from other docs.

## 8. The index (`index.sqlite`)

A cache built from the files. Deleting it is always safe. Tables (Phase 1 schema, extended in Phase 2):

| Table | Holds |
|---|---|
| `pages` | id, path, title, kind, created, modified, content hash |
| `sections` | path, kind, color, order |
| `links` | source page, target text, resolved page, byte range |
| `tags` | page, note tag, byte range |
| `attachments` | path, size, mime type, hash, referencing pages |
| `fts_pages` | SQLite FTS5 full-text index over page text |

On open, NoteCraft compares file modified times and sizes against the index, hashes the pages that differ (FNV-1a over the file, read up to the 64 MiB page limit), and reparses only what changed. The schema version is the SQLite `user_version` pragma. A database that is corrupt, not a database, or from a newer schema is deleted and rebuilt from the folder. The index is native only; the wasm32 build has no index. A notebook with 1,000 pages must list its sidebar in under 500 ms (SCRD section 11); the full reindex can finish in the background.

## 9. Limits

| Limit | Default | On overflow |
|---|---|---|
| Page file size | 64 MiB | opened read-only as plain text |
| Frontmatter size | 64 KiB | frontmatter kept as text, keys ignored |
| Canvas block size | 48 MiB | block preserved, page opens in flow layout |
| Ink strokes per page | 100,000 | extra strokes kept in file, not drawn, warning shown |
| Points per stroke | 20,000 | stroke simplified on load |
| Markdown nesting depth | 64 | deeper content shown as plain text |
| Folder depth | 8 | deeper folders ignored, warning shown |
| Links per page | 10,000 | extra links not indexed |

## 10. History and trash

- Version history: before overwriting a page, NoteCraft copies the old file to `.notecraft/history/<page-id>/<timestamp>.<ext>` at most once every 10 minutes per page, keeping 50 snapshots per page by default.
- Trash: deleted pages and sections move to `.notecraft/trash/<seconds>-<n>/<name>` with an `origin.toml` beside them recording the original path, kind, position, level, and time (PR 1.13). Undo moves them back. Listing, restoring, and purging after 30 days are Phase 3 (PR 3.5).

## 11. Compatibility promises

- The `format` number in `notebook.toml`, `.section.toml`, and the canvas block goes up only for breaking changes. NoteCraft reads every older format it ever wrote.
- A newer format than NoteCraft understands opens read-only with a message.
- Removing `.notecraft/` from a notebook loses only history, trash, and the index; all content survives.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. First full spec: folder layout, `.md` and `.notecraft` pages, containers, canvas block, note tags, page links, index, limits. Same day: backlinks, graph, embeds, and extra link forms moved to post-parity.md. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-15 | 0 | `paper: dotted` marked as a kept extra beyond OneNote; ruled (lined) paper draws a red margin line, with no new value. |
| 2026-10-15 | 1 | The folder reader (section 2, section 9 folder depth) is implemented. Section 2.3 spells out the file name rules: leading dots and spaces are trimmed too, Windows device names get an underscore, and names are cut to 255 bytes. |
| 2026-10-15 | 1 | PR 1.3 landed the reader: status line updated; frontmatter over 64 KiB is kept as text with its keys ignored, and a block with no closing line is not frontmatter; the body is parsed after the frontmatter with page-relative spans. |
| 2026-10-15 | 1 | PR 1.2 landed the index: section 8 states the refresh rule (time and size, then hash), the `user_version` migrations, and rebuild on corruption or a newer schema. |
| 2026-10-16 | 1 | PR 1.9 landed ink saving: container markers and the canvas block `ink` value are read and written (sections 3.3, 3.4, 9). Decisions: an invalid, `c0`, or duplicate container id gets a fresh id; ink is the only canvas key written so far and every other key is kept byte for byte; a markdown page keeps no canvas block, so its first stroke promotes it to `.notecraft`; a page whose block fails to parse, is over 48 MiB, has a newer format, or has over 100,000 strokes opens read-only with a notice, and the file is copied to `.notecraft/history/<page path>/canvas-unreadable.notecraft`; ink coordinates are clamped to 1,000,000 and an id is cut to 64 characters. |
| 2026-10-15 | 1 | PR 1.4 landed the writer: status line updated; edits are span replacements, text a user adds takes the file's dominant line ending (LF on a tie), a new frontmatter block goes after a byte order mark, and a value outside the YAML subset is written back as it was read. |
| 2026-10-16 | 1 | PR 1.13: structure edits write `notebook.toml` and `.section.toml` (order, levels, colors, names) with craft-atomic; trash items sit in a numbered folder with `origin.toml` instead of `.trash.toml`. |
| 2026-10-16 | 1 | PR 1.5: container markers are read for editing (`split_containers`, a view over the section 3.3 marker reader); the status line notes that new containers are marker lines appended to the page; positions wait for the canvas block (PR 1.9; saved since 2026-10-17). |
| 2026-10-16 | 1 | PR 1.6: section 3.2 lists the formatting markup the Home tab writes (underline, sub and superscript, font color, size, family, alignment comment); yellow highlight moved to Phase 1. |
| 2026-10-17 | 1 | Subpage collapse: section 2.2 documents the `collapsed` page entry key. |
| 2026-10-17 | 1 | Section 2.4: the display width of a resized picture is stored as `<!-- notecraft:width=N -->` after the image; SVG pictures and their limits. |
| 2026-10-17 | 1 | PR 1.9 follow-ups: the canvas block `containers` map is written (`x`, `y`, `w`, rounded to 2 decimals; an entry for a container that no longer exists is dropped, other keys are kept byte for byte); a `.md` page with a positioned container is promoted like one with ink; a block that did not parse can be replaced on request (`page.repair_canvas`), keeping the ink and positions that could be read, and the original is still in `.notecraft/history/`. The warning bar shows with a Repair button; Convert to Markdown is on the File Info page. |
| 2026-10-18 | 1 | Index schema version 3: `pages.collapsed` keeps the `collapsed` page entry flag so the navigation built from the index shows collapsed subpages. |
| 2026-10-18 | 1 | Section 3.4: pictures and file attachments placed on the page are listed in the canvas block `images` and `files` lists (`src`, `nth`, `x`, `y`); the markdown stays in the page text; the width stays the section 2.4 comment; a `.md` page with one promotes to `.notecraft`. Shapes are not stored yet. |
