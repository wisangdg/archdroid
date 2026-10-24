use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

mod adb;
mod capabilities;
mod device;
mod error;
mod waydroid;

use error::AppError;

#[derive(Serialize, Deserialize)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub package: String,
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiagnosticItem {
    pub key: String,
    pub title: String,
    pub status: String,
    pub value: String,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DiagnosticsReport {
    pub items: Vec<DiagnosticItem>,
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
/// Best effort: a read-only cache directory must never break a status check.
fn write_marker(marker: &Path) {
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(marker, "installed\n");
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
mkdir -p "$ARCHDROID_WORK/home" || {{
    echo "[ERROR] Could not create a private root home." >&2
    exit 1
}}
chmod 700 "$ARCHDROID_WORK/home" || {{
    echo "[ERROR] Could not secure the private root home." >&2
    exit 1
}}
export HOME="$ARCHDROID_WORK/home"
export ARCHDROID_WORK
export TMPDIR="$ARCHDROID_WORK"
trap 'rm -rf "$ARCHDROID_WORK"' EXIT HUP INT TERM
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
    off_main_thread(|| {
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
    off_main_thread(|| {
        match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.session_stop()) {
            Ok(_) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
    .await
}

/// Best effort kill of a cancellable long operation (`app install|remove`).
/// Succeeds even when there was nothing running, so a Cancel button can stay
/// simple.
#[tauri::command]
fn waydroid_cancel() -> CommandResult {
    waydroid::request_cancel();
    CommandResult::success()
}

#[tauri::command]
fn get_waydroid_apps() -> Vec<AppInfo> {
    let mut apps = Vec::new();
    if let Some(mut path) = dirs::data_local_dir() {
        path.push("applications");
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name().to_string_lossy().to_string();
                if file_name.starts_with("waydroid.") && file_name.ends_with(".desktop") {
                    let package = file_name.replace("waydroid.", "").replace(".desktop", "");

                    // Simple parse of desktop file
                    let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                    let name = content
                        .lines()
                        .find(|l| l.starts_with("Name="))
                        .map(|l| l.replace("Name=", ""))
                        .unwrap_or(package.clone());

                    let icon_path = content
                        .lines()
                        .find(|l| l.starts_with("Icon="))
                        .map(|l| l.replace("Icon=", ""));

                    let mut icon_b64 = None;
                    if let Some(path) = icon_path {
                        if let Ok(bytes) = std::fs::read(&path) {
                            let b64 = general_purpose::STANDARD.encode(&bytes);
                            icon_b64 = Some(format!("data:image/png;base64,{}", b64));
                        }
                    }

                    apps.push(AppInfo {
                        id: package.clone(),
                        name,
                        package,
                        icon: icon_b64,
                    });
                }
            }
        }
    }
    apps
}

#[tauri::command]
async fn waydroid_install_apk(path: String) -> CommandResult {
    off_main_thread(move || {
        match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.app_install(&path)) {
            Ok(_) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
    .await
}

#[tauri::command]
async fn waydroid_uninstall_app(package: String) -> CommandResult {
    off_main_thread(move || {
        match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.app_remove(&package)) {
            Ok(_) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
    .await
}

#[tauri::command]
async fn waydroid_launch_app(package: String) -> CommandResult {
    off_main_thread(move || {
        match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.app_launch(&package)) {
            Ok(_) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
    .await
}

#[tauri::command]
async fn waydroid_prop_set(key: String, value: String) -> CommandResult {
    off_main_thread(move || {
        match waydroid::Waydroid::detect().and_then(|waydroid| waydroid.prop_set(&key, &value)) {
            Ok(_) => CommandResult::success(),
            Err(error) => error.into(),
        }
    })
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
    cat /var/lib/waydroid/waydroid.log
else
    echo "No logs found at /var/lib/waydroid/waydroid.log"
fi
"#
    .to_string();

    stream_privileged_script(app, script, Completion::None)
        .await
        .into_command_result()
}

#[tauri::command]
fn open_shared_folder() -> CommandResult {
    let mut path = dirs::data_local_dir().unwrap_or_default();
    path.push("waydroid");
    path.push("data");
    path.push("media");
    path.push("0");

    // Create folder if it doesn't exist
    let _ = std::fs::create_dir_all(&path);

    match Command::new("xdg-open").arg(&path).output() {
        Ok(_) => CommandResult::success(),
        Err(error) => AppError::from_spawn(
            "xdg-open",
            "Install xdg-utils so a file manager can be asked to open the folder.",
            error,
        )
        .into(),
    }
}

#[tauri::command]
fn check_host_diagnostics() -> DiagnosticsReport {
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
        });
    } else {
        items.push(DiagnosticItem {
            key: "session".to_string(),
            title: "Display Session".to_string(),
            status: "warning".to_string(),
            value: "X11 Display".to_string(),
            detail: "Waydroid requires Wayland. Under X11, a nested compositor (Weston/cage) may be required.".to_string(),
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
            });
        } else {
            items.push(DiagnosticItem {
                key: "kvm".to_string(),
                title: "KVM Acceleration".to_string(),
                status: "warning".to_string(),
                value: "Permission Denied".to_string(),
                detail: "KVM device exists but user lacks write permission. Run: sudo usermod -aG kvm $USER".to_string(),
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
        });
    } else {
        items.push(DiagnosticItem {
            key: "binder".to_string(),
            title: "Android Binder IPC".to_string(),
            status: "ok".to_string(),
            value: "Managed by Daemon".to_string(),
            detail: "Binderfs will be mounted automatically when container daemon starts."
                .to_string(),
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
        });
    } else {
        items.push(DiagnosticItem {
            key: "gpu".to_string(),
            title: "GPU Acceleration".to_string(),
            status: "warning".to_string(),
            value: "Software Rendering".to_string(),
            detail: "Hardware DRI node not found. Android graphics will fall back to software rendering.".to_string(),
        });
    }

    // 5. Waydroid Container Service
    let output = Command::new("systemctl")
        .arg("is-active")
        .arg("waydroid-container")
        .output();

    let service_active = output
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false);

    if service_active {
        items.push(DiagnosticItem {
            key: "service".to_string(),
            title: "Container Daemon".to_string(),
            status: "ok".to_string(),
            value: "Service Active".to_string(),
            detail: "waydroid-container.service is actively running in background.".to_string(),
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
        });
    }

    DiagnosticsReport { items }
}

#[tauri::command]
async fn waydroid_init(app: AppHandle) -> CommandResult {
    let script = r#"
set -e
if ! command -v waydroid > /dev/null; then
    echo "==> Installing Waydroid from the system repositories..."
    pacman -S --noconfirm waydroid
fi
echo "==> Initializing Waydroid (this will download ~1GB of data)..."
if waydroid init; then
    echo ""
    echo "[SUCCESS] Waydroid initialized successfully!"
else
    echo "[ERROR] waydroid init failed. Check your network connection and try again."
    exit 1
fi
"#
    .to_string();
    stream_privileged_script(app, script, Completion::ClearInstallMarkers)
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
            write_marker(&marker);
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

    // Method 2: the container is offline, trust the marker written by a successful install
    if marker.exists() {
        return CommandResult::report(
            "installed",
            "GApps installed (verified marker). Start Waydroid for a live check.",
        );
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
            write_marker(&marker);
            return CommandResult::report("installed", "ARM translation (libhoudini) is active.");
        }
        if marker.exists() {
            let _ = std::fs::remove_file(&marker);
        }
        return CommandResult::report("not_installed", "ARM translation is not active.");
    }

    if marker.exists() {
        return CommandResult::report(
            "installed",
            "ARM translation (libhoudini) installed (verified marker).",
        );
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
            write_marker(&marker);
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
            "Features installed (verified marker). Start Waydroid for a live check.",
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
    stream_privileged_script(
        app,
        capabilities::install_script(),
        Completion::WriteMarker("multitouch_installed"),
    )
    .await
    .into_command_result_with_message(
        "Multitouch features written to the Waydroid overlay. Restart Waydroid so Android declares them.",
    )
}

/// One dashboard object, assembled from `waydroid status` plus the cheapest
/// probes. Values that need a running container degrade instead of failing:
/// this is a read-only overview, not an action.
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

fn json_result<T: Serialize>(value: &T) -> CommandResult {
    match serde_json::to_string(value) {
        Ok(json) => CommandResult::with_data(json),
        Err(failure) => AppError::Io(std::io::Error::other(failure.to_string())).into(),
    }
}

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

#[tauri::command]
async fn waydroid_status_json() -> CommandResult {
    off_main_thread(|| json_result(&status_snapshot())).await
}

/// LDPlayer `list3` shape. Waydroid is a single container, so the list always
/// has zero or one entry.
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

fn parse_memory_mb(meminfo: &str) -> u32 {
    meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kilobytes| kilobytes.parse::<u64>().ok())
        .map(|kilobytes| (kilobytes / 1024) as u32)
        .unwrap_or(0)
}

fn host_memory_mb() -> u32 {
    parse_memory_mb(&std::fs::read_to_string("/proc/meminfo").unwrap_or_default())
}

// ponytail: Waydroid is one container that inherits host CPU/RAM. `root` is
// true by design (`waydroid shell` is root), and resolution/DPI mirror the
// saved device profile.
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

#[tauri::command]
async fn waydroid_list_instances() -> CommandResult {
    off_main_thread(|| {
        let snapshot = status_snapshot();
        let profile = device::load_from(&device::config_path());
        json_result(&vec![single_instance(&snapshot, &profile)])
    })
    .await
}

#[tauri::command]
fn profile_capabilities() -> CommandResult {
    json_result(&device::FIELDS)
}

/// Saved profile overlaid with the values Android actually uses right now
/// (waydroid.cfg on disk, plus `wm size`/`wm density` when the session is up).
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
                    let attempt =
                        run_privileged_script_for_task(&app, task.id(), script, Completion::None);
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
#[tauri::command]
async fn waydroid_reboot(app: AppHandle) -> CommandResult {
    stream_privileged_script(
        app,
        "waydroid container restart".to_string(),
        Completion::None,
    )
    .await
    .into_command_result()
}

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

#[tauri::command]
async fn waydroid_getprop_all() -> CommandResult {
    off_main_thread(|| match adb::shell_active(&["getprop"]) {
        Ok(text) => CommandResult::with_data(text),
        Err(failure) => failure.into(),
    })
    .await
}

/// Arbitrary device shell line (LDPlayer `adb --command` parity). There is no
/// input validation on purpose: running the caller's command is the feature.
#[tauri::command]
async fn waydroid_adb_cmd(command: String) -> CommandResult {
    off_main_thread(move || match adb::shell_line_active(&command) {
        Ok(text) => CommandResult::with_data(text),
        Err(failure) => failure.into(),
    })
    .await
}

#[tauri::command]
async fn waydroid_wipe(app: AppHandle) -> CommandResult {
    let script = r#"
    set -eu
    echo "[SYSTEM] Stopping Waydroid services..."
    waydroid session stop || true
    systemctl stop waydroid-container || true
    echo "[SYSTEM] Wiping Waydroid data..."
    rm -rf /var/lib/waydroid
    rm -rf "$ARCHDROID_USER_HOME/.local/share/waydroid"
    rm -rf "$ARCHDROID_USER_HOME"/.local/share/applications/waydroid.*
    echo "[SYSTEM] Wipe complete! Please restart ArchDroid."
    "#
    .to_string();

    stream_privileged_script(app, script, Completion::ClearInstallMarkers)
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

/// Fetch exactly `rev` into a fresh repository. `repo` is a shell expansion such
/// as `"$repo"`, not a Rust path. The final `test` catches a server or checkout
/// that silently resolved a different object.
fn pinned_git_checkout(repo: &str, url: &str, rev: &str) -> String {
    format!(
        r#"git init -q {repo}
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

fn gapps_install_script() -> String {
    format!(
        r#"set -eu
if ! command -v git > /dev/null; then
    echo "[ERROR] git is required to download the pinned GApps helper." >&2
    exit 1
fi
if ! command -v unsquashfs > /dev/null || ! command -v lzip > /dev/null; then
    echo "[SYSTEM] Installing required dependencies (squashfs-tools, lzip)..."
    pacman -S --noconfirm squashfs-tools lzip
fi
repo="$ARCHDROID_WORK/waydroid_script"
{checkout}
echo "[SYSTEM] Setting up Python environment..."
python3 -m venv "$repo/venv"
"$repo/venv/bin/pip" install --no-cache-dir --disable-pip-version-check --no-input -q -r "$repo/requirements.txt"
cd "$repo"
echo "[SYSTEM] Injecting GApps into Android Image (this will take a while)..."
if ! PYTHONUNBUFFERED=1 "$repo/venv/bin/python3" main.py install gapps; then
    echo "[ERROR] Failed to install GApps." >&2
    exit 1
fi
echo "[SUCCESS] GApps installation complete! Please Start Waydroid."
"#,
        checkout = pinned_git_checkout("\"$repo\"", WAYDROID_SCRIPT_URL, WAYDROID_SCRIPT_REV),
    )
}

fn houdini_install_script() -> String {
    format!(
        r#"set -eu
if ! command -v git > /dev/null; then
    echo "[ERROR] git is required to download the pinned libhoudini helper." >&2
    exit 1
fi
if ! command -v unsquashfs > /dev/null || ! command -v lzip > /dev/null; then
    echo "[SYSTEM] Installing required dependencies (squashfs-tools, lzip)..."
    pacman -S --noconfirm squashfs-tools lzip
fi
repo="$ARCHDROID_WORK/waydroid_script"
{checkout}
echo "[SYSTEM] Setting up Python environment..."
python3 -m venv "$repo/venv"
"$repo/venv/bin/pip" install --no-cache-dir --disable-pip-version-check --no-input -q -r "$repo/requirements.txt"
cd "$repo"
echo "[SYSTEM] Installing ARM Translation / libhoudini (this may take several minutes)..."
if ! PYTHONUNBUFFERED=1 "$repo/venv/bin/python3" main.py install libhoudini; then
    echo "[ERROR] ARM Translation installation failed!" >&2
    exit 1
fi
echo "[SUCCESS] ARM Translation installation complete!"
"#,
        checkout = pinned_git_checkout("\"$repo\"", WAYDROID_SCRIPT_URL, WAYDROID_SCRIPT_REV),
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
async fn install_gapps(app: AppHandle) -> CommandResult {
    stream_privileged_script(
        app,
        gapps_install_script(),
        Completion::WriteMarker("gapps_installed"),
    )
    .await
    .into_command_result()
}

#[tauri::command]
async fn install_houdini(app: AppHandle) -> CommandResult {
    stream_privileged_script(
        app,
        houdini_install_script(),
        Completion::WriteMarker("houdini_installed"),
    )
    .await
    .into_command_result()
}

#[tauri::command]
async fn install_phantom(app: AppHandle) -> CommandResult {
    // Phantom installs into $HOME/.local and its system wrapper is disabled, so
    // it must not run through pkexec/sudo. Running it as the desktop user also
    // avoids executing the user's Rust toolchain as root.
    stream_user_script(app, phantom_install_script())
        .await
        .into_command_result()
}

#[tauri::command]
fn run_phantom() -> CommandResult {
    let script = "phantom";
    let terminal_cmd = format!(
        r#"
if command -v konsole > /dev/null; then konsole -e bash -c '{}';
elif command -v alacritty > /dev/null; then alacritty -e bash -c '{}';
elif command -v gnome-terminal > /dev/null; then gnome-terminal -- bash -c '{}';
elif command -v xterm > /dev/null; then xterm -e bash -c '{}';
else notify-send "No terminal emulator found"; fi
    "#,
        script, script, script, script
    );

    let _output = Command::new("bash").arg("-c").arg(&terminal_cmd).output();

    CommandResult::with_message("Launched Phantom Keymapper.")
}

/// The single-flight slot. Streaming operations share one terminal, so only
/// one of them may own the slot at a time.
static ACTIVE_TASK: Mutex<Option<ActiveTask>> = Mutex::new(None);
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

struct ActiveTask {
    id: String,
    label: &'static str,
}

/// Holds the single-flight slot until the operation finishes, including on
/// every early return or panic.
struct TaskGuard {
    id: String,
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
        active.replace(ActiveTask {
            id: id.clone(),
            label,
        });
        Ok(Self { id })
    }

    fn id(&self) -> &str {
        &self.id
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
fn flush_line(app: &AppHandle, task_id: &str, buffer: &mut String, captured: &Arc<Mutex<String>>) {
    if buffer.is_empty() {
        return;
    }
    emit_output(app, task_id, buffer.clone());
    if let Ok(mut text) = captured.lock() {
        if text.len() < MAX_CAPTURED_OUTPUT {
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

/// Spawn a command, stream both of its pipes to the UI and report how it went.
fn run_and_stream(app: &AppHandle, task_id: &str, mut command: Command) -> StreamResult {
    let program = command.get_program().to_string_lossy().to_string();
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            let error =
                AppError::from_spawn(&program, "Install the missing program and try again.", e);
            log::error!("{program} could not start: {error}");
            emit_output(app, task_id, format!("[ERROR] {error}"));
            return StreamResult::failure(error, false);
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let captured = Arc::new(Mutex::new(String::new()));
    let stdout_reader = spawn_line_reader(app.clone(), task_id, stdout, captured.clone());
    let stderr_reader = spawn_line_reader(app.clone(), task_id, stderr, captured.clone());

    let status = child.wait();

    let _ = stdout_reader.join();
    let _ = stderr_reader.join();

    let captured_text = captured.lock().map(|text| text.clone()).unwrap_or_default();
    let started = captured_text.contains(ELEVATED_SENTINEL);

    let outcome = match status {
        Ok(status) if status.success() => StreamResult::success(started),
        Ok(status) => StreamResult::failure(
            AppError::Exit {
                program: program.clone(),
                code: status.code(),
                output: captured_text,
            },
            started,
        ),
        Err(e) => {
            emit_output(
                app,
                task_id,
                format!("[ERROR] Lost track of the running command: {}", e),
            );
            StreamResult::failure(AppError::Io(e), started)
        }
    };
    if outcome.is_success() {
        log::info!("{program} finished");
    } else if let Err(error) = &outcome.outcome {
        log::warn!("{program} failed: {error}");
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
    fn apply(self) {
        match self {
            Self::None => {}
            Self::WriteMarker(name) => write_marker(&marker_file(name)),
            Self::ClearInstallMarkers => {
                for name in [
                    "gapps_installed",
                    "houdini_installed",
                    "multitouch_installed",
                ] {
                    let _ = std::fs::remove_file(marker_file(name));
                }
            }
        }
    }
}

fn finish_stream(app: &AppHandle, task_id: &str, attempt: &StreamResult, completion: Completion) {
    if attempt.is_success() {
        completion.apply();
    }
    let _ = app.emit("cmd-finished", attempt.finished_event(task_id));
}

/// Run a user-level script. Phantom installs into `$HOME/.local`, so it does
/// not need root at all; keeping it out of the privileged path also avoids
/// running a user-controlled Rust toolchain as root.
fn run_user_script(app: &AppHandle, task_id: &str, script: String) -> StreamResult {
    log::info!("user script requested:\n{script}");
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
    let attempt = run_and_stream(app, task_id, command);
    finish_stream(app, task_id, &attempt, Completion::None);
    attempt
}

/// Await the user-level script so the command only resolves after it exits.
async fn stream_user_script(app: AppHandle, script: String) -> StreamResult {
    let task = match TaskGuard::start("user script") {
        Ok(task) => task,
        Err(error) => return StreamResult::failure(error, false),
    };
    let task_id = task.id().to_string();
    let fallback_app = app.clone();
    let fallback_task_id = task_id.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        let _task = task;
        run_user_script(&app, &task_id, script)
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
) -> StreamResult {
    log::info!("root script requested:\n{script}");
    let body = match elevate_env(&script) {
        Ok(body) => body,
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
            let mut attempt = run_and_stream(app, task_id, command);

            // Exit code 126 means the user dismissed the polkit dialog, so do
            // not nag with a second prompt.
            if attempt.exit_code() == Some(126) {
                attempt = StreamResult::failure(
                    AppError::elevation_denied(
                        "The polkit authentication dialog was dismissed; nothing was changed.",
                    ),
                    attempt.started,
                );
                finish_stream(app, task_id, &attempt, completion);
                return attempt;
            }

            // If the shell started, the script itself failed; only fall back to
            // sudo when elevation never got as far as running the body.
            if attempt.started || forced == "pkexec" {
                if !attempt.started && !attempt.is_success() {
                    attempt = StreamResult::failure(
                        AppError::elevation_denied(
                            "polkit could not elevate here; no privileged shell was started.",
                        ),
                        false,
                    );
                }
                finish_stream(app, task_id, &attempt, completion);
                return attempt;
            }

            pkexec_failed = true;
            emit_output(
                app,
                task_id,
                "[SYSTEM] polkit could not elevate here, falling back to sudo...",
            );
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
    let mut attempt = run_and_stream(app, task_id, command);

    if !attempt.is_success() && !attempt.started {
        emit_output(app, task_id, "[SYSTEM] Root access failed. Install a polkit authentication agent (for example hyprpolkitagent or polkit-gnome), or start ArchDroid from a terminal so sudo can ask for the password.");
        attempt = StreamResult::failure(
            AppError::elevation_denied(
                "sudo could not elevate or the privileged shell never started. Install a polkit authentication agent (for example hyprpolkitagent or polkit-gnome), or start ArchDroid from a terminal so sudo can ask for the password.",
            ),
            false,
        );
    }

    finish_stream(app, task_id, &attempt, completion);
    attempt
}

/// Await the privileged script so the command only resolves after it exits.
async fn stream_privileged_script(
    app: AppHandle,
    script: String,
    completion: Completion,
) -> StreamResult {
    let task = match TaskGuard::start("privileged operation") {
        Ok(task) => task,
        Err(error) => return StreamResult::failure(error, false),
    };
    let task_id = task.id().to_string();
    let fallback_app = app.clone();
    let fallback_task_id = task_id.clone();
    let fallback_completion = completion;
    match tauri::async_runtime::spawn_blocking(move || {
        let _task = task;
        run_privileged_script_for_task(&app, &task_id, script, completion)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Defaults already log to stdout and `<app log dir>/<app name>.log`
        // (40 KB, keep-one rotation); Info keeps chatty deps out of the trace.
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            waydroid_status,
            waydroid_status_json,
            waydroid_is_running,
            waydroid_list_instances,
            profile_capabilities,
            profile_get,
            profile_apply,
            waydroid_reboot,
            waydroid_kill_app,
            waydroid_getprop_all,
            waydroid_adb_cmd,
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
            waydroid_cancel,
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
            check_host_diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sh_quote_wraps_and_escapes_single_quotes() {
        assert_eq!(sh_quote("/home/wdg"), "'/home/wdg'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }

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
        assert!(body.contains("mktemp -d /var/tmp/archdroid."));
        assert!(body.contains("chmod 700 \"$ARCHDROID_WORK\""));
        assert!(body.contains("trap 'rm -rf \"$ARCHDROID_WORK\"'"));
        assert!(body.contains(ELEVATED_SENTINEL));
        assert!(body.trim_end().ends_with("echo body"));
    }

    #[test]
    fn markers_are_written_by_the_unprivileged_app_process() {
        let root =
            std::env::temp_dir().join(format!("archdroid-marker-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let marker = root.join("gapps_installed");

        write_marker(&marker);

        assert_eq!(
            std::fs::read_to_string(&marker)
                .expect("the app must be able to read its own marker")
                .trim(),
            "installed"
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
        let gapps = gapps_install_script();
        assert!(gapps.contains(WAYDROID_SCRIPT_REV));
        assert!(gapps.contains("$ARCHDROID_WORK"));
        assert!(!gapps.contains("cd /tmp"));
        assert!(!gapps.contains("git pull"));
        assert!(!gapps.contains("archdroid_marker"));

        let houdini = houdini_install_script();
        assert!(houdini.contains(WAYDROID_SCRIPT_REV));
        assert!(houdini.contains("$ARCHDROID_WORK"));
        assert!(!houdini.contains("cd /tmp"));
        assert!(!houdini.contains("git pull"));
        assert!(!houdini.contains("archdroid_marker"));

        let phantom = phantom_install_script();
        assert!(phantom.contains(PHANTOM_REV));
        assert!(phantom.contains("INSTALL_SYSTEM_WRAPPER=0"));
        assert!(phantom.contains("mktemp -d"));
        assert!(!phantom.contains("git pull"));
    }

    #[test]
    fn third_party_installers_are_valid_bash() {
        for (name, script) in [
            ("gapps", gapps_install_script()),
            ("houdini", houdini_install_script()),
            ("phantom", phantom_install_script()),
        ] {
            let output = Command::new("bash")
                .args(["-n", "-c", &script])
                .output()
                .expect("failed to run bash");
            assert!(
                output.status.success(),
                "{name} installer is not valid bash: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
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

    #[test]
    fn single_flight_allows_only_one_active_task() {
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
        let report = check_host_diagnostics();
        assert_eq!(report.items.len(), 5);
        let keys: Vec<_> = report.items.iter().map(|i| i.key.as_str()).collect();
        assert!(keys.contains(&"session"));
        assert!(keys.contains(&"kvm"));
        assert!(keys.contains(&"binder"));
        assert!(keys.contains(&"gpu"));
        assert!(keys.contains(&"service"));
    }
}
