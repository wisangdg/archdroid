# Live GUI checks for the test VM

Scripts that drive the **real release binary** inside the disposable VM from
[`docs/P0-8.1-TEST-VM.md`](../../../docs/P0-8.1-TEST-VM.md). Results from
2026-11-01 are in [`docs/P0-8.2-MATRIX-RESULTS.md`](../../../docs/P0-8.2-MATRIX-RESULTS.md).

Run them as `tester` in the VM, with the release binary copied to
`~/archdroid` (`npx tauri build --no-bundle`, then `scp`).

## One-time setup in the VM

`WebKitWebDriver` is not in Arch's `webkit2gtk-4.1` package; `webkitgtk-6.0`
ships it. It must be the same WebKit release as `webkit2gtk-4.1`, so update
both together:

```bash
sudo pacman -Syu --needed webkitgtk-6.0 xorg-server-xvfb xdotool imagemagick python-gobject
```

## Each session

```bash
./guest-up.sh                     # Xvfb :99, headless weston, session D-Bus
. /tmp/session.env
TAURI_WEBVIEW_AUTOMATION=true WebKitWebDriver --port=4444 &
```

`TAURI_WEBVIEW_AUTOMATION=true` lets WebKitWebDriver attach to a release
build; without it Tauri does not allow automation.

| Script | What it checks |
|---|---|
| `csp_probe.py ~/archdroid ~/out` | Production CSP is enforced (eval, inline script, remote fetch/img/script are blocked with a `securitypolicyviolation`) and normal use (all tabs, Re-check, Diagnostic Bundle) produces no violation. |
| `dnd.sh <file> <name>` | Real XDND drag from a GTK3 source window (`dragsrc.py`) onto the app with `xdotool`; screenshot to `~/dnd/<name>.png`. The app must already be running. |
| `pkg_cycle.py ~/archdroid ~/dnd` | Uninstall F-Droid from the UI, then reinstall it by drag-and-drop, both starting with the container **frozen**; app count and card must follow. Needs a running Waydroid session and F-Droid installed. |
| `ui_cycle.py ~/archdroid ~/dnd` | Cancel a Houdini install after stage 2 (stepper keeps completed stages, no `/var/tmp/archdroid.*` left), then a factory reset (title and stages at completion). **Wipes Waydroid.** |
| `phantom_real.py ~/archdroid ~/dnd` | Install Phantom from the UI and launch the real `phantom-gui`. Needs Rust, a JDK and an Android SDK (`platforms;android-34`, `build-tools;34.0.0`, licence accepted by the owner); run under `bash -l` so `java` is on PATH. |
| `wldrag.sh X1 Y1 X2 Y2 <name>` | Wayland-native drag: start `weston --backend=x11-backend.so --use-pixman --socket=wayland-dnd` on `:99`, run the app and `dragsrc.py` with `GDK_BACKEND=wayland WAYLAND_DISPLAY=wayland-dnd`, then drag between the window coordinates seen in a screenshot. |
| `phantom_launch.py ~/archdroid` | Launch Keymapper with stub `phantom-gui` binaries (one exits 1, one keeps running). Tests the app's reporting, not Phantom itself. |

WebKitWebDriver allows one session at a time. If a script dies without
deleting its session, restart the driver (`pkill -x WebKitWebDriver`).
