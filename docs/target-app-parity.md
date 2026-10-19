# OneNote parity assessment

> Stage: pre-alpha (Phase 1 on main; v0.0.2 alpha next). Last reviewed: 2026-10-18. Last updated: 2026-10-18. Change: rows re-scored and the Now column filled for the state after Phase 1 merged into main (#55); page color and rule lines are P (not saved to the page file yet); font, size, and color per run are D (`<span style>`); earlier: OneNote import of every OneNote file type (`.one`, `.onex`, `.onetoc2`, `.onepkg`) moved into Phase 3 with full fidelity (PR 3.16, 3.17; maintainer decision, 2026-10-16); PR 1.10: the pen and input rows are partial; PR 1.8: undo and redo is partial; Ink Replay scheduled in PR 3.2 (maintainer decision, 2026-10-15); Ink to Math stays not planned; PR 1.11: the highlighter and both erasers are partial.


Target: OneNote for Windows as shipped around 2020 (OneNote 2016 desktop and the Microsoft 365 app of that era), with theme presets for 2016, 2019, 2021, and Microsoft 365. NoteCraft matches OneNote's workflow and layout one to one. It differs on purpose in one place: storage. OneNote keeps notebooks in a binary format; NoteCraft keeps them as markdown folders (docs/markdown.md).

Only OneNote features are assessed here. Optional extras OneNote does not have are in [post-parity.md](post-parity.md).

Scores follow [parity-checklist.md](parity-checklist.md): D = done, P = partial, M = missing. Percentages are rough, weighted by how often a typical OneNote user touches each feature. "Now" is the state on main after Phase 1 merged (2026-10-18): a P row counts about half, and most Phase 1 P rows wait only on the hands-on checklist in [roadmap.md](roadmap.md#hands-on-checklist-for-the-maintainer).

## Summary

Hours are the sum of the build plan PRs mapped to each area (ROADMAP.md has the same numbers). "Full parity" includes Phase 4.

| Area | Now | Alpha target | Beta target | 1.0 target | Hours to 1.0 | Hours to full parity |
|---|---:|---:|---:|---:|---|---|
| Canvas and freeform layout | 50% | 15% | 70% | 90% | 90-152 | 90-152 |
| Ink and drawing | 25% | 10% | 65% | 85% | 154-262 | 294-518 |
| Text and formatting | 60% | 30% | 80% | 95% | 156-268 | 156-268 |
| Notebook structure | 65% | 40% | 85% | 90% | 118-198 | 158-278 |
| Ribbon and chrome | 70% | 50% | 85% | 90% | 126-210 | 142-238 |
| Themes | 100% | 60% | 90% | 98% | 40-68 | 40-68 |
| Attachments and media | 40% | 15% | 70% | 75% | 52-91 | 152-271 |
| Page links, search, note tags | 0% | 0% | 65% | 85% | 92-154 | 92-154 |
| Print and export | 0% | 0% | 70% | 90% | 88-153 | 88-153 |
| Overall (user-facing areas) | about 45% | about 25% | about 70% | about 90% | 916-1,556 | 1,212-2,100 |

Shared work not tied to one area (file formats, command registry, localization, performance and hardening, browser build, scaffolding) adds 363-646 hours to 1.0 and 711-1,330 hours to full parity. ROADMAP.md lists it by dimension.

## 1. Canvas and freeform layout

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Click anywhere on a page and type; a note container appears there | Same; container saved with a `notecraft:container` marker and position | 1 | D |
| Drag a container by its top bar; resize its width | Same | 1 | P |
| Page grows right and down as content is added | Canvas extents fit content plus margin | 1 | D |
| Zoom 10% to 400%, zoom to page width | Same range; Ctrl+wheel, pinch, View tab | 0 | D |
| Rule lines and grid (View > Rule Lines), with the red margin line on ruled paper | `paper:` frontmatter: blank, lined, grid, dotted (dotted is a kept extra beyond OneNote, see post-parity.md) | 0 (draw, done), 1 (save, not yet: kept per page in the session only, docs/gaps.md) | P |
| Page color | `notecraft.color` frontmatter; set live with `view.page_color` and undoable, not yet written to the page file (docs/gaps.md) | 1 | P |
| Insert Space tool | `canvas.insert_space` command | 3 | M |
| Lasso and rectangle select across ink, text, images | Same | 3 | M |
| Arrange: bring forward, send back | Same; stored in canvas `z` list | 3 | M |
| Snap to grid | Grid snap and alignment guides | 3 | M |
| Page title area with date and time | Title from frontmatter or file name; created date under it | 0 (shell), 1 | D |
| New Window | Second window on the same notebook | 3 | M |

## 2. Ink and drawing

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Pens with color and thickness, pressure-sensitive | Pen tool with pressure from pen tablets and touch | 1 | P |
| Highlighter | Semi-transparent multiply-blend stroke | 1 | P |
| Stroke eraser and point eraser | Both | 1 | P |
| Lasso select ink, move, recolor | Same | 3 | M |
| Ink to Shape | Recognize lines, rectangles, ellipses, triangles, arrows | 3 | M |
| Shapes gallery | 16 presets | 3 | M |
| Ruler | Straight-edge guide for ink | 3 | M |
| Draw with mouse, pen, or touch; palm rejection | Input normalized in `notecraft-core` | 1 (mouse, pen), 3 (touch) | P |
| Ink to Text (handwriting recognition) | On-device recognizer | 4 | M |
| Search handwritten notes | Index recognized text | 4 | M |
| Ink replay | Strokes replayed in drawing order with a transport bar | 3 | M |
| Ink to Math | Not planned for 1.x | n/a | M |

## 3. Text and formatting

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Bold, italic, strikethrough | Markdown `**`, `*`, `~~` | 1 | D |
| Underline | `<u>` HTML inside markdown | 1 | D |
| Text highlight color | `==text==`, or `<mark>` for other colors | 1 (yellow), 2 (other colors) | P |
| Font, size, font color per run | One-property `<span style>` for color, size, and family (docs/markdown.md section 3.2) | 1 | D |
| Bullets, numbering, indent | Markdown lists | 1 | D |
| Styles (Heading 1 to 6, Quote, Code, Normal) | Markdown headings, blockquotes, code | 1 | D |
| To Do checkbox | GFM task list | 1 | D |
| Tables, insert and delete rows and columns, header row | GFM tables; table tools in Phase 3 | 1 (render), 3 (tools) | P |
| Equations | LaTeX math | 2 | M |
| Insert date, time, date and time | Same shortcuts (Alt+Shift+D, T, F) | 2 | M |
| Spell check | Spell check with suggestions | 3 | M |
| Find on page (Ctrl+F) | Same | 2 | M |

## 4. Notebook structure

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Notebook list, open and close notebooks | Notebook = folder; recents in File backstage | 0 (mock), 1 | D |
| Section tabs with colors | Folders; color in `.section.toml` | 0 (mock), 1 | D |
| Section groups | Folders of sections | 1 | D |
| Page list with subpages | `level` in `.section.toml` | 0 (mock), 1 | D |
| Drag to reorder and move pages and sections | Same; moves files and updates links | 1 | P |
| Undo and redo | Same | 1 | D |
| Notebook Recycle Bin | `.notecraft/trash/`, 30 days default, configurable | 3 | M |
| Page Versions | `.notecraft/history/` snapshots | 3 | M |
| Page templates | Template pages in a `Templates` section | 3 | M |
| Password-protected sections | Encrypted section pages (Phase 4 design) | 4 | M |

## 5. Ribbon and chrome

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| File (backstage), Home, Insert, Draw, History, Review, View tabs | Same tabs and groups, original icons | 0 | D |
| Quick Access Toolbar | Same, customizable | 3 | M |
| Collapsible ribbon (Ctrl+F1), key tips | Same | 0 | D |
| Navigation: notebooks, sections, pages | Same | 0 | D |
| Search box (Ctrl+E) with scopes | Same | 0 (shell), 2 | P |
| Full Page View (F11) | Same | 0 | D |
| Docked window (Ctrl+Alt+D) | Same | 4 | M |
| Keyboard shortcuts | OneNote defaults where they do not clash with the OS | 0 | D |
| Accessibility (keyboard, screen readers) | AccessKit | 0 | D |
| Web version | Browser build | 4 | M |
| Localized UI, right-to-left | String table (3), translations and RTL (4) | 3, 4 | M |

## 6. Themes

Four presets, each in light and dark (docs/themes.md). Palettes are original colors chosen to evoke each era, never sampled from Microsoft artwork.

| Preset | Phase | Score |
|---|---|---|
| OneNote 2016 | 0 | D |
| OneNote 2019 | 0 | D |
| OneNote 2021 | 0 | D |
| Microsoft 365 | 0 | D |

## 7. Attachments and media

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Insert pictures | PNG, JPEG, WebP, SVG | 1 | P |
| Insert file as attachment (icon) | File object with icon and name, opens in default app | 1 | D |
| Drag and drop files; paste pictures | Same | 1 | P |
| Insert file printout (PDF pages as images) | Render PDF pages to images on the canvas | 3 | M |
| Screen clipping | Not planned for 1.0; may join Phase 4 | n/a | M |
| Record audio and video, linked notes | Same, open codecs | 4 | M |
| Copy text from picture (OCR) | On-device OCR | 4 | M |
| Online video embed | Not planned (offline-first) | n/a | M |

## 8. Page links, search, and note tags

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Type `[[Page name]]` to link to a page; creates it if missing | Same | 2 | M |
| Links keep working after rename and move | Same | 2 | M |
| Copy link to page | `notecraft://` URL | 3 | M |
| Search across page, section, group, notebook, all notebooks | FTS5 index with the same scopes | 2 | M |
| Note tags (To Do, Important, Question, more), Customize Tags | Inline tags with original icons | 2 | M |
| Find Tags summary and summary page | Same | 2 | M |

## 9. Print and export

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Print with preview | Through craft-print (docs/printing.md) | 3 | M |
| Export page, section, or notebook as PDF | Same renderer as print | 3 | M |
| Export as single-file web page | Self-contained HTML | 3 | M |
| Export notebook | Zip of the notebook folder | 3 | M |
| Import OneNote sections and notebooks | `.one`, `.onex`, `.onetoc2`, and `.onepkg` import with full fidelity, from the published specs (Phase 4 content in PR 4.3) | 3 | M |

## 10. Not planned (with reasons)

| OneNote feature | Why not |
|---|---|
| Cloud sync and sharing through a Microsoft account | Microsoft service. NoteCraft notebooks are folders; any folder sync tool works. Native sync is Phase 4 research only. |
| Real-time co-authoring | Same |
| Microsoft Search, Researcher, Dictate, Immersive Reader, Translate | Microsoft services |
| Embedded Office documents with live preview | Needs proprietary renderers; shows an icon and opens externally instead |
| Per-run fonts and colors | Plain markdown cannot store them without turning pages into HTML |
| Send to OneNote printer, Outlook tasks, email page | Microsoft integrations |

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Re-scoped the same day to OneNote features only; recording, password sections, OCR, Find Tags, timestamps added; hours re-assessed bottom-up. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-15 | 0 | Rule lines row: ruled paper has the red margin line; dotted paper is a kept extra. |
| 2026-10-15 | 1 | Pen and input normalization rows scored P (PR 1.10): pen, smoothing, and mouse and touch capture work; strokes are not saved yet and no real pen was tried. |
| 2026-10-15 | 1 | Undo and redo row scored P (PR 1.8). |
| 2026-10-15 | 1 | Status line only: ink replay stays not planned; the design is in docs/architecture.md section 10.6 if the maintainer schedules it. |
| 2026-10-15 | 1 | Ink Replay scheduled in PR 3.2 (maintainer decision, 2026-10-15); split from the Ink to Math row, which stays not planned. |
| 2026-10-16 | 1 | PR 1.11: the highlighter and both erasers are partial (strokes are not saved yet; true multiply is later). |
| 2026-10-16 | 1 | OneNote import covers every OneNote file type with full fidelity, in Phase 3 (maintainer decision). |
| 2026-10-18 | 1 | Re-scored every row from parity-checklist.md after Phase 1 merged into main (#55) and filled the Now column (about 45% overall). Page color and rule lines are P: they apply live and undo, but are kept per page in the session and not written to the page file. Font, size, and color per run moved from n/a to D (one-property `<span style>`, PR 1.6). Text highlight is P (yellow done, other colors Phase 2). |
