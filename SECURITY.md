# Security Policy

ArchDroid relies on defense in depth and least privilege on the Linux host.

---

## 1. Privilege model and boundaries

### Desktop process and WebView (unprivileged)
- **Never runs as root.** The ArchDroid desktop app runs entirely as the normal desktop user. It must not and does not run as `root` (`sudo archdroid`).
- **Tauri IPC and capability scoping.** The Tauri v2 WebView is tightly isolated. No free shell plugin, arbitrary filesystem plugin, or HTTP plugin is exposed to the frontend. Only registered, validated IPC commands can be called by the interface.
- **Content Security Policy (CSP).** The production CSP blocks remote scripts, external iframes, and inline string injection (`default-src 'self'`). Fonts and graphics load entirely from the app bundle (no Google Fonts or third-party CDN).

### Privileged operations (root elevation)
Administrative operations (installing distro packages, initializing `/var/lib/waydroid`, injecting GApps/libhoudini) run under these boundaries:
1. **Explicit consent and package approval.**
   Before Polkit (`pkexec`) or `sudo` authentication starts, a dialog shows the exact command and the list of packages to be installed. The backend re-validates the approved list and refuses to run if any package outside it is requested.
2. **Private temporary directory.**
   Root scripts run inside a private temporary directory with mode `0700` (`mktemp -d`).
3. **Environment sanitizing.**
   Sensitive variables such as `SUDO_USER` and custom environment are cleared. The root cache is redirected to the private work directory (`$ARCHDROID_WORK/cache`) so it never changes the ownership of files in the user's home (`~/.cache`).
4. **Hash-verified third-party dependencies.**
   Supporting automation scripts (such as `waydroid_script`) lock every Python dependency by SHA-256 (`--require-hashes --only-binary=:all:`). The GApps and libhoudini archives are used only if their SHA-256 matches the pin in the code.

### Touch mode (input device access)
- **What is granted:** the udev rule `70-archdroid-touch.rules` gives the logged-in user (tag `uaccess`, the same pattern as Steam's `60-steam-input.rules`) access to `/dev/uinput` and to the `event*` nodes of **mice**. Devices tagged as keyboards (`ID_INPUT_KEYBOARD=1`) are deliberately excluded, so touch mode can never read keystrokes.
- **Consequence:** any process of the logged-in user can create virtual input devices (keyboards included) and read mouse movement. That is the same access Steam Input grants. Delete the rule file and run `udevadm control --reload-rules` to revoke it.
- **While active:** ArchDroid grabs the physical mice and forwards them through a virtual mouse. If ArchDroid stops or crashes, its file descriptors close and the kernel releases the grab, so the mouse works normally again.
- **No root at run time:** touch mode does not use root. Root is used once, by the "Enable access" button, on installs that did not ship the rule.

---

## 2. Supported versions

Security fixes are provided only for the active version on the `main` branch of the official repository:

| Version | Supported |
|---|---|
| `0.1.x` (latest) | :white_check_mark: |
| `< 0.1.0` | :x: |

---

## 3. Reporting a vulnerability

If you find a vulnerability in ArchDroid:
1. **Do not open a public GitHub issue.**
2. Report it privately through [GitHub Security Advisories](https://github.com/wisangdg/archdroid/security/advisories/new) in the official repository, or by email:
   - **Email:** `gwisang45@gmail.com`
   - **Subject:** `[SECURITY] ArchDroid vulnerability report`
3. Please include:
   - Reproduction steps or a proof of concept.
   - The affected ArchDroid version and Linux distribution.
   - Your analysis of the security impact.

Reports are acknowledged and verified within 3 working days, before a fix is released in a coordinated way.
