#!/usr/bin/env bash
# Push packaging/aur (PKGBUILD + .SRCINFO) to the AUR package archdroid-bin.
#
# Needs an AUR account with your SSH public key added
# (https://aur.archlinux.org -> My Account -> SSH Public Key). The first push
# creates the package. Run after the release is published and
# `makepkg -f` in packaging/aur passed (it verifies the signed SHA256SUMS).
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
src="$repo_root/packaging/aur"
pkgver="$(sed -n 's/^pkgver=//p' "$src/PKGBUILD")"
pkgrel="$(sed -n 's/^pkgrel=//p' "$src/PKGBUILD")"

# .SRCINFO must describe this PKGBUILD exactly; the AUR rejects a mismatch.
diff -q <(cd "$src" && makepkg --printsrcinfo) "$src/.SRCINFO" >/dev/null \
    || { echo "error: .SRCINFO is stale; run makepkg --printsrcinfo > .SRCINFO" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone -q ssh://aur@aur.archlinux.org/archdroid-bin.git "$work/aur"
cp "$src/PKGBUILD" "$src/.SRCINFO" "$work/aur/"
cd "$work/aur"
git add PKGBUILD .SRCINFO
if git diff --cached --quiet; then
    echo "AUR already has archdroid-bin $pkgver-$pkgrel"
    exit 0
fi
git commit -q -m "Update to $pkgver-$pkgrel"
git push -q origin HEAD:master
echo "Pushed archdroid-bin $pkgver-$pkgrel: https://aur.archlinux.org/packages/archdroid-bin"
