//! Diagnostic bundle: one plain-text report for support requests.
//!
//! Everything is collected without root, rendered as text the user can read
//! before sharing it, and passed through the same redaction as the app log.
//! ArchDroid never uploads it; the user decides where it goes.

use crate::{app_log_file, collect_diagnostics, marker_file, redact_with_home, waydroid};
use crate::{AppError, DiagnosticsReport};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

/// Per-log budget: enough for the last few tasks, small enough to paste.
const LOG_TAIL_BYTES: u64 = 32 * 1024;
const WAYDROID_LOG: &str = "/var/lib/waydroid/waydroid.log";
const MARKERS: [&str; 3] = [
    "gapps_installed",
    "houdini_installed",
    "multitouch_installed",
];

pub(crate) struct BundleInput {
    pub app_version: &'static str,
    pub generated_utc: String,
    pub os: String,
    pub kernel: String,
    pub desktop: String,
    pub waydroid_version: String,
    pub waydroid_status: String,
    pub diagnostics: DiagnosticsReport,
    pub markers: Vec<(&'static str, bool)>,
    pub app_log_tail: String,
    pub waydroid_log_tail: String,
}

/// Render the report and mask the home directory and secret-looking values.
pub(crate) fn render(input: &BundleInput, home: Option<&str>) -> String {
    let mut out = format!(
        "ArchDroid diagnostic bundle\n\
         Version: archdroid {}\n\
         Generated: {}\n\
         Review this file before sharing it. Home paths and token/password values\n\
         are masked. ArchDroid never uploads it.\n\n\
         == System ==\n\
         OS: {}\nKernel: {}\nDesktop: {}\n\n\
         == Waydroid ==\n\
         Version: {}\nStatus:\n{}\n\n\
         == Host diagnostics ==\n",
        input.app_version,
        input.generated_utc,
        input.os,
        input.kernel,
        input.desktop,
        input.waydroid_version,
        input.waydroid_status.trim_end(),
    );
    for item in &input.diagnostics.items {
        out.push_str(&format!(
            "[{}] {}: {} - {}\n",
            item.status, item.title, item.value, item.detail
        ));
        if let Some(fix) = &item.remediation {
            out.push_str(&format!("    fix: {fix}\n"));
        }
    }
    out.push_str(&format!(
        "Package manager: {}\n\n== Add-on markers ==\n",
        input
            .diagnostics
            .package_manager
            .as_deref()
            .unwrap_or("unknown")
    ));
    for (name, present) in &input.markers {
        out.push_str(&format!(
            "{name}: {}\n",
            if *present { "yes" } else { "no" }
        ));
    }
    out.push_str(&format!(
        "\n== ArchDroid log (last {} KiB) ==\n{}\n\n== Waydroid log (last {} KiB) ==\n{}\n",
        LOG_TAIL_BYTES / 1024,
        input.app_log_tail.trim_end(),
        LOG_TAIL_BYTES / 1024,
        input.waydroid_log_tail.trim_end(),
    ));
    redact_with_home(&out, home)
}

/// The last `max_bytes` of a file, decoded lossily.
pub(crate) fn read_tail(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max_bytes)))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// `YYYY-MM-DD HH:MM:SS UTC` from seconds since the epoch, without a date crate.
pub(crate) fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

fn os_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("PRETTY_NAME="))
                .map(|name| name.trim_matches('"').to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn desktop() -> String {
    let var = |key| std::env::var(key).unwrap_or_else(|_| "unset".to_string());
    format!(
        "{} (session: {}, WAYLAND_DISPLAY {})",
        var("XDG_CURRENT_DESKTOP"),
        var("XDG_SESSION_TYPE"),
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            "set"
        } else {
            "unset"
        }
    )
}

/// A log tail, or why it is missing; never escalates to read it.
fn log_tail_or_reason(path: &Path) -> String {
    let name = path.display();
    match read_tail(path, LOG_TAIL_BYTES) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            format!("{name} is not readable without root; not collected.")
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => format!("{name} not present."),
        Err(e) => format!("{name} could not be read: {e}"),
    }
}

/// Collect, render, and write the bundle (mode 0600) to the Downloads folder.
/// Returns the written path and the rendered text.
pub(crate) fn create(app: &AppHandle) -> Result<(PathBuf, String), AppError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let unavailable = |e: AppError| format!("unavailable: {e}");
    let (waydroid_version, waydroid_status) = match waydroid::Waydroid::detect() {
        Ok(tool) => (
            tool.version().unwrap_or_else(unavailable),
            tool.status().unwrap_or_else(unavailable),
        ),
        Err(e) => {
            let reason = unavailable(e);
            (reason.clone(), reason)
        }
    };
    let input = BundleInput {
        app_version: env!("CARGO_PKG_VERSION"),
        generated_utc: format_utc(now),
        os: os_name(),
        kernel: std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .map(|k| k.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        desktop: desktop(),
        waydroid_version,
        waydroid_status,
        diagnostics: collect_diagnostics(),
        markers: MARKERS
            .iter()
            .map(|name| (*name, marker_file(name).exists()))
            .collect(),
        app_log_tail: app_log_file(app)
            .and_then(|path| read_tail(&path, LOG_TAIL_BYTES).ok())
            .unwrap_or_else(|| "(no app log yet)".to_string()),
        waydroid_log_tail: log_tail_or_reason(Path::new(WAYDROID_LOG)),
    };
    let home = std::env::var("HOME").ok();
    let content = render(&input, home.as_deref());

    // `download_dir` only reads XDG user-dirs config; many minimal setups have
    // ~/Downloads without it.
    let dir = dirs::download_dir()
        .or_else(|| {
            dirs::home_dir()
                .map(|home| home.join("Downloads"))
                .filter(|d| d.is_dir())
        })
        .or_else(dirs::home_dir)
        .ok_or_else(|| AppError::command_failed("Could not find a Downloads or home folder."))?;
    let stamp: String = input
        .generated_utc
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let path = dir.join(format!("archdroid-diagnostics-{stamp}.txt"));
    write_private(&path, &content).map_err(AppError::Io)?;
    Ok((path, content))
}

/// Write `content` readable by the owner only: the bundle describes the host.
fn write_private(path: &Path, content: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticItem;

    fn sample() -> BundleInput {
        BundleInput {
            app_version: "9.9.9",
            generated_utc: "2026-11-01 01:02:03 UTC".to_string(),
            os: "Arch Linux".to_string(),
            kernel: "7.2.11-arch1-1".to_string(),
            desktop: "Hyprland (session: wayland, WAYLAND_DISPLAY set)".to_string(),
            waydroid_version: "1.6.3".to_string(),
            waydroid_status: "Session:\tRUNNING\n".to_string(),
            diagnostics: DiagnosticsReport {
                items: vec![DiagnosticItem {
                    key: "kvm".to_string(),
                    title: "KVM".to_string(),
                    status: "error".to_string(),
                    value: "Missing".to_string(),
                    detail: "/dev/kvm not found".to_string(),
                    remediation: Some("Enable virtualization in firmware".to_string()),
                }],
                package_manager: Some("pacman".to_string()),
            },
            markers: vec![("gapps_installed", true), ("houdini_installed", false)],
            app_log_tail: "read /home/alice/.cache/x token=abc123".to_string(),
            waydroid_log_tail: "/var/lib/waydroid/waydroid.log not present.".to_string(),
        }
    }

    #[test]
    fn render_includes_every_section_and_masks_home_and_secrets() {
        let text = render(&sample(), Some("/home/alice"));
        for needle in [
            "Version: archdroid 9.9.9",
            "OS: Arch Linux",
            "Status:\nSession:\tRUNNING",
            "[error] KVM: Missing - /dev/kvm not found",
            "    fix: Enable virtualization in firmware",
            "Package manager: pacman",
            "gapps_installed: yes",
            "houdini_installed: no",
            "read ~/.cache/x token=[redacted]",
            "waydroid.log not present.",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
        assert!(!text.contains("/home/alice"));
        assert!(!text.contains("abc123"));
    }

    #[test]
    fn format_utc_matches_known_dates() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00:00 UTC");
        // Release key creation time (docs/RELEASE-AND-UPDATES.md), checked with `date -u`.
        assert_eq!(format_utc(1_793_447_911), "2026-10-31 11:58:31 UTC");
    }

    #[test]
    fn read_tail_keeps_only_the_last_bytes() {
        let dir = std::env::temp_dir().join(format!("archdroid-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("log");
        std::fs::write(&path, "0123456789").unwrap();
        assert_eq!(read_tail(&path, 4).unwrap(), "6789");
        assert_eq!(read_tail(&path, 100).unwrap(), "0123456789");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_or_missing_log_is_explained_not_escalated() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("archdroid-wlog-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("missing.log");
        assert!(log_tail_or_reason(&missing).ends_with("missing.log not present."));

        let locked = dir.join("locked.log");
        std::fs::write(&locked, "secret").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let text = log_tail_or_reason(&locked);
        let _ = std::fs::remove_dir_all(&dir);
        // Root ignores file modes, so only a non-root run can check the message.
        if text != "secret" {
            assert!(
                text.ends_with("is not readable without root; not collected."),
                "{text}"
            );
        }
    }

    #[test]
    fn bundle_file_is_private_to_the_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("archdroid-bundle-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bundle.txt");
        write_private(&path, "report").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(mode, 0o600);
    }
}
