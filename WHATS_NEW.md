# What's New in NoteCraft

This file is compiled into the app and shown in the What's New dialog. Everything before
the first "##" line is ignored, so this paragraph is only for maintainers.

How to edit it at release time (docs/releasing.md section 2):

- Add a new "## <version>" section at the top, newest first, using the exact version from
  Cargo.toml. The dialog opens on the first launch after the version changes.
- Under it, write short user-facing lines. A line starting with "- " is a bullet, a line
  starting with "### " is a small heading, and any other line is a plain paragraph.
- Plain ASCII and US English only (AGENTS.md section 2). Keep each line under 300
  characters, at most 40 lines per version, and at most 50 versions.

## 0.0.2

NoteCraft now opens and saves real notebooks.

### Notebooks and pages
- Open a notebook folder from disk and see its sections, section groups, and pages.
- Pages are plain markdown files. Saving changes only the lines you edited, so a page you
  also edit in a text editor stays tidy.
- Create, rename, move, color, and delete pages, sections, section groups, and notebooks.
  Deleted items go to a trash folder inside the notebook, and every step can be undone.
- The page list reads from a fast index, shows page titles, and can fold away subpages.
- Rule lines, page color, and paper size are saved with each page.

### Writing
- Click anywhere on the page and type. Text lives in containers that you can move and
  resize.
- Bold, italic, lists, and the rest of the formatting commands work on selected text.
  Bold text uses your system's bold font.
- Cut, copy, and paste text, including pictures from the clipboard.

### Ink
- Draw with the pen and the highlighter, and choose their colors and thickness.
- Try the pencil and the textured pens: rainbow, galaxy, lava, ocean, and four metals.
- The highlighter blends with the page, so text and rule lines show through.
- Erase a whole stroke or just part of one. One drag of an eraser is one undo step.
- Ink is saved with the page. Pen pressure works on Windows.

### Pictures and files
- Insert pictures and attach files to a page, and open attachments from the page.
- Drag pictures and files anywhere on the page, and resize pictures by their corners.

### Fixes
- On Linux computers without a working graphics driver, NoteCraft now starts with a
  slower fallback renderer instead of closing.

### Safety
- Undo and redo for typing, ink, and notebook changes.
- Pages save by themselves every 30 seconds and when you switch away.
- After a crash, NoteCraft offers to restore the text you had not saved.
- If a page changes on disk while you are editing it, you choose which version to keep.

### Command line
- The notecraft-cli tool can show notebook info, print page text, check a notebook for
  problems, and run a command on a page.

## 0.0.1

The first preview: the window, the ribbon with the File, Home, Insert, Draw, History,
Review, View, and Help tabs, four theme presets with light and dark modes, the navigation
panes, and the About page. Most buttons were not connected yet and showed "Coming Soon".
