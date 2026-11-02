#!/usr/bin/env bash
# Fail if an AppImage (or an unpacked directory) needs a newer glibc than MAX.
#
# The 0.1.3 AppImage was built on Arch and bundled libraries that require
# glibc 2.44, so it did not start on Debian 12 (2.36) or Ubuntu 24.04 (2.39).
# Only hard requirements count: Rust's std links a few newer symbols weakly
# (pidfd_spawnp, GLIBC_2.39) and falls back at run time.
#
# Usage: scripts/release/check-glibc.sh <file.AppImage|dir> [MAX=2.35]
set -euo pipefail

target="${1:?usage: check-glibc.sh <file.AppImage|dir> [max-version]}"
max="${2:-2.35}"

work=""
cleanup() { if [[ -n "$work" ]]; then rm -rf "$work"; fi; }
trap cleanup EXIT

if [[ -d "$target" ]]; then
  root="$target"
else
  work="$(mktemp -d)"
  target="$(realpath "$target")"
  (cd "$work" && "$target" --appimage-extract >/dev/null)
  root="$work/squashfs-root"
fi

worst="0"
worst_file=""
while IFS= read -r -d '' file; do
  # Undefined, non-weak symbols only (objdump flags weak references with " w ").
  versions="$(objdump -T "$file" 2>/dev/null | awk '$2 != "w" && /\*UND\*/' | grep -oE 'GLIBC_[0-9]+\.[0-9]+' | sed 's/GLIBC_//' || true)"
  for v in $versions; do
    if [[ "$(printf '%s\n%s\n' "$worst" "$v" | sort -V | tail -1)" == "$v" && "$v" != "$worst" ]]; then
      worst="$v"
      worst_file="${file#"$root"/}"
    fi
  done
done < <(find "$root" -type f \( -name '*.so*' -o -perm -u+x \) -print0)

echo "highest required glibc: $worst ($worst_file)"
if [[ "$(printf '%s\n%s\n' "$worst" "$max" | sort -V | tail -1)" != "$max" ]]; then
  echo "error: needs glibc $worst, more than the supported floor $max" >&2
  exit 1
fi
