# Privacy Policy

ArchDroid keeps everything on your machine. This page lists what it stores locally and every network connection it makes.

---

## 1. No telemetry, no phone-home

- **No tracking.** ArchDroid contains no tracking, analytics, or telemetry library and no unique user identifier.
- **No data sent.** The app never sends host data, hardware details, your list of Android apps, or usage statistics to any server.
- **Network connections.** ArchDroid makes only two kinds of outgoing connections:
  1. **Connectivity probe (automatic, sends no data).** To check whether the host is online, ArchDroid opens a TCP connection to `1.1.1.1:53` (Cloudflare) and, only if that fails, to `8.8.8.8:53` (Google), with a 1-second timeout. The connection is closed immediately without sending a single byte: no DNS query, identifier, or host data. Those providers and your firewall can still see that your IP address opened a connection. The probe runs when the Setup screen or Settings → Host Diagnostics opens, when you press Re-check, when you create a Diagnostic Bundle, and before init, the GApps/Houdini installers, or the Phantom install.
  2. **Downloads you ask for**, straight from the upstream sources:
     - The official Android images (SourceForge / Waydroid OTA server), downloaded by `waydroid init`.
     - Distro packages through your package manager (`pacman`, `apt`, `dnf`, `zypper`).
     - For GApps/Houdini: the commit-pinned `waydroid_script` from GitHub, its hash-pinned Python packages (PyPI), and the GApps (MindTheGapps, GitHub) or libhoudini (GitHub) archive, which ArchDroid downloads itself and accepts only with the pinned SHA-256.
     - The Phantom v1.1.0 release tarball from GitHub, checked against its pinned SHA-256.

The app window itself cannot reach the network: its Content Security Policy only allows the app's own IPC.

---

## 2. Local data

| Data | Location | Purpose |
|---|---|---|
| **Add-on markers and installer cache** | `~/.cache/archdroid/` | Whether GApps, libhoudini, and multitouch were installed (`pending` until confirmed in a running container, then `verified`). |
| **Device profile** | `~/.config/io.github.archdroid/profile.json` | Saved display/device profile. Current release builds do not write this file; resolution and DPI are stored by Waydroid itself (`/var/lib/waydroid/waydroid.cfg`). |
| **Touch mode setting** | `~/.config/io.github.archdroid/touch-mode.json` | Only whether touch mode is on, so it comes back on at the next launch. Touch mode never records clicks or cursor positions. |
| **Application log** | `~/.local/share/io.github.wisangdg.archdroid/logs/` (the exact path is shown in Settings → Application Log) | Commands the app ran, their result and duration, and the progress/error lines of installers. Never uploaded. |
| **Diagnostic Bundle** | `~/Downloads/archdroid-diagnostics-*.txt` (or `~`), mode `0600` | Created **only** when you press Settings → Diagnostic Bundle → Create Bundle. Contains OS/kernel/Waydroid versions, host diagnostics, add-on status, and about the last 32 KiB of the application log and the Waydroid log. The application log records each command's arguments, so the bundle can contain **Android package names and APK file names** you installed, launched, or removed. Home paths and token/password values are masked; package and file names are not. Created without root and never uploaded; you decide whether to share it. |
| **Saved APK** | `~/Downloads/<package>-<time>.apks` (or `~`), mode `0600` | Created **only** when you press **Save APK** for an app. Contains that app's APK files (base and splits) as installed in Android; no app data, settings, or logins. Never uploaded. |
| **Waydroid system data** | `/var/lib/waydroid/` | Android system and vendor images and the container configuration (owned by root, managed by Waydroid). |
| **Android user data** | `~/.local/share/waydroid/` | Android internal storage, installed apps, and their data. |

Phantom stores its keymaps in its own configuration directory; see the [Phantom project](https://github.com/oliviermugishak/phantom).

---

## 3. Retention and deletion

You control all of this data. Delete it at any time by removing the directories above, or follow the [Uninstall](README.md#uninstall) steps in the README. Full list of locations: [docs/DATA-AND-UNINSTALL.md](docs/DATA-AND-UNINSTALL.md).
