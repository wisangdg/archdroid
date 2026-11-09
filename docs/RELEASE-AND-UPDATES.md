# Updates, distribution, and the release key

## Decision: manual updates, no in-app updater
ArchDroid has **no** Tauri updater and never downloads or runs its own artifacts. Reason: the app already asks for root; an updater would add a remote code execution path, and the CSP (`connect-src ipc:`) deliberately blocks every network connection from the WebView. So **no update ever runs an unsigned artifact**: you always install the package yourself.

### Upgrading
1. Verify the download as in [VERIFY-RELEASE.md](VERIFY-RELEASE.md)
   (`gpg --verify SHA256SUMS.asc SHA256SUMS`, `sha256sum -c --ignore-missing SHA256SUMS`).
2. Install it:
   - Arch: `sudo pacman -U archdroid-bin-<version>-1-x86_64.pkg.tar.zst`
   - Debian/Ubuntu: `sudo apt install ./ArchDroid_<version>_amd64.deb` (Tier 2; signed in `SHA256SUMS` since 0.1.3)
   - AppImage: replace the old file.
   - Fedora/RHEL family/openSUSE: `sudo dnf install ./ArchDroid-<version>-1.x86_64.rpm` or `sudo zypper install ./ArchDroid-<version>-1.x86_64.rpm` (Tier 2; signed in `SHA256SUMS` since 0.1.5)
3. The top bar shows the installed version (`v<version>` next to the ArchDroid name); compare it with <https://github.com/wisangdg/archdroid/releases>.
4. Every release has release notes: changes, changes to privileged or security behaviour, and migration steps if any.

### Rolling back
Install the previous verified package the same way; the package manager treats it as a downgrade:
```bash
sudo pacman -U archdroid-bin-<previous>-1-x86_64.pkg.tar.zst   # pacman warns "downgrading package"
sudo apt install --allow-downgrades ./ArchDroid_<previous>_amd64.deb
```
State that survives a rollback:
- **Add-on markers** (`~/.cache/archdroid/*_installed`): every release (since 0.1.0) reads `pending` and `verified`; markers from pre-release builds (`installed`) count as `pending`.
- **Waydroid's own data** (`/var/lib/waydroid`, `~/.local/share/waydroid`) is not touched by installing or removing ArchDroid.
- **`profile.json`**: a newer file stays readable (unknown fields are ignored). Release builds do not write it.

The rollback path was tested live (0.1.3 → 0.1.2 → 0.1.3 on Arch with GApps and libhoudini installed); see [P0-8.2-MATRIX-RESULTS.md](P0-8.2-MATRIX-RESULTS.md).

## Building a release
1. Tag the commit and let CI pass.
2. `.deb`, `.rpm`, and `.pkg.tar.zst`: download the `packages-<sha>` and `arch-<sha>` artifacts of the same workflow run (`gh run download <run> -n packages-<sha> -D src-tauri/target/release/bundle`, `gh run download <run> -n arch-<sha> -D packaging/arch`) and check `SHA256SUMS.ci`, `SHA256SUMS.arch`, and `BUILD-INFO.txt`. They come from the same Ubuntu 22.04 binary as the AppImage; the Arch package repackages that `.deb` in an `archlinux:base-devel` container. A local build (`npm run build:bundles -- --bundles deb,rpm`, `npm run package:arch`) still works; `generate-release.sh` holds it to the same glibc floor.
3. AppImage: download the [AppImage workflow](../.github/workflows/appimage.yml) artifact of the tagged commit (`gh run download <run> -n appimage-<sha> -D src-tauri/target/release/bundle/appimage`), check `SHA256SUMS.appimage` and that `BUILD-INFO.txt` names the tagged commit. It is built on Ubuntu 22.04 (glibc 2.35) without the bundled libwayland; a local Arch build needs glibc 2.44 and is rejected.
4. `scripts/release/generate-release.sh` copies the artifacts, refuses an AppImage, `.deb`, or `.rpm` whose binaries need glibc above 2.35 (`check-glibc.sh`; the packages are unpacked with `bsdtar`), and signs `SHA256SUMS`.
5. AUR (`archdroid-bin`): after publishing, set `pkgver` and the `.deb` checksum in `packaging/aur/PKGBUILD` (from the signed `SHA256SUMS`), run `makepkg -f` (it verifies `SHA256SUMS.asc` with the release key) and `makepkg --printsrcinfo > .SRCINFO`, commit both, then `scripts/release/publish-aur.sh` pushes them to the AUR (needs the maintainer's SSH key on the AUR account).

## Release GPG key
| | |
|---|---|
| Key ID | `18F47067ACC5BBF9` (ed25519, sign/certify) |
| Fingerprint | `EB02 90C2 138F A472 E18F 0203 18F4 7067 ACC5 BBF9` |
| Created | 2026-10-31 (epoch 1793447911) |
| Expiry | 2028-11-02 (set 2026-11-03; extend with `gpg --quick-set-expire <fpr> 2y` and republish before then) |
| Public key | `packaging/release/archdroid-release.asc` |
| Private key | only on the owner's machine/offline; **never in the repo** (guarded by `src/lib/release-hygiene.test.js`) |

### Owner actions (cannot be done from the repo)
- [x] Set a 2-year expiry (done 2026-11-03: expires 2028-11-02; `.asc` republished in the repo and from 0.1.5 in the release).
- [ ] Keep an encrypted backup of the private key and its revocation certificate (gpg created one in `~/.gnupg/openpgp-revocs.d/`) on separate offline media, and note the location here: `Backup location: ______`.
- [ ] Publish the fingerprint on a second channel besides the repo. A fingerprint that exists only in the repo does not help if the repo is compromised.
      Done: the fingerprint is in the release notes since
      [v0.1.1](https://github.com/wisangdg/archdroid/releases/tag/v0.1.1), and the key was uploaded to
      [keys.openpgp.org](https://keys.openpgp.org/search?q=EB0290C2138FA472E18F020318F47067ACC5BBF9) on 2026-11-03.
      The e-mail address was verified, so `gpg --recv-keys` imports the key with its user ID (checked 2026-11-03: the 0.1.5 release verifies with the keyserver copy alone). Open: the GitHub profile.

### Rotation
1. Create a new key and sign the rotation announcement with the old key.
2. Publish the new fingerprint on every channel; replace the `.asc`, `VERIFY-RELEASE.md`, and the default `GPG_KEY_ID` in `scripts/release/*.sh`.
3. Sign the next release with the new key; let the old key expire.

### Revocation (key leaked or lost)
1. `gpg --import <revocation certificate>`, upload it to the keyserver, and replace the `.asc` in the repo with the revoked version.
2. Announce it in Releases and on the second channel; mark every release since the leak as untrusted and re-issue it with a new key.
