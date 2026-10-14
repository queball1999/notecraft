## What this changes

<!-- One or two sentences. One logical change per PR. -->

## Related

- Issue: #
- Build plan: <!-- for example build_plan/02-phase-0-ui-shell.md, PR 0.3 -->

## Checklist

- [ ] `cargo xtask ci` passes locally (fmt, clippy, tests, text, assets, layers, wasm)
- [ ] No `unwrap`, `expect`, `panic!`, or `unsafe` outside tests
- [ ] New user actions are commands with stable ids
- [ ] No layering violations (nothing below L6 uses egui, eframe, winit, or rfd)
- [ ] Every new bundled asset has an ATTRIBUTION.md row
- [ ] Text is plain ASCII, US English (AGENTS.md section 2)
- [ ] Progress docs updated: checklist row, gaps.md, ROADMAP.md log, status lines

## Feature status

- [ ] Works end to end
- [ ] Partly works; the rest shows a "Coming Soon" badge
- [ ] Not user-visible

## Screenshots

<!-- Required for any ribbon, sidebar, canvas, or theme change. Light and dark. -->

## Testing performed

<!-- What you ran and on which OS. -->
