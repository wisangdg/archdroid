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
const WAYDROID_SCRIPT_URL: &str = "https://github.com/casualsnek/waydroid_script.git";
const WAYDROID_SCRIPT_REV: &str = "48dbfaf34a6ddbe78688c530f9ba1c26522aafb2";
const PHANTOM_URL: &str = "https://github.com/oliviermugishak/phantom.git";
const PHANTOM_REV: &str = "1a6b7edb423462c1bc546031ed86d9bf2cc1cffd";

/// Python dependencies for the pinned `waydroid_script` revision. The lockfile
/// is embedded at compile time, so the upstream checkout cannot replace it, and
/// `pip --require-hashes` rejects any wheel that does not match a reviewed
/// SHA-256. Updating it is a reviewed change because this code runs as root.
const WAYDROID_SCRIPT_LOCK: &str = include_str!("../resources/waydroid_script-requirements.lock");

/// Write the embedded lock into the private checkout and install exactly that
/// set of wheels. Both installers call this so their pip flags cannot drift.
fn waydroid_script_python_setup() -> String {
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
fn pinned_git_checkout(repo: &str, url: &str, rev: &str) -> String {
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
fn waydroid_script_dep_install(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mapped_dependencies(pm: platform::PackageManager) -> Vec<&'static str> {
        pm.mapped_packages(&["git", "python3", "python3-venv", "squashfs", "lzip"])
    }

    #[test]
    fn third_party_installers_are_pinned_and_use_private_workdirs() {
        let pacman = platform::PackageManager::Pacman;
        let approved = mapped_dependencies(pacman);
        let gapps = gapps_install_script(Some(pacman), &approved);
        let houdini = houdini_install_script(Some(pacman), &approved);
        assert!(gapps.contains("rm -rf /tmp/gapps /tmp/waydroid"));
        assert!(houdini.contains("rm -rf /tmp/houdiniunpack /tmp/waydroid"));
        for script in [&gapps, &houdini] {
            assert!(script.contains(WAYDROID_SCRIPT_REV));
            assert!(script.contains("$ARCHDROID_WORK"));
            assert!(!script.contains("cd /tmp"));
            assert!(!script.contains("git pull"));
            assert!(!script.contains("archdroid_marker"));
            assert!(script.contains("--require-hashes"));
            assert!(script.contains("--only-binary=:all:"));
            assert!(script.contains(r#"-r "$requirements_lock""#));
            assert!(!script.contains(r#"-r "$repo/requirements.txt""#));
        }

        let phantom = phantom_install_script();
        assert!(phantom.contains(PHANTOM_REV));
        assert!(phantom.contains("INSTALL_SYSTEM_WRAPPER=0"));
        assert!(phantom.contains("mktemp -d"));
        assert!(!phantom.contains("git pull"));
    }

    #[test]
    fn waydroid_script_lock_pins_every_requirement_with_hashes() {
        assert!(WAYDROID_SCRIPT_LOCK.contains(WAYDROID_SCRIPT_REV));
        assert!(WAYDROID_SCRIPT_LOCK.contains("--only-binary :all:"));

        let mut packages = 0;
        let mut current: Option<&str> = None;
        let mut current_has_hash = false;
        for line in WAYDROID_SCRIPT_LOCK.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("--hash=sha256:") {
                let hash = trimmed
                    .trim_end_matches('\\')
                    .trim()
                    .strip_prefix("--hash=sha256:")
                    .expect("checked prefix");
                assert_eq!(hash.len(), 64, "not a SHA-256 digest: {hash}");
                assert!(
                    hash.chars().all(|c| c.is_ascii_hexdigit()),
                    "not a hex digest: {hash}"
                );
                current_has_hash = true;
                continue;
            }
            if line.starts_with(char::is_whitespace) || trimmed.starts_with('-') {
                continue;
            }
            if let Some(name) = current {
                assert!(current_has_hash, "{name} has no --hash=sha256 entry");
            }
            current = Some(trimmed);
            current_has_hash = false;
            packages += 1;
        }
        if let Some(name) = current {
            assert!(current_has_hash, "{name} has no --hash=sha256 entry");
        }
        assert!(
            packages >= 10,
            "unexpectedly small lock: {packages} packages"
        );

        for line in WAYDROID_SCRIPT_LOCK
            .lines()
            .filter(|line| !line.trim_start().starts_with('#'))
        {
            for forbidden in [
                "--index-url",
                "--extra-index-url",
                "--find-links",
                "--trusted-host",
                "-e ",
                "http://",
                "https://",
            ] {
                assert!(
                    !line.contains(forbidden),
                    "lock contains forbidden source {forbidden}: {line}"
                );
            }
        }
    }

    #[test]
    fn waydroid_script_setup_writes_the_embedded_lock_and_requires_hashes() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("archdroid-lock-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = dir.join("waydroid_script");
        let bin = dir.join("bin");
        std::fs::create_dir_all(repo.join("venv/bin")).unwrap();
        std::fs::create_dir_all(&bin).unwrap();

        // Stub out the venv creation so this test never touches PyPI; the
        // pre-created pip only records the argv the installer would use.
        let python = bin.join("python3");
        std::fs::write(&python, "#!/bin/sh\nexit 0\n").unwrap();
        let pip = repo.join("venv/bin/pip");
        std::fs::write(
            &pip,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$ARCHDROID_TEST_PIP_ARGS\"\n",
        )
        .unwrap();
        for path in [&python, &pip] {
            let mut perms = std::fs::metadata(path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms).unwrap();
        }

        let script = format!(
            "set -eu\nrepo={}\n{}",
            sh_quote(&repo.to_string_lossy()),
            waydroid_script_python_setup()
        );
        let args_file = dir.join("pip-args");
        let output = Command::new("bash")
            .args(["-c", &script])
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("ARCHDROID_TEST_PIP_ARGS", &args_file)
            .output()
            .expect("failed to run bash");
        assert!(
            output.status.success(),
            "setup failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let written = std::fs::read_to_string(repo.join("archdroid-requirements.lock")).unwrap();
        assert_eq!(written, format!("{WAYDROID_SCRIPT_LOCK}\n"));

        let args = std::fs::read_to_string(&args_file).unwrap();
        assert!(args.contains("--require-hashes"));
        assert!(args.contains("--only-binary=:all:"));
        assert!(args.contains("-r"));
        assert!(args.contains("archdroid-requirements.lock"));
        assert!(!args.contains("requirements.txt"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn third_party_installers_are_valid_bash() {
        let pms = [
            Some(platform::PackageManager::Pacman),
            Some(platform::PackageManager::Apt),
            Some(platform::PackageManager::Dnf),
            Some(platform::PackageManager::Zypper),
            None,
        ];
        for pm in pms {
            let approved = pm.map(mapped_dependencies).unwrap_or_default();
            for (name, script) in [
                ("gapps", gapps_install_script(pm, &approved)),
                ("houdini", houdini_install_script(pm, &approved)),
            ] {
                let output = Command::new("bash")
                    .args(["-n", "-c", &script])
                    .output()
                    .expect("failed to run bash");
                assert!(
                    output.status.success(),
                    "{name} installer ({pm:?}) is not valid bash: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        let phantom = phantom_install_script();
        let output = Command::new("bash")
            .args(["-n", "-c", &phantom])
            .output()
            .expect("failed to run bash");
        assert!(
            output.status.success(),
            "phantom installer is not valid bash: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn installers_adapt_to_detected_package_manager() {
        let pacman = platform::PackageManager::Pacman;
        let pacman_gapps = gapps_install_script(Some(pacman), &mapped_dependencies(pacman));
        assert!(
            pacman_gapps.contains("pacman -S --needed --noconfirm git python squashfs-tools lzip")
        );

        let apt = platform::PackageManager::Apt;
        let apt_gapps = gapps_install_script(Some(apt), &mapped_dependencies(apt));
        assert!(apt_gapps.contains(
            "apt-get install -y --no-install-recommends git python3 python3-venv squashfs-tools lzip"
        ));

        let dnf = platform::PackageManager::Dnf;
        let dnf_gapps = gapps_install_script(Some(dnf), &mapped_dependencies(dnf));
        assert!(dnf_gapps.contains("dnf install -y git python3 squashfs-tools lzip"));

        let zypper = platform::PackageManager::Zypper;
        let zypper_gapps = gapps_install_script(Some(zypper), &mapped_dependencies(zypper));
        assert!(zypper_gapps.contains(
            "zypper --non-interactive install --no-recommends git python3 squashfs lzip"
        ));

        let unsupported_gapps = gapps_install_script(None, &[]);
        assert!(!unsupported_gapps.contains("pacman"));
        assert!(!unsupported_gapps.contains("apt-get"));
        assert!(unsupported_gapps.contains("[ERROR] Missing required tools:"));
        assert!(unsupported_gapps.contains("squashfs-tools"));
        assert!(unsupported_gapps.contains("lzip"));
    }

    #[test]
    fn root_installers_preinstall_every_dependency_before_checkout() {
        for pm in [
            platform::PackageManager::Pacman,
            platform::PackageManager::Apt,
            platform::PackageManager::Dnf,
            platform::PackageManager::Zypper,
        ] {
            let approved = mapped_dependencies(pm);
            for script in [
                gapps_install_script(Some(pm), &approved),
                houdini_install_script(Some(pm), &approved),
            ] {
                assert!(script.contains("command -v git"));
                assert!(script.contains("import venv, ensurepip"));
                assert!(script.contains("command -v unsquashfs"));
                assert!(script.contains("command -v lzip"));
                let install_at = script
                    .find("Installing approved tools via")
                    .expect("dependency install step missing");
                let checkout_at = script.find("git init -q").expect("checkout step missing");
                assert!(
                    install_at < checkout_at,
                    "dependencies are installed after checkout"
                );
            }
        }
    }

    #[test]
    fn installer_scripts_only_install_approved_packages() {
        let script = gapps_install_script(Some(platform::PackageManager::Pacman), &["git"]);
        assert!(script.contains("pacman -S --needed --noconfirm git"));
        assert!(!script.contains("pacman -S --needed --noconfirm git python"));
        assert!(script.contains("were not approved for installation"));
        assert!(script.contains("Review the package preview"));

        let no_approval = gapps_install_script(Some(platform::PackageManager::Pacman), &[]);
        assert!(!no_approval.contains("pacman -S --needed --noconfirm"));
        assert!(no_approval.contains("were not approved for installation"));
    }

    #[test]
    fn wipe_script_protects_paths_and_targets_only_waydroid_data() {
        let script = wipe_script();
        let output = Command::new("bash")
            .args(["-n", "-c", script])
            .output()
            .expect("bash should be executable");
        assert!(output.status.success());

        // Defense in depth check on ARCHDROID_USER_HOME
        assert!(script
            .contains(r#"[ -n "${ARCHDROID_USER_HOME:-}" ] && [ "$ARCHDROID_USER_HOME" != "/" ]"#));

        // Exact target paths
        assert!(script.contains("rm -rf /var/lib/waydroid"));
        assert!(script.contains(r#"rm -rf "$ARCHDROID_USER_HOME/.local/share/waydroid""#));
        assert!(script
            .contains(r#"rm -rf "$ARCHDROID_USER_HOME"/.local/share/applications/waydroid.*"#));

        // Must not contain general home or system deletion
        assert!(!script.contains("rm -rf $HOME"));
        assert!(!script.contains("rm -rf /home"));
        assert!(!script.contains("rm -rf /usr"));
        assert!(!script.contains("rm -rf /etc"));
    }

    #[test]
    fn installer_scripts_announce_their_steps() {
        let steps = |script: &str| -> Vec<String> {
            script
                .lines()
                .filter_map(|l| l.strip_prefix("echo \"[STEP] "))
                .map(|l| l.trim_end_matches('"').to_string())
                .collect()
        };
        let pm = Some(platform::PackageManager::Pacman);
        assert_eq!(
            steps(&gapps_install_script(pm, &[])),
            ["deps", "repo", "env", "inject"]
        );
        assert_eq!(
            steps(&houdini_install_script(pm, &[])),
            ["deps", "repo", "env", "inject"]
        );
        assert_eq!(steps(&phantom_install_script()), ["repo", "build"]);
        assert_eq!(steps(wipe_script()), ["stop", "wipe"]);
    }
}
