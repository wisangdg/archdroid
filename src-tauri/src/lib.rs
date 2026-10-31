use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufReader, IsTerminal, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager};

mod adb;
mod bundle;
mod capabilities;
#[cfg(feature = "dev-commands")]
mod device;
mod diagnostics;
mod elevate;
mod error;
mod markers;
mod platform;
mod preflight;
mod scripts;
mod stream;
mod waydroid;

use diagnostics::*;
use elevate::*;
use error::AppError;
use markers::*;
use preflight::*;
use scripts::*;
use stream::*;

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
    let home = std::env::var_os("HOME").and_then(|h| h.into_string().ok());
    redact_with_home(text, home.as_deref())
}

/// [`redact_text`] with the home directory passed in, so tests never have to
/// mutate the process environment (which other threads read concurrently).
fn redact_with_home(text: &str, home: Option<&str>) -> String {
    const SENSITIVE: [&str; 6] = ["token", "password", "passwd", "secret", "apikey", "api_key"];
    let mut text = text.to_string();
    if let Some(home) = home.filter(|home| home.len() > 1) {
        text = text.replace(home, "~");
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
        match bundle::read_tail(&path, MAX_LOG_VIEW_BYTES) {
            Ok(text) => CommandResult::with_data(redact_text(&text)),
            Err(e) => AppError::Io(e).into(),
        }
    })
    .await
}

/// What the Settings page shows after writing a diagnostic bundle.
#[derive(Serialize)]
struct DiagnosticBundle {
    /// Where the file was written, with the home directory shown as `~`.
    path: String,
    content: String,
}

#[tauri::command]
async fn diagnostic_bundle_create(
    app: AppHandle,
) -> std::result::Result<DiagnosticBundle, CommandResult> {
    tauri::async_runtime::spawn_blocking(move || bundle::create(&app))
        .await
        .map_err(|e| CommandResult::from(AppError::Io(std::io::Error::other(e.to_string()))))?
        .map(|(path, content)| DiagnosticBundle {
            path: redact_text(&path.to_string_lossy()),
            content,
        })
        .map_err(CommandResult::from)
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

pub(crate) fn sanitize_appimage_env() {
    // If running inside an AppImage, AppRun unconditionally sets PYTHONHOME and
    // PYTHONPATH pointing into the bundle's AppDir. Because the bundle does not
    // ship a full Python runtime, any host tools written in Python (like /usr/bin/waydroid)
    // fail immediately with "Fatal Python error: Failed to import encodings module".
    let in_appimage =
        std::env::var_os("APPDIR").is_some() || std::env::var_os("APPIMAGE").is_some();
    if !in_appimage {
        return;
    }
    let appdir = std::env::var("APPDIR").ok();
    for var in ["PYTHONHOME", "PYTHONPATH"] {
        if std::env::var(var).is_ok_and(|value| points_into_bundle(&value, appdir.as_deref())) {
            std::env::remove_var(var);
        }
    }
}

/// Whether a Python path variable points into the AppImage bundle. Kept free
/// of env access so tests never have to mutate the process environment.
fn points_into_bundle(value: &str, appdir: Option<&str>) -> bool {
    appdir.is_some_and(|dir| value.starts_with(dir)) || value.contains("appimage")
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
            diagnostic_bundle_create,
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
    fn package_preview_rejects_unknown_actions() {
        let result = tauri::async_runtime::block_on(preview_package_install("bogus".to_string()))
            .unwrap_err();
        assert_eq!(result.code.as_deref(), Some(error::code::INVALID_INPUT));
        assert!(result.message.unwrap().contains("init, gapps, or houdini"));
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
        let out = redact_with_home(
            "read /home/alice/.cache/x token=abc123 Bearer xyz\nPASSWORD: hunter2",
            Some("/home/alice"),
        );
        assert!(!out.contains("alice"));
        assert!(!out.contains("abc123") && !out.contains("xyz"));
        assert!(!out.contains("hunter2"));
        assert!(out.contains("~/.cache/x"));
        assert_eq!(
            redact_with_home("plain line", Some("/home/alice")),
            "plain line"
        );
    }

    #[test]
    fn points_into_bundle_flags_appdir_and_appimage_paths() {
        let appdir = Some("/tmp/.mount_App12345");
        assert!(points_into_bundle("/tmp/.mount_App12345/usr/", appdir));
        assert!(points_into_bundle(
            "/tmp/.mount_App12345/usr/share/pyshared/:",
            appdir
        ));
        assert!(points_into_bundle("/tmp/appimage_extracted_1/usr", None));
        assert!(!points_into_bundle("/usr/lib/python3.12", appdir));
        assert!(!points_into_bundle("/usr/lib/python3.12", None));
    }
}
