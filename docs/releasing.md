# Releasing

> Stage: pre-alpha (scaffolding). Last reviewed: 2026-10-14. Last updated: 2026-10-14. Change: app ID is now io.github.queball1999.notecraft and the repo is github.com/Queball1999/notecraft.

NoteCraft releases exactly the way DeckCraft, GridCraft, and WordCraft do: pushing to the `release` branch runs the shared release workflow in [storytold/craft-common-actions](https://github.com/storytold/craft-common-actions), which builds, signs, notarizes, packages, and drafts a GitHub Release. NoteCraft's own `.github/workflows/release.yml` is a thin caller.

## 1. Branches

| Branch | Purpose |
|---|---|
| `main` | Integration. Protected. CI must pass. |
| `release` | Protected. Every push builds a release. Only fast-forwards from `main`. |

## 2. Cutting a release

1. Pick the version (SemVer). Pre-1.0 releases are `0.x.y`; release candidates are `0.x.y-rc.n`.
2. On a `chore/release-<version>` branch off `main`:
   - `cargo xtask version set <version>` (updates every crate's version and `Cargo.lock`).
   - Add a `CHANGELOG.md` section for the version.
   - Update ROADMAP.md (stage and progress log) and the status lines of any docs the release changes.
   - Commit: `chore(release): <version>`. Open the PR into `main`, get it merged.
3. Fast-forward `release` to `main`:
   ```sh
   git fetch origin
   git checkout release
   git merge --ff-only origin/main
   git push origin release
   ```
4. Watch the Release workflow. The FreeBSD and Windows ARM64 workflows also run on the push to `release`.
5. When it is green, open the draft GitHub Release, check the artifacts and `SHA256SUMS.txt`, edit the notes, and publish.

Test run without releasing: run the Release workflow manually (workflow_dispatch) from any branch with a version override such as `0.1.0-rc.1`. Signing is skipped when secrets are absent, except on `release`, where the shared workflow requires them.

## 3. What the shared workflow builds

| Job | Output |
|---|---|
| macOS universal | signed and notarized `.dmg` |
| Windows x64 and arm64 | signed `.msi` (WiX 5) |
| Linux x86_64 and aarch64 | AppImage, `.deb`, `.rpm`, `.tar.gz` |
| Flatpak | `.flatpak` bundle |
| Linux riscv64 | `.deb`, `.rpm`, `.tar.gz` |
| FreeBSD x86_64 | `.tar.gz` |
| Web (wasm) | zip; off for NoteCraft until Phase 4 (`has-web: false` in release.yml) |
| Release | draft GitHub Release with every artifact |

## 4. The contract NoteCraft must meet

The shared workflow expects these in every craft app. Phase -1 creates them (build_plan/01-phase-minus-1-scaffolding.md, PR -1.5).

| Path or command | Used by |
|---|---|
| `cargo xtask version set <version>` | every build job, for version overrides |
| `packaging/env.sh` | shared shell settings sourced by the scripts below |
| `packaging/macos/import-cert.sh` | macOS signing keychain setup |
| `packaging/macos/package.sh --arch universal` | macOS build, sign, notarize, `.dmg` |
| `packaging/macos/verify.sh --arch universal` | optional signature and notarization check |
| `packaging/windows/package.ps1 -Arch <x64\|arm64>` | Windows build, sign, `.msi` |
| `packaging/linux/package.sh [--formats "deb rpm tar"]` | Linux packages |
| `packaging/linux/flatpak-bundle.sh <tarball>` | Flatpak |
| `packaging/linux/io.github.queball1999.notecraft.desktop` | desktop entry (also copied into the FreeBSD tarball) |
| `packaging/freebsd/package.sh` | FreeBSD tarball (optional; the workflow has a fallback) |
| `assets/app-icon/hicolor/` | icons for Linux and FreeBSD |
| Binaries `notecraft` and `notecraft-cli` | names the workflow copies and checks |
| Output in `dist/release/` | what the workflow uploads |

The scripts are adapted from GridCraft (ATTRIBUTION.md). `packaging-lint.yml` runs shellcheck, PowerShell analysis, and AppStream checks on them.

Version strings: `packaging/env.sh` and `package.ps1` export `NOTECRAFT_BUILD_SHA` and `NOTECRAFT_BUILD_DATE` themselves, so the shared workflow does not need a NoteCraft entry. `apps/notecraft/build_info.rs` (used by both binaries) reads those, then the workflow's generic `BUILD_SHA` and `BUILD_DATE`, then `git` and the clock, and embeds them; `--version` prints `NoteCraft <version> (<sha>, <date>)`.

NoteCraft-specific choices in the adapted files:

- MSI: new `UpgradeCode` (27ECD2DC-C5B0-4E04-987E-7A24F50DB30A) and component GUIDs, never shared with another app. Manufacturer is The NoteCraft Contributors. `.notecraft` opens in NoteCraft by default; `.md` is only offered under Open with.
- Linux: MIME type `application/x-notecraft` (a subclass of `text/markdown`) for `*.notecraft`; the desktop entry also lists `text/markdown`. The deb/rpm maintainer defaults to the placeholder `maintainers@notecraft.invalid`; set `NOTECRAFT_MAINTAINER` to a real address before publishing packages.
- macOS: exported type `io.github.queball1999.notecraft.page` (rank Owner) and markdown (rank Alternate). The DMG is a plain window with the app and an Applications link; GridCraft's DMG background and layout were not reused, and the script picks up `packaging/macos/dmg/background.tiff` and `dmg-layout.DS_Store` if NoteCraft ever adds its own.
- Icons: original artwork in `assets/app-icon/` (README there); `packaging/icons.sh` renders every size with resvg and packs the `.ico` and `.icns` with `cargo xtask ico` and `cargo xtask icns`, so it runs on any OS.

## 5. Secrets

Set in the repo's `release` environment, same names as the related craft apps:

| Secret | For |
|---|---|
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD` | macOS code signing |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | notarization |
| `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` | Windows signing (certificate file) |
| `AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT`, `AZURE_CERT_PROFILE` | Windows signing (Azure Trusted Signing) |

## 6. App identity

| Field | Value |
|---|---|
| App id (reverse DNS) | `io.github.queball1999.notecraft` (Flathub convention for a GitHub-hosted app; NoteCraft does not use the ArtCraft `ai.storyteller` namespace) |
| Display name | NoteCraft |
| Windows install folder | `NoteCraft` (`app-dir` in windows-arm64.yml) |
| URL scheme | `notecraft://` (registered by installers from Phase 3) |
| File associations | `.notecraft` (open); `.md` offered, not claimed by default |

## 7. After publishing

- Add a line to the ROADMAP.md progress log.
- If the release closes a milestone, update docs/roadmap.md.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-14 | -1 | Created. Release branch flow, shared workflow outputs, packaging contract, secrets, app identity. |
| 2026-10-14 | -1 | Renamed the project from QNote 2.0 to NoteCraft (`.qnote` pages are now `.notecraft`); stated that NoteCraft is related to, but independent of, the ArtCraft apps. |
| 2026-10-14 | -1 | Contract implemented; version info, MSI identity, file types, and DMG notes recorded. |
| 2026-10-14 | -1 | App ID is now io.github.queball1999.notecraft and the repo is github.com/Queball1999/notecraft. |
