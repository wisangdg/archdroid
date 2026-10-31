//! Root elevation: trusted program lookup, the sanitized environment prefixed
//! to every privileged script, and the pkexec/sudo command builders.

use std::path::PathBuf;
use std::process::Command;

use crate::error::AppError;
use crate::*;

/// Fixed search path for root scripts. The GUI's PATH is user controlled, so a
/// privileged script must never inherit it: a planted `git`, `python3`, `pacman`
/// or `bash` would otherwise run as root.
pub(crate) const SAFE_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// Directories searched for programs that are invoked as root or as the
/// privileged shell itself. User-writable directories are deliberately absent.
const TRUSTED_DIRS: [&str; 6] = [
    "/usr/local/sbin",
    "/usr/local/bin",
    "/usr/sbin",
    "/usr/bin",
    "/sbin",
    "/bin",
];

/// First line echoed by an elevated shell. Lets us tell "the script ran as root"
/// apart from "elevation itself failed" without parsing pkexec messages, which
/// are localized and vary between polkit versions.
pub(crate) const ELEVATED_SENTINEL: &str = "[SYSTEM] Elevated shell started";

/// Resolve a program name against the current PATH without spawning a shell.
pub(crate) fn resolve_program(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Resolve a program without consulting PATH. Used for `pkexec`, `sudo`,
/// `bash`, `getent` and the fixed `id` fallback.
pub(crate) fn trusted_program(program: &str) -> Option<PathBuf> {
    TRUSTED_DIRS
        .iter()
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| candidate.is_file())
}

/// Quote a value so it survives being embedded into a `sh` script.
pub(crate) fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// The desktop user's identity, resolved from the system account database
/// rather than from HOME/USER/PATH/XDG_* environment variables.
#[derive(Debug, Clone)]
pub(crate) struct UserContext {
    uid: u32,
    home: PathBuf,
}

/// Parse the first numeric field of a `/proc/self/status` entry such as `Uid:`.
fn parse_status_id(status: &str, key: &str) -> Option<u32> {
    status.lines().find_map(|line| {
        let rest = line.strip_prefix(key)?;
        rest.split_whitespace().next()?.parse().ok()
    })
}

fn current_uid() -> Option<u32> {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| parse_status_id(&status, "Uid:"))
        .or_else(|| {
            let id = trusted_program("id")?;
            let output = Command::new(id)
                .arg("-ru")
                .env_clear()
                .env("PATH", SAFE_PATH)
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            String::from_utf8_lossy(&output.stdout).trim().parse().ok()
        })
}

/// Extract a user's home from a passwd(5)-formatted record.
fn parse_passwd_home(text: &str, uid: u32) -> Option<PathBuf> {
    text.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() < 6 || fields[2].parse::<u32>().ok() != Some(uid) {
            return None;
        }
        let home = fields[5].trim();
        if home.is_empty() {
            None
        } else {
            Some(PathBuf::from(home))
        }
    })
}

/// Canonicalise and sanity-check the home directory read from passwd. Rejecting
/// `/` (and symlinks that resolve to it) matters because wipe runs `rm -rf`
/// below this path as root.
fn validated_home(home: PathBuf, uid: u32) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;

    if !home.is_absolute() || home == Path::new("/") {
        return None;
    }
    let home = std::fs::canonicalize(home).ok()?;
    if home == Path::new("/") || !home.is_dir() {
        return None;
    }
    let metadata = std::fs::metadata(&home).ok()?;
    if metadata.uid() != uid {
        return None;
    }
    Some(home)
}

/// Resolve a home for `uid` from /etc/passwd, falling back to getent for NSS
/// users (LDAP/SSSD). Both sources are read as root-controlled system data.
fn user_home(uid: u32) -> Option<PathBuf> {
    if let Ok(passwd) = std::fs::read_to_string("/etc/passwd") {
        if let Some(home) = parse_passwd_home(&passwd, uid) {
            if let Some(home) = validated_home(home, uid) {
                return Some(home);
            }
        }
    }

    let getent = trusted_program("getent")?;
    let output = Command::new(getent)
        .arg("passwd")
        .arg(uid.to_string())
        .env_clear()
        .env("PATH", SAFE_PATH)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    validated_home(parse_passwd_home(&text, uid)?, uid)
}

fn current_user_context() -> Option<UserContext> {
    let uid = current_uid()?;
    let home = user_home(uid)?;
    Some(UserContext { uid, home })
}

/// Prefix a root script with a sanitized environment. Never derive HOME, PATH,
/// cache paths or config locations from the GUI process: polkit/sudo may pass
/// attacker-controlled variables through, and root must not consume them.
pub(crate) fn elevate_env(script: &str) -> Result<String, AppError> {
    let context = current_user_context().ok_or_else(|| {
        AppError::command_failed(
            "Could not resolve the desktop user's home directory from the system account database; refusing to run a privileged script with an untrusted HOME.",
        )
    })?;
    Ok(elevate_env_for(&context, script))
}

pub(crate) fn elevate_env_for(context: &UserContext, script: &str) -> String {
    let home = sh_quote(&context.home.to_string_lossy());
    let path = sh_quote(SAFE_PATH);
    format!(
        r#"export PATH={path}
export USER=root
export LOGNAME=root
export ARCHDROID_USER_HOME={home}
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_SYSTEM=/dev/null
export GIT_CONFIG_NOSYSTEM=1
export GIT_TERMINAL_PROMPT=0
export GIT_ALLOW_PROTOCOL=https
export PIP_CONFIG_FILE=/dev/null
export PIP_NO_INPUT=1
export PIP_DISABLE_PIP_VERSION_CHECK=1
export PYTHONNOUSERSITE=1
export PYTHONUNBUFFERED=1
unset BASH_ENV ENV CDPATH IFS GLOBIGNORE SHELL \
    PYTHONPATH PYTHONHOME PYTHONSTARTUP PYTHONINSPECT \
    LD_PRELOAD LD_LIBRARY_PATH LD_AUDIT \
    SUDO_USER SUDO_UID SUDO_GID \
    GIT_DIR GIT_WORK_TREE GIT_CONFIG GIT_CONFIG_COUNT GIT_CONFIG_PARAMETERS \
    GIT_EXEC_PATH GIT_TEMPLATE_DIR GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES \
    GIT_INDEX_FILE GIT_NAMESPACE GIT_CEILING_DIRECTORIES \
    GIT_ASKPASS SSH_ASKPASS SUDO_ASKPASS \
    GIT_SSH GIT_SSH_COMMAND GIT_PROXY_COMMAND \
    GIT_SSL_NO_VERIFY GIT_SSL_CAINFO GIT_SSL_CAPATH \
    PIP_INDEX_URL PIP_EXTRA_INDEX_URL PIP_TRUSTED_HOST PIP_FIND_LINKS PIP_CONSTRAINT \
    PIP_CERT PIP_CLIENT_CERT \
    SSL_CERT_FILE SSL_CERT_DIR REQUESTS_CA_BUNDLE CURL_CA_BUNDLE \
    XDG_CONFIG_HOME XDG_CACHE_HOME XDG_DATA_HOME
export SHELL=/bin/bash
umask 022
echo "{sentinel} (uid=$(id -u), caller={caller})"
ARCHDROID_WORK="$(mktemp -d /var/tmp/archdroid.XXXXXXXXXX 2>/dev/null || mktemp -d /tmp/archdroid.XXXXXXXXXX)" || {{
    echo "[ERROR] Could not create a private work directory for the privileged installer." >&2
    exit 1
}}
chmod 700 "$ARCHDROID_WORK" || {{
    echo "[ERROR] Could not secure the privileged work directory." >&2
    exit 1
}}
mkdir -p "$ARCHDROID_WORK/home" "$ARCHDROID_WORK/cache" "$ARCHDROID_WORK/data" "$ARCHDROID_WORK/config" || {{
    echo "[ERROR] Could not create private root work directories." >&2
    exit 1
}}
chmod 700 "$ARCHDROID_WORK/home" "$ARCHDROID_WORK/cache" "$ARCHDROID_WORK/data" "$ARCHDROID_WORK/config" || {{
    echo "[ERROR] Could not secure the private root work directories." >&2
    exit 1
}}
export HOME="$ARCHDROID_WORK/home"
export ARCHDROID_WORK
export TMPDIR="$ARCHDROID_WORK"
export XDG_CACHE_HOME="$ARCHDROID_WORK/cache"
export XDG_DATA_HOME="$ARCHDROID_WORK/data"
export XDG_CONFIG_HOME="$ARCHDROID_WORK/config"
export WAYDROID_CONFIG=/var/lib/waydroid/waydroid.cfg
# The unprivileged app cannot signal this root process group (EPERM), so
# timeout/cancel writes a file instead. This root-side watcher sees it and
# signals its own group, which makes cancellation work for all children.
(
    while [ ! -e "$ARCHDROID_CANCEL_FILE" ]; do
        sleep 0.2
    done
    kill -TERM 0
) &
ARCHDROID_CANCEL_WATCH=$!
# The script body runs under `set -e`, which also applies inside traps: the
# watcher is usually gone by now (its `kill -TERM 0` hits itself), so a bare
# `kill` would fail and exit before `rm -rf`. A signal must also end the
# script instead of resuming the next root command after the trap.
archdroid_cleanup() {{
    kill "$ARCHDROID_CANCEL_WATCH" 2>/dev/null || true
    rm -rf "$ARCHDROID_WORK" /tmp/gapps /tmp/houdiniunpack /tmp/waydroid
}}
trap archdroid_cleanup EXIT
trap 'exit 143' HUP INT TERM
{script}
"#,
        home = home,
        path = path,
        sentinel = ELEVATED_SENTINEL,
        caller = context.uid,
        script = script,
    )
}

/// Environment variables that must not reach the privileged helper's shell.
/// `BASH_ENV` is especially important: bash reads it before the script body, so
/// unsetting it inside the body would be too late.
const PRIVILEGED_ENV_REMOVE: &[&str] = &[
    "BASH_ENV",
    "ENV",
    "CDPATH",
    "IFS",
    "GLOBIGNORE",
    "SHELL",
    "PYTHONPATH",
    "PYTHONHOME",
    "PYTHONSTARTUP",
    "PYTHONINSPECT",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_CONFIG",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_EXEC_PATH",
    "GIT_TEMPLATE_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_INDEX_FILE",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_ASKPASS",
    "SSH_ASKPASS",
    "SUDO_ASKPASS",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
    "GIT_PROXY_COMMAND",
    "GIT_SSL_NO_VERIFY",
    "GIT_SSL_CAINFO",
    "GIT_SSL_CAPATH",
    "PIP_CONFIG_FILE",
    "PIP_INDEX_URL",
    "PIP_EXTRA_INDEX_URL",
    "PIP_TRUSTED_HOST",
    "PIP_FIND_LINKS",
    "PIP_CONSTRAINT",
    "PIP_CERT",
    "PIP_CLIENT_CERT",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "XDG_CONFIG_HOME",
    "XDG_CACHE_HOME",
    "XDG_DATA_HOME",
];

/// Build a command that runs `body` as root through `helper` (pkexec or sudo).
/// Both the helper and the shell are resolved without PATH; dangerous variables
/// are removed from the helper environment before it can start bash.
pub(crate) fn elevated_command(helper: &str, body: &str) -> Option<Command> {
    let program = trusted_program(helper)?;
    let shell = trusted_program("bash")?;
    let mut command = Command::new(program);
    command
        .arg(shell)
        .arg("--noprofile")
        .arg("--norc")
        .arg("-c")
        .arg(body);
    command.env("PATH", SAFE_PATH);
    for key in PRIVILEGED_ENV_REMOVE {
        command.env_remove(key);
    }
    Some(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn sh_quote_wraps_and_escapes_single_quotes() {
        assert_eq!(sh_quote("/home/wdg"), "'/home/wdg'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn elevated_script_uses_a_sanitized_root_environment() {
        let context = UserContext {
            uid: 1234,
            home: PathBuf::from("/home/archdroid-test"),
        };
        let body = elevate_env_for(&context, "echo body");

        assert!(body.contains(&format!("export PATH={}", sh_quote(SAFE_PATH))));
        assert!(body.contains(&format!(
            "export ARCHDROID_USER_HOME={}",
            sh_quote("/home/archdroid-test")
        )));
        assert!(body.contains("export HOME=\"$ARCHDROID_WORK/home\""));
        assert!(!body.contains("export HOME='/home/archdroid-test'"));
        assert!(!body.contains("ARCHDROID_CACHE"));
        assert!(body.contains("GIT_CONFIG_GLOBAL=/dev/null"));
        assert!(body.contains("GIT_CONFIG_SYSTEM=/dev/null"));
        assert!(body.contains("PIP_CONFIG_FILE=/dev/null"));
        assert!(body.contains("PYTHONNOUSERSITE=1"));
        assert!(body.contains("SUDO_USER"));
        assert!(body.contains("export XDG_CACHE_HOME=\"$ARCHDROID_WORK/cache\""));
        assert!(body.contains("export XDG_DATA_HOME=\"$ARCHDROID_WORK/data\""));
        assert!(body.contains("export XDG_CONFIG_HOME=\"$ARCHDROID_WORK/config\""));
        assert!(body.contains("export WAYDROID_CONFIG=/var/lib/waydroid/waydroid.cfg"));
        assert!(body.contains("mktemp -d /var/tmp/archdroid."));
        assert!(body.contains("chmod 700 \"$ARCHDROID_WORK\""));
        assert!(body.contains("ARCHDROID_CANCEL_FILE"));
        assert!(body.contains("ARCHDROID_CANCEL_WATCH=$!"));
        assert!(body.contains("kill -TERM 0"));
        assert!(body.contains("kill \"$ARCHDROID_CANCEL_WATCH\" 2>/dev/null || true"));
        assert!(
            body.contains("rm -rf \"$ARCHDROID_WORK\" /tmp/gapps /tmp/houdiniunpack /tmp/waydroid")
        );
        assert!(body.contains("trap archdroid_cleanup EXIT"));
        assert!(body.contains("trap 'exit 143' HUP INT TERM"));
        assert!(body.contains(ELEVATED_SENTINEL));
        assert!(body.trim_end().ends_with("echo body"));
    }

    /// Runs the real prelude (as the test user) with a `set -eu` body, the way
    /// every installer script starts, and cancels it through the cancel file
    /// the root-side watcher polls. Root groups cannot be signalled by the app,
    /// so this watcher path is the only cancel that reaches them.
    #[test]
    fn cancel_file_ends_a_set_e_script_and_removes_its_work_dir() {
        use std::os::unix::process::CommandExt;
        use std::process::Stdio;
        use std::time::{Duration, Instant};

        let dir = std::env::temp_dir().join(format!("archdroid-prelude-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("test dir must be creatable");
        let cancel_file = dir.join("cancel");
        let work_file = dir.join("work");
        let context = UserContext {
            uid: 1234,
            home: PathBuf::from("/home/archdroid-test"),
        };
        let body = elevate_env_for(
            &context,
            "set -eu\necho \"$ARCHDROID_WORK\" > \"$WORK_FILE\"\nsleep 300\necho resumed >> \"$WORK_FILE\"\n",
        );
        let mut child = Command::new("bash")
            .args(["--noprofile", "--norc", "-c", &body])
            .env("ARCHDROID_CANCEL_FILE", &cancel_file)
            .env("WORK_FILE", &work_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // `kill -TERM 0` in the watcher must not reach the test runner.
            .process_group(0)
            .spawn()
            .expect("bash must start");

        let deadline = Instant::now() + Duration::from_secs(5);
        let work = loop {
            let text = std::fs::read_to_string(&work_file).unwrap_or_default();
            if !text.trim().is_empty() {
                break PathBuf::from(text.trim());
            }
            assert!(Instant::now() < deadline, "the script never started");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(work.is_dir());

        std::fs::write(&cancel_file, "cancel\n").expect("cancel file must be writable");
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = child.try_wait().expect("try_wait must not fail") {
                break status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                panic!("the watcher did not stop the script");
            }
            std::thread::sleep(Duration::from_millis(20));
        };

        let resumed = std::fs::read_to_string(&work_file).unwrap_or_default();
        let work_left = work.exists();
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!status.success());
        assert!(!work_left, "the EXIT trap must remove $ARCHDROID_WORK");
        assert!(
            !resumed.contains("resumed"),
            "a cancelled script must not run its next root command"
        );
    }

    #[test]
    fn elevated_script_is_valid_bash() {
        let context = UserContext {
            uid: 1234,
            home: PathBuf::from("/home/archdroid-test"),
        };
        let body = elevate_env_for(
            &context,
            "echo body\nif waydroid init; then\n  echo ok\nfi\n",
        );
        let output = Command::new("bash")
            .args(["-n", "-c", &body])
            .output()
            .expect("failed to run bash");
        assert!(
            output.status.success(),
            "generated script is not valid bash: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn passwd_and_proc_status_are_parsed_without_environment_input() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\nwdg:x:1000:1000::/home/wdg:/bin/bash\n";
        assert_eq!(
            parse_passwd_home(passwd, 1000),
            Some(PathBuf::from("/home/wdg"))
        );
        assert_eq!(parse_passwd_home(passwd, 1234), None);

        let status = "Name:\tbash\nUid:\t1000\t0\t0\t0\nGid:\t1000\t0\t0\t0\n";
        assert_eq!(parse_status_id(status, "Uid:"), Some(1000));
        assert_eq!(parse_status_id(status, "Gid:"), Some(1000));
    }

    #[test]
    fn home_validation_rejects_relative_root_and_foreign_owned_directories() {
        assert!(validated_home(PathBuf::from("relative"), 1000).is_none());
        assert!(validated_home(PathBuf::from("/"), 0).is_none());

        let dir = std::env::temp_dir().join(format!("archdroid-home-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let canonical = std::fs::canonicalize(&dir).unwrap();
        let uid = current_uid().expect("the current uid must be readable");
        assert_eq!(validated_home(dir.clone(), uid), Some(canonical));
        assert!(validated_home(dir.clone(), uid.wrapping_add(1)).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trusted_program_ignores_user_path() {
        let bash = trusted_program("bash").expect("bash should exist in a trusted directory");
        assert!(
            TRUSTED_DIRS
                .iter()
                .any(|dir| bash == Path::new(dir).join("bash")),
            "resolved bash outside the trusted directories: {}",
            bash.display()
        );
        assert!(trusted_program("archdroid-no-such-binary-12345").is_none());
    }

    #[test]
    fn resolve_program_only_returns_existing_files() {
        assert!(resolve_program("bash").is_some());
        assert!(resolve_program("archdroid-no-such-binary-12345").is_none());
    }

    #[test]
    fn elevated_command_skips_missing_helpers() {
        assert!(elevated_command("archdroid-no-such-binary-12345", "true").is_none());
        let command = elevated_command("bash", "true").expect("bash should be resolvable");
        assert!(command.get_args().any(|arg| arg == "-c"));
    }

    #[test]
    fn elevated_command_uses_a_fixed_path_and_drops_bash_env() {
        let command = elevated_command("bash", "true").expect("bash should be resolvable");
        let envs: Vec<_> = command.get_envs().collect();
        assert!(envs.iter().any(|(key, value)| {
            *key == "PATH" && *value == Some(std::ffi::OsStr::new(SAFE_PATH))
        }));
        assert!(envs
            .iter()
            .any(|(key, value)| *key == "BASH_ENV" && value.is_none()));
    }
}
