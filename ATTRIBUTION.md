# Attribution

Every asset bundled with NoteCraft (fonts, icons, images, sample notes, test data) has a row here. `cargo xtask assets` fails the build if a file under `assets/` or `testdata/` has no row.

Rules (from [AGENTS.md](AGENTS.md) section 1):

- Original work, or openly licensed under MIT, Apache-2.0, CC0, or OFL.
- No Microsoft, Adobe, or other third-party artwork, icons, fonts, themes, or templates.
- One row per file, or one row per directory when every file in it shares author, source, and license. Paths are relative to the repo root.

## Assets

| Path | Author | Source | License | Notes |
|---|---|---|---|---|
| `assets/app-icon/` | The NoteCraft Contributors | Original; every file is rendered from `notecraft.svg` by `packaging/icons.sh` | MIT OR Apache-2.0 | App icon (README in the folder) |

## Third-party code

Rust dependencies are tracked in `Cargo.lock`; only crates under MIT, Apache-2.0, or similarly permissive licenses are added (reviewed in each dependency PR). Code copied or adapted from another project, if any ever is, gets a row here:

| Path | Origin | License | Notes |
|---|---|---|---|
| `packaging/` | Scripts, WiX installer, Linux and macOS metadata adapted from [GridCraft](https://github.com/storytold/gridcraft) `packaging/`. Copyright (c) 2026 ArtCraft Team and the GridCraft contributors | MIT OR Apache-2.0 | Renamed for NoteCraft; new MSI GUIDs; file types, descriptions, and links rewritten; GridCraft DMG artwork not included |
| `xtask/src/` | Command layout, process helpers, and CI step summary adapted from the [GridCraft](https://github.com/storytold/gridcraft) xtask. Copyright (c) 2026 ArtCraft Team and the GridCraft contributors | MIT OR Apache-2.0 | Rewritten for NoteCraft; no code copied verbatim |
