# Phase 4: advanced (post-1.0)

> Stage: pre-alpha (scaffolding). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: project renamed to NoteCraft; earlier changes are in the revision history.

Goal: the remaining OneNote features, which are large and independent of each other: handwriting recognition and ink search, OneNote `.one` import, a browser build (OneNote has a web version), sync research, docked window, right-to-left layout, translations, OCR (copy text from picture), audio and video recording, and password-protected sections. Together with v1.0.0 these complete full OneNote parity. Order by user demand.

| | |
|---|---|
| Phase branch | `phase/4-advanced` |
| Hours | see "Estimate" |
| Checklist rows | IN-11, IN-12, FM-09, AT-08, AT-09, ST-12, SH-17 to SH-20 |

## Entry criteria

- v1.0.0 published.
- Each PR below starts with a short design note in its PR body (or a doc in `docs/` for the larger ones) reviewed by the maintainer before code. These estimates are the least certain in the plan; re-estimate after each design note.

## PR 4.1: handwriting recognition

| | |
|---|---|
| Branch | `feat/handwriting` |
| Hours | 120-220 |
| Rows | IN-11 |

Steps: on-device only (offline-first). Evaluate OS recognizers through safe wrapper crates (Windows Ink recognizer, macOS) against a bundled openly licensed model run by a pure-Rust inference crate. Recognition runs in the background per stroke group and stores text in the canvas block (`ink_text` per group) so search can use it. Draw > Ink to Text converts a selection.

Commits (after design approval):

1. `docs: add handwriting recognition design`
2. `feat(canvas): group strokes into words and lines`
3. `feat(engine): recognize stroke groups in the background`
4. `feat(storage): store recognized text in the canvas block`
5. `feat(ui): convert selected ink to text`

## PR 4.2: ink search

| | |
|---|---|
| Branch | `feat/ink-search` |
| Hours | 20-36 |
| Depends on | 4.1 |
| Rows | IN-12 |

Commits:

1. `feat(storage): index recognized ink text in fts`
2. `feat(ui): highlight matching ink in search results`

## PR 4.3: OneNote `.one` import (best effort)

| | |
|---|---|
| Branch | `feat/one-import` |
| Hours | 160-320 |
| Rows | FM-09 |

Clean-room rule: implement only from Microsoft's published open specifications for the OneNote file formats ([MS-ONESTORE] and [MS-ONE]), never from decompiled code or other implementations' source. Cite the spec sections in comments.

Steps: new crate `notecraft-import-one` (L2, no UI); parse sections (`.one`) leniently and bounded; map outlines to containers, ink to strokes, images and files to attachments, tags to inline tags, page hierarchy to levels; write a NoteCraft notebook folder; import report listing anything skipped. Fuzz the parser.

Commits:

1. `docs: add one import design with spec references`
2. `feat(import): parse onestore file structure`
3. `feat(import): map pages, outlines, and text`
4. `feat(import): map ink, images, and attachments`
5. `feat(ui): add file import wizard with report`
6. `test(import): fuzz the onestore parser`

## PR 4.4: sync research

| | |
|---|---|
| Branch | `docs/sync-research` |
| Hours | 40-80 |

Deliverable is a design doc (`docs/sync.md`) and a throwaway prototype, not a feature: how far folder sync tools already get users, conflict cases specific to `.notecraft` canvas blocks (merge by object id), and whether a NoteCraft sync service is worth building.

Commits:

1. `docs: add sync research and recommendation`

## PR 4.5: browser build

| | |
|---|---|
| Branch | `feat/web` |
| Hours | 60-110 |
| Rows | SH-18 |

Steps: create `apps/notecraft-web` (L7) with eframe web; storage backend over the browser's origin private file system and the File System Access API where available; `packaging/web/package.sh` for the shared release workflow; set `has-web: true` in `release.yml`.

Commits:

1. `feat(web): add notecraft-web app on eframe web`
2. `feat(storage): add browser file system backend`
3. `build(packaging): add web packaging script`
4. `ci: enable web build in release`

## PR 4.6: docked window

| | |
|---|---|
| Branch | `feat/dock` |
| Hours | 16-28 |
| Rows | SH-17 |

Commits:

1. `feat(app): dock the window to a screen edge`

## PR 4.7: right-to-left layout

| | |
|---|---|
| Branch | `feat/rtl` |
| Hours | 30-60 |
| Rows | SH-20 |

Commits:

1. `feat(ui): mirror layout for right-to-left locales`
2. `feat(markdown): support bidirectional text in containers`

## PR 4.8: first translations

| | |
|---|---|
| Branch | `feat/translations` |
| Hours | 50-100, plus native-speaker review time |
| Rows | SH-19 |

Commits:

1. `feat(ui): add <locale> translation` (one PR per locale, reviewed by a native speaker)

## PR 4.9: OCR in images

| | |
|---|---|
| Branch | `feat/ocr` |
| Hours | 40-70 |
| Rows | AT-09 |

Commits:

1. `docs: record ocr engine choice`
2. `feat(engine): extract text from images in the background`
3. `feat(storage): index image text for search`

## PR 4.10: audio and video recording

| | |
|---|---|
| Branch | `feat/recording` |
| Hours | 60-110 |
| Rows | AT-08 |

Steps: Insert > Record Audio and Record Video, as in OneNote. Saved to `attachments/` as open formats (Opus in Ogg for audio; video codec chosen in the design note, royalty-free only). Playback controls on the canvas object. Notes typed during a recording store a time offset in the canvas block, so clicking a note seeks playback (OneNote's linked notes). Capture through a cross-platform audio crate; camera capture is the riskiest part, so video can ship after audio.

Commits:

1. `docs: add recording design and codec choice`
2. `feat(app): record audio to an attachment`
3. `feat(canvas): add playback controls for recordings`
4. `feat(engine): link typed notes to recording time`
5. `feat(app): record video to an attachment`

## PR 4.11: password-protected sections

| | |
|---|---|
| Branch | `feat/section-password` |
| Hours | 40-80 |
| Rows | ST-12 |

Steps: Review > Password, as in OneNote. A protected section's pages are encrypted at rest (authenticated encryption with a key derived by a memory-hard KDF; crates chosen in the design note and reviewed by someone with crypto experience). The encrypted files cannot be read in other editors, which the UI states plainly when a user turns protection on. Lock after a configurable idle time; protected sections are excluded from the search index while locked. No password recovery: say so in the dialog.

Commits:

1. `docs: add section encryption design`
2. `feat(storage): encrypt and decrypt protected section pages`
3. `feat(ui): add password dialog and lock state`
4. `feat(storage): exclude locked sections from the index`
5. `test(storage): wrong password, truncated files, and tampering`

## PR 4.12: close Phase 4

Follow 00-conventions.md section 6 whenever the chosen subset is done; release as 1.x minor versions as items land.

## Estimate

| PR | Hours |
|---|---|
| 4.1 handwriting | 120-220 |
| 4.2 ink search | 20-36 |
| 4.3 `.one` import | 160-320 |
| 4.4 sync research | 40-80 |
| 4.5 browser build | 60-110 |
| 4.6 docked window | 16-28 |
| 4.7 right-to-left | 30-60 |
| 4.8 translations | 50-100 |
| 4.9 OCR | 40-70 |
| 4.10 audio and video recording | 60-110 |
| 4.11 password-protected sections | 40-80 |
| 4.12 close | 8-14 |
| Total | 644-1,228 |

## Exit criteria

- Every row listed at the top scored D.
- Full OneNote parity reached; docs/post-parity.md items may now be considered.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. WASM plugins moved to docs/post-parity.md; recording and password-protected sections added for full parity. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
