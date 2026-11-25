#!/usr/bin/env bash
# Frame pacing of an Android app or game in Waydroid, for "blurry while it
# moves" and "feels laggy" (IMPROVEMENT-PLAN.md §15, E7). It reads Android's
# own counters over adb and the host's display settings; nothing records the
# desktop.
#
# Usage: scripts/perf/game-frames.sh <package> [seconds]
#   Open the game first, run this, then play or scroll for <seconds> (15 by
#   default) until it prints the result.
#
# Needs a running Waydroid session and adb (the same adb ArchDroid uses, so
# the device is already authorized). Waydroid's IP comes from `waydroid status`.
set -euo pipefail

pkg=${1:?usage: game-frames.sh <package> [seconds]}
secs=${2:-15}

ip=$(waydroid status 2>/dev/null | awk -F':[ \t]+' '/^IP address/ {print $2}')
[[ $ip =~ ^[0-9.]+$ ]] || { echo "Waydroid is not running (no IP in 'waydroid status')." >&2; exit 1; }
serial="$ip:5555"
adb connect "$serial" >/dev/null 2>&1 || true
a() { adb -s "$serial" shell "$@" | tr -d '\r'; }
a true || { echo "adb cannot reach $serial." >&2; exit 1; }

echo "== Render path"
node=$(grep -s '^gralloc.gbm.device=' /var/lib/waydroid/waydroid_base.prop | cut -d= -f2 || true)
if [[ -n $node ]]; then
    driver=$(basename "$(readlink -f "/sys/class/drm/$(basename "$node")/device/driver")" 2>/dev/null || echo "?")
    echo "gralloc device: $node (kernel driver: $driver)"
fi
echo "gralloc: $(a getprop ro.hardware.gralloc)  egl: $(a getprop ro.hardware.egl)  vulkan: $(a getprop ro.hardware.vulkan)"
# SwiftShader or llvmpipe here means Android draws without the GPU.
sf=$(a dumpsys SurfaceFlinger)
echo "renderer: $(grep -m1 -E '^GLES:' <<<"$sf" || echo '?')"

echo "== Display"
echo "Android: $(a wm size | tail -1), $(a wm density | tail -1)"
if command -v hyprctl >/dev/null; then
    hyprctl monitors -j | python3 -c '
import json, sys
for m in json.load(sys.stdin):
    print(f"monitor {m[\"name\"]}: {m[\"width\"]}x{m[\"height\"]}@{m[\"refreshRate\"]:.0f} Hz, scale {m[\"scale\"]}, vrr {m.get(\"vrr\")}")'
fi

echo "== Frames while you play ($secs s)"
a dumpsys SurfaceFlinger --timestats -disable -clear >/dev/null
a dumpsys gfxinfo "$pkg" reset >/dev/null
a dumpsys SurfaceFlinger --timestats -enable >/dev/null
for ((left = secs; left > 0; left--)); do printf '\r%3d s left ' "$left"; sleep 1; done
printf '\r'
stats=$(a dumpsys SurfaceFlinger --timestats -dump)
a dumpsys SurfaceFlinger --timestats -disable >/dev/null

# Each layer of the package: frames, drops and FPS, then the frame-to-frame
# times on screen as percentiles from Android's present2present histogram.
awk -v pkg="$pkg" '
function report(   n, total, i, ms, seen, p, want, out) {
    if (!inlayer) return
    printf "%s\n  frames %s, dropped %s, janky %s, average %s fps\n", layer, frames, dropped, janky, fps
    total = 0; n = 0
    for (i = 1; i <= nb; i++) total += cnt[i]
    if (total > 0) {
        split("50 90 99", want, " ")
        out = "  present-to-present:"
        for (p = 1; p <= 3; p++) {
            seen = 0
            for (i = 1; i <= nb; i++) { seen += cnt[i]; if (seen >= total * want[p] / 100) { out = out sprintf(" p%s %s ms", want[p], b[i]); break } }
        }
        print out
    }
    inlayer = 0; nb = 0; inhist = 0
}
/^layerName = / { report(); inlayer = index($0, pkg) > 0; layer = substr($0, 13); next }
!inlayer && !/^(displayRefreshRate|renderRate) = / { next }
/^displayRefreshRate = / { display = $3 }
/^renderRate = / && !shown { print "Android: display " display " fps, render " $3 " fps"; shown = 1 }
/^totalFrames = / { frames = $3 }
/^jankyFrames = / { janky = $3 }
/^droppedFrames = / { dropped = $3 }
/^averageFPS = / { fps = $3 }
/^present2present histogram is as below:/ { inhist = 1; next }
inhist && /ms=/ {
    for (i = 1; i <= NF; i++) if (split($i, kv, "ms=") == 2) { nb++; b[nb] = kv[1]; cnt[nb] = kv[2] }
    next
}
inhist { inhist = 0 }
END { report() }
' <<<"$stats" | grep . || echo "(no SurfaceFlinger layer of $pkg drew frames: is it in front?)"

# Apps drawn by Android's UI toolkit also report janky frames here; games
# that draw into their own surface usually do not.
a dumpsys gfxinfo "$pkg" | grep -E '^(Total frames rendered|Janky frames|(50|90|99)th percentile)' || true
