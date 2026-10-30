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
  `$ARCHDROID_WORK`, and null Git/Python/pip user config. The desktop home is
  only passed separately as `ARCHDROID_USER_HOME` for the wipe operation, and is
  resolved from `/proc/self/status` plus `/etc/passwd`/`getent`, never from the
  GUI's `HOME`, `PATH`, XDG variables or cache paths.
- Install markers are written by the unprivileged app process after the elevated
  command exits successfully. Root no longer writes into the user cache.

Updating either revision is a reviewed change: a commit changes the complete
source tree that will run with the corresponding privilege.

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
