#!/usr/bin/env bash
# Verify release artifacts against SHA256SUMS and SHA256SUMS.asc signature.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
TARGET_DIR="${1:-${REPO_ROOT}/dist-release}"
PUBKEY="${2:-${REPO_ROOT}/packaging/release/archdroid-release.asc}"

if [[ ! -d "$TARGET_DIR" ]]; then
    echo "Error: target directory $TARGET_DIR does not exist" >&2
    exit 1
fi

if [[ ! -f "$TARGET_DIR/SHA256SUMS" ]]; then
    echo "Error: SHA256SUMS not found in $TARGET_DIR" >&2
    exit 1
fi

echo "==> 1. Verifying SHA-256 Checksums"
(
    cd "$TARGET_DIR"
    sha256sum -c SHA256SUMS
)

echo "==> 2. Verifying GPG Signature"
if [[ -f "$TARGET_DIR/SHA256SUMS.asc" ]]; then
    TMP_KEYRING="$(mktemp -d)"
    trap 'rm -rf "$TMP_KEYRING"' EXIT
    GNUPGHOME="$TMP_KEYRING" gpg --batch --quiet --import "$PUBKEY"
    GNUPGHOME="$TMP_KEYRING" gpg --batch --verify "$TARGET_DIR/SHA256SUMS.asc" "$TARGET_DIR/SHA256SUMS"
    echo "GPG signature verification succeeded."
else
    echo "Warning: SHA256SUMS.asc signature not found in $TARGET_DIR" >&2
    exit 1
fi

echo "==> All artifacts in $TARGET_DIR successfully verified!"
