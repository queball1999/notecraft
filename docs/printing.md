# Printing and PDF

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: project renamed to NoteCraft; earlier changes are in the revision history.

NoteCraft prints the same way the other craft apps do: render to PDF, then hand the PDF to `craft-print`, which talks to the operating system. One renderer serves Print, Export to PDF, and `notecraft-cli render --format pdf`.

Status: not implemented. Until Phase 3, File > Print and Export > PDF show "Coming Soon".

## 1. Pipeline

```text
page or section
  -> notecraft-engine::export::pdf   (L5)  lays out pages, draws text, ink, shapes, images
  -> PDF bytes
  -> Print:   notecraft (L7) -> craft-print -> winprint (Windows) or CUPS (macOS, Linux)
  -> Export:  craft-atomic write to the chosen path
```

- The PDF writer lives in `notecraft-engine` so PDFs can be produced without a window (tests, and any future automation). It must not depend on egui.
- Printing lives in the desktop app (`notecraft`), because `craft-print` talks to the OS.
- Never depend on `winprint` directly; `craft-print` wraps it (AGENTS.md section 6).

## 2. Print dialog

The desktop app uses the print settings dialog from `craft-ui`, so it looks and behaves like the other craft apps:

| Setting | Values |
|---|---|
| Printer | from `craft-print`'s printer list |
| Pages | current page, current section, page selection |
| Paper size | Letter (default in the US locale), A4, Legal, others from the printer |
| Orientation | portrait, landscape |
| Scale | fit to page width (default), actual size (100%), custom percent |
| Include | page title and date (on by default), background paper lines (off by default), page color (off by default) |
| Copies | 1 to 999 |

Print Preview shows the paginated PDF in the dialog before printing.

## 3. Paginating an infinite canvas

OneNote pages have no fixed size, so NoteCraft cuts the canvas into paper-sized tiles:

1. Compute the page bounds: the union of all containers, ink, shapes, and images, plus a 0.5 inch margin.
2. Scale: in "fit to page width", scale so the bounds' width fits the printable width. In "actual size", 96 canvas units = 1 inch.
3. Cut into tiles top to bottom. Avoid cutting through a line of text: move a cut up to the nearest gap between text lines if one exists within 15% of the tile height. Ink and images may be cut.
4. Repeat the page title on the first tile only. Put "Page n of m" in the footer.

## 4. PDF content

| Object | How it is written |
|---|---|
| Text | Real text with embedded font subsets, so PDFs are searchable and copyable |
| Ink | Vector paths; pressure becomes variable-width outlines |
| Highlighter | Paths with multiply blend and 50% opacity |
| Shapes | Vector paths |
| Images | Embedded at original resolution, downsampled above 300 DPI at print size |
| File attachments | Icon and file name; optionally embedded as PDF file attachments (setting) |
| Links | Page links and URLs become PDF link annotations; page links to pages in the same export jump inside the PDF |

Color management through `craft-cms` is an optional feature (`color-managed`), off by default.

## 5. Implementation steps (Phase 3)

Full detail in build_plan/05-phase-3-polish-parity.md, PRs 3.6 to 3.8.

1. `notecraft-engine::export::pdf`: page bounds, pagination, text, ink, shapes, images. Golden tests compare PDF text extraction and rasterized output.
2. `notecraft`: File > Print with the `craft-ui` dialog and `craft-print`; Export > PDF.
3. The Printing workflow (`.github/workflows/printing.yml`) runs app and UI tests on Windows, macOS, and Linux for any change that touches printing.

## 6. Testing without a printer

`craft-print` can target a file or a virtual printer on CI. Tests assert that the print job is created with the expected PDF bytes and settings; they never require physical paper.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Pipeline, dialog, pagination, PDF content, Phase 3 steps. Same day: CLI print path moved to post-parity. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
