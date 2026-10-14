# OneNote parity assessment

> Stage: pre-alpha (scaffolding). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: project renamed to NoteCraft; earlier changes are in the revision history.

Target: OneNote for Windows as shipped around 2020 (OneNote 2016 desktop and the Microsoft 365 app of that era), with theme presets for 2016, 2019, 2021, and Microsoft 365. NoteCraft matches OneNote's workflow and layout one to one. It differs on purpose in one place: storage. OneNote keeps notebooks in a binary format; NoteCraft keeps them as markdown folders (docs/markdown.md).

Only OneNote features are assessed here. Optional extras OneNote does not have are in [post-parity.md](post-parity.md).

Scores follow [parity-checklist.md](parity-checklist.md): D = done, P = partial, M = missing. Percentages are rough, weighted by how often a typical OneNote user touches each feature.

## Summary

Hours are the sum of the build plan PRs mapped to each area (ROADMAP.md has the same numbers). "Full parity" includes Phase 4.

| Area | Now | Alpha target | Beta target | 1.0 target | Hours to 1.0 | Hours to full parity |
|---|---:|---:|---:|---:|---|---|
| Canvas and freeform layout | 0% | 15% | 70% | 90% | 90-152 | 90-152 |
| Ink and drawing | 0% | 10% | 65% | 85% | 154-262 | 294-518 |
| Text and formatting | 0% | 30% | 80% | 95% | 156-268 | 156-268 |
| Notebook structure | 0% | 40% | 85% | 90% | 118-198 | 158-278 |
| Ribbon and chrome | 0% | 50% | 85% | 90% | 126-210 | 142-238 |
| Themes | 0% | 60% | 90% | 98% | 40-68 | 40-68 |
| Attachments and media | 0% | 15% | 70% | 75% | 52-91 | 152-271 |
| Page links, search, note tags | 0% | 0% | 65% | 85% | 92-154 | 92-154 |
| Print and export | 0% | 0% | 70% | 90% | 88-153 | 88-153 |
| Overall (user-facing areas) | 0% | about 25% | about 70% | about 90% | 916-1,556 | 1,212-2,100 |

Shared work not tied to one area (file formats, command registry, localization, performance and hardening, browser build, scaffolding) adds 363-646 hours to 1.0 and 711-1,330 hours to full parity. ROADMAP.md lists it by dimension.

## 1. Canvas and freeform layout

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Click anywhere on a page and type; a note container appears there | Same; container saved with a `notecraft:container` marker and position | 1 | M |
| Drag a container by its top bar; resize its width | Same | 1 | M |
| Page grows right and down as content is added | Canvas extents fit content plus margin | 1 | M |
| Zoom 10% to 400%, zoom to page width | Same range; Ctrl+wheel, pinch, View tab | 0 | M |
| Rule lines and grid (View > Rule Lines) | `paper:` frontmatter: blank, lined, grid, dotted | 0 (draw), 1 (save) | M |
| Page color | `notecraft.color` frontmatter | 1 | M |
| Insert Space tool | `canvas.insert_space` command | 3 | M |
| Lasso and rectangle select across ink, text, images | Same | 3 | M |
| Arrange: bring forward, send back | Same; stored in canvas `z` list | 3 | M |
| Snap to grid | Grid snap and alignment guides | 3 | M |
| Page title area with date and time | Title from frontmatter or file name; created date under it | 0 (shell), 1 | M |
| New Window | Second window on the same notebook | 3 | M |

## 2. Ink and drawing

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Pens with color and thickness, pressure-sensitive | Pen tool with pressure from pen tablets and touch | 1 | M |
| Highlighter | Semi-transparent multiply-blend stroke | 1 | M |
| Stroke eraser and point eraser | Both | 1 | M |
| Lasso select ink, move, recolor | Same | 3 | M |
| Ink to Shape | Recognize lines, rectangles, ellipses, triangles, arrows | 3 | M |
| Shapes gallery | 16 presets | 3 | M |
| Ruler | Straight-edge guide for ink | 3 | M |
| Draw with mouse, pen, or touch; palm rejection | Input normalized in `notecraft-core` | 1 (mouse, pen), 3 (touch) | M |
| Ink to Text (handwriting recognition) | On-device recognizer | 4 | M |
| Search handwritten notes | Index recognized text | 4 | M |
| Ink to Math, ink replay | Not planned for 1.x | n/a | M |

## 3. Text and formatting

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Bold, italic, strikethrough | Markdown `**`, `*`, `~~` | 1 | M |
| Underline | `<u>` HTML inside markdown | 1 | M |
| Text highlight color | `==text==`, or `<mark>` for other colors | 2 | M |
| Font, size, font color per run | Not representable in plain markdown; theme fonts only | n/a | M |
| Bullets, numbering, indent | Markdown lists | 1 | M |
| Styles (Heading 1 to 6, Quote, Code, Normal) | Markdown headings, blockquotes, code | 1 | M |
| To Do checkbox | GFM task list | 1 | M |
| Tables, insert and delete rows and columns, header row | GFM tables; table tools in Phase 3 | 1 (render), 3 (tools) | M |
| Equations | LaTeX math | 2 | M |
| Insert date, time, date and time | Same shortcuts (Alt+Shift+D, T, F) | 2 | M |
| Spell check | Spell check with suggestions | 3 | M |
| Find on page (Ctrl+F) | Same | 2 | M |

## 4. Notebook structure

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Notebook list, open and close notebooks | Notebook = folder; recents in File backstage | 0 (mock), 1 | M |
| Section tabs with colors | Folders; color in `.section.toml` | 0 (mock), 1 | M |
| Section groups | Folders of sections | 1 | M |
| Page list with subpages | `level` in `.section.toml` | 0 (mock), 1 | M |
| Drag to reorder and move pages and sections | Same; moves files and updates links | 1 | M |
| Undo and redo | Same | 1 | M |
| Notebook Recycle Bin | `.notecraft/trash/`, 30 days default, configurable | 3 | M |
| Page Versions | `.notecraft/history/` snapshots | 3 | M |
| Page templates | Template pages in a `Templates` section | 3 | M |
| Password-protected sections | Encrypted section pages (Phase 4 design) | 4 | M |

## 5. Ribbon and chrome

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| File (backstage), Home, Insert, Draw, History, Review, View tabs | Same tabs and groups, original icons | 0 | M |
| Quick Access Toolbar | Same, customizable | 3 | M |
| Collapsible ribbon (Ctrl+F1), key tips | Same | 0 | M |
| Navigation: notebooks, sections, pages | Same | 0 | M |
| Search box (Ctrl+E) with scopes | Same | 0 (shell), 2 | M |
| Full Page View (F11) | Same | 0 | M |
| Docked window (Ctrl+Alt+D) | Same | 4 | M |
| Keyboard shortcuts | OneNote defaults where they do not clash with the OS | 0 | M |
| Accessibility (keyboard, screen readers) | AccessKit | 0 | M |
| Web version | Browser build | 4 | M |
| Localized UI, right-to-left | String table (3), translations and RTL (4) | 3, 4 | M |

## 6. Themes

Four presets, each in light and dark (docs/themes.md). Palettes are original colors chosen to evoke each era, never sampled from Microsoft artwork.

| Preset | Phase | Score |
|---|---|---|
| OneNote 2016 | 0 | M |
| OneNote 2019 | 0 | M |
| OneNote 2021 | 0 | M |
| Microsoft 365 | 0 | M |

## 7. Attachments and media

| OneNote behavior | NoteCraft plan | Phase | Score |
|---|---|---|---|
| Insert pictures | PNG, JPEG, WebP, SVG | 1 | M |
| Insert file as attachment (icon) | File object with icon and name, opens in default app | 1 | M |
| Drag and drop files; paste pictures | Same | 1 | M |
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
| Import OneNote sections | `.one` import from the published spec | 4 | M |

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
