# Third-party installers

ArchDroid downloads these projects at install time. They are not vendored, so
their source and license remain with the upstream projects.

| Project | Upstream | Pinned revision | License | Privilege |
|---|---|---|---|---|
| waydroid_script | https://github.com/casualsnek/waydroid_script | `48dbfaf34a6ddbe78688c530f9ba1c26522aafb2` | GPL-3.0 | root (mounts and edits the Waydroid image) |
| Phantom | https://github.com/oliviermugishak/phantom | `1a6b7edb423462c1bc546031ed86d9bf2cc1cffd` | MIT | desktop user |

## How the installers are run

- `waydroid_script` is fetched into a fresh `mktemp -d` private directory, not a
  predictable `/tmp/waydroid_script` checkout. The commit is fetched by object id
  and `git rev-parse HEAD` is verified before the installer runs.
- Phantom is also fetched by pinned commit, but into a user-owned temporary
  directory and run as the desktop user with `INSTALL_SYSTEM_WRAPPER=0`. It
  installs under `$HOME/.local` and does not request root.
- Root scripts receive a fixed `PATH`, a private `HOME`/`TMPDIR` under
  `$ARCHDROID_WORK`, private `XDG_CACHE_HOME`/`XDG_DATA_HOME`/`XDG_CONFIG_HOME`
  under `$ARCHDROID_WORK`, and null Git/Python/pip user config. Sensitive desktop
  environment variables (`SUDO_USER`, `SUDO_UID`, `SUDO_GID`) are purged so
  upstream scripts cannot infer the user account or create root-owned files in
  `~/.cache`. The desktop home is only passed separately as `ARCHDROID_USER_HOME`
  for the wipe operation, and is resolved from `/proc/self/status` plus
  `/etc/passwd`/`getent`, never from the GUI's `HOME`, `PATH`, XDG variables or
  cache paths.
- Install markers are written by the unprivileged app process after the elevated
  command exits successfully. Root no longer writes into the user cache.

Updating either revision is a reviewed change: a commit changes the complete
source tree that will run with the corresponding privilege.

## Upstream audit and touched paths (P0-6.2)

### 1. `waydroid_script` (`48dbfaf34a6ddbe78688c530f9ba1c26522aafb2`)

- **Invocations by ArchDroid:**
  - `main.py install gapps` (GApps injection)
  - `main.py install libhoudini` (Intel Houdini ARM translation injection)
- **Target paths modified on the host:**
  - Waydroid image overlay: `/var/lib/waydroid/overlay/system/` (subdirectories:
    `priv-app/`, `product/`, `system_ext/`, `etc/permissions/`, `framework/`,
    `bin/`, `lib/`, `lib64/`). Permissions set to `0755` for directories/executables
    and `0644` for files.
  - Waydroid configuration: `/var/lib/waydroid/waydroid.cfg` (`ro.product.cpu.abilist`,
    `ro.dalvik.vm.native.bridge`, and Houdini bridge properties).
  - Loop-mount fallback (only if `mount_overlays = False`): `/tmp/waydroid`
    (mounted from `/var/lib/waydroid/images/system.img` with `mount -o rw` and
    unmounted via `umount`).
- **Temporary and extraction paths:**
  - `$ARCHDROID_WORK` (`/var/tmp/archdroid.*` or `/tmp/archdroid.*`, mode `0700`):
    contains private repo clone, venv, requirements lock, and isolated download
    cache (`$ARCHDROID_WORK/cache`). Completely wiped on exit via trap handler.
  - Hardcoded unpack paths in `/tmp`: `/tmp/gapps/extract` (GApps) and
    `/tmp/houdiniunpack` (Houdini). Pre-cleaned and wiped before and after
    execution and included in the privileged shell exit trap.
- **Binaries and executable payloads:**
  - `bin/<arch>/resetprop`: precompiled ELF binary embedded in `waydroid_script`
    repo. Verified as an Android Bionic binary (`interpreter /system/bin/linker64`
    or `linker`); does not execute on host glibc. Installed into container at
    `/system/etc/resetprop` and invoked by Android `init` via `resetprop.rc`.
  - `libhoudini.so`, `houdini`, `houdini64`: Intel proprietary ARM translation
    prebuilts for Android Bionic container runtime. Not executed on the host.
- **Network requests and integrity verification:**
  - GApps (Android 11): `open_gapps-*-11.0-pico-*.zip` from SourceForge, verified
    against hardcoded MD5 in `stuff/gapps.py`.
  - GApps (Android 13): `MindTheGapps-13.0.0-*.zip` from GitHub release
    `s1204IT/MindTheGappsBuilder`, verified against hardcoded MD5 in `stuff/gapps.py`.
  - Houdini (Android 11 & 13): Zip archives from
    `supremegamers/vendor_intel_proprietary_houdini` pinned to specific Git commit
    hashes (`81f2a51...` and `2f8f088...`) in URL, verified against hardcoded MD5
    in `stuff/houdini.py`.
  - Python dependencies: pinned to SHA-256 in `waydroid_script-requirements.lock`.
  - No arbitrary scripts (`.sh`, `.py`) are retrieved from unverified URLs or
    executed dynamically.

### 2. `phantom` (`1a6b7edb423462c1bc546031ed86d9bf2cc1cffd`)

- **Invocation by ArchDroid:** `bash "$repo/install.sh"`.
- **Privilege:** Unprivileged desktop user. Does not request root, polkit, or sudo.
- **Target paths modified:**
  - User binaries: `$HOME/.local/bin/phantom`, `$HOME/.local/bin/phantom-gui` (mode `0755`).
  - Android helper: `$HOME/.local/share/phantom/android/phantom-server.jar` (mode `0644`).
  - Config and profiles: `$HOME/.config/phantom/config.toml`, `$HOME/.config/phantom/profiles/*.json`.
  - Desktop integration: `$HOME/.local/share/applications/phantom-gui.desktop`,
    `$HOME/.local/share/icons/hicolor/scalable/apps/phantom.svg`.
  - Legacy cleanup: removes `$HOME/.local/bin/phantom-studio` if present.
  - Sudo-visible wrapper (`/usr/local/bin/phantom`): skipped via
    `export INSTALL_SYSTEM_WRAPPER=0`.
- **Network and compilation:**
  - Compiled locally via `cargo build --release` from the pinned checkout.
  - No unverified remote scripts are downloaded or executed.

## Python dependencies

`waydroid_script` runs from a private venv whose dependency set is embedded in
the ArchDroid binary as
`src-tauri/resources/waydroid_script-requirements.lock`. The checkout's own
`requirements.txt` is not read. Every direct and transitive package is pinned to
a version and SHA-256, and the installer runs
`pip install --require-hashes --only-binary=:all:` so PyPI cannot substitute a
different artifact for a pinned version. The direct requirements are tracked in
`src-tauri/resources/waydroid_script-requirements.in` and the lock is resolved
for Python 3.10+ from that directory with:

```sh
uv pip compile waydroid_script-requirements.in --universal \
    --python-version 3.10 --generate-hashes --only-binary :all: \
    --emit-build-options -o waydroid_script-requirements.lock
```

Updating the `waydroid_script` revision requires regenerating and reviewing the
lock in the same change. A dependency update is a change to code that runs as
root, so it follows the same review as a revision bump.

## Residual risk

`waydroid_script` still runs as root because it mounts and modifies the Waydroid
image. Its Python dependency set is hash-pinned, but the pinned upstream source
itself is not sandboxed. Treat upstream updates as part of the root attack
surface.

Phantom builds and runs with the desktop user's toolchain and configuration.
That is intentionally user-level: a compromised user toolchain can affect that
user, but it no longer receives root through this path.
