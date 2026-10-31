//! Shell bodies for the root and user installers. They are plain string
//! builders with no side effects, so the tests can assert on their content
//! without running anything.

use crate::*;

pub(crate) fn wipe_script() -> &'static str {
    r#"set -eu
[ -n "${ARCHDROID_USER_HOME:-}" ] && [ "$ARCHDROID_USER_HOME" != "/" ] || { echo "[ERROR] Invalid user home" >&2; exit 1; }
echo "[STEP] stop"
echo "[SYSTEM] Stopping Waydroid services..."
waydroid session stop || true
systemctl stop waydroid-container || true
echo "[STEP] wipe"
echo "[SYSTEM] Wiping Waydroid data..."
rm -rf /var/lib/waydroid
rm -rf "$ARCHDROID_USER_HOME/.local/share/waydroid"
rm -rf "$ARCHDROID_USER_HOME"/.local/share/applications/waydroid.*
echo "[SYSTEM] Wipe complete! Please restart ArchDroid."
"#
}

/// Upstream installer sources are pinned to immutable commits. Updating them is
/// a reviewed change, not something a branch or a user-writable `/tmp` checkout
/// can change behind our back.
pub(crate) const WAYDROID_SCRIPT_URL: &str = "https://github.com/casualsnek/waydroid_script.git";
pub(crate) const WAYDROID_SCRIPT_REV: &str = "48dbfaf34a6ddbe78688c530f9ba1c26522aafb2";
pub(crate) const PHANTOM_URL: &str = "https://github.com/oliviermugishak/phantom.git";
pub(crate) const PHANTOM_REV: &str = "1a6b7edb423462c1bc546031ed86d9bf2cc1cffd";

/// Python dependencies for the pinned `waydroid_script` revision. The lockfile
/// is embedded at compile time, so the upstream checkout cannot replace it, and
/// `pip --require-hashes` rejects any wheel that does not match a reviewed
/// SHA-256. Updating it is a reviewed change because this code runs as root.
pub(crate) const WAYDROID_SCRIPT_LOCK: &str =
    include_str!("../resources/waydroid_script-requirements.lock");

/// Write the embedded lock into the private checkout and install exactly that
/// set of wheels. Both installers call this so their pip flags cannot drift.
pub(crate) fn waydroid_script_python_setup() -> String {
    format!(
        r#"echo "[STEP] env"
echo "[SYSTEM] Setting up Python environment..."
python3 -m venv "$repo/venv"
requirements_lock="$repo/archdroid-requirements.lock"
printf '%s\n' {lock} > "$requirements_lock"
if ! grep -q -- '--hash=sha256:' "$requirements_lock"; then
    echo "[ERROR] The embedded Python dependency lock has no hashes." >&2
    exit 1
fi
"$repo/venv/bin/pip" install --no-cache-dir --disable-pip-version-check --no-input -q \
    --require-hashes --only-binary=:all: -r "$requirements_lock"
"#,
        lock = sh_quote(WAYDROID_SCRIPT_LOCK),
    )
}

/// Fetch exactly `rev` into a fresh repository. `repo` is a shell expansion such
/// as `"$repo"`, not a Rust path. The final `test` catches a server or checkout
/// that silently resolved a different object.
pub(crate) fn pinned_git_checkout(repo: &str, url: &str, rev: &str) -> String {
    format!(
        r#"echo "[STEP] repo"
git init -q {repo}
git -C {repo} remote add origin {url}
git -C {repo} fetch --depth 1 origin {rev}
git -C {repo} checkout -q --detach FETCH_HEAD
test "$(git -C {repo} rev-parse HEAD)" = {rev}
"#,
        repo = repo,
        url = sh_quote(url),
        rev = sh_quote(rev),
    )
}

/// Dependencies shared by the GApps and ARM Translation root scripts. Root
/// scripts run with `SAFE_PATH`, so these are the same programs preflight
/// checks. Install every missing package before the script does any work, so a
/// supported distro cannot fail after the root prompt because `git`, `venv`, or
/// the extraction tools were absent.
pub(crate) fn waydroid_script_dep_install(
    pm: Option<platform::PackageManager>,
    approved: &[&str],
    action: &str,
) -> String {
    let check = r#"missing=""
command -v git > /dev/null || missing="$missing git"
if ! command -v python3 > /dev/null; then
    missing="$missing python3"
elif ! python3 -c 'import venv, ensurepip' > /dev/null 2>&1; then
    missing="$missing python3-venv"
fi
command -v unsquashfs > /dev/null || missing="$missing squashfs-tools"
command -v lzip > /dev/null || missing="$missing lzip"
"#;

    match pm {
        Some(pm) if !approved.is_empty() => format!(
            r#"{check}if [ -n "$missing" ]; then
    echo "[SYSTEM] Installing approved tools via {name}: {packages}..."
    {install}
fi
{check}if [ -n "$missing" ]; then
    echo "[ERROR] Missing required tools:$missing were not approved for installation. Review the package preview and retry {action}." >&2
    exit 1
fi
"#,
            check = check,
            name = pm.name(),
            packages = approved.join(" "),
            install = pm.install_mapped_command(approved),
            action = action,
        ),
        Some(_) => format!(
            r#"{check}if [ -n "$missing" ]; then
    echo "[ERROR] Missing required tools:$missing were not approved for installation. Review the package preview and retry {action}." >&2
    exit 1
fi
"#,
            check = check,
            action = action,
        ),
        None => format!(
            r#"{check}if [ -n "$missing" ]; then
    echo "[ERROR] Missing required tools:$missing. Install them manually before {action}." >&2
    exit 1
fi
"#,
            check = check,
            action = action,
        ),
    }
}

pub(crate) fn gapps_install_script(
    pm: Option<platform::PackageManager>,
    approved: &[&str],
) -> String {
    format!(
        r#"set -eu
echo "[STEP] deps"
{dep_install}rm -rf /tmp/gapps /tmp/waydroid
repo="$ARCHDROID_WORK/waydroid_script"
{checkout}
{setup}
cd "$repo"
echo "[STEP] inject"
echo "[SYSTEM] Injecting GApps into Android Image (this will take a while)..."
if ! PYTHONUNBUFFERED=1 "$repo/venv/bin/python3" main.py install gapps; then
    rm -rf /tmp/gapps /tmp/waydroid
    echo "[ERROR] Failed to install GApps." >&2
    exit 1
fi
rm -rf /tmp/gapps /tmp/waydroid
echo "[SUCCESS] GApps installation complete! Please Start Waydroid."
"#,
        dep_install = waydroid_script_dep_install(pm, approved, "installing GApps"),
        checkout = pinned_git_checkout("\"$repo\"", WAYDROID_SCRIPT_URL, WAYDROID_SCRIPT_REV),
        setup = waydroid_script_python_setup(),
    )
}

pub(crate) fn houdini_install_script(
    pm: Option<platform::PackageManager>,
    approved: &[&str],
) -> String {
    format!(
        r#"set -eu
echo "[STEP] deps"
{dep_install}rm -rf /tmp/houdiniunpack /tmp/waydroid
repo="$ARCHDROID_WORK/waydroid_script"
{checkout}
{setup}
cd "$repo"
echo "[STEP] inject"
echo "[SYSTEM] Installing ARM Translation / libhoudini (this may take several minutes)..."
if ! PYTHONUNBUFFERED=1 "$repo/venv/bin/python3" main.py install libhoudini; then
    rm -rf /tmp/houdiniunpack /tmp/waydroid
    echo "[ERROR] ARM Translation installation failed!" >&2
    exit 1
fi
rm -rf /tmp/houdiniunpack /tmp/waydroid
echo "[SUCCESS] ARM Translation installation complete!"
"#,
        dep_install = waydroid_script_dep_install(pm, approved, "installing ARM Translation"),
        checkout = pinned_git_checkout("\"$repo\"", WAYDROID_SCRIPT_URL, WAYDROID_SCRIPT_REV),
        setup = waydroid_script_python_setup(),
    )
}

pub(crate) fn phantom_install_script() -> String {
    format!(
        r#"set -euo pipefail
if ! command -v git > /dev/null; then
    echo "[ERROR] git is required to download the pinned Phantom source." >&2
    exit 1
fi
work="$(mktemp -d "${{TMPDIR:-/tmp}}/archdroid-phantom.XXXXXXXXXX")"
trap 'rm -rf "$work"' EXIT HUP INT TERM
repo="$work/phantom"
{checkout}
export INSTALL_SYSTEM_WRAPPER=0
export PREFIX="$HOME/.local"
echo "[STEP] build"
echo "[SYSTEM] Running Phantom installer as the desktop user..."
if ! bash "$repo/install.sh"; then
    echo "[ERROR] Phantom installation failed!" >&2
    exit 1
fi
echo "[SUCCESS] Phantom Keymapper installed successfully!"
"#,
        checkout = pinned_git_checkout("\"$repo\"", PHANTOM_URL, PHANTOM_REV),
    )
}
