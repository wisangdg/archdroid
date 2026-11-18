//! Start Phantom's root daemon from ArchDroid, so Save & Apply works without a
//! terminal. Phantom's own start button runs `sudo -E phantom --daemon`, which
//! has no terminal to ask for the password when ArchDroid opened Phantom
//! (`sudo: conversation failed`); pkexec asks through the desktop's polkit
//! agent instead.
//!
//! The daemon learns who it serves from `SUDO_UID`/`SUDO_GID`/`SUDO_USER` and
//! finds the desktop through `XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY` and friends.
//! pkexec clears the environment, so they are passed explicitly. Root never
//! takes a value from this process's environment as is: the IDs and paths come
//! from the kernel and passwd, the display names only when they are plain
//! names.

use std::fs::File;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant};

use crate::elevate::{elevated_command, sh_quote, SAFE_PATH};
use crate::error::{AppError, Result};

/// Time for the password prompt plus the daemon's start-up.
const START_TIMEOUT: Duration = Duration::from_secs(120);

/// pkexec's exit codes for a dismissed prompt and a refused authorization.
const PKEXEC_DISMISSED: i32 = 126;
const PKEXEC_NOT_AUTHORIZED: i32 = 127;

/// The desktop session the daemon has to attach to.
#[derive(Debug, PartialEq)]
pub(crate) struct Session {
    pub uid: u32,
    pub gid: u32,
    pub user: Option<String>,
    pub wayland_display: Option<String>,
    pub display: Option<String>,
    pub hyprland_signature: Option<String>,
    pub session_type: Option<String>,
}

impl Session {
    fn current() -> Self {
        // SAFETY: getuid/getgid cannot fail and touch no memory.
        let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
        let var = |key: &str| std::env::var(key).ok();
        Self {
            uid,
            gid,
            user: std::fs::read_to_string("/etc/passwd")
                .ok()
                .and_then(|passwd| passwd_name(&passwd, uid)),
            wayland_display: var("WAYLAND_DISPLAY").filter(|v| is_name(v)),
            display: var("DISPLAY").filter(|v| is_x_display(v)),
            hyprland_signature: var("HYPRLAND_INSTANCE_SIGNATURE").filter(|v| is_name(v)),
            session_type: var("XDG_SESSION_TYPE").filter(|v| v == "wayland" || v == "x11"),
        }
    }

    fn runtime_dir(&self) -> PathBuf {
        PathBuf::from(format!("/run/user/{}", self.uid))
    }

    /// Where the daemon listens (Phantom's `config::socket_path`).
    fn socket(&self) -> PathBuf {
        self.runtime_dir().join("phantom.sock")
    }
}

/// A socket or instance name: letters, digits, `.`, `_`, `-`.
fn is_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A local X display such as `:0` or `:1.0`.
fn is_x_display(value: &str) -> bool {
    value.strip_prefix(':').is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .split('.')
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// The login name of `uid` in a passwd(5) file.
fn passwd_name(passwd: &str, uid: u32) -> Option<String> {
    passwd.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.len() >= 3 && fields[2].parse::<u32>().ok() == Some(uid) && is_name(fields[0]))
            .then(|| fields[0].to_string())
    })
}

/// The root shell line: `exec` the daemon with only the session variables.
pub(crate) fn daemon_body(phantom: &Path, session: &Session) -> String {
    let runtime_dir = session.runtime_dir();
    let mut vars = vec![
        format!("PATH={}", sh_quote(SAFE_PATH)),
        format!("SUDO_UID={}", session.uid),
        format!("SUDO_GID={}", session.gid),
        format!(
            "XDG_RUNTIME_DIR={}",
            sh_quote(&runtime_dir.to_string_lossy())
        ),
    ];
    let bus = runtime_dir.join("bus");
    if bus.exists() {
        vars.push(format!(
            "DBUS_SESSION_BUS_ADDRESS={}",
            sh_quote(&format!("unix:path={}", bus.display()))
        ));
    }
    let optional = [
        ("SUDO_USER", &session.user),
        ("WAYLAND_DISPLAY", &session.wayland_display),
        ("DISPLAY", &session.display),
        ("HYPRLAND_INSTANCE_SIGNATURE", &session.hyprland_signature),
        ("XDG_SESSION_TYPE", &session.session_type),
    ];
    for (key, value) in optional {
        if let Some(value) = value {
            vars.push(format!("{key}={}", sh_quote(value)));
        }
    }
    format!(
        "exec env -i {} {} --daemon",
        vars.join(" "),
        sh_quote(&phantom.to_string_lossy())
    )
}

/// True once the daemon accepts connections on its socket.
fn answers(socket: &Path) -> bool {
    UnixStream::connect(socket).is_ok()
}

/// Start the daemon as root and wait until it listens. The daemon keeps running
/// after ArchDroid closes, like one started from a terminal; F2 stops it.
pub(crate) fn start(phantom: &Path) -> Result<()> {
    let session = Session::current();
    let socket = session.socket();
    if answers(&socket) {
        return Ok(());
    }

    let log_path = crate::markers::marker_file("phantom-daemon.log");
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let log = File::create(&log_path)?;

    let mut command =
        elevated_command("pkexec", &daemon_body(phantom, &session)).ok_or_else(|| {
            AppError::not_found(
                "pkexec",
                "Install polkit to start the Phantom daemon from ArchDroid.",
            )
        })?;
    command
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log)
        // Its own process group: signals meant for ArchDroid do not reach it.
        .process_group(0);
    let mut child = command.spawn().map_err(|e| {
        AppError::from_spawn(
            "pkexec",
            "Install polkit to start the Phantom daemon from ArchDroid.",
            e,
        )
    })?;
    log::info!(
        "starting the Phantom daemon through pkexec; log: {}",
        log_path.display()
    );

    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if answers(&socket) {
            // Reap pkexec once the daemon ends, so it never lingers as a zombie.
            thread::spawn(move || {
                let _ = child.wait();
            });
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            return Err(AppError::command_failed(match status.code() {
                Some(PKEXEC_DISMISSED) => {
                    "The password prompt was closed, so the Phantom daemon was not started."
                        .to_string()
                }
                Some(PKEXEC_NOT_AUTHORIZED) => {
                    "Starting the Phantom daemon was not authorized.".to_string()
                }
                _ => format!(
                    "The Phantom daemon stopped right after starting ({status}). {}",
                    log_tail(&log_path)
                ),
            }));
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::command_failed(
                "The Phantom daemon did not start within 2 minutes.",
            ));
        }
        thread::sleep(Duration::from_millis(300));
    }
}

/// Ask a running daemon to exit (`phantom shutdown`, over its user-owned
/// socket, no password) and wait until it no longer listens.
pub(crate) fn stop(phantom: &Path) -> Result<()> {
    let socket = Session::current().socket();
    if !answers(&socket) {
        return Ok(());
    }
    crate::waydroid::run_program(
        phantom,
        &["shutdown".to_string()],
        crate::waydroid::TIMEOUT_QUICK,
        None,
    )?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while answers(&socket) {
        if Instant::now() > deadline {
            return Err(AppError::command_failed(
                "The Phantom daemon did not stop within 5 s.",
            ));
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

/// Phantom's config file. The daemon reads its touchscreen size from
/// `[screen]` there once, at start.
fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("phantom/config.toml"))
}

/// Make the daemon's touchscreen the size Android renders at, which is the
/// size the keymap was drawn on. Phantom refuses a profile whose screen
/// differs, and touches would land in the wrong place if it did not. Returns
/// whether the file changed; a missing file is left to the daemon, which
/// writes it from its example on the first start.
pub(crate) fn set_screen(width: u32, height: u32) -> Result<bool> {
    let Some(path) = config_path() else {
        return Ok(false);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let Some(updated) = with_screen(&text, width, height) else {
        return Ok(false);
    };
    let tmp = path.with_extension("toml.archdroid-tmp");
    std::fs::write(&tmp, updated)?;
    std::fs::rename(&tmp, &path)?;
    log::info!(
        "Phantom screen set to {width}x{height} in {}",
        path.display()
    );
    Ok(true)
}

/// `text` with `[screen]` set to `width` x `height`, every other line kept;
/// `None` when it already says so.
pub(crate) fn with_screen(text: &str, width: u32, height: u32) -> Option<String> {
    let wanted = [("width", width), ("height", height)];
    let mut out: Vec<String> = Vec::new();
    let mut in_screen = false;
    let mut seen_section = false;
    let mut found = [false, false];
    let finish_section = |out: &mut Vec<String>, found: &[bool; 2]| {
        // Missing keys go right after the section's last key/value line.
        let at = out
            .iter()
            .rposition(|line| !line.trim().is_empty())
            .map_or(out.len(), |i| i + 1);
        for (i, (key, value)) in wanted.iter().enumerate() {
            if !found[i] {
                out.insert(at, format!("{key} = {value}"));
            }
        }
    };
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if in_screen {
                finish_section(&mut out, &found);
            }
            in_screen = trimmed == "[screen]";
            seen_section |= in_screen;
        } else if in_screen {
            let key = trimmed.split('=').next().unwrap_or("").trim();
            if let Some(i) = wanted.iter().position(|(name, _)| *name == key) {
                found[i] = true;
                out.push(format!("{key} = {}", wanted[i].1));
                continue;
            }
        }
        out.push(line.to_string());
    }
    if in_screen {
        finish_section(&mut out, &found);
    }
    if !seen_section {
        out.push(String::new());
        out.push("[screen]".to_string());
        out.push(format!("width = {width}"));
        out.push(format!("height = {height}"));
    }
    let mut updated = out.join("\n");
    if text.ends_with('\n') {
        updated.push('\n');
    }
    (updated != text).then_some(updated)
}

/// The last lines of the daemon log, without colour codes, for an error message.
fn log_tail(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(3)..].join(" ");
    if tail.is_empty() {
        format!("Its log is empty: {}", path.display())
    } else {
        format!("Phantom said: {}", crate::strip_ansi(&tail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session {
            uid: 1000,
            gid: 1000,
            user: Some("wdg".to_string()),
            wayland_display: Some("wayland-1".to_string()),
            display: Some(":0".to_string()),
            hyprland_signature: Some("abc_123".to_string()),
            session_type: Some("wayland".to_string()),
        }
    }

    #[test]
    fn the_daemon_gets_the_session_and_nothing_else() {
        let body = daemon_body(Path::new("/home/wdg/.local/bin/phantom"), &session());
        assert!(body.starts_with("exec env -i PATH="));
        for expected in [
            "SUDO_UID=1000",
            "SUDO_GID=1000",
            "XDG_RUNTIME_DIR='/run/user/1000'",
            "SUDO_USER='wdg'",
            "WAYLAND_DISPLAY='wayland-1'",
            "DISPLAY=':0'",
            "HYPRLAND_INSTANCE_SIGNATURE='abc_123'",
            "XDG_SESSION_TYPE='wayland'",
        ] {
            assert!(body.contains(expected), "{expected} missing from {body}");
        }
        assert!(body.ends_with("'/home/wdg/.local/bin/phantom' --daemon"));
    }

    #[test]
    fn only_plain_names_from_the_environment_reach_root() {
        assert!(is_name("wayland-1"));
        assert!(!is_name("wayland-1; rm -rf /"));
        assert!(!is_name("../../etc"));
        assert!(!is_name(""));
        assert!(is_x_display(":0"));
        assert!(is_x_display(":1.0"));
        assert!(!is_x_display("host:0"));
        assert!(!is_x_display(":0 $(id)"));
        assert!(!is_x_display(":"));
    }

    const CONFIG: &str = "log_level = \"info\"\n\n# Fullscreen Android touch resolution.\n[screen]\nwidth = 1920\nheight = 1080\n\n[android]\nauto_launch = true\n";

    #[test]
    fn the_screen_is_set_and_everything_else_kept() {
        let updated = with_screen(CONFIG, 2560, 1440).unwrap();
        assert_eq!(
            updated,
            CONFIG.replace("1920", "2560").replace("1080", "1440")
        );
        assert_eq!(with_screen(&updated, 2560, 1440), None, "already right");
    }

    #[test]
    fn a_missing_screen_key_or_section_is_added() {
        let no_height = CONFIG.replace("height = 1080\n", "");
        let updated = with_screen(&no_height, 2560, 1440).unwrap();
        assert!(updated.contains("[screen]\nwidth = 2560\nheight = 1440\n\n[android]"));

        let no_section = "log_level = \"info\"\n";
        assert_eq!(
            with_screen(no_section, 2560, 1440).unwrap(),
            "log_level = \"info\"\n\n[screen]\nwidth = 2560\nheight = 1440\n"
        );
    }

    #[test]
    fn keys_of_other_sections_are_not_touched() {
        let text = "[mouse_touch]\nwidth = 3\n[screen]\nwidth = 1920\nheight = 1080\n";
        let updated = with_screen(text, 2560, 1440).unwrap();
        assert!(updated.starts_with("[mouse_touch]\nwidth = 3\n"));
    }

    /// Live: `ARCHDROID_PHANTOM=/path/phantom cargo test live_phantom_daemon -- --ignored`
    /// in a desktop session with a polkit agent. Starts the real daemon (F2 or
    /// `phantom shutdown` stops it).
    #[test]
    #[ignore]
    fn live_phantom_daemon() {
        let phantom = std::env::var("ARCHDROID_PHANTOM").expect("ARCHDROID_PHANTOM");
        start(Path::new(&phantom)).expect("daemon starts");
        assert!(answers(&Session::current().socket()));
    }

    #[test]
    fn the_user_name_comes_from_passwd() {
        let passwd = "root:x:0:0::/root:/bin/bash\nwdg:x:1000:1000::/home/wdg:/bin/zsh\n";
        assert_eq!(passwd_name(passwd, 1000).as_deref(), Some("wdg"));
        assert_eq!(passwd_name(passwd, 1001), None);
    }
}
