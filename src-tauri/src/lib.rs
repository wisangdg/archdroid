use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufReader, IsTerminal, Read, Seek, SeekFrom};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

mod adb;
mod capabilities;
#[cfg(feature = "dev-commands")]
mod device;
mod error;
mod platform;
mod waydroid;

use error::AppError;

#[derive(Serialize, Deserialize, Clone)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub package: String,
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiagnosticItem {
    pub key: String,
    pub title: String,
    pub status: String,
    pub value: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiagnosticsReport {
    pub items: Vec<DiagnosticItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
}

/// What a privileged installer would do to the host's package set.
///
/// `packages` holds distro package names when a supported package manager was
/// detected, otherwise the generic requirement names for manual installation.
/// `command` is the exact command the root script would run, so the UI can show
/// it before the user approves elevation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PackageInstallPreview {
    pub action: String,
    pub package_manager: Option<String>,
    pub packages: Vec<String>,
    pub command: Option<String>,
    pub requires_root: bool,
}

#[derive(Serialize, Deserialize)]
pub struct CommandResult {
    pub status: String,
    pub data: Option<String>,
    pub message: Option<String>,
    /// Stable machine readable reason, see `error::code`. `None` on success, so
    /// the UI can branch on it instead of matching prose.
    pub code: Option<String>,
}

impl CommandResult {
    /// A command that finished and has nothing interesting to say.
    fn success() -> Self {
        Self {
            status: "success".to_string(),
            data: None,
            message: None,
            code: None,
        }
    }

    /// A command whose stdout is the answer.
    fn with_data(data: String) -> Self {
        Self {
            status: "success".to_string(),
            data: Some(data),
            message: None,
            code: None,
        }
    }

    /// A command that succeeded and wants to say so in words.
    fn with_message(message: &str) -> Self {
        Self {
            status: "success".to_string(),
            data: None,
            message: Some(message.to_string()),
            code: None,
        }
    }

    /// The problem detection commands answer with their own status vocabulary
    /// (`installed`, `partial`, `not_installed`), so they set it explicitly.
    fn report(status: &str, message: &str) -> Self {
        Self {
            status: status.to_string(),
            data: None,
            message: Some(message.to_string()),
            code: None,
        }
    }
}

/// Keep the wire contract the UI already relies on - a `status` plus a human
/// readable `message` - and add the machine readable `code` next to it.
impl From<AppError> for CommandResult {
    fn from(error: AppError) -> Self {
        Self {
            status: "error".to_string(),
            data: None,
            message: Some(error.to_string()),
            code: Some(error.code().to_string()),
        }
    }
}

/// Sub directory of the user cache dir used for "this already happened" markers.
const CACHE_SUBDIR: &str = "archdroid";

/// Fixed search path for root scripts. The GUI's PATH is user controlled, so a
/// privileged script must never inherit it: a planted `git`, `python3`, `pacman`
/// or `bash` would otherwise run as root.
const SAFE_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

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
const ELEVATED_SENTINEL: &str = "[SYSTEM] Elevated shell started";

/// Upper bound for the output kept around for inspection.
const MAX_CAPTURED_OUTPUT: usize = 4096;

/// How long a terminated script gets to run its `trap ... EXIT` cleanup before
/// the whole process group is killed outright. `elevate_env_for` relies on that
/// trap to remove `$ARCHDROID_WORK`, so SIGKILL must not be the first signal.
const KILL_GRACE: Duration = Duration::from_secs(5);

/// Resolve a program name against the current PATH without spawning a shell.
pub(crate) fn resolve_program(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Resolve a program without consulting PATH. Used for `pkexec`, `sudo`,
/// `bash`, `getent` and the fixed `id` fallback.
fn trusted_program(program: &str) -> Option<PathBuf> {
    TRUSTED_DIRS
        .iter()
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| candidate.is_file())
}

fn cache_dir() -> PathBuf {
    let mut dir = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    dir.push(CACHE_SUBDIR);
    dir
}

fn marker_file(name: &str) -> PathBuf {
    cache_dir().join(name)
}

/// Persist a marker so a finished install is still known after a restart.
/// This is only called from the unprivileged app process, after a root command
/// has completed successfully; root never writes into the user cache.
/// A read-only cache directory must never break a status check, so callers
/// that only refresh a marker may ignore the error; install completion reports it.
fn write_marker(marker: &Path) -> std::io::Result<()> {
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(marker, MARKER_PENDING)
}

/// An install script exited 0 but no live check has confirmed the result yet.
const MARKER_PENDING: &str = "pending\n";
/// A live check against the running container confirmed the add-on.
const MARKER_VERIFIED: &str = "verified\n";

/// Marker state as the offline status path should report it.
#[derive(Debug, PartialEq)]
enum MarkerState {
    Absent,
    Pending,
    Verified,
}

fn marker_state(marker: &Path) -> MarkerState {
    match std::fs::read_to_string(marker) {
        Ok(text) if text == MARKER_VERIFIED => MarkerState::Verified,
        // Older builds wrote "installed\n" without a live check: treat as pending.
        Ok(_) => MarkerState::Pending,
        Err(_) => MarkerState::Absent,
    }
}

/// Status for an add-on whose container cannot be asked right now.
fn offline_marker_report(marker: &Path, what: &str) -> Option<CommandResult> {
    match marker_state(marker) {
        MarkerState::Verified => Some(CommandResult::report(
            "installed",
            &format!(
                "{what} installed (verified on the last live check). Start Waydroid to re-check."
            ),
        )),
        MarkerState::Pending => Some(CommandResult::report(
            "pending",
            &format!("{what} install finished but is not verified yet. Start Waydroid to verify."),
        )),
        MarkerState::Absent => None,
    }
}

/// Record a live-check confirmation; failure only costs the offline shortcut.
fn refresh_marker(marker: &Path) {
    let write = || -> std::io::Result<()> {
        if let Some(parent) = marker.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(marker, MARKER_VERIFIED)
    };
    if let Err(error) = write() {
        log::warn!("could not refresh {}: {error}", marker.display());
    }
}

/// Quote a value so it survives being embedded into a `sh` script.
fn sh_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// The desktop user's identity, resolved from the system account database
/// rather than from HOME/USER/PATH/XDG_* environment variables.
#[derive(Debug, Clone)]
struct UserContext {
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
fn elevate_env(script: &str) -> Result<String, AppError> {
    let context = current_user_context().ok_or_else(|| {
        AppError::command_failed(
            "Could not resolve the desktop user's home directory from the system account database; refusing to run a privileged script with an untrusted HOME.",
        )
    })?;
    Ok(elevate_env_for(&context, script))
}

fn elevate_env_for(context: &UserContext, script: &str) -> String {
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
trap 'kill "$ARCHDROID_CANCEL_WATCH" 2>/dev/null; rm -rf "$ARCHDROID_WORK" /tmp/gapps /tmp/houdiniunpack /tmp/waydroid' EXIT HUP INT TERM
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
fn elevated_command(helper: &str, body: &str) -> Option<Command> {
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

/// Run blocking work off the main thread. Tauri runs commands *without* the
/// `async` keyword on the main thread, where a slow `waydroid` call (up to
/// `TIMEOUT_LONG`) would freeze the whole UI; async commands on this helper
/// park the closure on the async runtime's blocking pool instead.
async fn off_main_thread(work: impl FnOnce() -> CommandResult + Send + 'static) -> CommandResult {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| AppError::Io(std::io::Error::other(error.to_string())).into())
}

/// Run a state-changing command while holding the single-flight slot.
///
/// These commands do not stream, so they never emit `cmd-started`/`cmd-finished`;
/// the guard is moved into the blocking closure, so the slot stays taken until
/// the child exits. A state change that would race a wipe, `waydroid init` or an
/// installer is rejected with `busy` instead of corrupting the image.
async fn run_exclusive<F>(label: &'static str, work: F) -> CommandResult
where
    F: FnOnce(&TaskGuard) -> CommandResult + Send + 'static,
{
    let task = match TaskGuard::start(label) {
        Ok(task) => task,
        Err(error) => return error.into(),
    };
    off_main_thread(move || work(&task)).await
}

#[tauri::command]
async fn waydroid_status() -> CommandResult {
    off_main_thread(
        || match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.status()) {
            Ok(text) => CommandResult::with_data(text),
            Err(error) => error.into(),
        },
    )
    .await
}

/// Cheap preflight: is waydroid on PATH at all, and which version does it report.
#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn waydroid_version() -> CommandResult {
    off_main_thread(
        || match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.version()) {
            Ok(version) => CommandResult::with_data(version),
            Err(error) => error.into(),
        },
    )
    .await
}

/// Best effort probe used by the status checks: `None` when waydroid is missing,
/// not initialised, or the command failed, so the caller can fall back to a
/// marker instead of reporting an error.
///
/// Waydroid only allows `shell` as root, so in an unprivileged session the live
/// probes below do not answer and the markers are what is left.
fn probe(run: impl FnOnce(&waydroid::Waydroid) -> error::Result<String>) -> Option<String> {
    let waydroid = waydroid::Waydroid::detect().ok()?;
    run(&waydroid).ok()
}

/// Live shell probe for the status cards. ADB is unprivileged and is tried
/// first. The root-only `waydroid shell` fallback is skipped when the session is
/// stopped or the container is frozen: that path cannot answer, and it may
/// prompt or hang instead of degrading to a cached marker.
fn live_shell(args: &[&str]) -> Option<String> {
    match adb::shell(args) {
        Ok(stdout) => Some(stdout),
        Err(error)
            if matches!(
                error.code(),
                error::code::SESSION_FROZEN | error::code::SESSION_NOT_RUNNING
            ) =>
        {
            None
        }
        Err(_) => probe(|waydroid| waydroid.shell(args)),
    }
}

#[tauri::command]
async fn waydroid_start() -> CommandResult {
    // Start the Waydroid session; an optional scheduler hint (e.g. ananicy-cpp) can be added later
    run_exclusive("session start", |_| {
        match waydroid::Waydroid::detect()
            .and_then(|waydroid| waydroid.session_start(waydroid::SESSION_START_GRACE_SECS))
        {
            Ok(_) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
    .await
}

#[tauri::command]
async fn waydroid_stop() -> CommandResult {
    // A failed exit is a failure again: this used to report success for any
    // spawn, so a refused stop looked like a stopped session.
    run_exclusive("session stop", |_| {
        match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.session_stop()) {
            Ok(_) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
    .await
}

/// Stop the streaming task that owns `task_id`.
///
/// A stale or unknown ID is deliberately a no-op success: a click that races
/// the end of one operation must not touch the operation that follows it.
#[tauri::command]
fn cancel_task(task_id: String) -> CommandResult {
    if cancel_active_task(&task_id) {
        CommandResult::success()
    } else {
        CommandResult::with_message("No matching operation is running.")
    }
}

#[tauri::command]
async fn get_waydroid_apps() -> Vec<AppInfo> {
    tauri::async_runtime::spawn_blocking(collect_waydroid_apps_sync)
        .await
        .unwrap_or_default()
}

/// Largest icon file read from a `.desktop` entry. Launcher icons are a few KB;
/// the cap keeps a hostile or wrong `Icon=` target from filling memory.
const MAX_ICON_BYTES: u64 = 512 * 1024;

fn icon_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    // Browsers do not render XPM, so it is rejected like any non-image.
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    if head.starts_with("<svg") || (head.starts_with("<?xml") && head.contains("<svg")) {
        return Some("image/svg+xml");
    }
    None
}

/// Read an `Icon=` target as a data URI. Only absolute paths to small regular
/// PNG/SVG files are accepted; anything else yields no icon.
fn icon_data_uri(path: &Path) -> Option<String> {
    if !path.is_absolute() {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ICON_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_ICON_BYTES).read_to_end(&mut bytes).ok()?;
    let mime = icon_mime(&bytes)?;
    Some(format!(
        "data:{mime};base64,{}",
        general_purpose::STANDARD.encode(&bytes)
    ))
}

/// Parsed `.desktop` entries keyed by path and modification time, so a poll
/// after an install only re-reads (and re-encodes icons for) changed files.
type AppCache = HashMap<PathBuf, (std::time::SystemTime, AppInfo)>;
static APP_CACHE: Mutex<Option<AppCache>> = Mutex::new(None);

fn collect_waydroid_apps_sync() -> Vec<AppInfo> {
    let Some(dir) = dirs::data_local_dir().map(|dir| dir.join("applications")) else {
        return Vec::new();
    };
    collect_apps_in(&dir)
}

fn collect_apps_in(dir: &Path) -> Vec<AppInfo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut guard = APP_CACHE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let old = guard.take().unwrap_or_default();
    let mut fresh = AppCache::new();
    let mut apps = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(package) = file_name
            .strip_prefix("waydroid.")
            .and_then(|rest| rest.strip_suffix(".desktop"))
        else {
            continue;
        };
        let path = entry.path();
        let mtime = entry.metadata().and_then(|m| m.modified()).ok();
        let app = match (mtime, old.get(&path)) {
            (Some(mtime), Some((cached, app))) if *cached == mtime => app.clone(),
            _ => parse_desktop_entry(&path, package),
        };
        if let Some(mtime) = mtime {
            fresh.insert(path, (mtime, app.clone()));
        }
        apps.push(app);
    }
    *guard = Some(fresh);
    apps
}

fn parse_desktop_entry(path: &Path, package: &str) -> AppInfo {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let field = |key: &str| {
        content
            .lines()
            .find_map(|l| l.strip_prefix(key))
            .map(str::to_string)
    };
    AppInfo {
        id: package.to_string(),
        name: field("Name=").unwrap_or_else(|| package.to_string()),
        package: package.to_string(),
        icon: field("Icon=").and_then(|icon| icon_data_uri(Path::new(&icon))),
    }
}

/// How much of each end of an APK to scan for the manifest entry. The local
/// header for `AndroidManifest.xml` is normally in the first block; the central
/// directory is at the end. This rejects text files and zips that are not APKs
/// before the slow install call, without adding a zip dependency.
const APK_SCAN_BYTES: u64 = 8 * 1024 * 1024;

fn contains_marker(bytes: &[u8], marker: &[u8]) -> bool {
    bytes.windows(marker.len()).any(|window| window == marker)
}

fn apk_looks_installable(path: &str) -> error::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() || (magic != *b"PK\x03\x04" && magic != *b"PK\x05\x06")
    {
        return Err(AppError::invalid_input(format!(
            "{path} is not an APK/ZIP archive."
        )));
    }

    let marker = b"AndroidManifest.xml";
    let mut scan = Vec::with_capacity(APK_SCAN_BYTES.min(len) as usize);
    file.seek(SeekFrom::Start(0))?;
    (&mut file).take(APK_SCAN_BYTES).read_to_end(&mut scan)?;
    if contains_marker(&scan, marker) {
        return Ok(());
    }

    if len > APK_SCAN_BYTES {
        scan.clear();
        file.seek(SeekFrom::End(-(APK_SCAN_BYTES as i64)))?;
        (&mut file).take(APK_SCAN_BYTES).read_to_end(&mut scan)?;
        if contains_marker(&scan, marker) {
            return Ok(());
        }
    }

    Err(AppError::invalid_input(format!(
        "{path} is a ZIP file without AndroidManifest.xml; it is not a valid APK."
    )))
}

/// Adb is authoritative when it is installed: `adb install` waits for the
/// Android package manager and `adb uninstall` reports the real result. Without
/// adb, `waydroid app install/remove` is used, and the obvious non-APK case is
/// rejected before the CLI can report a false success.
fn install_apk_blocking(path: String, cancel: Option<&AtomicBool>) -> error::Result<()> {
    apk_looks_installable(&path)?;
    if adb::available() {
        adb::install_apk(&path, cancel)?;
    } else {
        waydroid::Waydroid::detect()?.app_install(&path, cancel)?;
    }
    Ok(())
}

fn uninstall_app_blocking(package: String, cancel: Option<&AtomicBool>) -> error::Result<()> {
    let waydroid = waydroid::Waydroid::detect()?;
    if adb::available() {
        adb::uninstall_app(&package, cancel)?;
        return Ok(());
    }

    // Waydroid 1.6.3 `app remove` can exit 0 while the platform service logs
    // "Failed with code: -3" and leaves the package installed. The list is the
    // only proof, so never report success without it.
    waydroid.app_remove(&package, cancel)?;
    let mut last_error = None;
    for _ in 0..5 {
        if cancel.is_some_and(|c| c.load(Ordering::SeqCst)) {
            return Err(AppError::CancelRequested {
                program: "waydroid".to_string(),
            });
        }
        match waydroid.app_list() {
            Ok(list) => {
                if !waydroid::package_names(&list).contains(&package) {
                    return Ok(());
                }
                last_error = None;
            }
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(250));
    }

    Err(match last_error {
        Some(error) => AppError::command_failed(format!(
            "Could not verify whether {package} was uninstalled: {error}. Install adb (android-tools) for an authoritative result."
        )),
        None => AppError::command_failed(format!(
            "Waydroid reported success, but {package} is still installed (Waydroid 1.6.3 app remove can fail this way). Install adb (android-tools), or run \"sudo waydroid shell cmd package uninstall {package}\"."
        )),
    })
}

#[tauri::command]
async fn waydroid_install_apk(app: AppHandle, path: String) -> CommandResult {
    let task = match TaskGuard::start("APK install") {
        Ok(task) => task,
        Err(error) => return error.into(),
    };
    emit_started(&app, task.id());
    off_main_thread(move || {
        let cancel = task.cancel_flag();
        match install_apk_blocking(path, Some(&cancel)) {
            Ok(()) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
    .await
}

#[tauri::command]
async fn waydroid_uninstall_app(app: AppHandle, package: String) -> CommandResult {
    let task = match TaskGuard::start("app uninstall") {
        Ok(task) => task,
        Err(error) => return error.into(),
    };
    emit_started(&app, task.id());
    off_main_thread(move || {
        let cancel = task.cancel_flag();
        match uninstall_app_blocking(package, Some(&cancel)) {
            Ok(()) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
    .await
}

#[tauri::command]
async fn waydroid_launch_app(package: String) -> CommandResult {
    run_exclusive("app launch", move |_| {
        match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.app_launch(&package)) {
            Ok(_) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
    .await
}

#[tauri::command]
async fn waydroid_prop_set(key: String, value: String) -> CommandResult {
    run_exclusive(
        "property update",
        move |_| match waydroid::Waydroid::detect()
            .and_then(|waydroid| waydroid.prop_set(&key, &value))
        {
            Ok(_) => CommandResult::success(),
            Err(error) => error.into(),
        },
    )
    .await
}

#[tauri::command]
async fn waydroid_prop_get(key: String) -> CommandResult {
    off_main_thread(move || {
        match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.prop_get(&key)) {
            Ok(value) => CommandResult::with_data(value),
            Err(error) => error.into(),
        }
    })
    .await
}

#[tauri::command]
async fn waydroid_log(app: AppHandle) -> CommandResult {
    let script = r#"
if [ -f /var/lib/waydroid/waydroid.log ]; then
    tail -n 1000 /var/lib/waydroid/waydroid.log
else
    echo "No logs found at /var/lib/waydroid/waydroid.log"
fi
"#
    .to_string();

    stream_privileged_script(app, script, Completion::None, waydroid::TIMEOUT_NORMAL)
        .await
        .into_command_result()
}

/// Mask what must not reach a log or a support paste: the home directory and
/// the value of `token=`/`password=`-style assignments or `Bearer` headers.
fn redact_text(text: &str) -> String {
    const SENSITIVE: [&str; 6] = ["token", "password", "passwd", "secret", "apikey", "api_key"];
    let mut text = text.to_string();
    if let Some(home) = std::env::var_os("HOME").and_then(|h| h.into_string().ok()) {
        if home.len() > 1 {
            text = text.replace(&home, "~");
        }
    }
    text.lines()
        .map(|line| {
            let mut words: Vec<String> = line.split(' ').map(str::to_string).collect();
            for i in 0..words.len() {
                let lower = words[i].to_ascii_lowercase();
                if lower == "bearer" && i + 1 < words.len() {
                    words[i + 1] = "[redacted]".to_string();
                } else if let Some(eq) = lower.find(['=', ':']) {
                    let key =
                        lower[..eq].trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_');
                    if SENSITIVE.iter().any(|k| key.ends_with(k)) {
                        if eq + 1 < words[i].len() {
                            words[i] = format!("{}[redacted]", &words[i][..=eq]);
                        } else if i + 1 < words.len() {
                            words[i + 1] = "[redacted]".to_string();
                        }
                    }
                }
            }
            words.join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Newest `*.log` file in the app log directory.
fn app_log_file(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_log_dir().ok()?;
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

/// Most of the log the UI will ever show.
const MAX_LOG_VIEW_BYTES: u64 = 64 * 1024;

#[tauri::command]
async fn app_log_read(app: AppHandle) -> CommandResult {
    off_main_thread(move || {
        let Some(path) = app_log_file(&app) else {
            return CommandResult::with_data(String::new());
        };
        let read = || -> std::io::Result<String> {
            let mut file = std::fs::File::open(&path)?;
            let len = file.metadata()?.len();
            file.seek(SeekFrom::Start(len.saturating_sub(MAX_LOG_VIEW_BYTES)))?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(redact_text(&String::from_utf8_lossy(&bytes)))
        };
        match read() {
            Ok(text) => CommandResult::with_data(text),
            Err(e) => AppError::Io(e).into(),
        }
    })
    .await
}

/// Where the log lives, so Settings can tell the user.
#[tauri::command]
fn app_log_location(app: AppHandle) -> CommandResult {
    match app.path().app_log_dir() {
        Ok(dir) => CommandResult::with_data(redact_text(&dir.to_string_lossy())),
        Err(e) => AppError::Io(std::io::Error::other(e.to_string())).into(),
    }
}

#[tauri::command]
async fn app_log_clear(app: AppHandle) -> CommandResult {
    off_main_thread(move || match app_log_file(&app) {
        Some(path) => match std::fs::write(&path, b"") {
            Ok(()) => CommandResult::success(),
            Err(e) => AppError::Io(e).into(),
        },
        None => CommandResult::success(),
    })
    .await
}

fn check_network_connectivity() -> bool {
    let targets = [
        "1.1.1.1:53".parse::<std::net::SocketAddr>().unwrap(),
        "8.8.8.8:53".parse::<std::net::SocketAddr>().unwrap(),
    ];
    for target in &targets {
        if std::net::TcpStream::connect_timeout(target, std::time::Duration::from_secs(1)).is_ok() {
            return true;
        }
    }
    false
}

fn check_available_disk_space_kb(path: &Path) -> Option<u64> {
    let probe_path = if path.exists() {
        path
    } else if let Some(parent) = path.parent() {
        if parent.exists() {
            parent
        } else {
            Path::new("/")
        }
    } else {
        Path::new("/")
    };

    let output = Command::new("df")
        .args(["-Pk", probe_path.to_str().unwrap_or("/")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            if let Ok(avail) = parts[3].parse::<u64>() {
                return Some(avail);
            }
        }
    }
    None
}

fn format_disk_space(kb: u64) -> String {
    if kb >= 1_048_576 {
        format!("{:.1} GB Free", kb as f64 / 1_048_576.0)
    } else {
        format!("{:.0} MB Free", kb as f64 / 1024.0)
    }
}

fn check_python_venv_available() -> (bool, bool) {
    let has_python3 = resolve_program("python3").is_some() || trusted_program("python3").is_some();
    if !has_python3 {
        return (false, false);
    }
    let has_venv = Command::new("python3")
        .args(["-c", "import venv, ensurepip"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    (true, has_venv)
}

/// Root scripts run with `SAFE_PATH`, so preflight must not accept a `python3`
/// from a user-writable directory that disappears after elevation.
fn check_trusted_python_venv() -> (bool, bool) {
    let Some(python) = trusted_program("python3") else {
        return (false, false);
    };
    let has_venv = Command::new(python)
        .args(["-c", "import venv, ensurepip"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    (true, has_venv)
}

fn find_terminal_emulator() -> Option<String> {
    if let Ok(term) = std::env::var("TERMINAL") {
        let trimmed = term.trim();
        if !trimmed.is_empty()
            && (resolve_program(trimmed).is_some() || trusted_program(trimmed).is_some())
        {
            return Some(trimmed.to_string());
        }
    }
    let candidates = [
        "konsole",
        "alacritty",
        "gnome-terminal",
        "foot",
        "kitty",
        "ghostty",
        "xfce4-terminal",
        "xterm",
    ];
    for candidate in candidates {
        if resolve_program(candidate).is_some() || trusted_program(candidate).is_some() {
            return Some(candidate.to_string());
        }
    }
    None
}

fn has_elevation_tool() -> Option<&'static str> {
    if resolve_program("pkexec").is_some() || trusted_program("pkexec").is_some() {
        Some("pkexec")
    } else if resolve_program("sudo").is_some() || trusted_program("sudo").is_some() {
        Some("sudo")
    } else {
        None
    }
}

/// True when a polkit authentication agent process is running. `pkexec` has
/// nowhere to show its password prompt without one.
fn polkit_agent_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.bytes().all(|b| b.is_ascii_digit()))
            && std::fs::read_to_string(entry.path().join("comm"))
                .map(|comm| is_polkit_agent_name(comm.trim()))
                .unwrap_or(false)
    })
}

fn is_polkit_agent_name(comm: &str) -> bool {
    let comm = comm.to_ascii_lowercase();
    // `polkitd` is the daemon, not an authentication agent.
    comm != "polkitd" && (comm.contains("polkit") || comm.contains("policykit"))
}

fn has_program(name: &str) -> bool {
    resolve_program(name).is_some() || trusted_program(name).is_some()
}

/// Android SDK root, preferring the environment variables the upstream Phantom
/// build script already understands.
fn android_sdk_root() -> Option<PathBuf> {
    for key in ["ANDROID_SDK_ROOT", "ANDROID_HOME"] {
        if let Some(value) = std::env::var_os(key) {
            let path = PathBuf::from(value);
            if path.is_dir() {
                return Some(path);
            }
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let path = home.join("Android/Sdk");
    path.is_dir().then_some(path)
}

/// `android.jar` from `ANDROID_JAR` or a platform under the Android SDK.
fn find_android_jar() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("ANDROID_JAR") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
    }
    let platforms = android_sdk_root()?.join("platforms");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(platforms)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("android.jar"))
        .filter(|path| path.is_file())
        .collect();
    candidates.sort();
    candidates.pop()
}

/// `d8` from `D8`, PATH, the SDK build-tools, or cmdline-tools.
fn find_d8() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("D8") {
        let path = PathBuf::from(value);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Some(path) = resolve_program("d8").or_else(|| trusted_program("d8")) {
        return Some(path);
    }
    let sdk = android_sdk_root()?;
    if let Ok(entries) = std::fs::read_dir(sdk.join("build-tools")) {
        let mut candidates: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path().join("d8"))
            .filter(|path| path.is_file())
            .collect();
        candidates.sort();
        if let Some(path) = candidates.pop() {
            return Some(path);
        }
    }
    let cmdline_d8 = sdk.join("cmdline-tools/latest/bin/d8");
    cmdline_d8.is_file().then_some(cmdline_d8)
}

/// Generic dependency names shared by the GApps and ARM Translation installers.
/// Root scripts run with `SAFE_PATH`, so only trusted system locations count.
fn gapps_missing_dependencies() -> Vec<&'static str> {
    let mut missing = Vec::new();
    if trusted_program("git").is_none() {
        missing.push("git");
    }
    let (has_python3, has_venv) = check_trusted_python_venv();
    if !has_python3 {
        missing.push("python3");
    } else if !has_venv {
        missing.push("python3-venv");
    }
    if trusted_program("unsquashfs").is_none() {
        missing.push("squashfs");
    }
    if trusted_program("lzip").is_none() {
        missing.push("lzip");
    }
    missing
}

/// Actions that may need host packages before their privileged work starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PackageInstallAction {
    Init,
    Gapps,
    Houdini,
}

impl PackageInstallAction {
    fn parse(action: &str) -> error::Result<Self> {
        match action {
            "init" => Ok(Self::Init),
            "gapps" => Ok(Self::Gapps),
            "houdini" => Ok(Self::Houdini),
            other => Err(AppError::invalid_input(format!(
                "Unknown package installation action \"{other}\". Expected init, gapps, or houdini."
            ))),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Gapps => "gapps",
            Self::Houdini => "houdini",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Init => "initializing Waydroid",
            Self::Gapps => "installing GApps",
            Self::Houdini => "installing ARM Translation",
        }
    }

    /// Generic dependency names that are missing right now. The package manager
    /// maps these to distro package names before anything is shown or run.
    fn missing_packages(self) -> Vec<&'static str> {
        match self {
            Self::Init => {
                if trusted_program("waydroid").is_none() {
                    vec!["waydroid"]
                } else {
                    Vec::new()
                }
            }
            Self::Gapps | Self::Houdini => gapps_missing_dependencies(),
        }
    }
}

fn package_install_preview(
    action: PackageInstallAction,
    missing: &[&'static str],
    pm: Option<platform::PackageManager>,
) -> PackageInstallPreview {
    match pm {
        Some(pm) => {
            let mapped = pm.mapped_packages(missing);
            PackageInstallPreview {
                action: action.as_str().to_string(),
                package_manager: Some(pm.name().to_string()),
                packages: mapped.iter().map(|package| package.to_string()).collect(),
                command: (!mapped.is_empty()).then(|| pm.install_mapped_command(&mapped)),
                requires_root: true,
            }
        }
        None => PackageInstallPreview {
            action: action.as_str().to_string(),
            package_manager: None,
            packages: missing.iter().map(|package| package.to_string()).collect(),
            command: None,
            requires_root: true,
        },
    }
}

/// Validate the package list the user saw and approved against what the
/// installer still needs. Only exact distro names from the reviewed plan are
/// accepted, so an IPC caller cannot smuggle an arbitrary package or shell
/// fragment into the root script.
fn approved_install_packages(
    action: PackageInstallAction,
    required: &[&'static str],
    pm: Option<platform::PackageManager>,
    approved: Option<&[String]>,
) -> error::Result<Vec<&'static str>> {
    let Some(pm) = pm else {
        // The existing preflight reports the missing package manager before
        // elevation; there is no install plan to approve in that case.
        return Ok(Vec::new());
    };

    let mapped = pm.mapped_packages(required);
    if mapped.is_empty() {
        return Ok(Vec::new());
    }

    let approved = approved.ok_or_else(|| {
        AppError::approval_required(format!(
            "Installing system packages for {} requires explicit approval. Review the package preview and confirm again.",
            action.description()
        ))
    })?;

    let mut accepted: Vec<&'static str> = Vec::new();
    for name in approved {
        let matched = mapped
            .iter()
            .copied()
            .find(|candidate| *candidate == name)
            .ok_or_else(|| {
                AppError::invalid_input(format!(
                    "Package \"{name}\" is not part of the reviewed installation plan for {}. Review the package preview again.",
                    action.description()
                ))
            })?;
        if !accepted.contains(&matched) {
            accepted.push(matched);
        }
    }

    let unapproved: Vec<&str> = mapped
        .iter()
        .copied()
        .filter(|name| !accepted.contains(name))
        .collect();
    if !unapproved.is_empty() {
        return Err(AppError::approval_required(format!(
            "The following required packages were not approved: {}. Review the package preview and confirm again.",
            unapproved.join(", ")
        )));
    }

    Ok(accepted)
}

/// Probing for missing tools spawns `python3` and friends, so it runs off the
/// main thread like every other host probe.
#[tauri::command]
async fn preview_package_install(
    action: String,
) -> std::result::Result<PackageInstallPreview, CommandResult> {
    let action = PackageInstallAction::parse(&action).map_err(CommandResult::from)?;
    tauri::async_runtime::spawn_blocking(move || {
        let missing = action.missing_packages();
        package_install_preview(action, &missing, platform::PackageManager::detect())
    })
    .await
    .map_err(|e| CommandResult::from(AppError::Io(std::io::Error::other(e.to_string()))))
}

/// Both GApps and Houdini modify an existing Android image. Refuse before the
/// root prompt when the image is absent: the upstream script cannot repair that.
fn require_initialized_waydroid(action: &str) -> Result<(), AppError> {
    if trusted_program("waydroid").is_none() {
        return Err(AppError::not_found(
            "waydroid",
            &format!("Waydroid must be installed in a system path before {action}."),
        ));
    }
    if !waydroid::is_initialized() {
        return Err(AppError::not_initialized(format!(
            "Waydroid is not initialized. Run the setup wizard or `sudo waydroid init` before {action}."
        )));
    }
    Ok(())
}

fn installer_preflight(pm: Option<platform::PackageManager>, action: &str) -> Result<(), AppError> {
    if has_elevation_tool().is_none() {
        return Err(AppError::elevation_denied(
            "No root elevation tool (pkexec or sudo) found on host. Cannot perform administrative operations.",
        ));
    }
    if !check_network_connectivity() {
        return Err(AppError::command_failed(format!(
            "No internet connection detected. Internet access is required to {action}."
        )));
    }
    require_initialized_waydroid(action)?;
    if let Some(kb) = check_available_disk_space_kb(Path::new("/var/lib/waydroid")) {
        if kb < 500_000 {
            return Err(AppError::command_failed(format!(
                "Insufficient disk space: only {:.0} MB available. {} requires at least 500 MB.",
                kb as f64 / 1024.0,
                action
            )));
        }
    }
    let missing = gapps_missing_dependencies();
    if !missing.is_empty() && pm.is_none() {
        let names = missing.join(", ");
        return Err(AppError::not_found(
            &names,
            &format!(
                "Required dependencies ({names}) are missing, and no supported package manager (pacman, apt-get, dnf, zypper) was found on this Linux distribution. Please install them manually before {action}."
            ),
        ));
    }
    Ok(())
}

/// Phantom builds Rust binaries and an Android server jar as the desktop user.
/// None of these can be fixed after the build starts, so collect them first.
fn phantom_missing_dependencies() -> (Vec<&'static str>, Vec<&'static str>) {
    let mut packages = Vec::new();
    let mut sdk = Vec::new();
    if !has_program("git") {
        packages.push("git");
    }
    if !has_program("cargo") {
        packages.push("cargo");
    }
    if !has_program("javac") || !has_program("jar") {
        packages.push("jdk");
    }
    if !has_program("sed") {
        packages.push("sed");
    }
    if !has_program("install") {
        packages.push("coreutils");
    }
    if find_d8().is_none() {
        sdk.push("d8");
    }
    if find_android_jar().is_none() {
        sdk.push("android.jar");
    }
    (packages, sdk)
}

fn phantom_dependency_error(
    packages: &[&'static str],
    sdk: &[&'static str],
    pm: Option<platform::PackageManager>,
) -> AppError {
    let mut names = packages.to_vec();
    names.extend(sdk.iter().copied());

    let mut hints = Vec::new();
    if !packages.is_empty() {
        match pm {
            Some(pm) => hints.push(format!(
                "Install missing packages with: {}",
                pm.user_install_command(packages)
            )),
            None => hints.push(format!(
                "Install missing packages ({}) with your distribution's package manager.",
                packages.join(", ")
            )),
        }
    }
    if sdk.contains(&"d8") {
        hints.push(
            "Install Android SDK command-line tools so `d8` is on PATH, or set D8=/path/to/d8."
                .to_string(),
        );
    }
    if sdk.contains(&"android.jar") {
        hints.push(
            "Install an Android platform with sdkmanager (for example `sdkmanager \"platforms;android-34\"`), or set ANDROID_JAR=/path/to/android.jar."
                .to_string(),
        );
    }

    AppError::not_found(
        &names.join(", "),
        &format!(
            "Phantom requires missing dependencies: {}. {}",
            names.join(", "),
            hints.join(" ")
        ),
    )
}

fn phantom_preflight(pm: Option<platform::PackageManager>) -> Result<(), AppError> {
    let (packages, sdk) = phantom_missing_dependencies();
    if packages.is_empty() && sdk.is_empty() {
        return Ok(());
    }
    Err(phantom_dependency_error(&packages, &sdk, pm))
}

fn phantom_disk_preflight() -> Result<(), AppError> {
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
        AppError::not_found("HOME", "Cannot determine the desktop home directory.")
    })?;
    if let Some(kb) = check_available_disk_space_kb(&home) {
        if kb < 1_000_000 {
            return Err(AppError::command_failed(format!(
                "Insufficient disk space in {}: only {:.0} MB available. Phantom needs about 1 GB to build its Rust binaries and Android server jar.",
                home.display(),
                kb as f64 / 1024.0
            )));
        }
    }
    Ok(())
}

#[tauri::command]
async fn open_shared_folder() -> CommandResult {
    off_main_thread(open_shared_folder_sync).await
}

fn open_shared_folder_sync() -> CommandResult {
    let mut path = match dirs::data_local_dir() {
        Some(dir) => dir,
        None => {
            return AppError::not_found(
                "local data directory",
                "Cannot determine XDG local data directory.",
            )
            .into();
        }
    };
    path.push("waydroid");
    path.push("data");
    path.push("media");
    path.push("0");

    // Create folder if it doesn't exist
    let _ = std::fs::create_dir_all(&path);

    if resolve_program("xdg-open").is_none() && trusted_program("xdg-open").is_none() {
        return AppError::not_found(
            "xdg-open",
            "Install xdg-utils so a file manager can be asked to open the folder.",
        )
        .into();
    }

    match Command::new("xdg-open").arg(&path).spawn() {
        Ok(_) => CommandResult::with_message("Opened shared media folder in file manager."),
        Err(error) => AppError::from_spawn(
            "xdg-open",
            "Install xdg-utils so a file manager can be asked to open the folder.",
            error,
        )
        .into(),
    }
}

fn collect_diagnostics() -> DiagnosticsReport {
    let pm = platform::PackageManager::detect();
    let mut items = Vec::new();

    // 1. Session Type (Wayland vs X11)
    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|s| s.to_lowercase() == "wayland")
            .unwrap_or(false);

    if is_wayland {
        items.push(DiagnosticItem {
            key: "session".to_string(),
            title: "Display Session".to_string(),
            status: "ok".to_string(),
            value: "Wayland Native".to_string(),
            detail: "Wayland compositor detected. Waydroid runs with native direct rendering."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "session".to_string(),
            title: "Display Session".to_string(),
            status: "warning".to_string(),
            value: "X11 Display".to_string(),
            detail: "Waydroid requires Wayland. Under X11, a nested compositor (Weston/cage) may be required.".to_string(),
            remediation: Some("Run inside a Wayland compositor, or use cage: cage -- waydroid show-full-ui".to_string()),
        });
    }

    // 2. KVM Virtualization Acceleration
    let kvm_path = Path::new("/dev/kvm");
    if kvm_path.exists() {
        let is_writable = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(kvm_path)
            .is_ok();

        if is_writable {
            items.push(DiagnosticItem {
                key: "kvm".to_string(),
                title: "KVM Acceleration".to_string(),
                status: "ok".to_string(),
                value: "Hardware KVM Ready".to_string(),
                detail: "/dev/kvm is available with read/write access.".to_string(),
                remediation: None,
            });
        } else {
            items.push(DiagnosticItem {
                key: "kvm".to_string(),
                title: "KVM Acceleration".to_string(),
                status: "warning".to_string(),
                value: "Permission Denied".to_string(),
                detail: "KVM device exists but user lacks write permission.".to_string(),
                remediation: Some("sudo usermod -aG kvm $USER".to_string()),
            });
        }
    } else {
        items.push(DiagnosticItem {
            key: "kvm".to_string(),
            title: "KVM Acceleration".to_string(),
            status: "warning".to_string(),
            value: "Not Detected".to_string(),
            detail:
                "/dev/kvm not found. Ensure Intel VT-x or AMD-V is enabled in motherboard BIOS."
                    .to_string(),
            remediation: Some(
                "Enable VT-x/AMD-V in BIOS and load kernel module: sudo modprobe kvm".to_string(),
            ),
        });
    }

    // 3. Android Binder IPC
    let has_binder = Path::new("/dev/binderfs").exists()
        || Path::new("/dev/binder").exists()
        || Path::new("/dev/binderfs/binder").exists();

    if has_binder {
        items.push(DiagnosticItem {
            key: "binder".to_string(),
            title: "Android Binder IPC".to_string(),
            status: "ok".to_string(),
            value: "Binder Nodes Active".to_string(),
            detail: "Android kernel inter-process communication drivers are active.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "binder".to_string(),
            title: "Android Binder IPC".to_string(),
            status: "ok".to_string(),
            value: "Managed by Daemon".to_string(),
            detail: "Binderfs will be mounted automatically when container daemon starts."
                .to_string(),
            remediation: None,
        });
    }

    // 4. GPU 3D Passthrough
    let has_gpu = Path::new("/dev/dri/renderD128").exists() || Path::new("/dev/dri").exists();
    if has_gpu {
        items.push(DiagnosticItem {
            key: "gpu".to_string(),
            title: "GPU Acceleration".to_string(),
            status: "ok".to_string(),
            value: "Direct Rendering (DRI)".to_string(),
            detail: "Hardware Direct Rendering Manager node is available for 3D graphics."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "gpu".to_string(),
            title: "GPU Acceleration".to_string(),
            status: "warning".to_string(),
            value: "Software Rendering".to_string(),
            detail: "Hardware DRI node not found. Android graphics will fall back to software rendering.".to_string(),
            remediation: Some("Install graphics drivers supporting DRI (Direct Rendering Infrastructure).".to_string()),
        });
    }

    // 5. Waydroid Container Service
    let service_active = if let Some(systemctl) =
        resolve_program("systemctl").or_else(|| trusted_program("systemctl"))
    {
        waydroid::run_program(
            &systemctl,
            &["is-active".to_string(), "waydroid-container".to_string()],
            3,
            None,
        )
        .map(|stdout| stdout.trim() == "active")
        .unwrap_or(false)
    } else {
        false
    };

    if service_active {
        items.push(DiagnosticItem {
            key: "service".to_string(),
            title: "Container Daemon".to_string(),
            status: "ok".to_string(),
            value: "Service Active".to_string(),
            detail: "waydroid-container.service is actively running in background.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "service".to_string(),
            title: "Container Daemon".to_string(),
            status: "info".to_string(),
            value: "Service Inactive".to_string(),
            detail:
                "Container daemon is inactive. It will start automatically when session starts."
                    .to_string(),
            remediation: None,
        });
    }

    // 6. Waydroid CLI binary
    let has_waydroid =
        resolve_program("waydroid").is_some() || trusted_program("waydroid").is_some();
    if has_waydroid {
        items.push(DiagnosticItem {
            key: "waydroid".to_string(),
            title: "Waydroid Package".to_string(),
            status: "ok".to_string(),
            value: "Installed".to_string(),
            detail: "Waydroid command-line tool is available in system PATH.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "waydroid".to_string(),
            title: "Waydroid Package".to_string(),
            status: "error".to_string(),
            value: "Not Installed".to_string(),
            detail: "Waydroid command is missing. It must be installed before starting or initializing containers.".to_string(),
            remediation: pm.map(|p| p.user_install_command(&["waydroid"])),
        });
    }

    // 7. ADB / android-tools
    let has_adb = resolve_program("adb").is_some() || trusted_program("adb").is_some();
    if has_adb {
        items.push(DiagnosticItem {
            key: "adb".to_string(),
            title: "Android Debug Bridge".to_string(),
            status: "ok".to_string(),
            value: "Installed".to_string(),
            detail: "adb is available for unprivileged container queries and inspection."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "adb".to_string(),
            title: "Android Debug Bridge".to_string(),
            status: "warning".to_string(),
            value: "Not Installed".to_string(),
            detail: "adb is missing. App list, install/uninstall, and Android boot detection fall back to slower or unavailable paths.".to_string(),
            remediation: pm.map(|p| p.user_install_command(&["adb"])),
        });
    }

    // 8. Root Elevation Tool (polkit / sudo)
    let has_pkexec = has_program("pkexec");
    let has_sudo = has_program("sudo");
    if has_pkexec && !polkit_agent_running() {
        items.push(DiagnosticItem {
            key: "elevation".to_string(),
            title: "Privilege Elevation".to_string(),
            status: if has_sudo && std::io::stdin().is_terminal() {
                "warning"
            } else {
                "error"
            }
            .to_string(),
            value: "No Polkit Agent".to_string(),
            detail: "pkexec is installed but no polkit authentication agent is running, so the password prompt cannot appear. Start an agent in your session, or launch ArchDroid from a terminal so sudo can ask instead.".to_string(),
            remediation: Some(
                "Start one, e.g. hyprpolkitagent, polkit-gnome-authentication-agent-1, or lxqt-policykit-agent"
                    .to_string(),
            ),
        });
    } else if !has_pkexec && has_sudo && !std::io::stdin().is_terminal() {
        items.push(DiagnosticItem {
            key: "elevation".to_string(),
            title: "Privilege Elevation".to_string(),
            status: "error".to_string(),
            value: "sudo Needs a TTY".to_string(),
            detail: "Only sudo is available and ArchDroid has no terminal to ask for the password. Administrative operations will fail.".to_string(),
            remediation: Some(format!(
                "{}, or start ArchDroid from a terminal",
                pm.map(|p| p.user_install_command(&["polkit"]))
                    .unwrap_or_else(|| "Install polkit and a polkit agent".to_string())
            )),
        });
    } else if let Some(tool) = has_elevation_tool() {
        items.push(DiagnosticItem {
            key: "elevation".to_string(),
            title: "Privilege Elevation".to_string(),
            status: "ok".to_string(),
            value: match tool {
                "pkexec" => "Polkit (pkexec)".to_string(),
                _ => "sudo (terminal)".to_string(),
            },
            detail: format!(
                "Authentication agent '{tool}' is available for administrative operations."
            ),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "elevation".to_string(),
            title: "Privilege Elevation".to_string(),
            status: "error".to_string(),
            value: "Missing".to_string(),
            detail: "Neither pkexec nor sudo was found. Administrative operations will fail."
                .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["polkit"])),
        });
    }

    // 9. Git
    let has_git = resolve_program("git").is_some() || trusted_program("git").is_some();
    if has_git {
        items.push(DiagnosticItem {
            key: "git".to_string(),
            title: "Git Version Control".to_string(),
            status: "ok".to_string(),
            value: "Installed".to_string(),
            detail: "git is available to download pinned third-party helper checkouts.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "git".to_string(),
            title: "Git Version Control".to_string(),
            status: "warning".to_string(),
            value: "Not Installed".to_string(),
            detail: "git is required by GApps, ARM Translation, and Phantom installers."
                .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["git"])),
        });
    }

    // 10. Python 3 & venv
    let (has_py, has_venv) = check_python_venv_available();
    if has_py && has_venv {
        items.push(DiagnosticItem {
            key: "python".to_string(),
            title: "Python 3 & venv".to_string(),
            status: "ok".to_string(),
            value: "Ready".to_string(),
            detail: "Python 3 and venv module are ready for isolated script execution.".to_string(),
            remediation: None,
        });
    } else if has_py {
        items.push(DiagnosticItem {
            key: "python".to_string(),
            title: "Python 3 & venv".to_string(),
            status: "error".to_string(),
            value: "venv Missing".to_string(),
            detail: "Python 3 is present but python3-venv is missing. Debian/Ubuntu systems require python3-venv.".to_string(),
            remediation: pm.map(|p| p.user_install_command(&["python3-venv"])),
        });
    } else {
        items.push(DiagnosticItem {
            key: "python".to_string(),
            title: "Python 3 & venv".to_string(),
            status: "error".to_string(),
            value: "Not Installed".to_string(),
            detail: "Python 3 is required by GApps and ARM Translation installers.".to_string(),
            remediation: pm.map(|p| p.user_install_command(&["python3", "python3-venv"])),
        });
    }

    // 11. squashfs extraction tool
    let has_squash =
        resolve_program("unsquashfs").is_some() || trusted_program("unsquashfs").is_some();
    if has_squash {
        items.push(DiagnosticItem {
            key: "squashfs".to_string(),
            title: "SquashFS Tools".to_string(),
            status: "ok".to_string(),
            value: "unsquashfs Ready".to_string(),
            detail: "unsquashfs is available to unpack Android filesystem images.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "squashfs".to_string(),
            title: "SquashFS Tools".to_string(),
            status: "warning".to_string(),
            value: "Missing".to_string(),
            detail:
                "unsquashfs is required to extract Android images for GApps and ARM Translation."
                    .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["squashfs"])),
        });
    }

    // 12. lzip
    let has_lzip = resolve_program("lzip").is_some() || trusted_program("lzip").is_some();
    if has_lzip {
        items.push(DiagnosticItem {
            key: "lzip".to_string(),
            title: "Lzip Decompressor".to_string(),
            status: "ok".to_string(),
            value: "lzip Ready".to_string(),
            detail: "lzip is available to extract MindTheGapps and libhoudini archives."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "lzip".to_string(),
            title: "Lzip Decompressor".to_string(),
            status: "warning".to_string(),
            value: "Missing".to_string(),
            detail: "lzip is required to decompress archives for GApps and ARM Translation."
                .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["lzip"])),
        });
    }

    // 13. xdg-open
    let has_xdg = resolve_program("xdg-open").is_some() || trusted_program("xdg-open").is_some();
    if has_xdg {
        items.push(DiagnosticItem {
            key: "xdg_open".to_string(),
            title: "File Manager Launcher".to_string(),
            status: "ok".to_string(),
            value: "xdg-open Ready".to_string(),
            detail: "xdg-open is available to open shared storage in your default file manager."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "xdg_open".to_string(),
            title: "File Manager Launcher".to_string(),
            status: "info".to_string(),
            value: "Missing".to_string(),
            detail: "xdg-open is needed to open the Android shared media folder in a file manager."
                .to_string(),
            remediation: pm.map(|p| p.user_install_command(&["xdg-utils"])),
        });
    }

    // 14. Terminal Emulator for Phantom
    let terminal = find_terminal_emulator();
    if let Some(ref term) = terminal {
        items.push(DiagnosticItem {
            key: "terminal".to_string(),
            title: "Terminal Emulator".to_string(),
            status: "ok".to_string(),
            value: term.clone(),
            detail: format!(
                "Terminal emulator '{term}' detected to run Phantom Keymapper interactive shell."
            ),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "terminal".to_string(),
            title: "Terminal Emulator".to_string(),
            status: "info".to_string(),
            value: "Not Detected".to_string(),
            detail: "A desktop terminal emulator (alacritty, foot, kitty, konsole, gnome-terminal) is needed to launch Phantom Keymapper.".to_string(),
            remediation: pm.map(|p| p.user_install_command(&["alacritty"])),
        });
    }

    // 15. Disk Space
    let disk_kb = check_available_disk_space_kb(Path::new("/var/lib/waydroid"));
    if let Some(kb) = disk_kb {
        let (status, detail, remediation) = if kb >= 1_500_000 {
            (
                "ok",
                "Sufficient disk space available for system images and container operations.",
                None,
            )
        } else if kb >= 500_000 {
            (
                "warning",
                "Low disk space (< 1.5 GB). Initializing Waydroid requires ~1.2 GB of free space.",
                Some("Free up at least 1.5 GB on host root or /var filesystem.".to_string()),
            )
        } else {
            (
                "error",
                "Critically low disk space (< 500 MB). Waydroid container operations may fail.",
                Some("Free up disk space immediately on host root or /var filesystem.".to_string()),
            )
        };
        items.push(DiagnosticItem {
            key: "disk".to_string(),
            title: "Available Storage".to_string(),
            status: status.to_string(),
            value: format_disk_space(kb),
            detail: detail.to_string(),
            remediation,
        });
    } else {
        items.push(DiagnosticItem {
            key: "disk".to_string(),
            title: "Available Storage".to_string(),
            status: "info".to_string(),
            value: "Unknown".to_string(),
            detail: "Unable to query available disk space via df.".to_string(),
            remediation: None,
        });
    }

    // 16. Network Connectivity
    let online = check_network_connectivity();
    if online {
        items.push(DiagnosticItem {
            key: "network".to_string(),
            title: "Network Connection".to_string(),
            status: "ok".to_string(),
            value: "Connected".to_string(),
            detail: "Internet connectivity verified. Online downloads (system images, GApps, ARM translation) ready.".to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "network".to_string(),
            title: "Network Connection".to_string(),
            status: "warning".to_string(),
            value: "Offline".to_string(),
            detail: "No internet connection detected. Offline host cannot download container images or third-party packages.".to_string(),
            remediation: Some("Connect your Linux host to the internet to download required system images and packages.".to_string()),
        });
    }

    // 17. Waydroid Android image initialization
    if waydroid::is_initialized() {
        items.push(DiagnosticItem {
            key: "waydroid_image".to_string(),
            title: "Android System Image".to_string(),
            status: "ok".to_string(),
            value: "Initialized".to_string(),
            detail: "The Waydroid Android image is initialized and ready for sessions and add-ons."
                .to_string(),
            remediation: None,
        });
    } else {
        items.push(DiagnosticItem {
            key: "waydroid_image".to_string(),
            title: "Android System Image".to_string(),
            status: "warning".to_string(),
            value: "Not Initialized".to_string(),
            detail: "The Android image has not been initialized. GApps and ARM Translation need an initialized image."
                .to_string(),
            remediation: Some(
                "Run the Setup Wizard, or run: sudo waydroid init".to_string(),
            ),
        });
    }

    DiagnosticsReport {
        items,
        package_manager: pm.map(|p| p.name().to_string()),
    }
}

#[tauri::command]
async fn check_host_diagnostics() -> DiagnosticsReport {
    tauri::async_runtime::spawn_blocking(collect_diagnostics)
        .await
        .unwrap_or_else(|_| DiagnosticsReport {
            items: Vec::new(),
            package_manager: None,
        })
}

#[tauri::command]
async fn waydroid_init(app: AppHandle, approved_packages: Option<Vec<String>>) -> CommandResult {
    if has_elevation_tool().is_none() {
        return AppError::elevation_denied(
            "No root elevation tool (pkexec or sudo) found on host. Cannot initialize Waydroid.",
        )
        .into();
    }

    if !check_network_connectivity() {
        return AppError::command_failed(
            "No internet connection detected. An active internet connection is required to download Waydroid LineageOS system images (~1.2 GB).",
        )
        .into();
    }

    if let Some(kb) = check_available_disk_space_kb(Path::new("/var/lib/waydroid")) {
        if kb < 1_500_000 {
            return AppError::command_failed(format!(
                "Insufficient disk space on host: only {:.1} GB available. Waydroid initialization requires at least 1.5 GB.",
                kb as f64 / 1_048_576.0
            ))
            .into();
        }
    }

    // The privileged script runs with SAFE_PATH, so a waydroid binary in a
    // user-writable directory is not usable after elevation.
    let waydroid_missing = trusted_program("waydroid").is_none();
    let pm = platform::PackageManager::detect();

    // Fast-fail before root elevation if Waydroid is missing on an unsupported distribution
    if waydroid_missing && pm.is_none() {
        return AppError::not_found(
            "waydroid",
            "Waydroid is not installed, and no supported package manager (pacman, apt-get, dnf, zypper) was found on this Linux distribution. Please install 'waydroid' manually using your distribution's package manager before initializing.",
        )
        .into();
    }

    let required: Vec<&'static str> = if waydroid_missing {
        vec!["waydroid"]
    } else {
        Vec::new()
    };
    let approved = match approved_install_packages(
        PackageInstallAction::Init,
        &required,
        pm,
        approved_packages.as_deref(),
    ) {
        Ok(packages) => packages,
        Err(error) => return error.into(),
    };

    let install_snippet = match pm {
        Some(pm) if !approved.is_empty() => format!(
            r#"if ! command -v waydroid > /dev/null; then
    echo "==> Installing approved Waydroid package via {}..."
    {}
fi
if ! command -v waydroid > /dev/null; then
    echo "[ERROR] waydroid is still missing after the approved package installation. Install it manually and retry." >&2
    exit 1
fi
"#,
            pm.name(),
            pm.install_mapped_command(&approved)
        ),
        _ => String::new(),
    };

    let script = format!(
        r#"set -e
echo "[STEP] pkg"
{install_snippet}echo "[STEP] download"
echo "==> Initializing Waydroid (this will download ~1GB of data)..."
if waydroid init; then
    echo ""
    echo "[SUCCESS] Waydroid initialized successfully!"
else
    echo "[ERROR] waydroid init failed. Check your network connection and try again."
    exit 1
fi
"#
    );
    stream_privileged_script(
        app,
        script,
        Completion::ClearInstallMarkers,
        waydroid::TIMEOUT_LONG,
    )
    .await
    .into_command_result()
}

fn gapps_state() -> CommandResult {
    let marker = marker_file("gapps_installed");

    // Method 1: ask the running container. ADB needs no root (unlike
    // `waydroid shell`), so try it first; the root path remains for hosts
    // without android-tools. This is the only authoritative source, so it also
    // tells us when a previously written marker has gone stale.
    if let Some(stdout) = live_shell(&["pm", "list", "packages"]) {
        let has_play_store = stdout.lines().any(|l| l.contains("com.android.vending"));
        let has_play_services = stdout.lines().any(|l| l.contains("com.google.android.gms"));
        let has_play_framework = stdout.lines().any(|l| l.contains("com.google.android.gsf"));

        if has_play_store && has_play_services {
            refresh_marker(&marker);
            return CommandResult::report(
                "installed",
                "Google Play Store and Play Services are installed.",
            );
        }

        // The image no longer carries GApps (e.g. it was re-initialized or wiped
        // outside ArchDroid), so forget the marker instead of trusting it forever.
        if marker.exists() {
            let _ = std::fs::remove_file(&marker);
        }

        if has_play_store || has_play_services || has_play_framework {
            return CommandResult::report(
                "partial",
                "GApps partially installed. Consider reinstalling.",
            );
        }

        return CommandResult::report("not_installed", "GApps is not installed.");
    }

    // Method 2: the container is offline; report what the marker can vouch for.
    if let Some(report) = offline_marker_report(&marker, "GApps") {
        return report;
    }

    // Method 3: Fallback to waydroid app list (works offline but only shows launchable apps)
    if let Some(stdout) = probe(|waydroid| waydroid.app_list()) {
        if stdout.contains("com.android.vending") {
            return CommandResult::report("installed", "Play Store detected via app list.");
        }
    }

    // No evidence of GApps
    CommandResult::report("not_installed", "GApps is not installed.")
}

#[tauri::command]
async fn check_gapps_status() -> CommandResult {
    off_main_thread(gapps_state).await
}

fn houdini_state() -> CommandResult {
    let marker = marker_file("houdini_installed");

    // The container reports the ARM translation layer in its ABI list when it is
    // loaded. Prefer ADB for the same reason as gapps_state().
    if let Some(abilist) = live_shell(&["getprop", "ro.product.cpu.abilist"]) {
        let abilist = abilist.to_lowercase();
        if abilist.contains("arm") {
            refresh_marker(&marker);
            return CommandResult::report("installed", "ARM translation (libhoudini) is active.");
        }
        if marker.exists() {
            let _ = std::fs::remove_file(&marker);
        }
        return CommandResult::report("not_installed", "ARM translation is not active.");
    }

    if let Some(report) = offline_marker_report(&marker, "ARM translation (libhoudini)") {
        return report;
    }

    CommandResult::report("not_installed", "ARM Translation is not installed.")
}

#[tauri::command]
async fn check_houdini_status() -> CommandResult {
    off_main_thread(houdini_state).await
}

/// Google Play decides compatibility from the hardware features the device
/// declares, and the stock Waydroid vendor image omits the multitouch features
/// that games such as Plants vs Zombies 2 require. `pm list features` normally
/// goes through adb, which - unlike `waydroid shell` - needs no root; the shell
/// path is only a fallback for hosts without adb, where it usually needs root.
fn multitouch_state() -> CommandResult {
    let marker = marker_file("multitouch_installed");

    let features = adb::shell_line("pm list features")
        .ok()
        .or_else(|| probe(|waydroid| waydroid.shell(&["pm", "list", "features"])));

    if let Some(output) = features {
        let declared = capabilities::parse_features(&output);
        let missing = capabilities::missing_features(&declared);

        if missing.is_empty() {
            refresh_marker(&marker);
            return CommandResult::report(
                "installed",
                "Android declares the multitouch features Google Play filters on.",
            );
        }
        if capabilities::file_installed() {
            return CommandResult::report(
                "partial",
                "Features are installed but Android has not loaded them yet - restart Waydroid.",
            );
        }
        // Neither Android nor the overlay has the features: the marker is stale.
        let _ = std::fs::remove_file(&marker);
        return CommandResult::report(
            "not_installed",
            &format!(
                "{} multitouch feature(s) missing: {}",
                missing.len(),
                missing.join(", ")
            ),
        );
    }

    // No running session to ask: trust the file and the marker an install left
    // behind, and say that is what this answer is based on.
    if capabilities::file_installed() || marker.exists() {
        return CommandResult::report(
            "partial",
            "Features installed but not checked against a running Android. Start Waydroid for a live check.",
        );
    }

    CommandResult::report("not_installed", "Multitouch features are not declared.")
}

#[tauri::command]
async fn check_multitouch_status() -> CommandResult {
    off_main_thread(multitouch_state).await
}

/// Writes the feature declaration into the Waydroid overlay. Android reads it
/// only at container start, so the reply asks for a restart instead of claiming
/// the features are already live.
#[tauri::command]
async fn install_multitouch(app: AppHandle) -> CommandResult {
    if has_elevation_tool().is_none() {
        return AppError::elevation_denied(
            "No root elevation tool (pkexec or sudo) found on host. Cannot perform administrative operations.",
        )
        .into();
    }
    if let Err(error) = require_initialized_waydroid("declaring multitouch features") {
        return error.into();
    }
    stream_privileged_script(
        app,
        capabilities::install_script(),
        Completion::WriteMarker("multitouch_installed"),
        waydroid::TIMEOUT_NORMAL,
    )
    .await
    .into_command_result_with_message(
        "Multitouch features written to the Waydroid overlay. Restart Waydroid so Android declares them.",
    )
}

/// One dashboard object, assembled from `waydroid status` plus the cheapest
/// probes. Values that need a running container degrade instead of failing:
/// this is a read-only overview, not an action.
#[cfg(feature = "dev-commands")]
#[derive(Serialize, Deserialize)]
pub struct StatusSnapshot {
    pub available: bool,
    pub initialized: bool,
    pub running: bool,
    /// The session is up, but Waydroid suspended the container (no Android app
    /// is active). ADB commands hang while frozen, so the UI must not show this
    /// as "ready" even though the session is running.
    pub frozen: bool,
    pub booted: bool,
    /// `installed | partial | not_installed | error`
    pub gapps: String,
    /// `installed | not_installed | error`
    pub houdini: String,
    pub phantom: bool,
}

#[cfg(feature = "dev-commands")]
fn status_snapshot() -> StatusSnapshot {
    let mut snapshot = StatusSnapshot {
        available: false,
        initialized: false,
        running: false,
        frozen: false,
        booted: false,
        gapps: "unknown".to_string(),
        houdini: "unknown".to_string(),
        phantom: resolve_program("phantom").is_some(),
    };

    if let Ok(waydroid) = waydroid::Waydroid::detect() {
        snapshot.available = true;
        match waydroid.status() {
            Ok(text) => {
                snapshot.initialized = true;
                snapshot.running = waydroid::session_running(&text);
                snapshot.frozen = waydroid::container_frozen(&text);
            }
            // Only "not initialized" means the image is absent; every other
            // failure (d-bus down, container asleep) still leaves a configured image.
            Err(failure) if failure.code() == error::code::NOT_INITIALIZED => {}
            Err(_) => snapshot.initialized = true,
        }
    }

    // A frozen container never answers `adb shell`; asking would block for the
    // whole timeout. The passive probes below return "not now" on their own.
    if snapshot.running && !snapshot.frozen {
        snapshot.booted = adb::booted();
    }
    snapshot.gapps = gapps_state().status;
    snapshot.houdini = houdini_state().status;
    snapshot
}

#[cfg(feature = "dev-commands")]
fn json_result<T: Serialize>(value: &T) -> CommandResult {
    match serde_json::to_string(value) {
        Ok(json) => CommandResult::with_data(json),
        Err(failure) => AppError::Io(std::io::Error::other(failure.to_string())).into(),
    }
}

#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn waydroid_is_running() -> CommandResult {
    off_main_thread(
        || match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.status()) {
            Ok(text) => CommandResult::with_data(waydroid::session_running(&text).to_string()),
            // "Not initialized" simply means: not running.
            Err(failure) if failure.code() == error::code::NOT_INITIALIZED => {
                CommandResult::with_data("false".to_string())
            }
            Err(failure) => failure.into(),
        },
    )
    .await
}

#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn waydroid_status_json() -> CommandResult {
    off_main_thread(|| json_result(&status_snapshot())).await
}

/// Emulator instance info shape. Waydroid is a single container, so the list always
/// has zero or one entry.
#[cfg(feature = "dev-commands")]
#[derive(Serialize, Deserialize)]
pub struct InstanceInfo {
    pub playerid: u32,
    pub name: String,
    pub running: bool,
    pub android_started: bool,
    pub cpu: u32,
    pub memory: u32,
    pub root: bool,
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
}

#[cfg(feature = "dev-commands")]
fn parse_memory_mb(meminfo: &str) -> u32 {
    meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kilobytes| kilobytes.parse::<u64>().ok())
        .map(|kilobytes| (kilobytes / 1024) as u32)
        .unwrap_or(0)
}

#[cfg(feature = "dev-commands")]
fn host_memory_mb() -> u32 {
    parse_memory_mb(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default())
}

// ponytail: Waydroid is one container that inherits host CPU/RAM. `root` is
// true by design (`waydroid shell` is root), and resolution/DPI mirror the
// saved device profile.
#[cfg(feature = "dev-commands")]
fn single_instance(snapshot: &StatusSnapshot, profile: &device::DeviceProfile) -> InstanceInfo {
    InstanceInfo {
        playerid: 0,
        name: "waydroid".to_string(),
        running: snapshot.running,
        android_started: snapshot.booted,
        cpu: std::thread::available_parallelism()
            .map(|parallelism| parallelism.get() as u32)
            .unwrap_or(1),
        memory: host_memory_mb(),
        root: true,
        width: profile.width,
        height: profile.height,
        dpi: profile.dpi,
    }
}

#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn waydroid_list_instances() -> CommandResult {
    off_main_thread(|| {
        let snapshot = status_snapshot();
        let profile = device::load_from(&device::config_path());
        json_result(&vec![single_instance(&snapshot, &profile)])
    })
    .await
}

#[cfg(feature = "dev-commands")]
#[tauri::command]
fn profile_capabilities() -> CommandResult {
    json_result(&device::FIELDS)
}

/// Saved profile overlaid with the values Android actually uses right now
/// (waydroid.cfg on disk, plus `wm size`/`wm density` when the session is up).
#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn profile_get() -> CommandResult {
    off_main_thread(|| {
        let mut profile = device::load_from(&device::config_path());
        if let Ok(cfg) = std::fs::read_to_string(device::WAYDROID_CFG) {
            device::overlay_cfg(&mut profile, &cfg);
        }

        if let Ok(output) = adb::shell_line("wm size; wm density") {
            if let Some((width, height)) = device::parse_wm_size(&output) {
                profile.width = width;
                profile.height = height;
            }
            if let Some(dpi) = device::parse_wm_density(&output) {
                profile.dpi = dpi;
            }
        }
        json_result(&profile)
    })
    .await
}

/// Persist the profile, apply what Android can change live, and wait for the
/// privileged INI write before replying. The root part is streamed through
/// `cmd-output`/`cmd-finished`; a failed write is returned as an error instead
/// of claiming the identity was applied.
#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn profile_apply(app: AppHandle, profile: device::DeviceProfile) -> CommandResult {
    off_main_thread(move || {
        if let Err(message) = device::validate(&profile) {
            return AppError::invalid_input(message).into();
        }
        // Profile writes touch both adb and root-owned files. Hold the same
        // single-flight slot as the streaming installers so two apply calls
        // cannot interleave their writes.
        let task = match TaskGuard::start("profile update") {
            Ok(task) => task,
            Err(error) => return error.into(),
        };
        emit_started(&app, task.id());
        if let Err(error) = device::save_to(&device::config_path(), &profile) {
            return AppError::Io(error).into();
        }

        let mut report = device::ApplyReport {
            unsupported: device::unsupported_fields(),
            ..device::ApplyReport::default()
        };

        if profile.width > 0 {
            let command = format!("wm size {}x{}", profile.width, profile.height);
            match adb::shell_line_active(&command) {
                Ok(_) => report.applied.extend(["width", "height"]),
                Err(_) => report.deferred.extend(["width", "height"]),
            }
        }
        if profile.dpi > 0 {
            match adb::shell_line_active(&format!("wm density {}", profile.dpi)) {
                Ok(_) => report.applied.push("dpi"),
                Err(_) => report.deferred.push("dpi"),
            }
        }
        if !report.deferred.is_empty() {
            report.notes.push(
                "Resolution/DPI need a running session plus adb (android-tools); apply again once both are available."
                    .to_string(),
            );
        }

        let updates = device::property_updates(&profile);
        let read = |path: &str| match std::fs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(String::new()),
            Err(_) => None,
        };

        match read(device::WAYDROID_CFG) {
            Some(old_cfg) => {
                let cfg_changed =
                    device::changed_fields(&device::read_properties(&old_cfg), &updates);

                // `lxc.py` folds `[properties]` into waydroid_base.prop only while
                // the container is being prepared, and that file is what gets copied
                // to /vendor/waydroid.prop. Writing the cfg alone therefore leaves a
                // plain restart with the old identity. An empty or missing base.prop
                // is skipped on purpose: it means the container was never prepared,
                // and Waydroid folds the cfg in itself on the first start.
                let base_prop = read(device::WAYDROID_BASE_PROP).filter(|text| !text.trim().is_empty());
                let mut pending = cfg_changed;
                if let Some(base_prop) = base_prop.as_deref() {
                    for field in device::changed_fields(&device::read_propfile(base_prop), &updates) {
                        if !pending.contains(&field) {
                            pending.push(field);
                        }
                    }
                }

                if !pending.is_empty() {
                    let mut script = device::cfg_write_script(&device::apply_properties(&old_cfg, &updates));
                    if let Some(base_prop) = base_prop.as_deref() {
                        script.push_str(&device::base_prop_write_script(&device::apply_propfile(
                            base_prop, &updates,
                        )));
                    }
                    // This closure already runs on the blocking pool, so wait
                    // for the privileged write here instead of returning first.
                    let cancel = task.cancel_flag();
                    let attempt = run_privileged_script_for_task(
                        &app,
                        task.id(),
                        script,
                        Completion::None,
                        waydroid::TIMEOUT_NORMAL,
                        &cancel,
                    );
                    if !attempt.is_success() {
                        return attempt.into_command_result();
                    }

                    report.pending = pending;
                    report.restart_required = true;
                    report.notes.push(if base_prop.is_some() {
                        "Identity written to waydroid.cfg and waydroid_base.prop - the second file is what Android is fed at boot."
                            .to_string()
                    } else {
                        "Identity written to waydroid.cfg. waydroid_base.prop does not exist yet, so start Waydroid once and its container setup will fold these values in."
                            .to_string()
                    });
                    report.notes.push(
                        "Android only reads these when the container starts, so restart Waydroid and check again - this report cannot confirm the new identity before that."
                            .to_string(),
                    );

                    match adb::shell_line_active(
                        "getprop ro.product.model; getprop ro.product.manufacturer",
                    ) {
                        Ok(output) => {
                            let live = device::parse_getprop_values(&output);
                            if !live.is_empty() {
                                report.notes.push(format!(
                                    "Android reports right now: model={}, manufacturer={}.",
                                    live.first().cloned().unwrap_or_default(),
                                    live.get(1).cloned().unwrap_or_default()
                                ));
                            }
                        }
                        Err(_) => report.notes.push(
                            "The session is not running, so the current identity could not be read back."
                                .to_string(),
                        ),
                    }
                }
            }
            None => report.notes.push(
                "waydroid.cfg is not readable; identity fields were saved but not applied."
                    .to_string(),
            ),
        }

        json_result(&report)
    })
    .await
}

/// `waydroid container restart` needs root, so it goes through the elevation
/// helper and reports through `cmd-output`/`cmd-finished` like the installers.
#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn waydroid_reboot(app: AppHandle) -> CommandResult {
    stream_privileged_script(
        app,
        "waydroid container restart".to_string(),
        Completion::None,
        waydroid::TIMEOUT_NORMAL,
    )
    .await
    .into_command_result()
}

#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn waydroid_kill_app(package: String) -> CommandResult {
    off_main_thread(move || {
        if !adb::is_valid_package(&package) {
            return AppError::invalid_input(
                "That is not a valid Android package name (letters, digits, '.', '_').",
            )
            .into();
        }
        match adb::shell_active(&["am", "force-stop", &package]) {
            Ok(_) => CommandResult::success(),
            Err(failure) => failure.into(),
        }
    })
    .await
}

#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn waydroid_getprop_all() -> CommandResult {
    off_main_thread(|| match adb::shell_active(&["getprop"]) {
        Ok(text) => CommandResult::with_data(text),
        Err(failure) => failure.into(),
    })
    .await
}

/// Arbitrary device shell line (`adb --command` parity). There is no
/// input validation on purpose: running the caller's command is the feature.
#[cfg(feature = "dev-commands")]
#[tauri::command]
async fn waydroid_adb_cmd(command: String) -> CommandResult {
    off_main_thread(move || match adb::shell_line_active(&command) {
        Ok(text) => CommandResult::with_data(text),
        Err(failure) => failure.into(),
    })
    .await
}

fn wipe_script() -> &'static str {
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

#[tauri::command]
async fn waydroid_wipe(app: AppHandle) -> CommandResult {
    if has_elevation_tool().is_none() {
        return AppError::elevation_denied(
            "No root elevation tool (pkexec or sudo) found on host. Cannot perform administrative operations.",
        )
        .into();
    }

    stream_privileged_script(
        app,
        wipe_script().to_string(),
        Completion::ClearInstallMarkers,
        waydroid::TIMEOUT_LONG,
    )
    .await
    .into_command_result()
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

fn gapps_install_script(pm: Option<platform::PackageManager>, approved: &[&str]) -> String {
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

fn houdini_install_script(pm: Option<platform::PackageManager>, approved: &[&str]) -> String {
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

fn phantom_install_script() -> String {
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

#[tauri::command]
async fn install_gapps(app: AppHandle, approved_packages: Option<Vec<String>>) -> CommandResult {
    let pm = platform::PackageManager::detect();
    if let Err(error) = installer_preflight(pm, "installing GApps") {
        return error.into();
    }

    let required = gapps_missing_dependencies();
    let approved = match approved_install_packages(
        PackageInstallAction::Gapps,
        &required,
        pm,
        approved_packages.as_deref(),
    ) {
        Ok(packages) => packages,
        Err(error) => return error.into(),
    };

    stream_privileged_script(
        app,
        gapps_install_script(pm, &approved),
        Completion::WriteMarker("gapps_installed"),
        waydroid::TIMEOUT_LONG,
    )
    .await
    .into_command_result()
}

#[tauri::command]
async fn install_houdini(app: AppHandle, approved_packages: Option<Vec<String>>) -> CommandResult {
    let pm = platform::PackageManager::detect();
    if let Err(error) = installer_preflight(pm, "installing ARM Translation") {
        return error.into();
    }

    let required = gapps_missing_dependencies();
    let approved = match approved_install_packages(
        PackageInstallAction::Houdini,
        &required,
        pm,
        approved_packages.as_deref(),
    ) {
        Ok(packages) => packages,
        Err(error) => return error.into(),
    };

    stream_privileged_script(
        app,
        houdini_install_script(pm, &approved),
        Completion::WriteMarker("houdini_installed"),
        waydroid::TIMEOUT_LONG,
    )
    .await
    .into_command_result()
}

#[tauri::command]
async fn install_phantom(app: AppHandle) -> CommandResult {
    if !check_network_connectivity() {
        return AppError::command_failed(
            "No internet connection detected. Internet access is required to download Phantom source.",
        )
        .into();
    }

    let pm = platform::PackageManager::detect();
    if let Err(error) = phantom_preflight(pm) {
        return error.into();
    }
    if let Err(error) = phantom_disk_preflight() {
        return error.into();
    }

    // Phantom installs into $HOME/.local and its system wrapper is disabled, so
    // it must not run through pkexec/sudo. Running it as the desktop user also
    // avoids executing the user's Rust toolchain as root.
    stream_user_script(app, phantom_install_script(), waydroid::TIMEOUT_LONG)
        .await
        .into_command_result()
}

#[tauri::command]
async fn run_phantom() -> CommandResult {
    off_main_thread(run_phantom_sync).await
}

fn run_phantom_sync() -> CommandResult {
    // 1. Prefer phantom-gui directly without a terminal wrapper
    let gui_bin = resolve_program("phantom-gui")
        .or_else(|| trusted_program("phantom-gui"))
        .or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".local/bin/phantom-gui"))
                .filter(|p| p.is_file())
        });

    if let Some(gui_path) = gui_bin {
        return match Command::new(&gui_path).spawn() {
            Ok(_) => CommandResult::with_message("Launched Phantom Keymapper GUI."),
            Err(err) => {
                AppError::from_spawn("phantom-gui", "Failed to spawn Phantom GUI.", err).into()
            }
        };
    }

    // 2. Fallback to CLI client inside terminal if phantom-gui is missing
    let cli_bin = resolve_program("phantom")
        .or_else(|| trusted_program("phantom"))
        .or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".local/bin/phantom"))
                .filter(|p| p.is_file())
        });

    if let Some(cli_path) = cli_bin {
        let term = match find_terminal_emulator() {
            Some(term) => term,
            None => {
                return AppError::not_found(
                    "terminal emulator",
                    "No supported terminal emulator (alacritty, foot, kitty, konsole, gnome-terminal, xterm) found to run Phantom. Please install a terminal emulator.",
                )
                .into();
            }
        };

        let cli_str = cli_path.to_string_lossy().to_string();
        let result = match term.as_str() {
            "gnome-terminal" => Command::new("gnome-terminal")
                .args(["--", "bash", "-c", &cli_str])
                .spawn(),
            _ => Command::new(&term)
                .args(["-e", "bash", "-c", &cli_str])
                .spawn(),
        };

        return match result {
            Ok(_) => {
                CommandResult::with_message(&format!("Launched Phantom Keymapper via {term}."))
            }
            Err(err) => {
                AppError::from_spawn(&term, "Failed to spawn terminal emulator.", err).into()
            }
        };
    }

    // 3. Neither phantom-gui nor phantom is found
    AppError::not_found(
        "phantom-gui",
        "Phantom Keymapper is not installed. Please click 'Install / Update Phantom' to compile and install it first.",
    )
    .into()
}

/// The single-flight slot. Streaming operations share one terminal, so only
/// one of them may own the slot at a time.
static ACTIVE_TASK: Mutex<Option<ActiveTask>> = Mutex::new(None);
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

struct ActiveTask {
    id: String,
    label: &'static str,
    cancel: Arc<AtomicBool>,
}

/// Holds the single-flight slot until the operation finishes, including on
/// every early return or panic.
struct TaskGuard {
    id: String,
    cancel: Arc<AtomicBool>,
}

impl TaskGuard {
    fn start(label: &'static str) -> Result<Self, AppError> {
        let mut active = ACTIVE_TASK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(current) = active.as_ref() {
            return Err(AppError::busy(current.label));
        }
        let id = format!("task-{}", NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed));
        let cancel = Arc::new(AtomicBool::new(false));
        active.replace(ActiveTask {
            id: id.clone(),
            label,
            cancel: cancel.clone(),
        });
        Ok(Self { id, cancel })
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    #[cfg(test)]
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// Ask the active task to stop, but only when `task_id` still owns the slot.
///
/// A stale ID (the operation finished between the click and this call, or the
/// UI kept an old ID) is a no-op: it must never cancel the task that replaced
/// it. Returns whether a live task matched.
fn cancel_active_task(task_id: &str) -> bool {
    let active = ACTIVE_TASK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match active.as_ref() {
        Some(task) if task.id == task_id => {
            task.cancel.store(true, Ordering::SeqCst);
            true
        }
        _ => false,
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        let mut active = ACTIVE_TASK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.as_ref().is_some_and(|task| task.id == self.id) {
            *active = None;
        }
    }
}

/// Payload for `cmd-started`. The UI stores this ID and sends it back to
/// `cancel_task`, so Cancel can only stop the operation it is showing.
#[derive(Clone, Serialize)]
struct StreamStarted {
    task_id: String,
}

/// Payload for `cmd-output`. The task ID lets the UI ignore output that does
/// not belong to the operation it is currently showing.
#[derive(Clone, Serialize)]
struct StreamOutput {
    task_id: String,
    line: String,
}

/// Final payload for `cmd-finished`. The UI branches on `code`, so it never
/// has to parse the human-readable `message`.
#[derive(Clone, Serialize)]
struct StreamFinished {
    task_id: String,
    success: bool,
    code: Option<&'static str>,
    message: Option<String>,
}

struct StreamResult {
    outcome: Result<(), AppError>,
    /// True when the elevated shell actually started, i.e. elevation worked.
    started: bool,
}

impl StreamResult {
    fn success(started: bool) -> Self {
        Self {
            outcome: Ok(()),
            started,
        }
    }

    fn failure(error: AppError, started: bool) -> Self {
        Self {
            outcome: Err(error),
            started,
        }
    }

    fn is_success(&self) -> bool {
        self.outcome.is_ok()
    }

    fn timed_out(&self) -> bool {
        matches!(&self.outcome, Err(AppError::Timeout { .. }))
    }

    fn cancelled(&self) -> bool {
        matches!(&self.outcome, Err(AppError::CancelRequested { .. }))
    }

    fn finished_event(&self, task_id: &str) -> StreamFinished {
        match &self.outcome {
            Ok(()) => StreamFinished {
                task_id: task_id.to_string(),
                success: true,
                code: None,
                message: None,
            },
            Err(error) => StreamFinished {
                task_id: task_id.to_string(),
                success: false,
                code: Some(error.code()),
                message: Some(error.to_string()),
            },
        }
    }

    fn exit_code(&self) -> Option<i32> {
        match &self.outcome {
            Err(AppError::Exit { code, .. }) => *code,
            _ => None,
        }
    }

    fn into_command_result(self) -> CommandResult {
        match self.outcome {
            Ok(()) => CommandResult::success(),
            Err(error) => error.into(),
        }
    }

    fn into_command_result_with_message(self, message: &str) -> CommandResult {
        match self.outcome {
            Ok(()) => CommandResult::with_message(message),
            Err(error) => error.into(),
        }
    }
}

fn emit_started(app: &AppHandle, task_id: &str) {
    let _ = app.emit(
        "cmd-started",
        StreamStarted {
            task_id: task_id.to_string(),
        },
    );
}

fn emit_output(app: &AppHandle, task_id: &str, line: impl Into<String>) {
    let _ = app.emit(
        "cmd-output",
        StreamOutput {
            task_id: task_id.to_string(),
            line: line.into(),
        },
    );
}

/// Emit a completed line and remember the beginning of the output.
/// Recognise host conditions that make an installer fail half-way and say what
/// to do, instead of leaving the user with the raw tail of pip/git/cp output.
fn failure_hint(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("no space left on device") || lower.contains("disk quota exceeded") {
        Some("The disk filled up during the operation. Free at least 1.5 GB on the filesystem holding /var/lib/waydroid and /tmp, then retry; the add-on is not marked installed.")
    } else if [
        "could not resolve host",
        "temporary failure in name resolution",
        "network is unreachable",
        "connection timed out",
        "connection reset",
        "failed to establish a new connection",
        "unable to access 'https://",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        Some("The network dropped during the download. Check the connection and retry; nothing was marked installed and the retry starts from a clean work directory.")
    } else {
        None
    }
}

fn flush_line(app: &AppHandle, task_id: &str, buffer: &mut String, captured: &Arc<Mutex<String>>) {
    if buffer.is_empty() {
        return;
    }
    emit_output(app, task_id, buffer.clone());
    if let Ok(mut text) = captured.lock() {
        // Past the head budget, still keep lines that explain a failure so the
        // hint survives long installer output.
        if text.len() < MAX_CAPTURED_OUTPUT
            || (text.len() < 2 * MAX_CAPTURED_OUTPUT && failure_hint(buffer).is_some())
        {
            text.push_str(buffer);
            text.push('\n');
        }
    }
    buffer.clear();
}

/// Read a pipe byte by byte so progress bars that use carriage returns (pacman,
/// curl) are streamed as their own lines, and flush the trailing line at EOF.
fn spawn_line_reader(
    app: AppHandle,
    task_id: &str,
    reader: impl Read + Send + 'static,
    captured: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    let task_id = task_id.to_string();
    thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut buffer = String::new();
        let mut byte = [0; 1];
        loop {
            match reader.read(&mut byte) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let c = byte[0] as char;
            if c == '\n' || c == '\r' {
                flush_line(&app, &task_id, &mut buffer, &captured);
            } else {
                buffer.push(c);
            }
        }
        flush_line(&app, &task_id, &mut buffer, &captured);
    })
}

/// Why a child is no longer being waited for.
enum WaitOutcome {
    Exited(ExitStatus),
    TimedOut,
    Cancelled,
}

/// Send `signal` to the process group led by `pid`.
///
/// `kill` is resolved without PATH, like every other helper that runs with more
/// privilege than the caller. `--` stops the negative group ID from being read
/// as an option.
fn signal_process_group(pid: u32, signal: &str) -> bool {
    let Some(kill) = trusted_program("kill") else {
        return false;
    };
    Command::new(kill)
        .args([signal, "--", &format!("-{pid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Stop the child and everything it spawned.
///
/// SIGTERM goes to the whole group first so the elevated shell can run its EXIT
/// trap (`rm -rf "$ARCHDROID_WORK"`); a group that ignores it is SIGKILLed
/// after [`KILL_GRACE`].
// ponytail: process group only; a child that calls setsid() (sudo with
// use_pty, a daemonising installer) escapes. Move to a cgroup/PID namespace
// if that ever becomes a real failure.
fn terminate_process_group(child: &mut Child) {
    let pid = child.id();
    let term_sent = signal_process_group(pid, "-TERM");
    let deadline = Instant::now() + KILL_GRACE;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) => thread::sleep(Duration::from_millis(20)),
        }
    }

    // No trusted `kill`: at least stop the direct child instead of leaking it.
    if !term_sent {
        let _ = child.kill();
    }
    // Sweep the rest of the group too: a child may ignore SIGTERM or outlive
    // the shell that led the group.
    let _ = signal_process_group(pid, "-KILL");
    let _ = child.wait();
}

/// Ask the root-side watcher to stop its own process group. The app cannot
/// signal a root-owned group directly (EPERM).
fn request_privileged_cancel(cancel_path: Option<&Path>) {
    if let Some(path) = cancel_path {
        let _ = std::fs::write(path, "cancel\n");
    }
}

/// Wait for `child`, but stop its process group when the deadline elapses or
/// the task-scoped cancel flag is raised.
fn wait_with_deadline(
    child: &mut Child,
    timeout_secs: u64,
    cancel: &AtomicBool,
    cancel_path: Option<&Path>,
) -> std::io::Result<WaitOutcome> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(WaitOutcome::Exited(status)),
            Ok(None) if cancel.load(Ordering::SeqCst) => {
                // The root-side watcher handles the elevated group; the local
                // kill is the only option for user-level scripts.
                request_privileged_cancel(cancel_path);
                terminate_process_group(child);
                return Ok(WaitOutcome::Cancelled);
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                request_privileged_cancel(cancel_path);
                terminate_process_group(child);
                return Ok(WaitOutcome::TimedOut);
            }
            Err(error) => {
                request_privileged_cancel(cancel_path);
                terminate_process_group(child);
                return Err(error);
            }
        }
    }
}

/// Map a finished wait onto the typed result the UI sees.
///
/// `started` comes from the sentinel line the elevated shell prints first, so
/// "elevation failed" and "the script ran and failed" stay distinguishable
/// without parsing pkexec/sudo messages.
fn stream_outcome(
    program: &str,
    wait: std::io::Result<WaitOutcome>,
    captured_text: String,
    timeout_secs: u64,
    emit_error: impl FnOnce(String),
) -> StreamResult {
    let started = captured_text.contains(ELEVATED_SENTINEL);
    match wait {
        Ok(WaitOutcome::Exited(status)) if status.success() => StreamResult::success(started),
        Ok(WaitOutcome::Exited(status)) => {
            if let Some(hint) = failure_hint(&captured_text) {
                emit_error(format!("[HINT] {hint}"));
            }
            StreamResult::failure(
                AppError::Exit {
                    program: program.to_string(),
                    code: status.code(),
                    output: captured_text,
                },
                started,
            )
        }
        Ok(WaitOutcome::TimedOut) => {
            let error = AppError::Timeout {
                program: program.to_string(),
                seconds: timeout_secs,
            };
            emit_error(format!("[ERROR] {error}"));
            StreamResult::failure(error, started)
        }
        Ok(WaitOutcome::Cancelled) => {
            let error = AppError::CancelRequested {
                program: program.to_string(),
            };
            emit_error(format!("[ERROR] {error}"));
            StreamResult::failure(error, started)
        }
        Err(error) => {
            emit_error(format!(
                "[ERROR] Lost track of the running command: {error}"
            ));
            StreamResult::failure(AppError::Io(error), started)
        }
    }
}

/// Spawn a command, stream both of its pipes to the UI and report how it went.
fn run_and_stream(
    app: &AppHandle,
    task_id: &str,
    mut command: Command,
    timeout_secs: u64,
    cancel: &AtomicBool,
    cancel_path: Option<&Path>,
) -> StreamResult {
    let program = command.get_program().to_string_lossy().to_string();
    // The UI can press Cancel between `cmd-started` and this spawn. Do not open
    // a polkit/sudo prompt for a task the user already stopped.
    if cancel.load(Ordering::SeqCst) {
        let error = AppError::CancelRequested {
            program: program.clone(),
        };
        emit_output(app, task_id, format!("[ERROR] {error}"));
        return StreamResult::failure(error, false);
    }
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Own process group: a deadline can then stop pkexec/sudo, bash and
        // everything they spawned together instead of just the direct child.
        .process_group(0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            let error =
                AppError::from_spawn(&program, "Install the missing program and try again.", e);
            log::error!(
                "{program} could not start: {}",
                redact_text(&error.to_string())
            );
            emit_output(app, task_id, format!("[ERROR] {error}"));
            return StreamResult::failure(error, false);
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let captured = Arc::new(Mutex::new(String::new()));
    let stdout_reader = spawn_line_reader(app.clone(), task_id, stdout, captured.clone());
    let stderr_reader = spawn_line_reader(app.clone(), task_id, stderr, captured.clone());

    let wait = wait_with_deadline(&mut child, timeout_secs, cancel, cancel_path);
    if let Some(path) = cancel_path {
        let _ = std::fs::remove_file(path);
    }

    let _ = stdout_reader.join();
    let _ = stderr_reader.join();

    let captured_text = captured.lock().map(|text| text.clone()).unwrap_or_default();
    let outcome = stream_outcome(&program, wait, captured_text, timeout_secs, |line| {
        emit_output(app, task_id, line)
    });
    if outcome.is_success() {
        log::info!("{program} finished");
    } else if let Err(error) = &outcome.outcome {
        log::warn!("{program} failed: {}", redact_text(&error.to_string()));
    }
    outcome
}

/// Markers are written by the unprivileged app process *after* a root script
/// exits successfully. Root never writes into the user cache, so a symlink
/// planted under `~/.cache` cannot redirect a root-owned write.
#[derive(Clone, Copy)]
enum Completion {
    None,
    WriteMarker(&'static str),
    ClearInstallMarkers,
}

impl Completion {
    /// Returns a user-visible warning if the marker could not be written.
    fn apply(self) -> Option<String> {
        match self {
            Self::None => None,
            Self::WriteMarker(name) => write_marker(&marker_file(name)).err().map(|error| {
                format!("[WARNING] Installed, but could not record it in the cache ({error}); status may show \"not installed\" until the next live check.")
            }),
            Self::ClearInstallMarkers => {
                for name in [
                    "gapps_installed",
                    "houdini_installed",
                    "multitouch_installed",
                ] {
                    let _ = std::fs::remove_file(marker_file(name));
                }
                None
            }
        }
    }
}

/// Apply a completion marker only after a successful run. Timeout and cancel
/// are failures, so they must never leave an "installed" marker behind.
fn apply_completion(attempt: &StreamResult, completion: Completion) -> Option<String> {
    if attempt.is_success() {
        completion.apply()
    } else {
        // A reinstall that got root and then failed may have left the add-on
        // half-replaced, so an older marker no longer vouches for anything.
        // A refused prompt touched nothing and keeps it.
        if let (true, Completion::WriteMarker(name)) = (attempt.started, completion) {
            let _ = std::fs::remove_file(marker_file(name));
        }
        None
    }
}

fn finish_stream(app: &AppHandle, task_id: &str, attempt: &StreamResult, completion: Completion) {
    if let Some(warning) = apply_completion(attempt, completion) {
        emit_output(app, task_id, warning);
    }
    let _ = app.emit("cmd-finished", attempt.finished_event(task_id));
}

/// Run a user-level script. Phantom installs into `$HOME/.local`, so it does
/// not need root at all; keeping it out of the privileged path also avoids
/// running a user-controlled Rust toolchain as root.
fn run_user_script(
    app: &AppHandle,
    task_id: &str,
    script: String,
    timeout_secs: u64,
    cancel: &AtomicBool,
) -> StreamResult {
    log::info!("user script requested ({} lines)", script.lines().count());
    let Some(shell) = trusted_program("bash") else {
        let error = AppError::not_found(
            "bash",
            "Install bash or start ArchDroid from a normal desktop session.",
        );
        emit_output(app, task_id, format!("[ERROR] {error}"));
        let attempt = StreamResult::failure(error, false);
        finish_stream(app, task_id, &attempt, Completion::None);
        return attempt;
    };
    let mut command = Command::new(shell);
    command
        .arg("--noprofile")
        .arg("--norc")
        .arg("-c")
        .arg(&script);
    let attempt = run_and_stream(app, task_id, command, timeout_secs, cancel, None);
    finish_stream(app, task_id, &attempt, Completion::None);
    attempt
}

/// Await the user-level script so the command only resolves after it exits.
async fn stream_user_script(app: AppHandle, script: String, timeout_secs: u64) -> StreamResult {
    let task = match TaskGuard::start("user script") {
        Ok(task) => task,
        Err(error) => return StreamResult::failure(error, false),
    };
    let task_id = task.id().to_string();
    emit_started(&app, &task_id);
    let cancel = task.cancel_flag();
    let fallback_app = app.clone();
    let fallback_task_id = task_id.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        let _task = task;
        run_user_script(&app, &task_id, script, timeout_secs, &cancel)
    })
    .await
    {
        Ok(attempt) => attempt,
        Err(error) => {
            let error = AppError::Io(std::io::Error::other(format!(
                "user script task failed: {error}"
            )));
            emit_output(&fallback_app, &fallback_task_id, format!("[ERROR] {error}"));
            let attempt = StreamResult::failure(error, false);
            finish_stream(&fallback_app, &fallback_task_id, &attempt, Completion::None);
            attempt
        }
    }
}

/// What to do after the polkit attempt.
#[derive(Debug, PartialEq, Eq)]
enum ElevationStep {
    /// The attempt is final: report it and never open a sudo prompt.
    Stop,
    /// Elevation never started, so sudo may still work.
    FallBackToSudo,
}

/// Decide whether a failed polkit attempt may still try sudo.
///
/// Cancel, a dismissed dialog (exit 126) and a deadline are terminal. A script
/// that actually started keeps its own exit result. Only an attempt that never
/// started is allowed to fall back.
fn pkexec_step(attempt: &mut StreamResult, forced: &str) -> ElevationStep {
    if attempt.cancelled() {
        return ElevationStep::Stop;
    }

    if attempt.exit_code() == Some(126) {
        let started = attempt.started;
        *attempt = StreamResult::failure(
            AppError::elevation_denied(
                "The polkit authentication dialog was dismissed; nothing was changed.",
            ),
            started,
        );
        return ElevationStep::Stop;
    }

    if attempt.timed_out() {
        return ElevationStep::Stop;
    }

    if attempt.started || forced == "pkexec" {
        if !attempt.started && !attempt.is_success() {
            *attempt = StreamResult::failure(
                AppError::elevation_denied(
                    "polkit could not elevate here; no privileged shell was started.",
                ),
                false,
            );
        }
        return ElevationStep::Stop;
    }

    ElevationStep::FallBackToSudo
}

/// Replace "sudo never elevated" with a typed elevation error.
///
/// A script that started and exited non-zero keeps its exit result, so a failed
/// installer is not misreported as a failed password prompt.
fn sudo_failure(attempt: StreamResult) -> Result<StreamResult, AppError> {
    if attempt.is_success() || attempt.started || attempt.timed_out() || attempt.cancelled() {
        return Ok(attempt);
    }
    Err(AppError::elevation_denied(
        "sudo could not elevate or the privileged shell never started. Install a polkit authentication agent (for example hyprpolkitagent or polkit-gnome), or start ArchDroid from a terminal so sudo can ask for the password.",
    ))
}

/// Run a script that needs root and wait for it to exit.
///
/// The script body is written as if it were already root: there are no `sudo`
/// calls inside. `elevate_env` supplies a fixed PATH, a passwd-derived HOME and
/// a private work directory, regardless of what polkit/sudo passed through.
///
/// Elevation is tried through polkit first, because a polkit password dialog is
/// drawn by a desktop agent and therefore also works when ArchDroid was started
/// from an app launcher. `sudo` remains the fallback because it can still prompt
/// when the app was started from a terminal. Force either one with
/// `ARCHDROID_ELEVATE=pkexec|sudo`.
fn run_privileged_script_for_task(
    app: &AppHandle,
    task_id: &str,
    script: String,
    completion: Completion,
    timeout_secs: u64,
    cancel: &AtomicBool,
) -> StreamResult {
    log::info!("root script requested ({} lines)", script.lines().count());
    // A user-owned path is the only out-of-band signal that works both ways:
    // the app can create it without root, and the root watcher can see it.
    let cancel_path = std::env::temp_dir().join(format!(
        "archdroid-cancel-{}-{}",
        std::process::id(),
        task_id
    ));
    let _ = std::fs::remove_file(&cancel_path);
    let body = match elevate_env(&script) {
        Ok(body) => format!(
            "ARCHDROID_CANCEL_FILE={}\n{}",
            sh_quote(&cancel_path.to_string_lossy()),
            body
        ),
        Err(error) => {
            emit_output(app, task_id, format!("[ERROR] {error}"));
            let attempt = StreamResult::failure(error, false);
            finish_stream(app, task_id, &attempt, completion);
            return attempt;
        }
    };
    let forced = std::env::var("ARCHDROID_ELEVATE").unwrap_or_default();
    let mut pkexec_failed = false;

    if forced != "sudo" {
        if let Some(command) = elevated_command("pkexec", &body) {
            emit_output(
                app,
                task_id,
                "[SYSTEM] Requesting root access via polkit...",
            );
            let mut attempt = run_and_stream(
                app,
                task_id,
                command,
                timeout_secs,
                cancel,
                Some(&cancel_path),
            );
            match pkexec_step(&mut attempt, &forced) {
                ElevationStep::Stop => {
                    finish_stream(app, task_id, &attempt, completion);
                    return attempt;
                }
                ElevationStep::FallBackToSudo => {
                    pkexec_failed = true;
                    emit_output(
                        app,
                        task_id,
                        "[SYSTEM] polkit could not elevate here, falling back to sudo...",
                    );
                }
            }
        } else if forced == "pkexec" {
            let error = AppError::elevation_denied(
                "ARCHDROID_ELEVATE=pkexec but pkexec is not available, so root access cannot be requested.",
            );
            emit_output(app, task_id, format!("[ERROR] {error}"));
            let attempt = StreamResult::failure(error, false);
            finish_stream(app, task_id, &attempt, completion);
            return attempt;
        }
    }

    let Some(command) = elevated_command("sudo", &body) else {
        let detail = if forced == "sudo" {
            "ARCHDROID_ELEVATE=sudo but sudo is not available, so root access cannot be requested."
        } else if pkexec_failed {
            "polkit could not elevate and sudo is not available, so root access cannot be requested."
        } else {
            "Neither pkexec nor sudo is available, so root access cannot be requested."
        };
        let error = AppError::elevation_denied(detail);
        emit_output(app, task_id, format!("[ERROR] {error}"));
        let attempt = StreamResult::failure(error, false);
        finish_stream(app, task_id, &attempt, completion);
        return attempt;
    };

    emit_output(app, task_id, "[SYSTEM] Requesting root access via sudo...");
    let attempt = run_and_stream(
        app,
        task_id,
        command,
        timeout_secs,
        cancel,
        Some(&cancel_path),
    );

    let attempt = match sudo_failure(attempt) {
        Ok(attempt) => attempt,
        Err(error) => {
            emit_output(app, task_id, "[SYSTEM] Root access failed. Install a polkit authentication agent (for example hyprpolkitagent or polkit-gnome), or start ArchDroid from a terminal so sudo can ask for the password.");
            StreamResult::failure(error, false)
        }
    };

    finish_stream(app, task_id, &attempt, completion);
    attempt
}

/// Await the privileged script so the command only resolves after it exits.
async fn stream_privileged_script(
    app: AppHandle,
    script: String,
    completion: Completion,
    timeout_secs: u64,
) -> StreamResult {
    let task = match TaskGuard::start("privileged operation") {
        Ok(task) => task,
        Err(error) => return StreamResult::failure(error, false),
    };
    let task_id = task.id().to_string();
    emit_started(&app, &task_id);
    let cancel = task.cancel_flag();
    let fallback_app = app.clone();
    let fallback_task_id = task_id.clone();
    let fallback_completion = completion;
    match tauri::async_runtime::spawn_blocking(move || {
        let _task = task;
        run_privileged_script_for_task(&app, &task_id, script, completion, timeout_secs, &cancel)
    })
    .await
    {
        Ok(attempt) => attempt,
        Err(error) => {
            let error = AppError::Io(std::io::Error::other(format!(
                "privileged script task failed: {error}"
            )));
            emit_output(&fallback_app, &fallback_task_id, format!("[ERROR] {error}"));
            let attempt = StreamResult::failure(error, false);
            finish_stream(
                &fallback_app,
                &fallback_task_id,
                &attempt,
                fallback_completion,
            );
            attempt
        }
    }
}

pub(crate) fn sanitize_appimage_env() {
    // If running inside an AppImage, AppRun unconditionally sets PYTHONHOME and
    // PYTHONPATH pointing into the bundle's AppDir. Because the bundle does not
    // ship a full Python runtime, any host tools written in Python (like /usr/bin/waydroid)
    // fail immediately with "Fatal Python error: Failed to import encodings module".
    let in_appimage =
        std::env::var_os("APPDIR").is_some() || std::env::var_os("APPIMAGE").is_some();
    if in_appimage {
        if let Ok(pyhome) = std::env::var("PYTHONHOME") {
            if std::env::var("APPDIR")
                .map(|d| pyhome.starts_with(&d))
                .unwrap_or(false)
                || pyhome.contains("appimage")
            {
                std::env::remove_var("PYTHONHOME");
            }
        }
        if let Ok(pypath) = std::env::var("PYTHONPATH") {
            if std::env::var("APPDIR")
                .map(|d| pypath.starts_with(&d))
                .unwrap_or(false)
                || pypath.contains("appimage")
            {
                std::env::remove_var("PYTHONPATH");
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    sanitize_appimage_env();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Defaults already log to stdout and `<app log dir>/<app name>.log`
        // (40 KB, keep-one rotation); Info keeps chatty deps out of the trace.
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        // Only commands the UI actually calls are registered. Backend commands
        // without a UI live behind `dev-commands` so a production build cannot
        // reach arbitrary Android shell or root-writing IPC; re-register one
        // unconditionally only together with its UI and test.
        .invoke_handler(tauri::generate_handler![
            waydroid_status,
            app_log_read,
            app_log_location,
            app_log_clear,
            #[cfg(feature = "dev-commands")]
            waydroid_status_json,
            #[cfg(feature = "dev-commands")]
            waydroid_is_running,
            #[cfg(feature = "dev-commands")]
            waydroid_list_instances,
            #[cfg(feature = "dev-commands")]
            profile_capabilities,
            #[cfg(feature = "dev-commands")]
            profile_get,
            #[cfg(feature = "dev-commands")]
            profile_apply,
            #[cfg(feature = "dev-commands")]
            waydroid_reboot,
            #[cfg(feature = "dev-commands")]
            waydroid_kill_app,
            #[cfg(feature = "dev-commands")]
            waydroid_getprop_all,
            #[cfg(feature = "dev-commands")]
            waydroid_adb_cmd,
            #[cfg(feature = "dev-commands")]
            waydroid_version,
            waydroid_start,
            waydroid_stop,
            waydroid_init,
            waydroid_wipe,
            get_waydroid_apps,
            waydroid_install_apk,
            waydroid_uninstall_app,
            waydroid_launch_app,
            waydroid_prop_set,
            waydroid_prop_get,
            cancel_task,
            waydroid_log,
            open_shared_folder,
            check_gapps_status,
            check_houdini_status,
            check_multitouch_status,
            install_gapps,
            install_houdini,
            install_multitouch,
            install_phantom,
            run_phantom,
            check_host_diagnostics,
            preview_package_install
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apk_check_rejects_text_and_manifestless_zips() {
        let dir = std::env::temp_dir().join(format!("archdroid-apk-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let text = dir.join("text.apk");
        std::fs::write(&text, "this is not an APK\n").unwrap();
        let error = apk_looks_installable(text.to_str().unwrap()).unwrap_err();
        assert_eq!(error.code(), error::code::INVALID_INPUT);
        assert!(error.to_string().contains("not an APK/ZIP"));

        let manifestless = dir.join("manifestless.apk");
        std::fs::write(&manifestless, b"PK\x03\x04notes.txt").unwrap();
        let error = apk_looks_installable(manifestless.to_str().unwrap()).unwrap_err();
        assert_eq!(error.code(), error::code::INVALID_INPUT);
        assert!(error.to_string().contains("without AndroidManifest.xml"));

        let apk = dir.join("ok.apk");
        std::fs::write(&apk, b"PK\x03\x04AndroidManifest.xml").unwrap();
        apk_looks_installable(apk.to_str().unwrap()).unwrap();

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn mapped_dependencies(pm: platform::PackageManager) -> Vec<&'static str> {
        pm.mapped_packages(&["git", "python3", "python3-venv", "squashfs", "lzip"])
    }

    #[test]
    fn sh_quote_wraps_and_escapes_single_quotes() {
        assert_eq!(sh_quote("/home/wdg"), "'/home/wdg'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }

    #[cfg(feature = "dev-commands")]
    #[test]
    fn meminfo_total_becomes_megabytes() {
        assert_eq!(
            parse_memory_mb("MemTotal:       32768000 kB\nMemFree: 1024 kB"),
            32000
        );
        assert_eq!(parse_memory_mb(""), 0);
        assert_eq!(parse_memory_mb("MemTotal: not-a-number kB"), 0);
    }

    #[test]
    fn markers_live_in_the_app_cache_dir() {
        let marker = marker_file("gapps_installed");
        assert_eq!(marker.file_name().unwrap(), "gapps_installed");
        assert_eq!(marker.parent().unwrap().file_name().unwrap(), CACHE_SUBDIR);
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
        assert!(body.contains(
            "trap 'kill \"$ARCHDROID_CANCEL_WATCH\" 2>/dev/null; rm -rf \"$ARCHDROID_WORK\" /tmp/gapps /tmp/houdiniunpack /tmp/waydroid'"
        ));
        assert!(body.contains(ELEVATED_SENTINEL));
        assert!(body.trim_end().ends_with("echo body"));
    }

    #[test]
    fn markers_are_written_by_the_unprivileged_app_process() {
        let root =
            std::env::temp_dir().join(format!("archdroid-marker-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let marker = root.join("gapps_installed");

        write_marker(&marker).unwrap();

        assert_eq!(
            std::fs::read_to_string(&marker)
                .expect("the app must be able to read its own marker")
                .trim(),
            "pending"
        );
        let _ = std::fs::remove_dir_all(&root);
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
    fn package_preview_lists_distro_names_and_the_exact_command() {
        let missing = ["git", "python3", "python3-venv", "squashfs", "lzip"];
        let pacman = package_install_preview(
            PackageInstallAction::Gapps,
            &missing,
            Some(platform::PackageManager::Pacman),
        );
        assert_eq!(pacman.action, "gapps");
        assert_eq!(pacman.package_manager.as_deref(), Some("pacman"));
        assert_eq!(
            pacman.packages,
            vec!["git", "python", "squashfs-tools", "lzip"]
        );
        assert_eq!(
            pacman.command.as_deref(),
            Some("pacman -S --needed --noconfirm git python squashfs-tools lzip")
        );
        assert!(pacman.requires_root);

        let apt = package_install_preview(
            PackageInstallAction::Houdini,
            &missing,
            Some(platform::PackageManager::Apt),
        );
        assert_eq!(
            apt.packages,
            vec!["git", "python3", "python3-venv", "squashfs-tools", "lzip"]
        );
        assert!(apt
            .command
            .as_deref()
            .unwrap()
            .contains("apt-get install -y --no-install-recommends"));

        let unsupported = package_install_preview(PackageInstallAction::Init, &["waydroid"], None);
        assert_eq!(unsupported.packages, vec!["waydroid"]);
        assert_eq!(unsupported.command, None);
        assert_eq!(unsupported.package_manager, None);
    }

    #[test]
    fn package_approval_requires_every_reviewed_package() {
        let required = ["git", "python3", "python3-venv", "squashfs", "lzip"];
        let pm = Some(platform::PackageManager::Pacman);

        let missing = approved_install_packages(PackageInstallAction::Gapps, &required, pm, None)
            .unwrap_err();
        assert_eq!(missing.code(), error::code::APPROVAL_REQUIRED);

        let partial = vec!["git".to_string()];
        let partial_error =
            approved_install_packages(PackageInstallAction::Gapps, &required, pm, Some(&partial))
                .unwrap_err();
        assert_eq!(partial_error.code(), error::code::APPROVAL_REQUIRED);
        assert!(partial_error.to_string().contains("python"));

        let reviewed = platform::PackageManager::Pacman.mapped_packages(&required);
        let reviewed: Vec<String> = reviewed.iter().map(|name| name.to_string()).collect();
        assert_eq!(
            approved_install_packages(PackageInstallAction::Gapps, &required, pm, Some(&reviewed))
                .unwrap(),
            vec!["git", "python", "squashfs-tools", "lzip"]
        );

        let mut smuggled = reviewed.clone();
        smuggled.push("bash".to_string());
        let smuggled_error =
            approved_install_packages(PackageInstallAction::Gapps, &required, pm, Some(&smuggled))
                .unwrap_err();
        assert_eq!(smuggled_error.code(), error::code::INVALID_INPUT);
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
    fn package_preview_rejects_unknown_actions() {
        let result = tauri::async_runtime::block_on(preview_package_install("bogus".to_string()))
            .unwrap_err();
        assert_eq!(result.code.as_deref(), Some(error::code::INVALID_INPUT));
        assert!(result.message.unwrap().contains("init, gapps, or houdini"));
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

    /// The UI reads `status`, `data` and `message`; `code` is the addition that
    /// lets it stop matching prose.
    #[test]
    fn a_typed_error_becomes_the_wire_contract_the_ui_reads() {
        let result: CommandResult =
            AppError::not_initialized("Waydroid is not initialized, run \"waydroid init\"").into();
        assert_eq!(result.status, "error");
        assert_eq!(result.code.as_deref(), Some(error::code::NOT_INITIALIZED));
        assert!(result.data.is_none());
        assert!(result.message.unwrap().contains("waydroid init"));
    }

    #[test]
    fn success_reports_no_error_code() {
        let result = CommandResult::with_data("Session:\tRUNNING".to_string());
        assert_eq!(result.status, "success");
        assert_eq!(result.code, None);
        assert_eq!(result.data.as_deref(), Some("Session:\tRUNNING"));
    }

    #[test]
    fn a_successful_stream_maps_to_a_success_result_only_after_it_exits() {
        let result = StreamResult::success(true).into_command_result_with_message("written");
        assert_eq!(result.status, "success");
        assert_eq!(result.message.as_deref(), Some("written"));
        assert_eq!(result.code, None);
    }

    #[test]
    fn a_failed_stream_maps_to_a_typed_error_result() {
        let result = StreamResult::failure(
            AppError::elevation_denied("The polkit authentication dialog was dismissed."),
            false,
        )
        .into_command_result();
        assert_eq!(result.status, "error");
        assert_eq!(result.code.as_deref(), Some(error::code::ELEVATION_DENIED));
        assert!(result.message.unwrap().contains("dismissed"));
    }

    #[test]
    fn stream_exit_codes_stay_available_for_the_polkit_dismissal_path() {
        let attempt = StreamResult::failure(
            AppError::Exit {
                program: "pkexec".to_string(),
                code: Some(126),
                output: String::new(),
            },
            false,
        );
        assert!(!attempt.is_success());
        assert_eq!(attempt.exit_code(), Some(126));
    }

    /// `ACTIVE_TASK` is process-global, so tests that manipulate the slot must
    /// not run concurrently with each other.
    static TASK_SLOT_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_task_slot_for_test() -> std::sync::MutexGuard<'static, ()> {
        TASK_SLOT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn single_flight_allows_only_one_active_task() {
        let _slot = lock_task_slot_for_test();
        let first = TaskGuard::start("first task").expect("the slot must start empty");
        let first_id = first.id().to_string();
        assert!(first_id.starts_with("task-"));

        let error = match TaskGuard::start("second task") {
            Ok(_) => panic!("a second task must not acquire the active slot"),
            Err(error) => error,
        };
        assert_eq!(error.code(), error::code::BUSY);
        assert!(error.to_string().contains("first task"), "{error}");

        drop(first);
        let next = TaskGuard::start("third task").expect("dropping must release the slot");
        assert_ne!(first_id, next.id());
    }

    /// Every non-streaming command that changes Waydroid state goes through
    /// `run_exclusive`, so a wipe/init/installer holding the slot makes them
    /// fail with `busy` instead of racing the image.
    #[test]
    fn state_changing_commands_are_rejected_while_a_privileged_task_runs() {
        let _slot = lock_task_slot_for_test();
        let privileged =
            TaskGuard::start("privileged operation").expect("the slot must start empty");

        for label in [
            "session start",
            "session stop",
            "APK install",
            "app uninstall",
            "property update",
            "app launch",
        ] {
            let result =
                tauri::async_runtime::block_on(run_exclusive(label, |_| CommandResult::success()));
            assert_eq!(result.status, "error", "{label} must be rejected");
            assert_eq!(
                result.code.as_deref(),
                Some(error::code::BUSY),
                "{label} must report busy"
            );
            assert!(
                result
                    .message
                    .as_deref()
                    .unwrap_or("")
                    .contains("privileged operation"),
                "{label} must name the operation that owns the slot"
            );
        }

        drop(privileged);
        let result = tauri::async_runtime::block_on(run_exclusive("session start", |_| {
            CommandResult::success()
        }));
        assert_eq!(result.status, "success", "the slot must be free again");
        assert_eq!(result.code, None);
    }

    #[test]
    fn cancel_only_matches_the_task_that_owns_the_slot() {
        let _slot = lock_task_slot_for_test();
        assert!(!cancel_active_task("task-none"), "no task must be a no-op");

        let task = TaskGuard::start("cancellable task").expect("the slot must start empty");
        let task_id = task.id().to_string();
        assert!(!task.is_cancelled());

        assert!(!cancel_active_task("task-999999"));
        assert!(
            !task.is_cancelled(),
            "a stale ID must not cancel the active task"
        );

        assert!(cancel_active_task(&task_id));
        assert!(task.is_cancelled(), "the matching task must be cancelled");
    }

    #[test]
    fn a_stale_cancel_cannot_reach_the_next_task() {
        let _slot = lock_task_slot_for_test();
        let first = TaskGuard::start("first task").expect("the slot must start empty");
        let stale_id = first.id().to_string();
        drop(first);

        let second = TaskGuard::start("second task").expect("the slot must be reusable");
        assert!(!cancel_active_task(&stale_id));
        assert!(
            !second.is_cancelled(),
            "the replacement task must stay untouched"
        );
        assert!(cancel_active_task(second.id()));
        assert!(second.is_cancelled());
    }

    fn wait_for_pid_file(path: &Path) -> i32 {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(pid) = text.trim().parse() {
                    return pid;
                }
            }
            assert!(Instant::now() < deadline, "pid file was never written");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn process_is_gone(pid: i32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let alive = Command::new("kill")
                .args(["-0", &pid.to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if !alive {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// A deadline must stop the shell, the child it spawned, and let the EXIT
    /// trap clean up before the group is SIGKILLed.
    #[test]
    fn a_script_past_its_deadline_is_killed_with_its_children_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!(
            "archdroid-deadline-{}-{}",
            std::process::id(),
            NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let work = dir.join("work");
        let pid_file = dir.join("grandchild.pid");
        std::fs::create_dir_all(&work).expect("test workdir must be creatable");

        let mut command = Command::new("bash");
        command
            .args([
                "--noprofile",
                "--norc",
                "-c",
                r#"trap 'rm -rf "$WORK"' EXIT TERM
sleep 300 &
echo $! > "$PID_FILE"
wait
"#,
            ])
            .env("WORK", &work)
            .env("PID_FILE", &pid_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = command.spawn().expect("bash must start");

        let grandchild = wait_for_pid_file(&pid_file);
        let started = Instant::now();
        let cancel = AtomicBool::new(false);
        let outcome =
            wait_with_deadline(&mut child, 1, &cancel, None).expect("waiting must not fail");

        assert!(matches!(outcome, WaitOutcome::TimedOut));
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the deadline was not enforced"
        );
        assert!(!work.exists(), "the EXIT trap must run before SIGKILL");
        assert!(
            process_is_gone(grandchild),
            "the child spawned by the script must not be orphaned"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Cancel uses the same group termination as a deadline, so it must also
    /// stop grandchildren and let the EXIT trap clean the work directory.
    #[test]
    fn a_cancelled_script_is_killed_with_its_children_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!(
            "archdroid-cancel-{}-{}",
            std::process::id(),
            NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let work = dir.join("work");
        let pid_file = dir.join("grandchild.pid");
        std::fs::create_dir_all(&work).expect("test workdir must be creatable");

        let mut command = Command::new("bash");
        command
            .args([
                "--noprofile",
                "--norc",
                "-c",
                r#"trap 'rm -rf "$WORK"' EXIT TERM
sleep 300 &
echo $! > "$PID_FILE"
wait
"#,
            ])
            .env("WORK", &work)
            .env("PID_FILE", &pid_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = command.spawn().expect("bash must start");

        let grandchild = wait_for_pid_file(&pid_file);
        let cancel = AtomicBool::new(true);
        let started = Instant::now();
        let outcome =
            wait_with_deadline(&mut child, 30, &cancel, None).expect("waiting must not fail");

        assert!(matches!(outcome, WaitOutcome::Cancelled));
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "cancel was not enforced"
        );
        assert!(!work.exists(), "the EXIT trap must run before SIGKILL");
        assert!(
            process_is_gone(grandchild),
            "the child spawned by the script must not be orphaned"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_script_that_finishes_before_its_deadline_keeps_its_exit_code() {
        let mut command = Command::new("bash");
        command
            .args(["--noprofile", "--norc", "-c", "exit 7"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = command.spawn().expect("bash must start");

        let cancel = AtomicBool::new(false);
        match wait_with_deadline(&mut child, 30, &cancel, None).expect("waiting must not fail") {
            WaitOutcome::Exited(status) => assert_eq!(status.code(), Some(7)),
            WaitOutcome::TimedOut => panic!("a quick script must not time out"),
            WaitOutcome::Cancelled => panic!("a quick script must not be cancelled"),
        }
    }

    /// Run a script for real so the mapping is tested against a genuine
    /// `ExitStatus`, not a hand-written one.
    fn run_bash(script: &str) -> ExitStatus {
        Command::new("bash")
            .args(["--noprofile", "--norc", "-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("bash must start")
    }

    fn exit_failure(program: &str, code: i32, started: bool) -> StreamResult {
        StreamResult::failure(
            AppError::Exit {
                program: program.to_string(),
                code: Some(code),
                output: String::new(),
            },
            started,
        )
    }

    #[test]
    fn an_installer_that_exits_1_becomes_an_exit_error_not_a_success() {
        let status = run_bash("echo 'installer exploded' >&2; exit 1");
        let attempt = stream_outcome(
            "bash",
            Ok(WaitOutcome::Exited(status)),
            "installer exploded".to_string(),
            30,
            |_| panic!("an exit code is already visible; no extra error line is needed"),
        );

        assert!(!attempt.is_success());
        assert!(!attempt.started);
        assert_eq!(attempt.exit_code(), Some(1));
        let event = attempt.finished_event("task-exit");
        assert_eq!(event.code, Some(error::code::EXIT));
        assert!(
            event.message.unwrap_or_default().contains("exit code 1"),
            "the exit code must be in the message"
        );
    }

    #[test]
    fn an_exit_after_the_privileged_shell_started_is_still_an_exit() {
        let status = run_bash("exit 1");
        let attempt = stream_outcome(
            "bash",
            Ok(WaitOutcome::Exited(status)),
            format!("{ELEVATED_SENTINEL}\nsomething failed\n"),
            30,
            |_| {},
        );

        assert!(attempt.started, "the sentinel proves the body ran as root");
        assert_eq!(attempt.exit_code(), Some(1));
        assert_eq!(
            attempt.finished_event("task-root").code,
            Some(error::code::EXIT)
        );
    }

    #[test]
    fn a_dismissed_polkit_dialog_is_terminal_and_never_falls_back_to_sudo() {
        let mut attempt = exit_failure("pkexec", 126, false);

        assert_eq!(pkexec_step(&mut attempt, ""), ElevationStep::Stop);
        let event = attempt.finished_event("task-126");
        assert_eq!(event.code, Some(error::code::ELEVATION_DENIED));
        assert!(
            event.message.unwrap_or_default().contains("dismissed"),
            "the user must be told the dialog was dismissed"
        );
        assert_eq!(attempt.exit_code(), None, "126 is no longer an exit code");
    }

    #[test]
    fn a_polkit_failure_that_never_started_may_fall_back_to_sudo() {
        let mut attempt = exit_failure("pkexec", 1, false);

        assert_eq!(pkexec_step(&mut attempt, ""), ElevationStep::FallBackToSudo);
        assert_eq!(
            attempt.exit_code(),
            Some(1),
            "the attempt must stay untouched for the next step"
        );
    }

    #[test]
    fn forcing_pkexec_never_falls_back_even_when_it_never_started() {
        let mut attempt = exit_failure("pkexec", 1, false);

        assert_eq!(pkexec_step(&mut attempt, "pkexec"), ElevationStep::Stop);
        assert_eq!(
            attempt.finished_event("task-forced").code,
            Some(error::code::ELEVATION_DENIED)
        );
    }

    #[test]
    fn a_script_that_started_and_failed_does_not_open_a_second_prompt() {
        let mut attempt = exit_failure("bash", 1, true);

        assert_eq!(pkexec_step(&mut attempt, ""), ElevationStep::Stop);
        assert_eq!(
            attempt.finished_event("task-body").code,
            Some(error::code::EXIT)
        );
    }

    #[test]
    fn a_hung_or_cancelled_polkit_prompt_never_opens_a_sudo_prompt() {
        let mut timed_out = StreamResult::failure(
            AppError::Timeout {
                program: "pkexec".to_string(),
                seconds: 1,
            },
            false,
        );
        assert_eq!(pkexec_step(&mut timed_out, ""), ElevationStep::Stop);
        assert_eq!(
            timed_out.finished_event("task-hang").code,
            Some(error::code::TIMEOUT)
        );

        let mut cancelled = StreamResult::failure(
            AppError::CancelRequested {
                program: "pkexec".to_string(),
            },
            false,
        );
        assert_eq!(pkexec_step(&mut cancelled, ""), ElevationStep::Stop);
        assert_eq!(
            cancelled.finished_event("task-cancel").code,
            Some(error::code::CANCELLED)
        );
    }

    #[test]
    fn a_sudo_that_never_elevated_becomes_elevation_denied() {
        let error = match sudo_failure(exit_failure("sudo", 1, false)) {
            Ok(_) => panic!("a sudo failure that never started must be replaced"),
            Err(error) => error,
        };
        assert_eq!(error.code(), error::code::ELEVATION_DENIED);
        assert!(
            error.to_string().contains("polkit authentication agent"),
            "{error}"
        );
    }

    #[test]
    fn a_sudo_script_that_started_and_failed_keeps_its_exit_code() {
        let attempt = sudo_failure(exit_failure("bash", 2, true))
            .expect("a script that ran as root keeps its result");
        assert_eq!(attempt.exit_code(), Some(2));
        assert_eq!(
            attempt.finished_event("task-sudo").code,
            Some(error::code::EXIT)
        );
    }

    #[test]
    fn a_hanging_command_reports_timeout_and_streams_the_reason() {
        let mut emitted = None;
        let attempt = stream_outcome(
            "bash",
            Ok(WaitOutcome::TimedOut),
            String::new(),
            5,
            |line| emitted = Some(line),
        );

        assert_eq!(
            attempt.finished_event("task-timeout").code,
            Some(error::code::TIMEOUT)
        );
        assert!(
            emitted.unwrap_or_default().contains("within 5s"),
            "the timeout line must tell the user the deadline"
        );
    }

    #[test]
    fn a_timeout_after_elevation_stays_started_and_is_terminal() {
        let attempt = StreamResult::failure(
            AppError::Timeout {
                program: "bash".to_string(),
                seconds: 1,
            },
            true,
        );
        assert!(attempt.started);
        assert!(attempt.timed_out());
        assert!(!attempt.is_success());
        assert_eq!(attempt.exit_code(), None);
        assert_eq!(
            attempt.finished_event("task-1").code,
            Some(error::code::TIMEOUT)
        );
    }

    #[test]
    fn a_cancelled_stream_is_terminal_and_not_an_elevation_failure() {
        let attempt = StreamResult::failure(
            AppError::CancelRequested {
                program: "bash".to_string(),
            },
            false,
        );
        assert!(attempt.cancelled());
        assert!(!attempt.timed_out());
        assert!(!attempt.is_success());
        assert_eq!(attempt.exit_code(), None);
        assert_eq!(
            attempt.finished_event("task-2").code,
            Some(error::code::CANCELLED)
        );
    }

    #[test]
    fn a_cancelled_stream_never_writes_its_completion_marker() {
        let marker = marker_file("cancel_test_marker");
        let _ = std::fs::remove_file(&marker);

        apply_completion(
            &StreamResult::failure(
                AppError::CancelRequested {
                    program: "installer".to_string(),
                },
                true,
            ),
            Completion::WriteMarker("cancel_test_marker"),
        );
        assert!(
            !marker.exists(),
            "a cancel must not leave an installed marker"
        );
    }

    #[test]
    fn a_timed_out_stream_never_writes_its_completion_marker() {
        let marker = marker_file("timeout_test_marker");
        let _ = std::fs::remove_file(&marker);

        apply_completion(
            &StreamResult::failure(
                AppError::Timeout {
                    program: "installer".to_string(),
                    seconds: 1,
                },
                true,
            ),
            Completion::WriteMarker("timeout_test_marker"),
        );
        assert!(
            !marker.exists(),
            "a timeout must not leave an installed marker"
        );

        apply_completion(
            &StreamResult::success(true),
            Completion::WriteMarker("timeout_test_marker"),
        );
        assert!(marker.exists(), "the gate must still apply a real success");
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn a_failed_installer_never_writes_an_installed_marker() {
        let marker = marker_file("exit_test_marker");
        let _ = std::fs::remove_file(&marker);

        apply_completion(
            &exit_failure("installer", 1, true),
            Completion::WriteMarker("exit_test_marker"),
        );
        assert!(
            !marker.exists(),
            "a non-zero exit must not look like a completed install"
        );
    }

    #[test]
    fn installer_markers_follow_reinstall_success_failure_and_removal() {
        let marker = marker_file("reinstall_test_marker");
        let _ = std::fs::remove_file(&marker);
        let completion = Completion::WriteMarker("reinstall_test_marker");

        // Fresh success: pending until a live check confirms it.
        apply_completion(&StreamResult::success(true), completion);
        assert_eq!(marker_state(&marker), MarkerState::Pending);
        assert_eq!(
            offline_marker_report(&marker, "X").unwrap().status,
            "pending"
        );

        // Live check confirms; a second successful install is idempotent.
        refresh_marker(&marker);
        assert_eq!(marker_state(&marker), MarkerState::Verified);
        apply_completion(&StreamResult::success(true), completion);
        assert_eq!(marker_state(&marker), MarkerState::Pending);
        refresh_marker(&marker);

        // A refused prompt touched nothing: the verified marker survives.
        apply_completion(
            &StreamResult::failure(AppError::elevation_denied("dismissed"), false),
            completion,
        );
        assert_eq!(marker_state(&marker), MarkerState::Verified);

        // Failing half-way as root invalidates the old marker.
        apply_completion(&exit_failure("installer", 1, true), completion);
        assert_eq!(marker_state(&marker), MarkerState::Absent);
        assert!(offline_marker_report(&marker, "X").is_none());

        // A legacy "installed" marker is not trusted as verified.
        std::fs::write(&marker, "installed\n").unwrap();
        assert_eq!(marker_state(&marker), MarkerState::Pending);
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn failure_hints_name_disk_and_network_problems() {
        assert!(
            failure_hint("cp: error writing 'x': No space left on device")
                .unwrap()
                .contains("disk filled up")
        );
        assert!(failure_hint(
            "fatal: unable to access 'https://github.com/x': Could not resolve host: github.com"
        )
        .unwrap()
        .contains("network dropped"));
        assert!(failure_hint("ERROR: Failed to establish a new connection").is_some());
        assert!(failure_hint("[ERROR] Failed to install GApps.").is_none());
    }

    #[test]
    fn a_denied_elevation_never_writes_an_installed_marker() {
        let marker = marker_file("elevation_test_marker");
        let _ = std::fs::remove_file(&marker);

        apply_completion(
            &StreamResult::failure(AppError::elevation_denied("dismissed"), false),
            Completion::WriteMarker("elevation_test_marker"),
        );
        assert!(
            !marker.exists(),
            "a refused password prompt must not look like a completed install"
        );
    }

    #[test]
    fn clear_install_markers_only_clears_on_success() {
        let m1 = marker_file("gapps_installed");
        let m2 = marker_file("houdini_installed");
        let m3 = marker_file("multitouch_installed");

        write_marker(&m1).unwrap();
        write_marker(&m2).unwrap();
        write_marker(&m3).unwrap();

        assert!(m1.exists());
        assert!(m2.exists());
        assert!(m3.exists());

        // A failed attempt must NOT clear markers
        apply_completion(
            &exit_failure("wipe", 1, true),
            Completion::ClearInstallMarkers,
        );
        assert!(m1.exists(), "failure must not clear markers");
        assert!(m2.exists(), "failure must not clear markers");
        assert!(m3.exists(), "failure must not clear markers");

        // A cancelled attempt must NOT clear markers
        apply_completion(
            &StreamResult::failure(
                AppError::CancelRequested {
                    program: "wipe".to_string(),
                },
                true,
            ),
            Completion::ClearInstallMarkers,
        );
        assert!(m1.exists(), "cancel must not clear markers");
        assert!(m2.exists(), "cancel must not clear markers");
        assert!(m3.exists(), "cancel must not clear markers");

        // A successful attempt MUST clear markers
        apply_completion(
            &StreamResult::success(true),
            Completion::ClearInstallMarkers,
        );
        assert!(!m1.exists(), "successful wipe must clear gapps marker");
        assert!(!m2.exists(), "successful wipe must clear houdini marker");
        assert!(!m3.exists(), "successful wipe must clear multitouch marker");
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
    fn finished_event_is_structured_even_for_success() {
        let value =
            serde_json::to_value(StreamResult::success(true).finished_event("task-7")).unwrap();
        assert_eq!(value["task_id"], "task-7");
        assert_eq!(value["success"], true);
        assert!(value["code"].is_null());
        assert!(value["message"].is_null());
    }

    #[test]
    fn started_event_carries_the_task_id() {
        let value = serde_json::to_value(StreamStarted {
            task_id: "task-11".to_string(),
        })
        .unwrap();
        assert_eq!(value["task_id"], "task-11");
        assert_eq!(value.as_object().unwrap().len(), 1);
    }

    #[test]
    fn output_event_carries_the_task_id_with_the_line() {
        let value = serde_json::to_value(StreamOutput {
            task_id: "task-9".to_string(),
            line: "installing".to_string(),
        })
        .unwrap();
        assert_eq!(value["task_id"], "task-9");
        assert_eq!(value["line"], "installing");
    }

    #[test]
    fn finished_event_carries_the_error_code_instead_of_only_a_boolean() {
        for (error, expected) in [
            (
                AppError::elevation_denied("dismissed"),
                error::code::ELEVATION_DENIED,
            ),
            (
                AppError::Timeout {
                    program: "installer".to_string(),
                    seconds: 60,
                },
                error::code::TIMEOUT,
            ),
            (
                AppError::CancelRequested {
                    program: "installer".to_string(),
                },
                error::code::CANCELLED,
            ),
        ] {
            let value =
                serde_json::to_value(StreamResult::failure(error, false).finished_event("task-8"))
                    .unwrap();
            assert_eq!(value["task_id"], "task-8");
            assert_eq!(value["success"], false);
            assert_eq!(value["code"], expected);
            assert!(value["message"].is_string());
        }
    }

    #[test]
    fn check_host_diagnostics_returns_core_items() {
        let report = collect_diagnostics();
        assert!(report.items.len() >= 16);
        let keys: Vec<_> = report.items.iter().map(|i| i.key.as_str()).collect();
        assert!(keys.contains(&"session"));
        assert!(keys.contains(&"kvm"));
        assert!(keys.contains(&"binder"));
        assert!(keys.contains(&"gpu"));
        assert!(keys.contains(&"service"));
        assert!(keys.contains(&"waydroid"));
        assert!(keys.contains(&"adb"));
        assert!(keys.contains(&"elevation"));
        assert!(keys.contains(&"git"));
        assert!(keys.contains(&"python"));
        assert!(keys.contains(&"squashfs"));
        assert!(keys.contains(&"lzip"));
        assert!(keys.contains(&"xdg_open"));
        assert!(keys.contains(&"terminal"));
        assert!(keys.contains(&"disk"));
        assert!(keys.contains(&"network"));
        assert!(keys.contains(&"waydroid_image"));
    }

    #[test]
    fn icon_loader_accepts_small_images_and_rejects_the_rest() {
        let dir = std::env::temp_dir().join(format!("archdroid-icon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, bytes: &[u8]| {
            let path = dir.join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        let png = write("a.png", b"\x89PNG\r\n\x1a\nrest");
        assert!(icon_data_uri(&png)
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let svg = write("a.svg", b"<svg xmlns='http://www.w3.org/2000/svg'/>");
        assert!(icon_data_uri(&svg)
            .unwrap()
            .starts_with("data:image/svg+xml;base64,"));
        assert!(icon_data_uri(&write("a.xpm", b"/* XPM */")).is_none());
        assert!(icon_data_uri(&write("passwd", b"root:x:0:0")).is_none());
        let big = write("big.png", &vec![0u8; MAX_ICON_BYTES as usize + 1]);
        assert!(icon_data_uri(&big).is_none());
        assert!(icon_data_uri(&dir).is_none());
        assert!(icon_data_uri(Path::new("relative.png")).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
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

    #[test]
    fn app_list_reuses_unchanged_entries_and_drops_removed_ones() {
        let dir = std::env::temp_dir().join(format!("archdroid-apps-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let desktop = dir.join("waydroid.com.example.desktop");
        std::fs::write(&desktop, "Name=Example\n").unwrap();
        std::fs::write(dir.join("other.desktop"), "Name=Other\n").unwrap();

        let first = collect_apps_in(&dir);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].name, "Example");
        assert_eq!(collect_apps_in(&dir)[0].name, "Example");

        std::fs::remove_file(&desktop).unwrap();
        assert!(collect_apps_in(&dir).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Budget: 200 desktop entries with 4 KiB PNG icons list in < 250 ms cold
    /// and < 25 ms warm (cache hit). Generous for CI; locally it is far lower.
    #[test]
    fn app_list_meets_its_time_budget() {
        let dir = std::env::temp_dir().join(format!("archdroid-bench-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut icon = b"\x89PNG\r\n\x1a\n".to_vec();
        icon.resize(4096, 0);
        let icon_path = dir.join("icon.png");
        std::fs::write(&icon_path, icon).unwrap();
        for i in 0..200 {
            std::fs::write(
                dir.join(format!("waydroid.pkg{i}.desktop")),
                format!("Name=App {i}\nIcon={}\n", icon_path.display()),
            )
            .unwrap();
        }
        let start = std::time::Instant::now();
        assert_eq!(collect_apps_in(&dir).len(), 200);
        let cold = start.elapsed();
        let start = std::time::Instant::now();
        assert_eq!(collect_apps_in(&dir).len(), 200);
        let warm = start.elapsed();
        eprintln!("app list: cold {cold:?}, warm {warm:?}");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(cold.as_millis() < 250, "cold {cold:?}");
        assert!(warm.as_millis() < 25, "warm {warm:?}");
    }

    #[test]
    fn redaction_masks_home_and_secrets() {
        std::env::set_var("HOME", "/home/alice");
        let out =
            redact_text("read /home/alice/.cache/x token=abc123 Bearer xyz\nPASSWORD: hunter2");
        assert!(!out.contains("alice"));
        assert!(!out.contains("abc123") && !out.contains("xyz"));
        assert!(!out.contains("hunter2"));
        assert!(out.contains("~/.cache/x"));
        assert_eq!(redact_text("plain line"), "plain line");
    }

    #[test]
    fn polkit_agent_names_exclude_the_daemon() {
        assert!(is_polkit_agent_name("polkit-gnome-au"));
        assert!(is_polkit_agent_name("hyprpolkitagent"));
        assert!(is_polkit_agent_name("lxqt-policykit"));
        assert!(!is_polkit_agent_name("polkitd"));
        assert!(!is_polkit_agent_name("bash"));
    }

    #[test]
    fn preflight_helpers_report_system_resources() {
        // Disk probe on root must report positive available storage
        let disk_root = check_available_disk_space_kb(Path::new("/"));
        assert!(disk_root.is_some());
        assert!(disk_root.unwrap() > 0);

        // format_disk_space helper checks
        assert_eq!(format_disk_space(2_097_152), "2.0 GB Free");
        assert_eq!(format_disk_space(512_000), "500 MB Free");

        // Elevation tool must return pkexec or sudo on standard Linux host
        let elevation = has_elevation_tool();
        assert!(elevation.is_some());
        let elev = elevation.unwrap();
        assert!(elev == "pkexec" || elev == "sudo");
    }

    #[test]
    fn dependency_probes_only_report_known_names_once() {
        let gapps = gapps_missing_dependencies();
        let allowed_gapps = ["git", "python3", "python3-venv", "squashfs", "lzip"];
        for name in &gapps {
            assert!(
                allowed_gapps.contains(name),
                "unexpected dependency: {name}"
            );
        }
        let mut unique = gapps.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), gapps.len(), "duplicate dependency reported");

        let (packages, sdk) = phantom_missing_dependencies();
        let allowed_packages = ["git", "cargo", "jdk", "sed", "coreutils"];
        for name in &packages {
            assert!(
                allowed_packages.contains(name),
                "unexpected package dependency: {name}"
            );
        }
        for name in &sdk {
            assert!(
                ["d8", "android.jar"].contains(name),
                "unexpected SDK dependency: {name}"
            );
        }
    }

    #[test]
    fn phantom_dependency_error_includes_package_and_sdk_remediation() {
        let error = phantom_dependency_error(
            &["cargo", "jdk"],
            &["d8", "android.jar"],
            Some(platform::PackageManager::Pacman),
        );
        let message = error.to_string();
        assert!(message.contains("cargo, jdk, d8, android.jar"), "{message}");
        assert!(
            message.contains("sudo pacman -S --needed rust jdk-openjdk"),
            "{message}"
        );
        assert!(message.contains("D8=/path/to/d8"), "{message}");
        assert!(
            message.contains("ANDROID_JAR=/path/to/android.jar"),
            "{message}"
        );

        let unsupported = phantom_dependency_error(&["git"], &[], None);
        assert!(
            unsupported
                .to_string()
                .contains("distribution's package manager"),
            "{unsupported}"
        );
    }

    #[test]
    fn sanitize_appimage_env_cleans_polluted_python_vars() {
        std::env::set_var("APPDIR", "/tmp/appimage_extracted_12345");
        std::env::set_var("PYTHONHOME", "/tmp/appimage_extracted_12345/usr/");
        std::env::set_var(
            "PYTHONPATH",
            "/tmp/appimage_extracted_12345/usr/share/pyshared/:",
        );

        sanitize_appimage_env();

        assert!(std::env::var("PYTHONHOME").is_err());
        assert!(std::env::var("PYTHONPATH").is_err());

        // Cleanup test env
        std::env::remove_var("APPDIR");
    }
}
