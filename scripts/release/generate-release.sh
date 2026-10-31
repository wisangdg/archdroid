#!/usr/bin/env bash
# Generate release bundle staging directory with SHA256SUMS and detached GPG signature.
# Official Tier 1 artifacts are placed in the root of dist-release.
# Unverified Tier 2 preview artifacts (.deb, .rpm) are placed in dist-release/preview/.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUT_DIR="${REPO_ROOT}/dist-release"
PREVIEW_DIR="${OUT_DIR}/preview"
KEY_ID="${GPG_KEY_ID:-18F47067ACC5BBF9}"
# Stage only artifacts of the version being released: older builds left in the
# bundle directories must never be signed into a newer SHA256SUMS.
VERSION="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' "$REPO_ROOT/package.json")"
[[ -n "$VERSION" ]] || { echo "Error: could not read version from package.json" >&2; exit 1; }

mkdir -p "$OUT_DIR"
rm -rf "$OUT_DIR"/*

echo "==> Staging Tier 1 (official verified) $VERSION artifacts in $OUT_DIR"

# 1. Arch Linux package (Tier 1)
ARCH_PKG="$(ls -1t "$REPO_ROOT/packaging/arch"/archdroid-bin-"$VERSION"-[0-9]*-x86_64.pkg.tar.zst 2>/dev/null | head -n1 || true)"
if [[ -f "$ARCH_PKG" ]]; then
    cp -v "$ARCH_PKG" "$OUT_DIR/"
else
    echo "Error: Arch package for $VERSION not found, run npm run package:arch" >&2
    exit 1
fi

# 2. AppImage (Tier 1 Universal)
APPIMAGE_PKG="$REPO_ROOT/src-tauri/target/release/bundle/appimage/ArchDroid_${VERSION}_amd64.AppImage"
if [[ -f "$APPIMAGE_PKG" ]]; then
    cp -v "$APPIMAGE_PKG" "$OUT_DIR/"
else
    echo "Error: AppImage for $VERSION not found, run npm run build:bundles" >&2
    exit 1
fi

# 3. Public Key
cp -v "$REPO_ROOT/packaging/release/archdroid-release.asc" "$OUT_DIR/"

echo "==> Generating Tier 1 SHA256SUMS"
(
    cd "$OUT_DIR"
    sha256sum $(ls *.pkg.tar.zst *.AppImage 2>/dev/null) > SHA256SUMS
)

echo "==> Signing SHA256SUMS with GPG key $KEY_ID"
(
    cd "$OUT_DIR"
    gpg --batch --yes --default-key "$KEY_ID" --armor --detach-sign --output SHA256SUMS.asc SHA256SUMS
)

echo "==> Verifying Tier 1 release bundle"
(
    cd "$OUT_DIR"
    gpg --verify SHA256SUMS.asc SHA256SUMS
    sha256sum -c SHA256SUMS
)

# 4. Optional: Stage Tier 2 preview artifacts in preview/ subdirectory
mkdir -p "$PREVIEW_DIR"
DEB_PKG="$REPO_ROOT/src-tauri/target/release/bundle/deb/ArchDroid_${VERSION}_amd64.deb"
RPM_PKG="$REPO_ROOT/src-tauri/target/release/bundle/rpm/ArchDroid-${VERSION}-1.x86_64.rpm"

if [[ -f "$DEB_PKG" ]]; then
    cp -v "$DEB_PKG" "$PREVIEW_DIR/"
fi
if [[ -f "$RPM_PKG" ]]; then
    cp -v "$RPM_PKG" "$PREVIEW_DIR/"
fi

if compgen -G "$PREVIEW_DIR/*" > /dev/null; then
    (
        cd "$PREVIEW_DIR"
        sha256sum $(ls *.deb *.rpm 2>/dev/null) > SHA256SUMS
    )
    echo "==> Tier 2 preview artifacts staged in $PREVIEW_DIR"
fi

echo "==> Release staging ready at $OUT_DIR"
ls -la "$OUT_DIR"
