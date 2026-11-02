#!/usr/bin/env bash
# Rebuild the AppImage that `tauri build --bundles appimage` produced, without
# the bundled libwayland-* libraries.
#
# Built on Ubuntu 22.04 (for a low glibc floor), the AppImage bundles
# libwayland 1.20. A host's Mesa EGL is built against its own, newer
# libwayland; with the old one first on the library path, WebKit aborts with
# "Could not create default EGL display: EGL_BAD_PARAMETER" and the window
# stays blank (seen on Arch, glibc 2.44). Every host that can show a GTK window
# has libwayland, so using the host's copy is safe.
#
# Usage: scripts/release/repack-appimage.sh   (after the tauri AppImage build)
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
bundle="$repo_root/src-tauri/target/release/bundle/appimage"
appdir="$bundle/ArchDroid.AppDir"
plugin="${LINUXDEPLOY_PLUGIN_APPIMAGE:-$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage}"

[[ -d "$appdir" ]] || { echo "error: $appdir not found; run the tauri AppImage build first" >&2; exit 1; }
[[ -x "$plugin" ]] || { echo "error: $plugin not found (tauri downloads it during the AppImage build)" >&2; exit 1; }

shopt -s nullglob
images=("$bundle"/ArchDroid_*.AppImage)
[[ ${#images[@]} -eq 1 ]] || { echo "error: expected one ArchDroid_*.AppImage in $bundle" >&2; exit 1; }
name="$(basename "${images[0]}")"

removed=("$appdir"/usr/lib/libwayland-*.so*)
rm -f "${removed[@]}"
echo "removed ${#removed[@]} bundled libwayland libraries"

rm -f "${images[0]}"
(
  cd "$bundle"
  ARCH=x86_64 LDAI_OUTPUT="$name" APPIMAGE_EXTRACT_AND_RUN=1 "$plugin" --appdir "$appdir" >/dev/null
)
[[ -f "$bundle/$name" ]] || { echo "error: repack did not produce $name" >&2; exit 1; }

# Prove the result: no libwayland inside.
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
(cd "$work" && "$bundle/$name" --appimage-extract >/dev/null)
if find "$work/squashfs-root" -name 'libwayland-*.so*' | grep -q .; then
  echo "error: $name still bundles libwayland" >&2
  exit 1
fi
echo "repacked $name without libwayland"
