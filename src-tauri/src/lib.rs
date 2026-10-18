use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};

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
const CACHE_SUBDIR: &str = "droiddeck";

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

fn cache_dir() -> PathBuf {
    let mut dir = dirs::cache_dir().unwrap_or_else(std::env::temp_dir);
    dir.push(CACHE_SUBDIR);
    dir
}

fn marker_file(name: &str) -> PathBuf {
    cache_dir().join(name)
}

/// Persist a marker so a finished install is still known after a restart.
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

/// Prefix a script with the environment that polkit/sudo would otherwise strip,
/// so privileged scripts still write markers into the *user* cache directory.
///
/// Elevated scripts run with a restrictive umask, so markers are created as
/// `root:root 600` by default, which the unprivileged app could not read back.
/// `droiddeck_marker` hands them to the desktop user instead.
fn elevate_env(script: &str) -> String {
    let home = dirs::home_dir().unwrap_or_default();
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/local/bin:/usr/bin:/bin".to_string());
    let cache = cache_dir();
    format!(
        r#"export HOME={home}
export PATH={path}
export DROIDDECK_CACHE={cache}
mkdir -p "$DROIDDECK_CACHE"
chmod 755 "$DROIDDECK_CACHE" 2>/dev/null || true
chown "${{PKEXEC_UID:-${{SUDO_UID:-0}}}}" "$DROIDDECK_CACHE" 2>/dev/null || true
droiddeck_marker() {{
    echo installed > "$DROIDDECK_CACHE/$1"
    chmod 644 "$DROIDDECK_CACHE/$1" 2>/dev/null || true
    chown "${{PKEXEC_UID:-${{SUDO_UID:-0}}}}" "$DROIDDECK_CACHE/$1" 2>/dev/null || true
}}
echo "{sentinel} (uid=$(id -u))"
{script}
"#,
        home = sh_quote(&home.to_string_lossy()),
        path = sh_quote(&path),
        cache = sh_quote(&cache.to_string_lossy()),
        sentinel = ELEVATED_SENTINEL,
        script = script,
    )
}

/// Build a command that runs `body` as root through `helper` (pkexec or sudo).
fn elevated_command(helper: &str, body: &str) -> Option<Command> {
    let program = resolve_program(helper)?;
    let shell = resolve_program("bash").unwrap_or_else(|| PathBuf::from("/usr/bin/bash"));
    let mut command = Command::new(program);
    command.arg(shell).arg("-c").arg(body);
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
fn waydroid_log(app: AppHandle) -> CommandResult {
    let script = r#"
if [ -f /var/lib/waydroid/waydroid.log ]; then
    cat /var/lib/waydroid/waydroid.log
else
    echo "No logs found at /var/lib/waydroid/waydroid.log"
fi
"#
    .to_string();

    stream_privileged_script(app, script);
    CommandResult::success()
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
fn waydroid_init(app: AppHandle) -> CommandResult {
    let script = r#"
set -e
if ! command -v waydroid > /dev/null; then
    echo "==> Installing Waydroid from the system repositories..."
    pacman -S --noconfirm waydroid
fi
echo "==> Initializing Waydroid (this will download ~1GB of data)..."
if waydroid init; then
    rm -f "$DROIDDECK_CACHE/gapps_installed" "$DROIDDECK_CACHE/houdini_installed"
    echo ""
    echo "[SUCCESS] Waydroid initialized successfully!"
else
    echo "[ERROR] waydroid init failed. Check your network connection and try again."
    exit 1
fi
"#
    .to_string();
    stream_privileged_script(app, script);
    CommandResult::success()
}

#[tauri::command]
async fn check_gapps_status() -> CommandResult {
    off_main_thread(|| {
        let marker = marker_file("gapps_installed");

        // Method 1: ask the running container. This is the only authoritative source,
        // so it also tells us when a previously written marker has gone stale.
        if let Some(stdout) = probe(|waydroid| waydroid.shell(&["pm", "list", "packages"])) {
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
            // outside DroidDeck), so forget the marker instead of trusting it forever.
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
    })
    .await
}

#[tauri::command]
async fn check_houdini_status() -> CommandResult {
    off_main_thread(|| {
        let marker = marker_file("houdini_installed");

        // The container reports the ARM translation layer in its ABI list when it is loaded.
        if let Some(abilist) =
            probe(|waydroid| waydroid.shell(&["getprop", "ro.product.cpu.abilist"]))
        {
            let abilist = abilist.to_lowercase();
            if abilist.contains("arm") {
                write_marker(&marker);
                return CommandResult::report(
                    "installed",
                    "ARM translation (libhoudini) is active.",
                );
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
    })
    .await
}

#[tauri::command]
fn waydroid_wipe(app: AppHandle) -> CommandResult {
    let script = r#"
    echo "[SYSTEM] Stopping Waydroid services..."
    waydroid session stop || true
    systemctl stop waydroid-container || true
    echo "[SYSTEM] Wiping Waydroid data..."
    rm -rf /var/lib/waydroid
    rm -rf "$HOME/.local/share/waydroid"
    rm -rf "$HOME"/.local/share/applications/waydroid.*
    rm -f "$DROIDDECK_CACHE/gapps_installed" "$DROIDDECK_CACHE/houdini_installed"
    echo "[SYSTEM] Wipe complete! Please restart DroidDeck."
    "#
    .to_string();

    stream_privileged_script(app, script);
    CommandResult::success()
}

#[tauri::command]
fn install_gapps(app: AppHandle) -> CommandResult {
    let script = r#"
set -e
cd /tmp
if ! command -v unsquashfs > /dev/null || ! command -v lzip > /dev/null; then
    echo "[SYSTEM] Installing required dependencies (squashfs-tools, lzip)..."
    pacman -S --noconfirm squashfs-tools lzip
fi
if [ ! -d "waydroid_script" ]; then
    echo "[SYSTEM] Downloading Waydroid Script..."
    git clone https://github.com/casualsnek/waydroid_script.git
fi
cd waydroid_script
echo "[SYSTEM] Setting up Python environment..."
python3 -m venv venv
venv/bin/pip install -q -r requirements.txt
echo "[SYSTEM] Injecting GApps into Android Image (this will take a while)..."
if ! PYTHONUNBUFFERED=1 venv/bin/python3 main.py install gapps; then
    echo "[ERROR] Failed to install GApps."
    exit 1
fi
droiddeck_marker gapps_installed
echo "[SUCCESS] GApps installation complete! Please Start Waydroid."
"#
    .to_string();

    stream_privileged_script(app, script);
    CommandResult::success()
}

#[tauri::command]
fn install_houdini(app: AppHandle) -> CommandResult {
    let script = r#"
set -e
cd /tmp
# Install system dependencies
if ! command -v unsquashfs > /dev/null || ! command -v lzip > /dev/null; then
    echo "[SYSTEM] Installing required dependencies (squashfs-tools, lzip)..."
    pacman -S --noconfirm squashfs-tools lzip
fi
# Clone or update the repository
if [ ! -d "waydroid_script" ]; then
    echo "[SYSTEM] Cloning waydroid_script..."
    git clone https://github.com/casualsnek/waydroid_script.git
fi
cd waydroid_script
# Setup virtual environment and install python requirements
if [ ! -d "venv" ]; then
    echo "[SYSTEM] Setting up Python virtual environment..."
    python3 -m venv venv
fi
echo "[SYSTEM] Installing Python requirements..."
venv/bin/pip install -q -r requirements.txt
echo "[SYSTEM] Installing ARM Translation / libhoudini (this may take several minutes)..."
if ! PYTHONUNBUFFERED=1 venv/bin/python3 main.py install libhoudini; then
    echo "[ERROR] ARM Translation installation failed!"
    exit 1
fi
droiddeck_marker houdini_installed
echo "[SUCCESS] ARM Translation installation complete!"
    "#
    .to_string();

    stream_privileged_script(app, script);
    CommandResult::success()
}

#[tauri::command]
fn install_phantom(app: AppHandle) -> CommandResult {
    let script = r#"
set -e
cd /tmp
if [ ! -d "phantom" ]; then
    echo "[SYSTEM] Cloning Phantom Keymapper..."
    git clone https://github.com/oliviermugishak/phantom.git
    cd phantom
else
    cd phantom
    echo "[SYSTEM] Updating Phantom Keymapper..."
    # The repository may have been cloned by the desktop user before DroidDeck
    # started running this script as root.
    git -c safe.directory="$PWD" pull
fi
echo "[SYSTEM] Running install script..."
chmod +x install.sh
if ./install.sh; then
    echo "[SUCCESS] Phantom Keymapper installed successfully!"
else
    echo "[ERROR] Phantom installation failed!"
    exit 1
fi
    "#
    .to_string();

    stream_privileged_script(app, script);
    CommandResult::success()
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

struct StreamResult {
    success: bool,
    code: Option<i32>,
    /// True when the elevated shell actually started, i.e. elevation worked.
    started: bool,
}

/// Emit a completed line and remember the beginning of the output.
fn flush_line(app: &AppHandle, buffer: &mut String, captured: &Arc<Mutex<String>>) {
    if buffer.is_empty() {
        return;
    }
    let _ = app.emit("cmd-output", buffer.clone());
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
    reader: impl Read + Send + 'static,
    captured: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
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
                flush_line(&app, &mut buffer, &captured);
            } else {
                buffer.push(c);
            }
        }
        flush_line(&app, &mut buffer, &captured);
    })
}

/// Spawn a command, stream both of its pipes to the UI and report how it went.
fn run_and_stream(app: &AppHandle, mut command: Command) -> StreamResult {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            let _ = app.emit(
                "cmd-output",
                format!("[ERROR] Could not start command: {}", e),
            );
            return StreamResult {
                success: false,
                code: None,
                started: false,
            };
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let captured = Arc::new(Mutex::new(String::new()));
    let stdout_reader = spawn_line_reader(app.clone(), stdout, captured.clone());
    let stderr_reader = spawn_line_reader(app.clone(), stderr, captured.clone());

    let status = child.wait();

    let _ = stdout_reader.join();
    let _ = stderr_reader.join();

    let started = captured
        .lock()
        .map(|text| text.contains(ELEVATED_SENTINEL))
        .unwrap_or(false);

    match status {
        Ok(status) => StreamResult {
            success: status.success(),
            code: status.code(),
            started,
        },
        Err(e) => {
            let _ = app.emit(
                "cmd-output",
                format!("[ERROR] Lost track of the running command: {}", e),
            );
            StreamResult {
                success: false,
                code: None,
                started,
            }
        }
    }
}

/// Run a script that needs root.
///
/// The script body is written as if it were already root: there are no `sudo`
/// calls inside. `elevate_env` restores HOME/PATH so install scripts still write
/// into the user's home directory instead of `/root`.
///
/// Elevation is tried through polkit first, because a polkit password dialog is
/// drawn by a desktop agent and therefore also works when DroidDeck was started
/// from an app launcher. `sudo` remains the fallback because it can still prompt
/// when the app was started from a terminal. Force either one with
/// `DROIDDECK_ELEVATE=pkexec|sudo`.
fn stream_privileged_script(app: AppHandle, script: String) {
    thread::spawn(move || {
        let body = elevate_env(&script);
        let forced = std::env::var("DROIDDECK_ELEVATE").unwrap_or_default();

        if forced != "sudo" {
            if let Some(command) = elevated_command("pkexec", &body) {
                let _ = app.emit(
                    "cmd-output",
                    "[SYSTEM] Requesting root access via polkit...".to_string(),
                );
                let attempt = run_and_stream(&app, command);

                // Exit code 126 means the user dismissed the polkit dialog, so do
                // not nag with a second prompt. Any other failure that never
                // started the shell is treated as "no agent available".
                let dismissed = attempt.code == Some(126);
                if attempt.started || dismissed || forced == "pkexec" {
                    let _ = app.emit("cmd-finished", attempt.success);
                    return;
                }

                let _ = app.emit(
                    "cmd-output",
                    "[SYSTEM] polkit could not elevate here, falling back to sudo...".to_string(),
                );
            }
        }

        let Some(command) = elevated_command("sudo", &body) else {
            let error = AppError::elevation_denied(
                "Neither pkexec nor sudo is available, so root access cannot be requested.",
            );
            let _ = app.emit("cmd-output", format!("[ERROR] {error}"));
            let _ = app.emit("cmd-finished", false);
            return;
        };

        let _ = app.emit(
            "cmd-output",
            "[SYSTEM] Requesting root access via sudo...".to_string(),
        );
        let attempt = run_and_stream(&app, command);

        if !attempt.success && !attempt.started {
            let _ = app.emit("cmd-output", "[SYSTEM] Root access failed. Install a polkit authentication agent (for example hyprpolkitagent or polkit-gnome), or start DroidDeck from a terminal so sudo can ask for the password.".to_string());
        }

        let _ = app.emit("cmd-finished", attempt.success);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            waydroid_status,
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
            install_gapps,
            install_houdini,
            install_phantom,
            run_phantom
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
    fn markers_live_in_the_app_cache_dir() {
        let marker = marker_file("gapps_installed");
        assert_eq!(marker.file_name().unwrap(), "gapps_installed");
        assert_eq!(marker.parent().unwrap().file_name().unwrap(), CACHE_SUBDIR);
    }

    #[test]
    fn elevated_script_keeps_user_environment_and_announces_itself() {
        let body = elevate_env("echo body");
        assert!(body.contains("export HOME='"));
        assert!(body.contains("export DROIDDECK_CACHE='"));
        assert!(body.contains(CACHE_SUBDIR));
        assert!(body.contains(ELEVATED_SENTINEL));
        assert!(body.contains("droiddeck_marker()"));
        assert!(body.trim_end().ends_with("echo body"));
    }

    /// Regression guard: elevated shells run with a restrictive umask, and markers
    /// created by root are unreadable for the app unless they are chmodded back.
    #[test]
    fn markers_stay_readable_for_the_app_under_a_restrictive_umask() {
        use std::os::unix::fs::PermissionsExt;

        let cache_root =
            std::env::temp_dir().join(format!("droiddeck-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache_root);

        std::env::set_var("XDG_CACHE_HOME", &cache_root);
        let script = elevate_env("droiddeck_marker gapps_installed\n");
        std::env::remove_var("XDG_CACHE_HOME");

        let output = Command::new("bash")
            .arg("-c")
            .arg(format!("umask 077\n{}", script))
            .output()
            .expect("failed to run bash");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let marker = cache_root.join(CACHE_SUBDIR).join("gapps_installed");
        let contents =
            std::fs::read_to_string(&marker).expect("the app must be able to read its own marker");
        assert_eq!(contents.trim(), "installed");

        let mode = std::fs::metadata(&marker).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o644,
            "marker should not keep the root-only umask permissions"
        );

        let _ = std::fs::remove_dir_all(&cache_root);
    }

    #[test]
    fn elevated_script_is_valid_bash() {
        let body = elevate_env("echo body\nif waydroid init; then\n  echo ok\nfi\n");
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
    fn resolve_program_only_returns_existing_files() {
        assert!(resolve_program("bash").is_some());
        assert!(resolve_program("droiddeck-no-such-binary-12345").is_none());
    }

    #[test]
    fn elevated_command_skips_missing_helpers() {
        assert!(elevated_command("droiddeck-no-such-binary-12345", "true").is_none());
        let command = elevated_command("bash", "true").expect("bash should be resolvable");
        assert!(command.get_args().any(|arg| arg == "-c"));
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
}
