# The NoteCraft file format: markdown and page links

> Stage: pre-alpha (Phase 2 closed; v0.2.0 next). Last reviewed: 2026-11-05. Last updated: 2026-11-05. Change: Phase 2 close review (stage line, review date); earlier: page ids and times written on first save of a `.notecraft` page (feat/page-ids, LK-03 Missing narrowed); earlier: Customize Tags edits the default tags (rename, icon, colors), hides and resets them, and orders the list that sets Ctrl+1 to Ctrl+9 (LK-06); earlier: section 5.4 as built in PR 2.3 (link rewriting on rename, outside renames by id) and index schema version 8 (`pages.fm_id`); earlier: note tags (section 4: the default gallery with slugs, the tag grammar, and index rows) as built in PR 2.5; earlier: section 5.2 and 5.3 as built in PR 2.2 (picker, step 4 also matches paths, creating a missing page); section 8 index schema version 6 (links_hash, page_aliases); earlier: section 3.2 spells out the colored highlight form; earlier: equations (section 3.2) and their limits (section 9) as built in PR 2.7; earlier: page style is saved in the `notecraft:` frontmatter map (new section 2.5: `paper`, `paper_spacing`, `paper_size`, `rule_color`, `color`); earlier: stage line: Phase 1 merged into main (#55); earlier: pictures and file attachments get positions in the canvas block (`images`, `files`, section 3.4); ink stroke `texture` key (textured pens and the pencil); earlier: index schema version 3 adds pages.collapsed; `collapsed` key in section page entries; earlier: PR 1.5 text editing in containers (runs are a view over the section 3.3 markers); PR 1.9 landed (.notecraft pages and saving ink; one save path with auto-save); the folder reader (section 2) is implemented; file name rules in section 2.3 spelled out; reading implemented (PR 1.3); frontmatter limits and entry spans noted. PR 1.2: index refresh and rebuild rules (section 8); writing implemented (PR 1.4); trash layout (`origin.toml` per deleted item) and structure edits writing the order files (PR 1.13). PR 1.6: text formatting (bold, italic, strikethrough, underline, sub and superscript, headings, lists, indent, tasks, quote, code, alignment as commands; TX-02 to TX-06 scored D); Picture display width comment and SVG pictures (section 2.4); PR 1.5 text editing in containers (runs are a view over the section 3.3 markers); container positions written to the canvas block; repair of a block that did not parse (PR 1.9 follow-ups); section 5.1 reader rules (PR 2.1).

This doc is the spec for how NoteCraft stores notebooks on disk and how pages link to each other. The design goal: a notebook is a plain folder of markdown that stays readable and editable in any text editor, while NoteCraft still keeps the freeform canvas, ink, and layout that make it feel like OneNote.

Status: ink saving, container markers, and container positions are implemented (PR 1.9). The folder reader (section 2: notebooks, groups, sections, page order, settings files, folder depth) is implemented in `notecraft-storage` (PR 1.1). Phase 1 builds the rest of the reader and the writer (build_plan/03-phase-1-core-engine.md). Phase 2 builds page links, search, and note tags (build_plan/04-phase-2-links-search-tags.md). Reading is implemented (PR 1.3): `notecraft-markdown` parses a page into blocks that keep their source byte spans, reads the frontmatter subset of section 3.1, and the canvas shows the blocks in flow layout (section 3.5, read-only). Minimal-diff writing is implemented (PR 1.4): edits are span replacements on the original text, new blocks use the canonical style, and pages are written through `craft-atomic`. Typing in containers is implemented (PR 1.5). Not yet: auto-save from the editor (PR 1.7), page links, note tags, and equations (Phase 2). See build_plan/03-phase-1-core-engine.md and build_plan/04-phase-2-links-search-tags.md.

## 1. Design rules

1. The folder is the truth. Every piece of user content lives in files a person can read. The SQLite index is a cache that NoteCraft can delete and rebuild at any time.
2. Text stays text. Page text is markdown in the page file, never hidden inside JSON.
3. Minimal diffs. When NoteCraft saves, it rewrites only the parts of a file that changed. A page edited in another editor keeps its formatting, spacing, and line endings.
4. Lenient reading. Unknown frontmatter keys, unknown HTML, and malformed canvas data are kept or skipped, never fatal. A page NoteCraft cannot fully parse still opens as plain text.
5. Bounded reading. Every parser caps file size, nesting depth, stroke count, and point count (limits in section 9).
6. Crash-safe writing. Every write goes through `craft-atomic` (write to a temp file, flush, rename).
7. Pages are UTF-8. NoteCraft writes UTF-8 only. A page file that is not valid UTF-8 is never rewritten by a normal save: it opens read-only, decoded for display, and a bar says so. UTF-16 (with a byte order mark, or text that is mostly ASCII with a NUL in every other byte) is decoded as UTF-16; any other invalid bytes are read as Windows-1252 (all 256 bytes map to a character, so nothing is lost). The command `page.convert_encoding` (Convert to UTF-8 on the bar) first keeps the original bytes as `.notecraft/history/<page path>/original-<hash>.bak` (hash of the bytes, so one copy per distinct file), then writes the same text as UTF-8 with its line endings, drops the byte order mark, and makes the page editable. It is not undoable, and it refuses (writes nothing) when the file no longer holds what the page shows. Read-only holds through reloads, outside edits, recovery restore, and saves: unsaved text is never written over a file that is not UTF-8.

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

Promotion: when you add ink, a shape, or drag a container off the default flow on a `.md` page, NoteCraft renames it to `.notecraft` with the same base name. Page links resolve by base name without extension (section 5.3), so promotion never breaks a link. If a file with the new name is already there (a stray `A.notecraft` beside `A.md`), the page gets a numbered name (`A 2.notecraft`) instead of failing, and if the section list or the old file cannot be updated the new file is removed again, so a page is never left as two copies. NoteCraft never demotes automatically; the `page.convert_to_markdown` command does it on request, warns that canvas data will be dropped, and first keeps the whole page in `.notecraft/history/<page path>/before-convert-<hash>.notecraft`.

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

### 2.5 Page style

Rule lines, the rule line color, the page color, and the paper size are kept per page in the `notecraft:` map of the page frontmatter (section 3.1), as OneNote keeps them per page. All keys are optional, and a key that holds the default is not written.

| Key | Values | Default (key absent) |
|---|---|---|
| `paper` | `blank`, `lined`, `grid`, `dotted` (dotted is a kept extra beyond OneNote, see post-parity.md) | `blank` |
| `paper_spacing` | for `lined`: `narrow`, `college`, `standard`, `wide`; for `grid`: `small`, `medium`, `large`, `xlarge`; ignored for `blank` and `dotted` | `college` for `lined`, `medium` for `grid` |
| `paper_size` | `letter`, `legal`, `a4`, `a5` | auto (the page grows with its content) |
| `rule_color` | `#RRGGBB` or `#RRGGBBAA`: the color of the rule lines and grid | the theme's own |
| `color` | `#RRGGBB` or `#RRGGBBAA`: the page background | the theme's canvas color |

```yaml
notecraft:
  paper: lined
  paper_spacing: wide
  paper_size: a4
  rule_color: "#336699"
  color: "#FFF8E1"
```

Reading is lenient: a missing key, an unknown value, a malformed color, or a `notecraft:` that is not a one-level map leaves the default, and the file is never rejected. Writing changes only the keys whose value changed, through the minimal-diff frontmatter rewrite: other keys in the map (such as `id`), their comments, and every other frontmatter entry keep their bytes. A key goes away when its value returns to the default, and the `notecraft:` map goes away when it has no keys left. A frontmatter block is added only when a style key has to be written, so a page that never had a style stays as it was. Both `.md` and `.notecraft` pages carry the keys. If `notecraft:` is written as a flow map (`notecraft: {paper: lined}`) NoteCraft neither reads nor rewrites it, so a style change on such a page is not saved. A new page created while Always Create Pages with Rule Lines is on starts with the `paper` key (and `paper_spacing` when it is not the default).

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
                        # more page style keys: paper_spacing, paper_size, rule_color (section 2.5)
---
```

NoteCraft fills in `created`, `modified`, and `notecraft.id` the first time it saves a page. For a `.md` page, NoteCraft adds frontmatter only if the page needs it (an id is needed once something links to the page by `notecraft://` URL); otherwise a plain `.md` file stays plain.

As built (feat/page-ids, `session/page_ids.rs`): the one save path stamps a `.notecraft` page, and a `.md` page when it becomes a `.notecraft` page on its first stroke or picture. The keys go in through the minimal-diff frontmatter edit, so unknown keys, comments, and key order keep their bytes: `created` and `modified` as top-level keys (RFC 3339 to the minute, with the clock's offset, `Z` for UTC), `id` inside the `notecraft:` map. `created` and `notecraft.id` are written once and never changed; `modified` is rewritten by each save that changes the file (a save that finds the same content writes nothing and keeps it). The id is `pg_` plus 12 Crockford base32 characters from the standard library's random keys, the system time, and a counter (no new dependency). Not stamped: a plain `.md` page (it has no id, so an outside rename of one is not offered), a read-only page, a page in conflict, a frontmatter over the size cap, and a `notecraft:` written as a flow map. Without a clock (the browser build) `created` and `modified` are skipped; without an id source (the browser build has none) the id is skipped; a page is then saved as before. The stamps are added when the file is written, not while typing: the unsaved text, the recovery journal, and the comparison with an outside edit never include them, and after the write the session adopts the stamped text and moves the caret and the recorded undo edits by the added bytes. Two files with the same id (a page copied outside NoteCraft) are never paired by the outside-rename check.

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
| Highlight | `==text==` (yellow); `<mark style="background-color: #RRGGBB">text</mark>` for other colors (see below) | 1 (yellow), 2 (other colors) |
| Underline, sub and superscript | `<u>`, `<sub>`, `<sup>` | 1 |
| Font color, size, family | `<span style="color:#RRGGBB">`, `<span style="font-size:12pt">`, `<span style="font-family:Name">` (one property per span) | 1 |
| Paragraph alignment | a `<!-- notecraft:align=center -->` comment line above the paragraph (`left` is the default and writes nothing) | 1 |

Highlight colors. Yellow (`#FFF176`, the first swatch of Home > Text Highlight Color) is always written `==text==`. Any other color is written `<mark style="background-color: #RRGGBB">text</mark>` with an uppercase six-digit color, and No Color removes either form. Reading is lenient and bounded: the tag must fit in 240 bytes; the style may use `background-color` or `background` (the first that holds a color wins), with `#RGB`, `#RRGGBB`, `#RRGGBBAA` (alpha dropped), or `rgb(r, g, b)`; `!important` and extra shorthand words such as `none` are ignored. A `<mark>` with no style is the default yellow. A `<mark>` whose style holds no readable color (a named color such as `green`, for example) stays plain text, like any other HTML NoteCraft does not write. Editing a page never rewrites a highlight it did not touch, so a file written by hand in another spelling keeps its bytes.

Footnotes, embeds, and heading links are not OneNote features and are not planned before full parity (post-parity.md). NoteCraft still preserves them as text if a page contains them.

Equations are LaTeX math: `$x^2$` is set in the line of text, and `$$rac{a}{b}$$` is set on a line of its own (a formula may span lines between `$$` lines). A dollar sign that starts a number (`$5 and $10`) is not math, `\$` is a literal dollar sign, and `$` inside code spans and code blocks is plain text. A formula is kept as its source text; the canvas typesets it, and shows the source while the caret is in the block. A formula over 4 KiB, or nested deeper than 32 levels, is not typeset and stays visible as source text (section 9).

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
- An ink stroke may carry `"texture"`, the look of a textured pen: `pencil`, `rainbow`, `galaxy`, `lava`, `ocean`, `gold`, `silver`, `bronze`, or `rose_gold`. The key is written only for a textured stroke, and only on a pen stroke (a highlighter has none). A stroke with no texture, a texture this version does not know (any case is read), or a value that is not text is a plain stroke in its `color`, so a page reopens as a plain color stroke in an older or newer reader and loses nothing else. The `color` of a textured stroke is a stand-in for that fallback; only `pencil` is drawn in its `color`. Textures are drawn procedurally, so no image is stored, and a texture follows the stroke along its length, so it looks the same at any zoom and on any machine.
- Pictures and file attachments that sit at a place of their own are listed in `images` and `files`. The picture or file stays in the page text as its markdown (`![alt](src)` or a paragraph that is one link `[name](src)`), usually at the end of the last container, so a text editor still shows it. An entry ties to that markdown by `src` and `nth`: `nth` (a whole number, 0 when left out) says which occurrence of that link in the page text, counting `![..](src)` images (or file paragraphs) in reading order. The entry holds only the top-left corner `x`, `y` in page units. A picture's width is the width comment of section 2.4 (the height follows its proportions); a file object has a fixed size. A picture or file with no entry flows inside its container as before. An entry needs a `src` and a finite `x` and `y`; one without is skipped, `id` is made up when missing or repeated, and at most 10,000 entries of each kind are read. An entry whose link is no longer in the page text (the user deleted it) is dropped on the next save, so deleting the markdown deletes the object. Other keys inside a changed `images` or `files` list (such as `w` and `h` from other writers) are not kept.
- A `.md` page promotes to `.notecraft` as soon as it has a picture or file entry, as it does for ink and container positions (section 3.5). Convert to markdown drops the entries and keeps the links, so the pictures flow in the text again.
- `z` is the paint order, back to front. Ids missing from `z` paint on top in file order.
- A canvas block that fails to parse is kept byte-for-byte in a `.notecraft/history/<page path>/canvas-unreadable-<hash>.notecraft` snapshot (the hash is of the file text, so opening the same broken page again writes nothing new, and a changed file gets its own snapshot), and the page opens in flow layout with a warning bar. NoteCraft never deletes the original block until the user saves.
- Only one canvas block per file. Extra blocks are treated as normal code.

### 3.5 Flow layout

A `.md` page, or a `.notecraft` page with no positions, opens in flow layout: containers stack top to bottom at a fixed width, like a document. Moving anything off the flow (a container, a picture, a file attachment, or a stroke of ink) promotes the page and records positions.

## 4. Note tags

OneNote's note tags are stored as inline tags in the text, so they stay readable in any editor. A tag belongs to the paragraph, heading, or list item that holds it (OneNote tags a paragraph). Applying a tag from the Tags gallery or with Ctrl+1 to Ctrl+9 puts `#slug ` at the start of the paragraph text, after any list marker, check box, heading marker, or quote prefix, and applying it again takes it away.

### 4.1 The default gallery

The first nine have the shortcuts Ctrl+1 to Ctrl+9, as in OneNote; Ctrl+0 removes all tags from the paragraph.

| # | OneNote tag | Stored as | Shown as |
|---|---|---|---|
| 1 | To Do | `- [ ]` task list item (preferred) or `#todo` | check box |
| 2 | Important | `#important` | star icon |
| 3 | Question | `#question` | question mark icon |
| 4 | Remember for later | `#remember` | highlight behind the text |
| 5 | Definition | `#definition` | highlight behind the text |
| 6 | Highlight | `#highlight` | highlight behind the text |
| 7 | Contact | `#contact` | person icon |
| 8 | Address | `#address` | map pin icon |
| 9 | Phone number | `#phone` | telephone icon |
|  | Web site to visit | `#website` | globe icon |
|  | Idea | `#idea` | light bulb icon |
|  | Password | `#password` | key icon |
|  | Critical | `#critical` | exclamation mark icon |
|  | Project A | `#project-a` | flag icon |
|  | Project B | `#project-b` | flag icon |
|  | Movie to see | `#movie` | film icon |
|  | Book to read | `#book` | book icon |
|  | Music to listen to | `#music` | music note icon |
|  | Source for article | `#source` | quotation marks icon |
|  | Remember for blog | `#blog` | pencil icon |
|  | Discuss with Person A | `#discuss-a` | speech bubble icon |
|  | Discuss with Person B | `#discuss-b` | speech bubble icon |
|  | Discuss with manager | `#discuss-manager` | speech bubble icon |
|  | Send in email | `#email` | envelope icon |
|  | Schedule meeting | `#meeting` | calendar icon |
|  | Call back | `#callback` | telephone with arrow icon |
|  | To Do priority 1 | `#todo-1` | check box, red icon |
|  | To Do priority 2 | `#todo-2` | check box, orange icon |
|  | Client request | `#client` | briefcase icon |

Remember for later, Definition, and Highlight are highlight-style: the paragraph text gets a highlight color (yellow, amber, or cyan) and no icon is drawn. The tag name is not shown on the canvas; the paragraph that holds the caret shows its raw markdown, tag names included. Icons are original drawings made in code (see ATTRIBUTION.md); they hang in the margin left of the paragraph, or come first on the line inside a list.

Custom tags (Customize Tags dialog) are stored as `#<slug>`. The slug is made from the name when the tag is added (lowercase ASCII letters, digits, and single hyphens; `tag` when the name has none; a number is added if the slug is taken) and does not change when the tag is renamed, so pages keep their tags. A custom tag has a name (1 to 40 characters), one icon from the set of original icons, an optional font color, and an optional highlight color; at most 50. They are saved in `config.toml` under `[tags]`. Deleting a custom tag leaves the `#slug` text in pages; it then shows as plain text.

Default tags can be changed too, and the page text never changes: a default tag keeps its slug whatever name, icon, or colors it is given. The changes are saved in `config.toml` under `[tags]` as `[[overrides]]` entries holding only the fields that differ from the built-in value, so a later change to a default still applies to what was not overridden. A color of `none` in an override means the built-in color was cleared. Deleting a default tag from the list (`hidden`) only takes it out of the gallery and the Ctrl+number list; `#slug` text in pages keeps its icon. The `order` entry lists the slugs of the gallery in order when it is not the built-in order; the first nine are Ctrl+1 to Ctrl+9. Damaged entries are dropped one by one.

### 4.2 The tag grammar

- A tag is `#` followed by a name. The `#` is the first character of the paragraph text or comes right after whitespace. So `a#b`, `(#b)`, `*#b*`, `\#b`, and `http://x/#b` are not tags.
- The name starts with an ASCII letter and goes on with ASCII letters, digits, `-`, and `_`, at most 64 bytes. Trailing `-` and `_` are not part of the name. Names compare ignoring case and are stored in lowercase in the index.
- The name must end at the end of the text, at whitespace, or before one of `. , ; : ! ? ) ] } " ' * ~ <`. Any other next character (a non-ASCII letter, `/`, `#`, `@`) means the word is not a tag.
- Nothing inside a code span, a code block, raw HTML or an HTML tag, an autolink (`<...>`), a link or image (`[text](target)`), or a page link (`[[...]]`) is a tag. Table cells are not tagged. A heading's leading `#` marks are not tags, because a name must start with a letter.
- A word that looks like a tag but is not in the gallery stays plain text on the canvas and is still recorded in the index.
- A task list item (`- [ ]` or `- [x]`) is a To Do tag on its paragraph; its check box is the tag. A paragraph with both a check box and `#todo` counts once.
- An unknown or hostile input never fails the reader: scanning is bounded (64 tags per paragraph, 2 KiB look-ahead for code spans, tags, and links).

A limit of the canonical serializer, used only when a whole block is rewritten from its parsed form: it cannot tell a literal `\#word` from a tag, so a rewritten block can turn an escaped `\#word` at the start of a word into a tag. Edits made with the formatting commands and typing change only the bytes touched.

### 4.3 Tags in the index

The index `tags` table (section 8) has one row per tag per paragraph: the page, the tag name, the byte span of `#name` (or of the `[ ]` box of a task item), the byte span of the paragraph, the paragraph text without tag names (at most 300 bytes), and for To Do whether it is checked. The engine fills it after each index refresh for pages that are new or changed (the page content hash is recorded when a page's tags are read), the same way search text is filled. `Index::tagged_items(scope, tags)` lists the tagged items in a section, a section group, a page, or the notebook, in notebook order.

A frontmatter `tags:` list is preserved if present but has no UI before full parity (a page tag pane is a post-parity option).

## 5. Page links

### 5.1 Syntax

OneNote lets you type `[[Page name]]` to link to a page (and creates the page if it does not exist). NoteCraft stores exactly that text.

| Form | Meaning |
|---|---|
| `[[Page]]` | link to a page by name |
| `[[Page\|shown text]]` | link with different display text |
| `[[Section/Page]]` | disambiguate by section path |

Inside a table cell the pipe must be escaped as `\|` (GFM rule), and there `\|` separates the target from the shown text. Outside a table only a plain `|` separates them, and `\|` is a literal pipe in the target. An escaped opening bracket (`\[[x]]`) is plain text, never a link, so text that happens to read `[[x]]` survives a save and reopen. NoteCraft writes `[[Page|Page]]` as `[[Page]]` when it rewrites the block that holds it; blocks it does not rewrite keep their bytes (minimal-diff writes).

### 5.2 Writing links

Typing `[[` opens a page picker. Choosing a page inserts `[[Name]]` using the shortest form that resolves uniquely: the base name when step 1 to 3 then reach exactly that page, else the path from the folder of the current page when that is enough, else the path from the notebook.

The picker (PR 2.2) lists the other pages of the notebook, filtered by what is typed after the brackets (case and accents ignored; titles that start with the text first, then titles that contain it, then paths that contain it), at most 50. Up and Down move, Enter or Tab or a click inserts, Esc closes and keeps the typed text. Typing a `]`, `|`, `[`, or a line break, moving the caret out of the text after the brackets, or clicking elsewhere ends it. Choosing replaces the typed text (and a closing `]]` right after the caret) with the link as one undo step. `[[` inside code does not open it. Pasting a `notecraft://` URL inside the same notebook offers to convert it to a page link.

### 5.3 Resolution order

Given `[[target]]`, NoteCraft looks for, in order:

1. a page whose path relative to the notebook root, without extension, equals `target`;
2. a page whose path relative to the current section equals `target`;
3. a page whose base name equals `target` exactly, if exactly one exists;
4. the same, case-insensitive and Unicode-normalized (NFC);
5. a page whose frontmatter `aliases` contains `target`.

If step 3, 4, or 5 finds more than one page, NoteCraft picks the one closest to the current page in the folder tree, marks the link as ambiguous (dotted underline), and lists all candidates on hover. If nothing matches, the link shows as unresolved (dashed); clicking it creates the page in the current section.

As built (PR 2.2, `notecraft_markdown::LinkIndex`):

- Steps 1 and 2 compare the exact text; step 3 compares the exact base name; step 5 compares an alias exactly. Step 4 compares the base name, the path from the notebook, and the path from the section, all after Unicode NFC normalization and lower casing, so `[[work/plan]]` finds `Work/Plan`. The first step that finds anything decides; a later step is not tried.
- "Closest" is the number of folder steps between the folder of the page with the link and the folder of the candidate (up to the shared parent, then down). Ties go to the shorter path, then to the path in alphabetical order, so the answer never depends on listing order. At most 32 candidates are kept.
- Clicking a link follows it (`link.follow`): a resolved link shows its page, in any section of the notebook; an ambiguous link shows the closest candidate; an unresolved link adds a page to the current section, named after the last part of the target (`[[Section/Page]]` creates `Page`), and shows it. The new page is one undo step, as with `page.new`.
- A click on a link follows it only on a block that does not hold the caret (that block shows its source text). Shift+click is left to text selection. Ink tools draw instead of following.

### 5.4 Renames and moves

Renaming or moving a page through NoteCraft updates every page link that pointed to it, across the notebook, as one undoable command (`page.rename`). The setting `links.update_on_rename` can turn this off. Renames done outside NoteCraft are detected by the file watcher using the page `notecraft.id`; NoteCraft then offers to fix links.

As built (PR 2.3, `notecraft-engine/src/link_rename.rs`):

- Covered: `page.rename`, `page.move` to another section, and `section.rename` and `section.move` for sections and groups (every page path below changes). For each link in the notebook (links on a moved page too, since they resolve from its section) that resolves now, NoteCraft checks whether it still resolves to the same page after the change. If not, the target is replaced by the shortest form that resolves to that page (section 5.2). A link that resolved to nothing is left alone.
- Shown text follows OneNote: a link with display text keeps it (`[[New|the old text]]`). A link without display text whose target was the page's plain name (`[[Old]]`, ignoring case) shows the new name (`[[New]]`; `[[Section/New|New]]` when the name alone is no longer enough). Any other link without display text (`[[Section/Old]]`, an alias) keeps the text it showed (`[[New|Section/Old]]`). In a table cell the separator stays `\|`.
- Only the link text between the brackets changes; every other byte of every page stays as it was. Links in code, and escaped links, are not links and are not touched. A page name that cannot be written inside a link (it holds `[`, `]`, or `|`) is counted and left alone.
- One undo step (a notebook-level transaction, label "Rename page" and so on) holds the rename and every link edit. Pages that are open are edited in memory through the usual save path (their text stays unsaved until it saves, with the usual conflict checks); pages not loaded are rewritten on disk atomically, only if the bytes at that place are still what was planned, otherwise the whole step is made without link edits and a toast says so. Read-only pages (cut short or not UTF-8) and pages in conflict are skipped and listed in the toast.
- `links.update_on_rename` (default on) is a setting in `config.toml` (`[links] update_on_rename`) and a command of the same name.
- Outside renames: the refresh reads `notecraft.id` (section 3.1) of new and changed pages into `pages.fm_id` (schema version 8; an empty value means the page has no id). A page that vanished in one refresh while exactly one new page appeared with the same id is a rename. If some link would stop working, a bar asks "This page was renamed outside NoteCraft. Update N links to it?" with Update links (`links.fix_after_rename`, one undo step) and Not now (`links.fix_dismiss`). Pages without an id are never offered: `.notecraft` pages get one on their first save by NoteCraft (section 3.1), `.md` pages have none. An id found on two or more pages pairs nothing.

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
| `page_aliases` | page, frontmatter alias (read for link resolution step 5) |
| `tags` | page, note tag, byte range |
| `attachments` | path, size, mime type, hash, referencing pages |
| `fts_pages` | SQLite FTS5 full-text index over page text |

On open, NoteCraft compares file modified times and sizes against the index, hashes the pages that differ (FNV-1a over the file, read up to the 64 MiB page limit), and reparses only what changed. The schema version is the SQLite `user_version` pragma. A database that is corrupt, not a database, or from a newer schema is deleted and rebuilt from the folder. The index is native only; the wasm32 build has no index. A notebook with 1,000 pages must list its sidebar in under 500 ms (SCRD section 11); the full reindex can finish in the background.

Page links (schema version 6): a page's `links` rows (the link text and the byte range of the whole `[[...]]` in the file) and `page_aliases` rows are rebuilt when its content hash differs from the `links_hash` they were read from, in the same background refresh that rebuilds its search text, and the CLI `index` command does the same. The `resolved_page` of every link (the closest page, for an ambiguous link; empty when unresolved) is recomputed against all pages whenever a page was added, removed, renamed, moved, or edited, so a link resolves as soon as its page appears. NoteCraft's own saves skip the refresh as before unless the page's links, their positions, or its aliases changed. The canvas does not read these rows: it resolves the links of the page shown from the open notebook's tree (and the aliases the refresh last reported), so it is current while the user types.

## 9. Limits

| Limit | Default | On overflow |
|---|---|---|
| Page file size | 64 MiB | opened read-only as plain text |
| Page encoding | UTF-8 | UTF-16 and Windows-1252 pages open read-only, decoded (section 1, rule 7) |
| Frontmatter size | 64 KiB | frontmatter kept as text, keys ignored |
| Canvas block size | 48 MiB | block preserved, page opens in flow layout |
| Ink strokes per page | 100,000 | extra strokes kept in file, not drawn, warning shown |
| Points per stroke | 20,000 | stroke simplified on load |
| Markdown nesting depth | 64 | deeper content shown as plain text |
| Folder depth | 8 | deeper folders ignored, warning shown |
| Links per page | 10,000 | extra links not indexed |
| Formula size | 4 KiB | not typeset, shown as source text |
| Formula nesting depth | 32 | not typeset, shown as source text |

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
| 2026-10-18 | 1 | Section 3.4: an ink stroke may carry a `texture` (pencil, rainbow, galaxy, lava, ocean, gold, silver, bronze, rose_gold); unknown or missing means a plain color stroke. |
| 2026-10-18 | 1 | Section 3.4: pictures and file attachments placed on the page are listed in the canvas block `images` and `files` lists (`src`, `nth`, `x`, `y`); the markdown stays in the page text; the width stays the section 2.4 comment; a `.md` page with one promotes to `.notecraft`. Shapes are not stored yet. |
| 2026-10-18 | 1 | Stage line: Phase 1 merged into main (#55); the v0.0.2 alpha is next. |
| 2026-10-18 | 1 | Section 2.5 (new): page style keys in the `notecraft:` frontmatter map: `paper` (blank, lined, grid, dotted), `paper_spacing`, `paper_size`, `rule_color`, and `color`; lenient reading, minimal-diff writing, a key is removed at its default. |
| 2026-10-24 | 2 | Section 3.2 and section 9: equations (`$...$` and `$$...$$`) as built in PR 2.7, with the 4 KiB and 32-level formula limits. |
| 2026-10-24 | 2 | Section 5.1 implemented (PR 2.1): the reader skips code spans, code blocks, and raw HTML, treats `\|` as the separator in table cells, ignores an escaped opening bracket, and reads at most 1,024 bytes between the brackets. |
| 2026-10-24 | 2 | Section 5.1: `\|` is the separator only inside table cells; an escaped opening bracket is plain text; a rewritten block drops a redundant shown text. |
| 2026-10-24 | 2 | Sections 2.3 and 3.4: unreadable-block snapshots are named by content hash (`canvas-unreadable-<hash>.notecraft`) and written once; Convert to Markdown keeps a `before-convert-<hash>.notecraft` snapshot; a page change that meets a same-stem file gets a numbered name and a partial failure is rolled back. |
| 2026-10-24 | 1 | Encoding safety (fix/encoding-safety): section 1 rule 7 and the section 9 limits row. A page that is not valid UTF-8 (UTF-16, Windows-1252) opens read-only, correctly decoded, and `page.convert_encoding` keeps a hashed backup of the original bytes, then rewrites it as UTF-8. Decision: a BOM-less UTF-16 guess needs 4 or more pairs with NULs in at least 30 percent of one byte position and none in the other. |
| 2026-11-05 | 2 | Section 3.2: the `<mark>` highlight form spelled out (written form, lenient reading, limits). Stage line updated. |
| 2026-11-05 | 2 | Sections 5.2, 5.3, and 8 as built in PR 2.2: the picker and its keys, how "closest" is measured, step 4 also matches paths, clicking a link follows it or creates the page, the `links` rows are filled by the refresh, and index schema version 6 adds `pages.links_hash` and `page_aliases`. |
| 2026-11-05 | 2 | Section 4 rewritten (PR 2.5): the 29 default tags with slugs and shortcuts, the exact `#name` grammar, custom tags and settings, and the `tags` index rows. |
| 2026-11-05 | 2 | Section 5.4 as built in PR 2.3: which commands rewrite links, how shown text is kept, the single undo step, safety rules for open, unloaded, read-only, and conflicted pages, and outside renames found by `notecraft.id`; index schema version 8 adds `pages.fm_id`. |
| 2026-11-05 | 2 | Section 4.1: default tags can be restyled, hidden, and reordered; what `[tags]` keeps (feat/customize-default-tags). |
| 2026-11-05 | 2 | Section 3.1 as built (feat/page-ids): `notecraft.id`, `created`, and `modified` written on the first save of a `.notecraft` page; section 5.4: duplicate ids pair nothing. |
| 2026-11-05 | 2 | Phase 2 close: reviewed (Last reviewed bumped), stage line set to Phase 2 closed. |
