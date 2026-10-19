# Releasing

> Stage: pre-alpha (Phase 1 on main; v0.0.2 alpha next). Last reviewed: 2026-10-15. Last updated: 2026-10-18. Change: stage line: Phase 1 merged into main (#55); earlier: release step for WHATS_NEW.md; earlier: linked docs/install.md for per-artifact install steps.

Pushing to the `release` branch runs `.github/workflows/release.yml`, which builds, signs (when secrets exist), notarizes, packages, and drafts a GitHub Release. The release flow and packaging scripts follow the related craft apps, but the workflow itself is NoteCraft's own: the shared release workflow in craft-common-actions builds macOS on a paid `macos-15-xlarge` runner, and it has no license, so NoteCraft cannot copy it. NoteCraft's workflow uses only GitHub's standard hosted runners, which are free for public repositories.

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
   - Add a `## <version>` section at the top of `WHATS_NEW.md` (repo root; the instructions are at the top of that file). It is compiled into the app and shown in the What's New dialog on the first launch after an update. Write short, plain, user-facing lines. A unit test fails if Cargo.toml has a version with no section.
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

Test run without releasing: run the Release workflow manually (workflow_dispatch) from any branch with a version override such as `0.1.0-rc.1`. Signing is skipped when secrets are absent: Windows builds are unsigned and macOS builds are ad-hoc signed and not notarized. Delete the draft release a test run creates.

## 3. What the release workflow builds

| Job | Runner (all free for public repos) | Output |
|---|---|---|
| macOS universal | `macos-15` | signed and notarized `.dmg` and CLI zip (ad-hoc signed without secrets) |
| Windows x64 and arm64 | `windows-latest` | `.msi` (WiX 5) and portable zip; signed when secrets exist |
| Linux x86_64 and aarch64 | `ubuntu-22.04`, `ubuntu-22.04-arm` | AppImage, `.deb`, `.rpm`, `.tar.gz` |
| Flatpak x86_64 and aarch64 | `ubuntu-24.04`, `ubuntu-24.04-arm` | `.flatpak` bundle |
| FreeBSD x86_64 | `ubuntu-24.04` with a FreeBSD VM | `.tar.gz` |
| Release | `ubuntu-24.04` | draft GitHub Release with every artifact and `SHA256SUMS.txt` |

Install steps per artifact are in [install.md](install.md).

Not built yet: Linux riscv64 (needs a cross toolchain; add if users ask) and the web build (Phase 4).

## 4. What the workflow calls

The same layout the related craft apps use, so their scripts and fixes can be compared side by side. Phase -1 created these (build_plan/01-phase-minus-1-scaffolding.md, PR -1.5).

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

Version strings: `packaging/env.sh` and `package.ps1` export `NOTECRAFT_BUILD_SHA` and `NOTECRAFT_BUILD_DATE` themselves, and the release workflow sets them too. `apps/notecraft/build_info.rs` (used by both binaries) reads those, then generic `BUILD_SHA` and `BUILD_DATE` if a CI system sets them, then `git` and the clock, and embeds them; `--version` prints `NoteCraft <version> (<sha>, <date>)`.

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
| 2026-10-14 | -1 | Releases run NoteCraft's own workflow on free standard runners. |
| 2026-10-15 | 1 | Linked install.md from section 3; the Flatpak smoke test also runs the desktop binary. |
| 2026-10-18 | 1 | Section 2 gains a release step: write the version section in `WHATS_NEW.md`. |
| 2026-10-18 | 1 | Stage line: Phase 1 merged into main (#55); the v0.0.2 alpha is next. |
