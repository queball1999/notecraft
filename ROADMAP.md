# NoteCraft roadmap

> Stage: pre-alpha (scaffolding). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: project renamed from QNote 2.0 to NoteCraft (related to, but independent of, the ArtCraft apps). Target: OneNote 2020 era and Microsoft 365 (canvas and ink), on a markdown-first file format that is easy to edit in any text editor.

NoteCraft is a free, open-source, clean-room note-taking app that remakes OneNote one to one (freeform canvas, ink, ribbon, notebook structure, themes), backed by a markdown-first, folder-based file format that is easy to manage and edit in any other editor. Built in pure Rust with egui.

This file is the stage summary and progress log. The detailed assessments live in `docs/`:

| File | Holds |
|---|---|
| [docs/target-app-parity.md](docs/target-app-parity.md) | Parity assessment vs OneNote (canvas, ink, themes, structure) |
| [docs/markdown.md](docs/markdown.md) | The markdown-first file format and page links |
| [docs/gaps.md](docs/gaps.md) | Every known shortfall, ranked: the work list. Pick from the top |
| [docs/roadmap.md](docs/roadmap.md) | Current focus, the alpha plan, milestones |
| [docs/parity-checklist.md](docs/parity-checklist.md) | Feature rows scored D/P/M; `cargo xtask parity` regenerates its summary |
| [docs/architecture.md](docs/architecture.md) | How NoteCraft is built (layers, data flow, quality gates) |
| [docs/craft-libs.md](docs/craft-libs.md) | Which shared craft-libs crates NoteCraft uses and how |
| [docs/printing.md](docs/printing.md) | Print and PDF strategy (craft-print, which wraps winprint) |
| [docs/themes.md](docs/themes.md) | The four OneNote theme presets |
| [docs/releasing.md](docs/releasing.md) | Release, signing, notarization, packaging |
| [docs/post-parity.md](docs/post-parity.md) | Optional features OneNote does not have; considered only after full parity |
| [docs/mcp.md](docs/mcp.md), [docs/control-protocol.md](docs/control-protocol.md), [docs/cli.md](docs/cli.md) | Automation specs (mostly post-parity) |

The execution plan for agents (phases, branches, commit messages, PRs) is in [`build_plan/`](build_plan/README.md). It covers OneNote parity only.

## By dimension (estimated, pre-alpha)

Hours are summed bottom-up from every PR in `build_plan/` (each PR carries a generous range that includes tests, docs, and review). "To 1.0" covers Phases -1 to 3 plus beta hardening. "Phase 4" is the rest of full OneNote parity.

| Dimension | Ready | To 1.0 | Phase 4 | Full parity | Doc |
|---|---:|---|---|---|---|
| Canvas and freeform layout | 0% | 90-152 | 0 | 90-152 | [parity](docs/target-app-parity.md#1-canvas-and-freeform-layout) |
| Ink and drawing | 0% | 154-262 | 140-256 | 294-518 | [parity](docs/target-app-parity.md#2-ink-and-drawing) |
| Text and markdown editing | 0% | 156-268 | 0 | 156-268 | [markdown](docs/markdown.md) |
| Notebook structure | 0% | 118-198 | 40-80 | 158-278 | [parity](docs/target-app-parity.md#4-notebook-structure) |
| Ribbon, chrome, accessibility | 0% | 126-210 | 16-28 | 142-238 | [parity](docs/target-app-parity.md#5-ribbon-and-chrome) |
| Themes (4 presets x 2 modes) | 0% | 40-68 | 0 | 40-68 | [themes](docs/themes.md) |
| Attachments and media | 0% | 52-91 | 100-180 | 152-271 | [parity](docs/target-app-parity.md#7-attachments-and-media) |
| Page links, search, note tags | 0% | 92-154 | 0 | 92-154 | [markdown](docs/markdown.md#5-page-links) |
| Print and export | 0% | 88-153 | 0 | 88-153 | [printing](docs/printing.md) |
| File formats (.md, .notecraft, .one import) | 0% | 104-176 | 160-320 | 264-496 | [gaps](docs/gaps.md#file-formats) |
| Command registry, undo/redo | 0% | 32-54 | 0 | 32-54 | [architecture](docs/architecture.md) |
| Localization | 0% | 30-50 | 80-160 | 110-210 | [gaps](docs/gaps.md#localization) |
| Performance and 1.0 hardening | 0% | 110-210 | 0 | 110-210 | [roadmap](docs/roadmap.md) |
| Browser build and sync research | 0% | 0 | 100-190 | 100-190 | [Phase 4](build_plan/06-phase-4-advanced.md) |
| Scaffolding, CI, release | 0% | 87-156 | 8-14 | 95-170 | [Phase -1](build_plan/01-phase-minus-1-scaffolding.md) |
| Total | 0% | 1,279-2,202 | 644-1,228 | 1,923-3,430 | |

Calendar, at about 30 focused hours a week: v1.0 in 43-74 weeks; full parity in 64-115 weeks.

**Overall: 0% ready.** No code yet. Every feature is "Coming Soon" until Phase 0 (UI shell) and Phase 1 (core engine) land.

## By phase

| Phase | Theme | Hours | Ends with |
|---|---|---|---|
| -1 | Scaffolding and CI/CD | 55-98 | CI green, release pipeline proven |
| 0 | UI shell | 214-360 | internal 0.0.1 |
| 1 | Core engine | 444-756 | v0.1.0 alpha |
| 2 | Page links, search, note tags | 130-222 | v0.2.0 |
| 3 | Polish and parity | 356-606 | v0.9.0 beta |
| Hardening | Beta to 1.0 | 80-160 | v1.0.0 |
| 4 | Remaining parity | 644-1,228 | full OneNote parity (1.x) |

## Alpha gate (v0.1.0)

The workflows that must work end to end for v0.1.0. Full table with per-row evidence: [docs/roadmap.md](docs/roadmap.md#alpha-gate) (mirrors [SCRD.md section 5](SCRD.md)).

## How estimates are maintained

- Each PR in `build_plan/` has an hour range; phase totals are the sum of their PRs, and the tables above are the sum of the phases. `cargo xtask parity --check` (part of `cargo xtask ci`) fails if a phase total or a By phase row drifts.
- At each phase close, replace estimates for finished work with actual hours and re-estimate the remaining phases.

## Progress log

| Date | What landed |
|---|---|
| 2026-10-13 | Docs set started to the craftrules progress-docs standard; craft-libs and winprint (via craft-print) integration decided. No code yet. |
| 2026-10-14 | Docs set completed (13 docs plus post-parity.md), `build_plan/` written with six phase files, root files and licenses added, CI/CD aligned with the related craft apps (shared craft-common-actions workflows), plain-ASCII US English rule added, scope narrowed to OneNote parity, and all hours re-assessed bottom-up. No code yet. |
| 2026-10-14 | Project renamed from QNote 2.0 to NoteCraft: crates, binaries, the `.notecraft` page format, the notebook data folder, URL scheme, env vars, app id, and repo name. Docs now state NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | Phase -1 PR -1.1: Cargo workspace with shared never-crash lints, crate skeletons for L0 to L6, notecraft and notecraft-cli apps, placeholder xtask. |
| 2026-10-14 | Phase -1 PR -1.2: cargo xtask ci runs fmt, clippy -D warnings, and test, with a step summary. |
| 2026-10-14 | Phase -1 PR -1.3: cargo xtask text, assets, and layers gates, run by cargo xtask ci. |
| 2026-10-14 | Phase -1 PR -1.4: cargo xtask wasm (L0 to L5 on wasm32) and cargo xtask parity (checklist summary and build plan hour totals); cargo xtask ci now runs all 8 gates. |
