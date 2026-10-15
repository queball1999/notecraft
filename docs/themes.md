# Themes

> Stage: pre-alpha (Phase -1 closed; Phase 0 next). Last reviewed: 2026-10-14. Last updated: 2026-10-15. Change: new token canvas_margin_rule for the ruled-paper margin line.

NoteCraft ships four theme presets that evoke each era of OneNote, each in a light and a dark mode, for eight variants. Themes change colors, spacing, corner radius, and control style. They never change layout or behavior.

Status: the token system, theme commands, saved settings, and system mode landed in PR 0.3, and the four presets and the OS UI font in PR 0.4 (`crates/notecraft-ui-egui/src/themes/`, build_plan/02-phase-0-ui-shell.md). A test checks that the code matches the tables in section 4 and that every variant passes section 6.

## 1. Clean-room rule

All colors below are original values picked by eye to suggest each era. They are not sampled from Microsoft screenshots, installers, or style guides, and no Microsoft icons or fonts are bundled. When tuning a palette, adjust by eye against your own memory of the era; never use a color picker on Microsoft software.

Note on the original SCRD theme section: the SCRD describes the 2016 and 2019 ribbons as blue. OneNote's identity color in those versions is purple, so the presets below are purple-family. SCRD.md is updated to match.

## 2. Fonts

NoteCraft does not bundle Segoe UI. It asks the OS for its UI font at runtime:

| OS | UI font | Fallback |
|---|---|---|
| Windows 10 and 11 | Segoe UI (Microsoft 365 preset: Segoe UI Variable when present) | egui's built-in font |
| macOS | system font (San Francisco) | egui's built-in font |
| Linux | fontconfig `sans-serif` | egui's built-in font |

Decision (PR 0.4): NoteCraft bundles no UI font of its own. When the OS font is missing or cannot be parsed, it falls back to the fonts built into egui, which ship inside the egui dependency rather than as NoteCraft assets. So there is no ATTRIBUTION.md row and no `craft-fonts-ref` in the release workflows; revisit if the egui fallback looks wrong on a real Linux desktop. Every OS font file is size capped and parsed with skrifa (the parser egui uses) before egui loads it, because egui panics on font data it cannot parse. Note text defaults to the UI font at 11 pt, matching OneNote's default size.

## 3. Tokens

Every preset defines the same tokens. Widgets read tokens only; no widget hard-codes a color.

| Token | Used for |
|---|---|
| `accent` | ribbon background (2016, 2019), selected tab underline, focus rings |
| `accent_text` | text on `accent` |
| `ribbon_bg` | ribbon body |
| `ribbon_tab_bg` | tab strip |
| `ribbon_text` | ribbon labels |
| `sidebar_bg`, `sidebar_text`, `sidebar_selected` | navigation pane |
| `canvas_bg` | page background when the page has no color |
| `canvas_rule` | rule lines, grid, and dots |
| `canvas_margin_rule` | the red margin line on ruled paper (decorative, no contrast requirement) |
| `text`, `text_muted` | note text, secondary text |
| `border` | separators and control borders |
| `hover`, `pressed` | control states |
| `badge_bg`, `badge_text` | "Coming Soon" badge |
| `radius` | corner radius in px |
| `spacing` | base spacing unit in px |
| `shadow` | `none`, `subtle`, or `soft` |
| `button_style` | `flat`, `raised`, or `filled`. Ribbon buttons have no fill at rest in every preset, so they sit on the ribbon background; `raised` keeps a border at rest |

## 4. Proposed palettes

Values as tuned in PR 0.4. Change a table and the code together: a unit test fails when they differ. Record every change in the revision history.

Tuning so far: the OneNote 2016 dark accent moved from `#9A64C4` to `#915EB8`, and the OneNote 2019 dark accent from `#9461BD` to `#915FB9`, because white `accent_text` on the original values was below WCAG AA (4.21:1 and 4.46:1). Further tuning by eye waits for the ribbon (PR 0.5), the first place the accent and tab strip colors show.

### OneNote 2016

Flat purple ribbon with white text, light gray navigation, square corners, no shadows.

| Token | Light | Dark |
|---|---|---|
| `accent` | `#7B3FA6` | `#915EB8` |
| `accent_text` | `#FFFFFF` | `#FFFFFF` |
| `ribbon_bg` | `#F4F1F7` | `#2B2730` |
| `ribbon_tab_bg` | `#7B3FA6` | `#3A2F45` |
| `ribbon_text` | `#2A2A2A` | `#EDEAF0` |
| `sidebar_bg` | `#ECEAEE` | `#252228` |
| `sidebar_text` | `#2A2A2A` | `#E4E1E8` |
| `sidebar_selected` | `#D9CCE6` | `#4A3D57` |
| `canvas_bg` | `#FFFFFF` | `#1E1E1E` |
| `canvas_rule` | `#C9DDF0` | `#34404C` |
| `canvas_margin_rule` | `#D86868` | `#9A4646` |
| `text` | `#1F1F1F` | `#EDEDED` |
| `border` | `#D2CDD8` | `#403A47` |
| `radius` | 0 | 0 |
| `spacing` | 4 | 4 |
| `shadow` | none | none |
| `button_style` | flat | flat |

### OneNote 2019

Slightly deeper purple, cleaner spacing, 2 px corners.

| Token | Light | Dark |
|---|---|---|
| `accent` | `#6E3196` | `#915FB9` |
| `accent_text` | `#FFFFFF` | `#FFFFFF` |
| `ribbon_bg` | `#F7F5F9` | `#29252E` |
| `ribbon_tab_bg` | `#6E3196` | `#372C42` |
| `ribbon_text` | `#262626` | `#ECE8F0` |
| `sidebar_bg` | `#F0EEF2` | `#232026` |
| `sidebar_text` | `#262626` | `#E2DEE6` |
| `sidebar_selected` | `#DDD0EA` | `#463A54` |
| `canvas_bg` | `#FFFFFF` | `#1D1D1D` |
| `canvas_rule` | `#CADCEB` | `#333E49` |
| `canvas_margin_rule` | `#D46262` | `#984549` |
| `text` | `#1E1E1E` | `#ECECEC` |
| `border` | `#D6D1DC` | `#3E3845` |
| `radius` | 2 | 2 |
| `spacing` | 5 | 5 |
| `shadow` | none | none |
| `button_style` | flat | flat |

### OneNote 2021

Light ribbon with a purple accent only on the selected tab, muted colors, minimal shadows.

| Token | Light | Dark |
|---|---|---|
| `accent` | `#7A45A8` | `#A77FD0` |
| `accent_text` | `#FFFFFF` | `#1B1B1B` |
| `ribbon_bg` | `#FAFAFA` | `#262626` |
| `ribbon_tab_bg` | `#FAFAFA` | `#262626` |
| `ribbon_text` | `#2B2B2B` | `#E8E8E8` |
| `sidebar_bg` | `#F3F2F4` | `#202020` |
| `sidebar_text` | `#2B2B2B` | `#E0E0E0` |
| `sidebar_selected` | `#E6DDF0` | `#3D3447` |
| `canvas_bg` | `#FFFFFF` | `#1B1B1B` |
| `canvas_rule` | `#D3E1EE` | `#30383F` |
| `canvas_margin_rule` | `#DA6C6C` | `#9C4A4A` |
| `text` | `#202020` | `#EAEAEA` |
| `border` | `#E0DEE3` | `#3A3A3A` |
| `radius` | 3 | 3 |
| `spacing` | 6 | 6 |
| `shadow` | subtle | subtle |
| `button_style` | flat | flat |

### Microsoft 365

Fluent-style: soft surfaces, rounded corners, light blur feel (simulated with layered translucent fills; egui has no backdrop blur), accent on selection only.

| Token | Light | Dark |
|---|---|---|
| `accent` | `#8250B8` | `#B48EDB` |
| `accent_text` | `#FFFFFF` | `#161616` |
| `ribbon_bg` | `#F6F4F8` | `#2A2830` |
| `ribbon_tab_bg` | `#EFEBF3` | `#24222A` |
| `ribbon_text` | `#242424` | `#EEEEEE` |
| `sidebar_bg` | `#F1EEF5` | `#211F26` |
| `sidebar_text` | `#242424` | `#E6E6E6` |
| `sidebar_selected` | `#E3D8F0` | `#3F3550` |
| `canvas_bg` | `#FFFFFF` | `#1A1A1C` |
| `canvas_rule` | `#D5E2EF` | `#2F3640` |
| `canvas_margin_rule` | `#D66874` | `#9A4752` |
| `text` | `#1C1C1C` | `#EDEDED` |
| `border` | `#E2DDE8` | `#3B3842` |
| `radius` | 6 | 6 |
| `spacing` | 6 | 6 |
| `shadow` | soft | soft |
| `button_style` | filled | filled |

`text_muted`, `hover`, `pressed`, `badge_bg`, and `badge_text` are derived from the tokens above in code (mixes toward `text` or `canvas_bg`) unless a preset overrides them.

## 5. Settings

| Setting | Values | Default |
|---|---|---|
| `theme.preset` | `onenote2016`, `onenote2019`, `onenote2021`, `m365` | `m365` |
| `theme.mode` | `light`, `dark`, `system` | `system` |

Commands: `view.theme.set` (args: `preset`), `view.mode.toggle`, `view.mode.set`. The preset and mode are view state in the engine session (`notecraft-engine/src/view.rs`), so the commands need no colors; the egui crate turns them into a style. In `system` mode, `view.mode.toggle` switches to the opposite of what the OS shows. Unknown setting values fall back to the defaults.

## 6. Contrast

Every preset must pass WCAG AA (4.5:1) for `text` on `canvas_bg`, `ribbon_text` on `ribbon_bg`, `accent_text` on `accent`, and `sidebar_text` on `sidebar_bg` and `sidebar_selected`. A unit test in `notecraft-ui-egui` checks all eight variants.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Clean-room rule, fonts, tokens, proposed palettes for four presets in light and dark, contrast test. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | 0 | Token system, theme commands, settings, and system mode implemented (PR 0.3); presets still share the Microsoft 365 palette. |
| 2026-10-14 | 0 | Four presets and the OS UI font implemented (PR 0.4). OneNote 2016 and 2019 dark accents darkened to pass WCAG AA. Font decision: no bundled fallback; egui's built-in fonts are the fallback. |
| 2026-10-14 | 0 | Ribbon buttons have no fill at rest in every preset; `raised` keeps its border (fix after PR 0.5). |
| 2026-10-15 | 0 | New token `canvas_margin_rule` (red margin line on ruled paper), a soft red in light and a dim red in dark for each preset; not contrast checked because it is decorative. |
| 2026-10-15 | 0 | `canvas_margin_rule` made stronger in every preset (light and dark) so the margin line reads clearly, after maintainer review. |
