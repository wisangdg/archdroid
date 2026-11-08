//! Logs: the Waydroid log viewer, the application log panel, the Diagnostic
//! Bundle command, and the redaction every log line and support paste passes.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::stream::{stream_privileged_script, stream_user_script, Completion};
use crate::{bundle, off_main_thread, sh_quote, waydroid, CommandResult};

pub(crate) const WAYDROID_LOG: &str = "/var/lib/waydroid/waydroid.log";

/// Root is only needed when the log exists and the user may not read it.
/// Waydroid creates it `0644` on usual hosts, and a missing file needs no root
/// to report either.
pub(crate) fn waydroid_log_needs_root(path: &Path) -> bool {
    matches!(
        std::fs::File::open(path),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied
    )
}

pub(crate) fn waydroid_log_script(path: &str) -> String {
    let path = sh_quote(path);
    format!(
        r#"
if [ -f {path} ]; then
    tail -n 1000 {path}
else
    echo "No logs found at "{path}
fi
"#
    )
}

#[tauri::command]
pub(crate) async fn waydroid_log(app: AppHandle) -> CommandResult {
    let script = waydroid_log_script(WAYDROID_LOG);
    if waydroid_log_needs_root(Path::new(WAYDROID_LOG)) {
        stream_privileged_script(app, script, Completion::None, waydroid::TIMEOUT_NORMAL)
            .await
            .into_command_result()
    } else {
        stream_user_script(app, script, waydroid::TIMEOUT_NORMAL)
            .await
            .into_command_result()
    }
}

/// Mask what must not reach a log or a support paste: the home directory and
/// the value of `token=`/`password=`-style assignments or `Bearer` headers.
pub(crate) fn redact_text(text: &str) -> String {
    let home = std::env::var_os("HOME").and_then(|h| h.into_string().ok());
    redact_with_home(text, home.as_deref())
}

/// [`redact_text`] with the home directory passed in, so tests never have to
/// mutate the process environment (which other threads read concurrently).
pub(crate) fn redact_with_home(text: &str, home: Option<&str>) -> String {
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
pub(crate) fn app_log_file(app: &AppHandle) -> Option<PathBuf> {
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
pub(crate) async fn app_log_read(app: AppHandle) -> CommandResult {
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
pub(crate) struct DiagnosticBundle {
    /// Where the file was written, with the home directory shown as `~`.
    path: String,
    content: String,
}

#[tauri::command]
pub(crate) async fn diagnostic_bundle_create(
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
pub(crate) fn app_log_location(app: AppHandle) -> CommandResult {
    match app.path().app_log_dir() {
        Ok(dir) => CommandResult::with_data(redact_text(&dir.to_string_lossy())),
        Err(e) => AppError::Io(std::io::Error::other(e.to_string())).into(),
    }
}

#[tauri::command]
pub(crate) async fn app_log_clear(app: AppHandle) -> CommandResult {
    off_main_thread(move || match app_log_file(&app) {
        Some(path) => match std::fs::write(&path, b"") {
            Ok(()) => CommandResult::success(),
            Err(e) => AppError::Io(e).into(),
        },
        None => CommandResult::success(),
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_waydroid_log_needs_root_only_when_it_is_unreadable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("archdroid-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("waydroid.log");
        assert!(!waydroid_log_needs_root(&log), "missing file");
        std::fs::write(&log, "line\n").unwrap();
        std::fs::set_permissions(&log, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!waydroid_log_needs_root(&log), "0644 file");
        std::fs::set_permissions(&log, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Root reads any file, so the unreadable case only holds for a user.
        let is_root = std::fs::read_to_string("/proc/self/status")
            .map(|s| {
                s.lines()
                    .any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0"))
            })
            .unwrap_or(false);
        assert_eq!(waydroid_log_needs_root(&log), !is_root);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_waydroid_log_script_tails_the_quoted_path() {
        let script = waydroid_log_script("/var/lib/way droid/waydroid.log");
        assert!(
            script.contains("tail -n 1000 '/var/lib/way droid/waydroid.log'"),
            "{script}"
        );
        let check = std::process::Command::new("bash")
            .args(["-n", "-c", &script])
            .status()
            .expect("bash");
        assert!(check.success(), "{script}");
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
}
