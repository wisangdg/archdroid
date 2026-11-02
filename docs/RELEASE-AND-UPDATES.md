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
   - The `.rpm` is a preview and is not attached to releases; build it from source.
3. The sidebar shows the installed version (`ArchDroid v<version>`); compare it with <https://github.com/wisangdg/archdroid/releases>.
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

## Release GPG key
| | |
|---|---|
| Key ID | `18F47067ACC5BBF9` (ed25519, sign/certify) |
| Fingerprint | `EB02 90C2 138F A472 E18F 0203 18F4 7067 ACC5 BBF9` |
| Created | 2026-10-31 (epoch 1793447911) |
| Expiry | **not set (does not expire)** |
| Public key | `packaging/release/archdroid-release.asc` |
| Private key | only on the owner's machine/offline; **never in the repo** (guarded by `src/lib/release-hygiene.test.js`) |

### Owner actions (cannot be done from the repo)
- [ ] Set a 2-year expiry: `gpg --quick-set-expire EB0290C2138FA472E18F020318F47067ACC5BBF9 2y`, then republish the `.asc`.
- [ ] Keep an encrypted backup of the private key and its revocation certificate (gpg created one in `~/.gnupg/openpgp-revocs.d/`) on separate offline media, and note the location here: `Backup location: ______`.
- [ ] Publish the fingerprint on a second channel besides the repo: the owner's GitHub profile, and the key on a keyserver (`gpg --keyserver hkps://keys.openpgp.org --send-keys 18F47067ACC5BBF9`). A fingerprint that exists only in the repo does not help if the repo is compromised.
      Partly done: the fingerprint is in the release notes since
      [v0.1.1](https://github.com/wisangdg/archdroid/releases/tag/v0.1.1); profile and keyserver are not.

### Rotation
1. Create a new key and sign the rotation announcement with the old key.
2. Publish the new fingerprint on every channel; replace the `.asc`, `VERIFY-RELEASE.md`, and the default `GPG_KEY_ID` in `scripts/release/*.sh`.
3. Sign the next release with the new key; let the old key expire.

### Revocation (key leaked or lost)
1. `gpg --import <revocation certificate>`, upload it to the keyserver, and replace the `.asc` in the repo with the revoked version.
2. Announce it in Releases and on the second channel; mark every release since the leak as untrusted and re-issue it with a new key.
