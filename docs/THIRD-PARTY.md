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

## Residual risk

`waydroid_script` still runs as root because it mounts and modifies the Waydroid
image. Its Python requirements are installed from PyPI and are not hash-pinned,
so pinning the repository does not sandbox that code. Treat upstream updates and
the Python dependency chain as part of the root attack surface.

Phantom builds and runs with the desktop user's toolchain and configuration.
That is intentionally user-level: a compromised user toolchain can affect that
user, but it no longer receives root through this path.
