# Installing NoteCraft

> Stage: pre-alpha (Phase 1 on main; v0.0.2 alpha next). Last reviewed: 2026-10-15. Last updated: 2026-10-18. Change: stage line: Phase 1 merged into main (#55); earlier: created; Linux install steps and a Flatpak troubleshooting entry.

Release builds are attached to each GitHub Release at https://github.com/queball1999/notecraft/releases, with a `SHA256SUMS.txt` file. In the names below, `<version>` is the release version and `<arch>` is `x86_64` or `aarch64`. The full artifact list is in [releasing.md](releasing.md) section 3.

## Linux

| Artifact | Install | Run |
|---|---|---|
| `.deb` (Debian, Ubuntu, Mint, Pop!_OS) | `sudo apt install ./notecraft-<version>-linux-<arch>.deb` | `notecraft` |
| `.rpm` (Fedora, openSUSE, RHEL) | `sudo dnf install ./notecraft-<version>-linux-<arch>.rpm` | `notecraft` |
| AppImage (any distro) | `chmod +x notecraft-<version>-linux-<arch>.AppImage` | `./notecraft-<version>-linux-<arch>.AppImage` |
| `.tar.gz` | `tar -xzf notecraft-<version>-linux-<arch>.tar.gz` | `bin/notecraft` inside the extracted folder |
| Flatpak bundle | `flatpak install --user ./notecraft-<version>-linux-<arch>.flatpak` | `flatpak run io.github.queball1999.notecraft` |

The Flatpak install fetches the `org.freedesktop.Platform` runtime from Flathub if it is not already installed. The command-line tool is `notecraft-cli`; in the Flatpak, run it with `flatpak run --command=notecraft-cli io.github.queball1999.notecraft --version`.

### Troubleshooting: "ldconfig failed, exit status 256"

Flatpak prints this at install time when it cannot start its bubblewrap (`bwrap`) sandbox on the host. The bundle itself is not damaged: the release job installs the same bundle on a clean Ubuntu 24.04 runner, and the `.deb` built from the same binaries works. The usual causes are:

- Unprivileged user namespaces are restricted. Ubuntu 24.04 and later set `kernel.apparmor_restrict_unprivileged_userns=1`, which blocks a `flatpak` or `bwrap` that lacks its AppArmor profile (for example one not installed from the distro's package).
- The host is a container, WSL, or a VM or LXC guest without user namespace support.

To check, run `bwrap --ro-bind / / true`. If it prints an error such as "creating new namespace failed" or "Operation not permitted", Flatpak cannot run on this host as it is configured. Fixes:

1. Use the distro's own `flatpak` package, which ships the AppArmor profile for `bwrap`, instead of a build from another source.
2. Or allow the sandbox with `sudo sysctl kernel.apparmor_restrict_unprivileged_userns=0`. Caution: this lifts the restriction for every program on the machine, which weakens a hardening measure; it lasts until reboot unless you also set it in `/etc/sysctl.d/`.
3. Flatpak does not work inside WSL or in containers without user namespaces. Use the `.deb`, `.rpm`, or AppImage there.

## Windows and macOS

Windows: run the `.msi`, or unzip the portable zip. macOS: open the `.dmg` and drag NoteCraft to Applications. Builds are unsigned or ad-hoc signed unless the release says otherwise.

## Revision history

| Date | Phase | Change |
|---|---|---|
| 2026-10-15 | 1 | Created. Linux install steps per artifact and a Flatpak "ldconfig failed, exit status 256" troubleshooting entry. |
| 2026-10-18 | 1 | Stage line: Phase 1 merged into main (#55); the v0.0.2 alpha is next. |
