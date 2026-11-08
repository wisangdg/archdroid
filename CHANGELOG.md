# Changelog

All notable changes to ArchDroid are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Changed
- **New "Control Center" layout.** A top bar replaces the sidebar: the four sections, the shared folder, and the engine state with Start/Restart/Stop, visible on every page. My Apps is now a list (icon, name, package, User/System tag) next to a detail panel for the selected app with Launch, Force stop, Keymapping, Copy package ID, and Uninstall, so no action hides behind a right-click any more. Double-click a row to launch, use Up/Down to move through the list, and the menu key or Shift+F10 for the context menu. Apps without an icon show their initials instead of an emoji.
- Text on green (primary) buttons is now dark, for contrast; white on the accent green was below 4.5:1.

### Added
- **Touch mode** (Keymapping page, Hyprland). A left click or drag over an Android window becomes a finger touch through a virtual touchscreen, so games and in-app web pages that ignore Waydroid's mouse input work, such as the birth-date picker in Plants vs Zombies 2. Waydroid's own `persist.waydroid.fake_touch` cannot do this: it changes only the event source, and Android WebView treats any event whose tool type is "mouse" and whose button state is set as a mouse event. A fixed upstream patch is drafted in `docs/upstream/`. Clicks outside Android windows stay normal, and the setting is remembered across launches. Packages install `/usr/lib/udev/rules.d/70-archdroid-touch.rules`, giving the logged-in user `/dev/uinput` and mice (not keyboards); other installs get an **Enable access** button.
- **Force stop** for the selected app (`adb shell am force-stop`; needs adb and a running engine). `waydroid_kill_app` is now part of the release build, with its UI and tests.

### Fixed
- **View Logs** (Settings) reads the Waydroid log without a root prompt when the file is readable (Waydroid creates it with mode `0644`); root is only requested when the user may not read it.
- Retry after a failed **Enable access** (touch mode) now asks for consent again; it did nothing.
- The Install Tools multitouch card is read on start (also with the engine stopped) and after every finished task, instead of only when the session came online.
- Settings reads the Multi-Window, Invert Colors, and display properties again when the session comes online. Before, they were read once at start, usually while the engine was stopped, and showed defaults. The two toggles are disabled while the engine is stopped, because Waydroid cannot change properties then.
- When `waydroid status` itself fails (for example the 0.1.4 AppImage's `ImportError`, or a timeout), the app now says "Waydroid Not Responding" with Waydroid's error, once per distinct error, instead of only showing "Engine Stopped". A missing or uninitialized Waydroid keeps its own setup screen.

## [0.1.5] - 2026-11-03

Fixes the 0.1.4 AppImage on current distributions, detects a missing Waydroid package on the RHEL family, attaches a signed `.rpm`, and gives the release key an expiry date. First release tested on real hardware.

### Security
- The release key now expires on 2028-11-02 (it had no expiry) and is published on keys.openpgp.org. Its fingerprint is unchanged: `EB02 90C2 138F A472 E18F 0203 18F4 7067 ACC5 BBF9`. Re-import `archdroid-release.asc` to see the new expiry.

### Distribution
- The `.rpm` is now attached to releases and covered by the signed `SHA256SUMS` (tested on Fedora 44, openSUSE Tumbleweed, and AlmaLinux 10 with EPEL).

### Tested
- First run on real hardware (Intel + NVIDIA hybrid laptop, Hyprland, Arch): the CI AppImage starts the engine, opens an Android app, and stops the engine from the UI. AlmaLinux 10: the `.rpm` installs with EPEL. Debian 12 (generic kernel): init from the setup screen and engine start.

### Fixed
- AppImage: every Waydroid command failed on hosts with a new glib (seen on Arch with glib 2.90, Intel + NVIDIA, Hyprland): the AppImage passed its own `LD_LIBRARY_PATH`, `GI_TYPELIB_PATH`, `PATH`, `XDG_DATA_DIRS`, ... to host programs, so `/usr/bin/waydroid` loaded the bundled Ubuntu 22.04 glib and stopped with `undefined symbol: g_once_init_leave_pointer`. The app showed "Engine Stopped" and could not start, stop, or read Waydroid. Host programs are now started without the AppImage's paths; a test keeps every new host command on that path. The `.pkg.tar.zst` and `.deb` were not affected.
- On dnf hosts the setup screen now detects that no repository offers `waydroid` (AlmaLinux 10, RHEL, and Rocky have it in neither the base nor the EPEL repositories): the item reads "Not in Repositories" and Initialize is blocked, instead of suggesting `sudo dnf install -y waydroid`. Fedora is unchanged. The query is `dnf --cacheonly repoquery`, which tells an unknown package from a missing cache.
- Initialize reported "Waydroid initialized successfully" when `waydroid init` had done nothing: Fedora's waydroid 1.6.3 has no default OTA channels, logs "You must provide 'System OTA' and 'Vendor OTA' URLs", and exits 0. Initialize now passes the official channels (`https://ota.waydro.id/system`, `/vendor`, the images the consent dialog names) unless the distribution set its own in `channels.cfg`, and reports success only when Waydroid wrote its configuration.
- The setup screen gives the image size as ~1.2 GB, like the Initialize dialog (it said ~1 GB).

## [0.1.4] - 2026-11-03

Hardening release: pinned add-on archives, failure logging, repository checks before root, and packages that install on Debian 12 and openSUSE. The AppImage is now built in CI on Ubuntu 22.04.

### Security
- ArchDroid now downloads the GApps (MindTheGapps) and libhoudini archives itself and uses them only if their SHA-256 matches the pin in the code (the "Verified Download" stage in the stepper). Before, only `waydroid_script` checked them, with MD5. A mismatching file is deleted and the install stops before the image is changed.

### Distribution
- Release AppImages are now built by a CI workflow on Ubuntu 22.04 (glibc 2.35), without the bundled libwayland, and `generate-release.sh` rejects an AppImage that needs a newer glibc (`scripts/release/check-glibc.sh`). The 0.1.3 AppImage was built on Arch and needs glibc 2.44, so it does not start on Debian 12 (seen: `GLIBC_2.38 not found`) or Ubuntu 24.04.
- The `.rpm` no longer requires `waydroid` (it is now a Recommends): openSUSE has no official waydroid package, and `zypper install` failed with "nothing provides 'waydroid'". dnf still installs it on Fedora.
- The setup screen and `waydroid_init` now ask the package manager, without root, whether any repository offers `waydroid`. If none does (Debian/Ubuntu without the Waydroid repository, openSUSE), the Waydroid item reads "Not in Repositories" with where to get it, and Initialize is blocked instead of failing after the root prompt.

### Observability
- The application log now records every installer's `[STEP]`/`[ERROR]`/`[SUCCESS]` lines, the last 40 output lines of a failed task, and every error shown in the UI (for example the Phantom preflight), without repeating an identical message. A Diagnostic Bundle therefore contains why an install failed.
- The `waydroid status` poll (every 5 s) is no longer written to the log (debug level), so it no longer pushes everything else out of the part of the log a Diagnostic Bundle includes.

### Changed
- The Keymapping page no longer promises "Zero Ban Risk"; it now says "No APK Changes" and notes that games with strict anti-cheat may still detect it.

### Fixed
- The uninstall guide uses the real `.deb`/`.rpm` package name (`arch-droid`, not `archdroid`), and the install examples use the real file names (`ArchDroid_*.deb`, `ArchDroid-*.rpm`); the old commands failed.
- Start Engine now reports "Start Failed" with the reason Waydroid gave (for example `OSError: container failed to start`) when the container fails to start. Before, `waydroid session start` exited 0 after about 10 s and ArchDroid only showed "Session Slow to Start". A successful start is reported as soon as the status says RUNNING.

### Documentation
- README, PRIVACY, VERIFY-RELEASE, RELEASE-AND-UPDATES, and this changelog are now in English. Claims that did not match the code were corrected: release builds do not write `profile.json`; setup has no image-type choice; the PKGBUILD needs a local build first; and, in the 0.1.0 entry below, the setup wizard, the preset names, and the list of host checks.

## [0.1.3] - 2026-11-01

Fix release after the live Tier 2 test on Ubuntu 24.04. The `.deb` is now attached and signed.

### Fixed
- The `.deb` (Tier 2 preview) could not be installed on Ubuntu 24.04 because it required `waydroid`, which is not in Ubuntu's official repositories. `waydroid` is now a Recommends; the ArchDroid setup screen already handles a host without Waydroid.
- On `apt` hosts, the "Waydroid Package" diagnostic now points to the official Waydroid repository when `apt` cannot find `waydroid` (for example Ubuntu 24.04).
- The app list now refreshes by itself while the session runs. Before, on the first boot after init (while Waydroid was still writing launcher entries) it stayed at "0 applications" until another action.
- The "Session Slow to Start" warning now points to the Diagnostic Bundle, which contains the Waydroid log. `waydroid session start` can exit 0 even when the container failed to start.

## [0.1.2] - 2026-11-01

Fix release after live tests of the runtime CSP and native drag-and-drop in the VM.

### Added
- `archdroid --version` (or `-V`) prints the version without opening a window, as the upgrade guide in `docs/RELEASE-AND-UPDATES.md` describes.
- Diagnostic Bundle (Settings → Diagnostic Bundle): one text file for bug reports with OS/kernel/Waydroid versions, host diagnostics, add-on status, and recent logs. Collected without root, redacted, saved with mode `0600` in `~/Downloads`, and never uploaded.

### Fixed
- Long log lines in Settings (Application Log, Diagnostic Bundle) now wrap instead of widening the card.
- `PRIVACY.md` names the real locations of preferences and the application log.
- `PRIVACY.md` now discloses the automatic connectivity probe (a TCP connection without data to `1.1.1.1:53`, then `8.8.8.8:53` if that fails), which has existed since 0.1.0. Before, it said every network connection happened only at the user's request.
- The Diagnostic Bundle panel, README, and `PRIVACY.md` say that the logs in the bundle can contain Android package names and APK file names.
- After Cancel, the stepper keeps the completed stages marked and stops spinning instead of going back to stage 1.
- The wipe terminal title no longer says "in Progress..." after it finished.
- The WebView favicon uses the ArchDroid icon; it pointed to a missing `/vite.svg`.
- Installing or uninstalling an APK while the Waydroid container is frozen now updates the app list right away. Before, the container was frozen again right after `adb install`, before Waydroid wrote the launcher entry, so an installed app did not appear until the container woke up.
- "Launch Keymapper" now reports an error when `phantom-gui` (or the terminal fallback) exits right after starting, instead of showing "Keymapper Launched". A process that keeps running is reaped so it does not become a zombie.
- The message after Cancel no longer says "No modifications were completed"; completed stages stay applied, and the interrupted stage may be incomplete.

## [0.1.1] - 2026-11-01

First published release. The 0.1.0 artifacts were never released: the live VM smoke test found a cleanup bug on Cancel/timeout of root tasks.

### Fixed
- Cancel or timeout of a root task (GApps, Houdini, multitouch, init, wipe) now really removes the private work directory `/var/tmp/archdroid.*`. Before, `set -e` stopped the trap before `rm -rf`, leaving root-owned downloads and venvs behind.
- A cancelled root task now stops where it is; the script no longer goes on to the next root command after the signal.
- Installer progress bars (tqdm/pip) no longer show garbage (`â▯▯`): output is now decoded as UTF-8 per line, not per byte.
- The terminal modal no longer overflows the 1100×720 window, so the Cancel button in the header and the footer buttons stay visible.

## [0.1.0] - 2026-11-01

### Added
- **App Library & Game Launcher**:
  - Visual management of the Android apps installed in the Waydroid container.
  - Automatic app icon extraction, instant search (`Ctrl+F`), and category filters (All/User/System).
  - Right-click context menu (Launch, Stop, Uninstall, App Info).
- **Drag-and-Drop APK Installer**:
  - Install an APK by dragging the `.apk` file onto the app window.
- **Container Initialization & Setup Screen**:
  - Automatic detection of Waydroid and the Android image.
  - A setup screen with host preflight checks; Initialize runs `waydroid init` after an explicit consent dialog and shows each step with a live log. *(Corrected: an earlier version of this entry described a VANILLA/GAPPS image choice, which the app does not have.)*
- **System Add-on Injection (GApps & ARM Translation)**:
  - Google Mobile Services injection (MindTheGapps / OpenGApps).
  - Intel libhoudini ARM translation layer, to run 3D Android games on x86_64 CPUs.
  - Multitouch hardware declaration for Google Play Store compatibility.
- **Security & Privilege Boundaries**:
  - An explicit consent dialog and a review of the distro packages (package approval gating) before any root escalation.
  - Refusal to run when a distro package outside the approved list is requested.
  - Root scripts run in a private directory with mode `0700` (`mktemp -d`).
  - Sensitive environment cleanup (`SUDO_USER`) and the root cache redirected to `$ARCHDROID_WORK/cache` so it never touches the user's directories.
  - SHA-256 hash checks on every Python dependency of the installer (`--require-hashes`).
  - A strict production Content Security Policy (`default-src 'self'`) with no remote fonts, CDNs, or inline script.
  - Tauri v2 IPC capabilities that block arbitrary shell/fs/http plugin access.
- **Display Profiles & Custom DPI**:
  - Quick resolution presets (Tablet Full HD, Standard 720p, Mobile Phone, Desktop 2K / QHD) and a custom resolution. *(Corrected preset names.)*
  - Custom DPI, verified by reading the Android property back.
- **Smart Keymapping (Phantom Integration)**:
  - Integration with the Phantom virtual-touchscreen keymapper, running unprivileged.
- **Host Diagnostics**:
  - 17 live host checks (KVM, binder, `/dev/dri`, Wayland, polkit/sudo, tools, disk, network, Android image). *(Corrected: there is no memory check.)*
- **Multi-Distro Packaging & Verification**:
  - Official Tier 1 target for Arch Linux: `packaging/arch/PKGBUILD` and the binary package `archdroid-bin-*.pkg.tar.zst`.
  - A standalone AppImage, with the AppRun's own `PYTHONHOME`/`PYTHONPATH` cleared for host tools.
  - Experimental Tier 2 targets (preview `.deb` and `.rpm`).
  - Official release signing with an Ed25519 GPG key (`18F47067ACC5BBF9`) and the automated `verify-release.sh`.
- **Reliable Operations**:
  - Every long task has a `task_id`, a deadline, and a Cancel button (including APK install/remove); a second task is refused with code `busy`.
  - APK install and uninstall are verified through `adb` (fallback `waydroid app`), so "success" means the package really was installed or removed.
  - Property and resolution changes are read back from Android; the success toast only appears if the value really changed.
  - The device profile is versioned and saved atomically, with migration for old files.
- **First Run & Log**:
  - A staged setup screen (Waydroid not installed, not initialized, initializing, ready).
  - An application log panel with the log file location and a clear button; root script contents are never written to the log and error messages are redacted.
- **Accessibility**:
  - Full keyboard navigation, focus traps in modals, and ARIA semantics for app cards, menus, and dialogs.
- **Documentation & Community Standards**:
  - `README.md` with installation, the root privilege model, troubleshooting, and clean uninstall.
  - `SECURITY.md` with a private vulnerability reporting process.
  - `PRIVACY.md` committing to zero telemetry, zero analytics, and zero phone-home.
  - `NOTICES.md` with third-party licenses and attribution.
