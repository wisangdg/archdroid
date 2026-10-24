//! Android shell access through ADB.
//!
//! `waydroid shell` requires root (`actionNeedRoot` in waydroid 1.6.3
//! `tools/__init__.py`), but the container's ADB daemon does not: the host only
//! needs its own `adb` and the container IP. App controls therefore go through
//! `adb shell` instead of a polkit prompt per action.
//!
//! Two details make this less trivial than `adb shell`:
//!
//! * A frozen LXC container accepts an ADB connection but never answers a
//!   shell request, so a passive probe must check `waydroid status` first and
//!   return [`AppError::SessionFrozen`] instead of hanging for the timeout.
//!   Explicit user actions opt into resuming through the system D-Bus API.
//! * The first connection is often `unauthorized`. The host public key is added
//!   to the container's `adb_keys` file and the transport is reconnected once.
//!
//! ADB operations are serialized: resuming and re-freezing a container around
//! two concurrent shells would let one of them freeze the other mid-command.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{AppError, Result};
use crate::resolve_program;
use crate::waydroid::{self, Waydroid, TIMEOUT_QUICK};

const HINT: &str = "Install adb (android-tools) to run Android commands, or use root access.";
const WAYDROID_ADB_PORT: u16 = 5555;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const RESUME_TIMEOUT: Duration = Duration::from_secs(2);

/// ponytail: one global gate for every adb operation. The commands are short;
/// if a long-running device command ever needs to overlap with status probes,
/// replace this with a small connection/lease object.
static ADB_GATE: Mutex<()> = Mutex::new(());

/// Run one arbitrary device shell line. That is the point of `adb_cmd`: the
/// caller explicitly asks for a shell command inside Android.
pub fn shell_line(line: &str) -> Result<String> {
    shell(&[line])
}

/// Active variant: an explicit user command may resume a frozen container.
pub fn shell_line_active(line: &str) -> Result<String> {
    shell_active(&[line])
}

/// Passive shell: never changes the container state. Used by status probes and
/// read-only screens, where a frozen container should degrade to "not now"
/// instead of unfreezing Android in the background.
pub fn shell(args: &[&str]) -> Result<String> {
    shell_mode(args, false)
}

/// Explicit user action: resume a frozen container first, then freeze it again
/// afterwards if this call was the one that resumed it.
pub fn shell_active(args: &[&str]) -> Result<String> {
    shell_mode(args, true)
}

fn shell_mode(args: &[&str], allow_unfreeze: bool) -> Result<String> {
    let _gate = ADB_GATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Resolve adb first: the status/DBus path must not start when the host has
    // no way to run the command anyway.
    let adb = resolve_program("adb").ok_or_else(|| AppError::not_found("adb", HINT))?;
    let waydroid = Waydroid::detect()?;
    let mut status = waydroid.status()?;

    if !waydroid::session_running(&status) {
        return Err(AppError::session_not_running(
            "Start the session from the dashboard, then try again.",
        ));
    }

    let mut unfroze = false;
    let outcome = (|| -> Result<String> {
        if waydroid::container_frozen(&status) {
            if !allow_unfreeze {
                return Err(AppError::session_frozen(
                    "Open the Waydroid window or launch an app to resume it, then try again.",
                ));
            }
            unfroze = true;
            status = resume_container(&waydroid)?;
        }

        let ip = waydroid::device_ip(&status).ok_or_else(|| {
            AppError::session_not_running(
                "Android is still starting and has no container IP address yet.",
            )
        })?;
        let serial = serial_for_ip(&ip);

        // Target the Waydroid transport explicitly. Without `-s`, a connected
        // phone or a second emulator makes every command fail with "more than
        // one device".
        if allow_unfreeze {
            // Android can ask to suspend again while ADB is connecting and
            // authorizing, so keep checking until the command can actually run.
            ensure_ready_active(&adb, &waydroid, &serial)?;
        } else {
            ensure_device(&adb, &serial)?;
            if waydroid::container_frozen(&waydroid.status()?) {
                return Err(AppError::session_frozen(
                    "Android suspended the container while ADB was connecting. Open the Waydroid window and try again.",
                ));
            }
        }

        let mut argv = vec!["-s".to_string(), serial, "shell".to_string()];
        argv.extend(args.iter().map(|arg| (*arg).to_string()));
        waydroid::run_program(&adb, &argv, TIMEOUT_QUICK, false)
    })();

    if unfroze {
        freeze_container();
    }
    outcome
}

/// Best effort "Android finished booting" probe. A frozen/stopped session is
/// simply "not booted yet"; it must not wait for a shell timeout.
pub fn booted() -> bool {
    shell(&["getprop", "sys.boot_completed"]).is_ok_and(|output| output.trim() == "1")
}

/// Android package names are letters, digits, `.` and `_`. Everything else is
/// rejected before it can reach `am force-stop` as an argv word, because adb
/// joins the words into one device shell line.
pub fn is_valid_package(package: &str) -> bool {
    !package.is_empty()
        && package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

fn serial_for_ip(ip: &str) -> String {
    if ip.contains(':') {
        ip.to_string()
    } else {
        format!("{ip}:{WAYDROID_ADB_PORT}")
    }
}

fn wait_for_resume(waydroid: &Waydroid) -> Result<String> {
    let deadline = Instant::now() + RESUME_TIMEOUT;
    loop {
        let status = waydroid.status()?;
        if !waydroid::session_running(&status) {
            return Err(AppError::session_not_running(
                "The session stopped while the container was resuming.",
            ));
        }
        if waydroid::container_running(&status) && waydroid::device_ip(&status).is_some() {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(AppError::session_frozen(
                "The container did not resume in time. Open the Waydroid window and try again.",
            ));
        }
        thread::sleep(Duration::from_millis(200));
    }
}

/// Unfreeze, then retry if Android immediately requested suspend again. A
/// single pending suspend can re-freeze the container in the same second; the
/// next `Unfreeze` usually sticks, and a bounded retry turns a confusing hang
/// into either a working command or a fast, typed error.
fn resume_container(waydroid: &Waydroid) -> Result<String> {
    let mut last_error = None;
    for attempt in 1..=3 {
        unfreeze_container()?;
        match wait_for_resume(waydroid) {
            Ok(status) => return Ok(status),
            Err(error) => {
                if error.code() == crate::error::code::SESSION_NOT_RUNNING {
                    return Err(error);
                }
                log::warn!("container did not stay resumed (attempt {attempt}): {error}");
                last_error = Some(error);
                thread::sleep(Duration::from_millis(200));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| AppError::session_frozen("The container did not resume.")))
}

/// ADB setup (connect, auth-key merge, reconnect) can take a few seconds, which
/// is long enough for Android to suspend the container again. Resume and retry
/// a bounded number of times instead of letting the shell hang on a frozen LXC.
fn ensure_ready_active(adb: &Path, waydroid: &Waydroid, serial: &str) -> Result<()> {
    for _ in 0..3 {
        if waydroid::container_frozen(&waydroid.status()?) {
            resume_container(waydroid)?;
        }

        match ensure_device(adb, serial) {
            Ok(()) => {
                if !waydroid::container_frozen(&waydroid.status()?) {
                    return Ok(());
                }
            }
            Err(error) => {
                if !waydroid::container_frozen(&waydroid.status()?) {
                    return Err(error);
                }
            }
        }
    }

    Err(AppError::session_frozen(
        "Android suspended the container again before the command could run. Open the Waydroid window and try again.",
    ))
}

/// `id.waydro.Container` exposes Freeze/Unfreeze on the system bus and its D-Bus
/// policy allows the desktop user, which is exactly how `waydroid prop get/set`
/// unfreezes a container without root. `dbus-send` avoids pulling in a D-Bus
/// crate for two one-shot calls.
fn dbus_container(method: &str) -> Result<String> {
    let dbus = resolve_program("dbus-send").ok_or_else(|| {
        AppError::session_frozen(
            "dbus-send is not available, so the suspended container cannot be resumed automatically. \
             Open the Waydroid window to resume it.",
        )
    })?;
    let args = vec![
        "--system".to_string(),
        "--print-reply".to_string(),
        "--dest=id.waydro.Container".to_string(),
        "/ContainerManager".to_string(),
        format!("id.waydro.ContainerManager.{method}"),
    ];
    waydroid::run_program(&dbus, &args, 10, false)
}

fn unfreeze_container() -> Result<()> {
    dbus_container("Unfreeze").map(|_| ()).map_err(|error| {
        AppError::session_frozen(format!("Could not resume the container: {error}"))
    })
}

fn freeze_container() {
    if let Err(error) = dbus_container("Freeze") {
        log::warn!("could not freeze the Waydroid container after an ADB command: {error}");
    }
}

/// Parse `adb devices` output for one serial. Keeping this pure makes the
/// physical-phone + Waydroid case testable without an adb server.
fn parse_device_state<'a>(devices: &'a str, serial: &str) -> Option<&'a str> {
    devices.lines().find_map(|line| {
        let mut columns = line.split_whitespace();
        if columns.next() == Some(serial) {
            columns.next()
        } else {
            None
        }
    })
}

fn device_state(adb: &Path, serial: &str) -> Result<Option<String>> {
    let output = waydroid::run_program(adb, &["devices".to_string()], 5, false)?;
    Ok(parse_device_state(&output, serial).map(str::to_string))
}

fn reconnect(adb: &Path, serial: &str) {
    // `adb connect` exits 0 even when it prints "failed to connect"; the state
    // poll after this call is the real success check.
    let _ = waydroid::run_program(
        adb,
        &["disconnect".to_string(), serial.to_string()],
        5,
        false,
    );
    let _ = waydroid::run_program(adb, &["connect".to_string(), serial.to_string()], 5, false);
}

fn ensure_device(adb: &Path, serial: &str) -> Result<()> {
    let deadline = Instant::now() + CONNECT_TIMEOUT;
    let mut authorized = false;
    let mut last_state;

    loop {
        let state = device_state(adb, serial)?;
        if state.as_deref() == Some("device") {
            return Ok(());
        }
        last_state = state;

        if last_state.as_deref() == Some("unauthorized") && !authorized {
            authorize_host(adb, serial).map_err(|error| {
                AppError::adb_unauthorized(format!(
                    "Could not add this host's ADB key to the container: {error}"
                ))
            })?;
            authorized = true;
        }

        if Instant::now() >= deadline {
            break;
        }

        reconnect(adb, serial);
        thread::sleep(Duration::from_millis(500));
    }

    Err(match last_state.as_deref() {
        Some("unauthorized") => AppError::adb_unauthorized(
            "The host key was added but the device still reports unauthorized. Restart Waydroid, then try again.",
        ),
        Some("offline") => AppError::command_failed(format!(
            "adb sees the Waydroid device at {serial} but it is offline. Resume the container and try again."
        )),
        Some(other) => AppError::command_failed(format!(
            "adb could not bring the Waydroid device at {serial} online (last state: {other})."
        )),
        None => AppError::command_failed(format!(
            "adb could not reach the Waydroid device at {serial}. Check that Waydroid is running and has an IP address."
        )),
    })
}

fn authorize_host(adb: &Path, serial: &str) -> Result<()> {
    let keys_path = device_adb_keys_path()?;
    let (key, generated) = host_public_key(adb)?;
    append_key(&keys_path, &key)?;

    if generated {
        // The adb server read its host key set at startup; a key generated
        // afterwards is not used until the server restarts. The next state poll
        // starts it again.
        let _ = waydroid::run_program(adb, &["kill-server".to_string()], 5, false);
    }

    log::info!(
        "authorized this host's adb key for {serial} in {}",
        keys_path.display()
    );
    Ok(())
}

fn host_public_key(adb: &Path) -> Result<(String, bool)> {
    let home = dirs::home_dir().ok_or_else(|| {
        AppError::command_failed("Cannot locate the home directory to find the adb key.")
    })?;
    let android = home.join(".android");
    let public = android.join("adbkey.pub");
    let private = android.join("adbkey");

    if public.is_file() {
        if !private.is_file() {
            return Err(AppError::command_failed(format!(
                "{} is missing while {} exists. Restore the private key, or move both files aside so ArchDroid can generate a new pair.",
                private.display(),
                public.display()
            )));
        }
        let key = std::fs::read_to_string(&public)?;
        let key = key.trim();
        if key.is_empty() {
            return Err(AppError::command_failed(format!(
                "{} is empty; adb cannot use it.",
                public.display()
            )));
        }
        return Ok((key.to_string(), false));
    }

    if private.exists() {
        // `adb keygen` would overwrite the private key and silently invalidate
        // every device that already trusts it. Ask for a deliberate recovery.
        return Err(AppError::command_failed(format!(
            "{} is missing while {} exists. Restore the public key, or move both files aside so ArchDroid can generate a new pair.",
            public.display(),
            private.display()
        )));
    }

    if !android.exists() {
        std::fs::create_dir_all(&android)?;
        std::fs::set_permissions(&android, std::fs::Permissions::from_mode(0o700))?;
    }

    if let Err(error) = waydroid::run_program(
        adb,
        &["keygen".to_string(), private.to_string_lossy().to_string()],
        10,
        false,
    ) {
        // Do not leave a half-generated pair behind: the next attempt would
        // otherwise trip the "private exists, public missing" guard above.
        let _ = std::fs::remove_file(&private);
        let _ = std::fs::remove_file(&public);
        return Err(error);
    }

    let key = std::fs::read_to_string(&public)?;
    let key = key.trim();
    if key.is_empty() {
        return Err(AppError::command_failed(format!(
            "adb generated an empty public key at {}.",
            public.display()
        )));
    }
    Ok((key.to_string(), true))
}

fn device_adb_keys_path() -> Result<PathBuf> {
    let data = dirs::data_local_dir().ok_or_else(|| {
        AppError::command_failed("Cannot locate the user data directory (XDG_DATA_HOME).")
    })?;
    Ok(data.join("waydroid/data/misc/adb/adb_keys"))
}

/// Add one public key to the container's authorized-key file. Appending keeps
/// other hosts authorized; the file is plain text with one key per line.
fn append_key(path: &Path, key: &str) -> Result<()> {
    let key = key.trim();
    if key.is_empty() || key.contains('\n') || key.contains('\r') {
        return Err(AppError::command_failed(
            "Refusing to write an invalid ADB public key.",
        ));
    }

    let existing = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    if existing.lines().any(|line| line.trim() == key) {
        return Ok(());
    }

    let existed = path.exists();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.write_all(key.as_bytes())?;
    file.write_all(b"\n")?;

    if !existed {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_file(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("archdroid-adb-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("adb_keys")
    }

    #[test]
    fn package_names_are_checked_before_they_reach_the_shell() {
        assert!(is_valid_package("org.fdroid.fdroid"));
        assert!(is_valid_package("com.android.vending"));
        assert!(!is_valid_package(""));
        assert!(!is_valid_package("org.fdroid.fdroid; rm -rf /"));
        assert!(!is_valid_package("com.foo bar"));
        assert!(!is_valid_package("com.foo\nam start"));
    }

    #[test]
    fn the_waydroid_serial_gets_the_adb_port() {
        assert_eq!(serial_for_ip("192.168.240.112"), "192.168.240.112:5555");
        assert_eq!(
            serial_for_ip("192.168.240.112:5555"),
            "192.168.240.112:5555"
        );
    }

    #[test]
    fn adb_devices_is_parsed_per_serial_not_per_first_line() {
        let output = "List of devices attached\n\
                      emulator-5554\tdevice\n\
                      192.168.240.112:5555\tunauthorized\n\
                      \n";
        assert_eq!(
            parse_device_state(output, "192.168.240.112:5555"),
            Some("unauthorized")
        );
        assert_eq!(parse_device_state(output, "emulator-5554"), Some("device"));
        assert_eq!(parse_device_state(output, "10.0.0.2:5555"), None);
        assert_eq!(parse_device_state("List of devices attached\n", "x"), None);
    }

    #[test]
    fn appending_the_host_key_preserves_other_keys_and_does_not_duplicate() {
        let path = key_file("append");
        std::fs::write(&path, "existing-key").unwrap();

        append_key(&path, "host-key").unwrap();
        append_key(&path, "host-key").unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "existing-key\nhost-key\n");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_new_adb_keys_file_is_readable_by_the_container() {
        let path = key_file("new-file");
        append_key(&path, "host-key").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "host-key\n");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn multiline_key_material_is_rejected() {
        let path = key_file("multiline");
        let error = append_key(&path, "one\ntwo").unwrap_err();
        assert_eq!(error.code(), crate::error::code::COMMAND_FAILED);
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
