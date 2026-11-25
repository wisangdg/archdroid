# UI probes

Measurements behind the performance and UI items in
`docs/archive/IMPROVEMENT-PLAN-2026-11.md` §13, and E7 in `IMPROVEMENT-PLAN.md` §15.
They drive the production bundle (`npm run build` first) in headless Chromium
through chromedriver, with a mocked Tauri bridge (`lib.mjs`).

| Script | Reports |
|---|---|
| `node scripts/perf/layout.mjs` | top bar height, clipped elements and scroll height at eight window sizes |
| `node scripts/perf/audit.mjs` | text sizes and colour pairs below WCAG AA contrast |
| `node scripts/perf/perf.mjs` | main-thread time while idle, while typing in search, and per terminal line |
| `node scripts/perf/stepper.mjs` | stepper stage after a long download, last-line visibility, dialog focus |
| `node scripts/perf/misc.mjs` | start-up commands, display profile flow, tab state, toasts |

`game-frames.sh` measures a real game instead, on the host with Waydroid
running: `scripts/perf/game-frames.sh <package> [seconds]`, then play or scroll
until it prints. It reads Android's render path (GPU or SwiftShader/llvmpipe),
its size, the monitors (Hyprland), and the game layer's frames, drops, janky
frames and p50/p90/p99 frame-to-frame time from SurfaceFlinger's `timestats`,
over adb; nothing records the desktop.

Screenshots go to `$PERF_OUT` (default: a folder in the system temp directory).

The real binary is measured in the test VM with
`scripts/test-vm/live/perf_probe.py PHASE...` (IPC timings, idle CPU, scrolling,
View Logs), started like the other live probes: WebKitWebDriver on port 4444.
