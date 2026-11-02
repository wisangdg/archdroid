# Verifying ArchDroid release artifacts

Every official ArchDroid release ships a `SHA256SUMS` checksum file and a detached GPG signature, `SHA256SUMS.asc`. Since 0.1.3, `SHA256SUMS` covers the `.pkg.tar.zst`, the `.AppImage`, and the `.deb`.

## Release signing key
- **Owner:** `ArchDroid Release Signing Key <gwisang45@gmail.com>`
- **Key ID:** `18F47067ACC5BBF9`
- **Fingerprint:** `EB02 90C2 138F A472 E18F  0203 18F4 7067 ACC5 BBF9`
- **Key file:** [`packaging/release/archdroid-release.asc`](../packaging/release/archdroid-release.asc) (also attached to every release as `archdroid-release.asc`)

Compare the fingerprint with a second source you trust (for example the release notes) before relying on it.

---

## Manual verification

### 1. Import the public key
```bash
gpg --import archdroid-release.asc
```

### 2. Verify the checksum signature
Put `SHA256SUMS` and `SHA256SUMS.asc` in the same directory as the files you downloaded:
```bash
gpg --verify SHA256SUMS.asc SHA256SUMS
```
A valid signature prints:
```text
gpg: Good signature from "ArchDroid Release Signing Key <gwisang45@gmail.com>"
Primary key fingerprint: EB02 90C2 138F A472 E18F  0203 18F4 7067 ACC5 BBF9
```
gpg may also warn that the key is not certified with a trusted signature; that only means you have not signed the key yourself. The fingerprint check is what matters.

### 3. Verify the files
```bash
sha256sum -c --ignore-missing SHA256SUMS
```
Every file you downloaded must say `OK`, for example:
```text
archdroid-bin-0.1.3-1-x86_64.pkg.tar.zst: OK
ArchDroid_0.1.3_amd64.AppImage: OK
ArchDroid_0.1.3_amd64.deb: OK
```
`--ignore-missing` skips the artifacts you did not download. If a file you did download is reported as `FAILED`, or not listed at all, do not install it.

---

## Automated verification
The repository has a script that verifies with an isolated keyring:
```bash
./scripts/release/verify-release.sh [path/to/downloaded-files]
```
or through npm:
```bash
npm run release:verify
```
