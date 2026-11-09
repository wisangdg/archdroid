#!/usr/bin/env bash
# Put the AppImage tools the Tauri bundler downloads from
# github.com/tauri-apps/binary-releases into its cache (~/.cache/tauri), so
# `tauri build --bundles appimage` does not fetch them itself.
#
# Why: the bundler makes one attempt and checks nothing. On 2026-11-09 the
# release download URLs answered 504 for hours (AppRun, then linuxdeploy) while
# the same assets downloaded fine through the GitHub API, which is used here
# with retries. Each file is checked against the SHA-256 of the copy every
# release so far was built with.
#
# linuxdeploy is stored the way the bundler leaves it: with the AppImage magic
# bytes (offsets 8-10, "AI\x02") zeroed, so it runs without FUSE.
set -euo pipefail

cache="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
mkdir -p "$cache"
auth=()
[[ -n "${GITHUB_TOKEN:-}" ]] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")

# fetch <asset id> <file> <sha256 after patching> <patch magic: yes|no>
fetch() {
    local id="$1" file="$2" sha="$3" patch="$4" tmp
    tmp="$(mktemp "$cache/.$file.XXXXXX")"
    curl -fsSL --retry 6 --retry-all-errors --retry-delay 5 "${auth[@]}" \
        -H "Accept: application/octet-stream" \
        -o "$tmp" "https://api.github.com/repos/tauri-apps/binary-releases/releases/assets/$id"
    if [[ "$patch" == yes ]]; then
        printf '\0\0\0' | dd of="$tmp" bs=1 seek=8 count=3 conv=notrunc status=none
    fi
    if ! echo "$sha  $tmp" | sha256sum -c --status -; then
        echo "error: $file does not match the pinned SHA-256 $sha" >&2
        rm -f "$tmp"
        exit 1
    fi
    chmod 770 "$tmp"
    mv "$tmp" "$cache/$file"
    echo "$file ok"
}

fetch 274691722 AppRun-x86_64 \
    f30140a43a0a59e46db21bdefdf749b9e9f2c6946e92afabbacf98b8ae73fb4f no
fetch 182515537 linuxdeploy-x86_64.AppImage \
    20eebde3c18ae2e44279bd624fc72482503aece216d5d77f10932235342f71c1 yes
