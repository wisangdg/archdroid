//! Phantom, the keymapper ArchDroid installs and drives: where it is, the
//! Keymapping page's status, Launch, Save & Apply (load a keymap, starting the
//! root daemon when it is not running) and Stop. The daemon itself is in
//! `phantom_daemon`.

use crate::error::AppError;
use crate::{find_terminal_emulator, resolve_program, trusted_program, LAUNCH_GRACE};
use crate::{keymap, off_main_thread, phantom_daemon, watch_launch, waydroid, CommandResult};
use crate::{PHANTOM_BIN_SHA256, PHANTOM_VERSION};
use std::path::Path;

/// Phantom's installer puts its binaries in `$HOME/.local/bin`, which is often
/// not on the PATH a desktop launcher gives the app, so look there as well.
pub(crate) fn find_phantom(name: &str) -> Option<std::path::PathBuf> {
    resolve_program(name)
        .or_else(|| trusted_program(name))
        .or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".local/bin").join(name))
                .filter(|p| p.is_file())
        })
}

/// What the Keymapping page shows before anyone presses Launch: "installed"
/// (GUI found), "partial" (only the terminal client) or "not_installed".
fn phantom_state() -> CommandResult {
    if find_phantom("phantom-gui").is_some() {
        CommandResult::report("installed", "Phantom is installed.")
    } else if find_phantom("phantom").is_some() {
        CommandResult::report(
            "partial",
            "Only the Phantom terminal client is installed; Launch opens it in a terminal.",
        )
    } else {
        CommandResult::report("not_installed", "Phantom is not installed.")
    }
}

#[tauri::command]
pub(crate) async fn phantom_status() -> CommandResult {
    off_main_thread(phantom_state).await
}

/// Load a game's saved keymap into the running Phantom daemon. When the daemon
/// has to be started (as root), the first call answers `APPROVAL_REQUIRED`
/// with what would run; the editor asks, and calls again with the SHA-256 the
/// user approved as `daemon_sha256`.
#[tauri::command]
pub(crate) async fn keymap_apply(package: String, daemon_sha256: Option<String>) -> CommandResult {
    off_main_thread(move || apply_keymap(&package, daemon_sha256.as_deref())).await
}

/// Load a game's saved keymap into Phantom, starting or restarting the daemon
/// with the keymap's screen size when it needs to and `approved` is set.
fn apply_keymap(package: &str, approved: Option<&str>) -> CommandResult {
    if approved.is_some_and(|sha256| !phantom_daemon::is_sha256(sha256)) {
        return AppError::invalid_input("The approved Phantom SHA-256 is not a SHA-256.").into();
    }
    let path = match keymap::profile_path(package) {
        Ok(path) if path.is_file() => path,
        Ok(_) => return AppError::command_failed("Save the keymap first.").into(),
        Err(error) => return error.into(),
    };
    let Some(phantom) = find_phantom("phantom") else {
        return AppError::not_found("phantom", "Install Phantom from the Keymapping page first.")
            .into();
    };
    let args = ["load".to_string(), path.to_string_lossy().into_owned()];
    let load = || waydroid::run_program(&phantom, &args, waydroid::TIMEOUT_QUICK, None);
    // The size Android renders at, which the keymap was drawn on.
    let screen = keymap::load(package).ok().flatten().and_then(|profile| {
        let side = |key: &str| {
            profile["screen"][key]
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
        };
        Some((side("width")?, side("height")?))
    });
    let result = match load() {
        // `load` only talks to the daemon, which reads its touchscreen size
        // from Phantom's config once, at start: set the size, then start
        // (password prompt) or restart it, and load again.
        Err(error) => {
            let detail = error.to_string();
            let missing = detail.contains("cannot connect to daemon");
            let wrong_size = detail.contains("does not match daemon touchscreen");
            if !missing && !wrong_size {
                Err(error)
            } else if let Some(sha256) = approved {
                let restarted = (|| {
                    if let Some((width, height)) = screen {
                        phantom_daemon::set_screen(width, height)?;
                    }
                    if wrong_size {
                        phantom_daemon::stop(&phantom)?;
                    }
                    phantom_daemon::start(&phantom, sha256)
                })();
                if let Err(error) = restarted {
                    return AppError::command_failed(format!(
                        "The keymap is saved, but the Phantom daemon was not started: {error}"
                    ))
                    .into();
                }
                load()
            } else {
                return daemon_consent(&phantom);
            }
        }
        ok => ok,
    };
    match result {
        Ok(_) => CommandResult::with_message("Keymap loaded into Phantom."),
        Err(error) => AppError::command_failed(phantom_load_failure(&error.to_string())).into(),
    }
}

/// Starting the daemon runs Phantom as root, so the user approves it first.
/// `data` tells the editor what would run: the program, its SHA-256, and
/// whether that is the Phantom release ArchDroid installs and verified.
fn daemon_consent(phantom: &Path) -> CommandResult {
    let sha256 = match phantom_daemon::sha256_of(phantom) {
        Ok(sha256) => sha256,
        Err(error) => return error.into(),
    };
    let consent = serde_json::json!({
        "path": phantom.to_string_lossy(),
        "sha256": sha256,
        "verified": sha256 == PHANTOM_BIN_SHA256,
        "version": PHANTOM_VERSION,
        "copy": phantom_daemon::DAEMON_COPY,
    });
    let mut result = CommandResult::from(AppError::approval_required(
        "Starting the Phantom daemon needs administrator rights.",
    ));
    result.data = Some(consent.to_string());
    result
}

/// At start-up, stop a daemon an earlier ArchDroid left holding the keyboard.
pub(crate) fn release_stale_daemon() {
    let engine_running = waydroid::Waydroid::detect()
        .and_then(|waydroid| waydroid.status())
        .is_ok_and(|status| waydroid::session_running(&status));
    phantom_daemon::release_stale(find_phantom("phantom").as_deref(), engine_running);
}

/// What Save & Apply says when `phantom load` fails. `load` only talks to
/// the root daemon; opening Phantom (Keymapping → Launch) does not start it.
fn phantom_load_failure(output: &str) -> String {
    let detail = strip_ansi(output);
    if detail.contains("cannot connect to daemon") {
        return "The Phantom daemon started but did not take the keymap, so it was saved but not loaded. Press Save & Apply again.".to_string();
    }
    format!(
        "Phantom did not load the keymap: {detail} The keymap is saved; press Save & Apply again once Phantom runs with the same screen size."
    )
}

/// Phantom logs with terminal colours (`ESC[31m`), which a toast shows as
/// boxes.
pub(crate) fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI: ESC [ parameters, then one final byte in @..~.
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

pub(crate) fn stop_phantom_daemon() {
    if !phantom_daemon::running() {
        return;
    }
    if let Some(phantom) = find_phantom("phantom") {
        if let Err(error) = phantom_daemon::stop(&phantom) {
            log::warn!("could not stop the Phantom daemon: {error}");
        }
    }
}

/// `"running"` while a Phantom daemon holds the keyboard, else `"stopped"`.
#[tauri::command]
pub(crate) async fn phantom_daemon_state() -> CommandResult {
    off_main_thread(|| {
        let state = if phantom_daemon::running() {
            "running"
        } else {
            "stopped"
        };
        CommandResult::with_data(state.to_string())
    })
    .await
}

/// Keymapping → Stop: release the keyboard (the same as F2 in the game).
#[tauri::command]
pub(crate) async fn phantom_daemon_stop() -> CommandResult {
    off_main_thread(|| {
        let Some(phantom) = find_phantom("phantom") else {
            return AppError::not_found("phantom", "Phantom is not installed.").into();
        };
        match phantom_daemon::stop(&phantom) {
            Ok(()) => CommandResult::with_message("Phantom stopped; the keyboard is released."),
            Err(error) => error.into(),
        }
    })
    .await
}

#[tauri::command]
pub(crate) async fn run_phantom() -> CommandResult {
    off_main_thread(run_phantom_sync).await
}

fn run_phantom_sync() -> CommandResult {
    // 1. Prefer phantom-gui directly without a terminal wrapper
    if let Some(gui_path) = find_phantom("phantom-gui") {
        return match crate::hostenv::host_command(&gui_path).spawn() {
            Ok(child) => match watch_launch(child, LAUNCH_GRACE) {
                Ok(()) => CommandResult::with_message("Launched Phantom Keymapper GUI."),
                Err(status) => AppError::command_failed(format!(
                    "Phantom GUI exited right after launch ({status}). Run phantom-gui from a terminal to see why."
                ))
                .into(),
            },
            Err(err) => {
                AppError::from_spawn("phantom-gui", "Failed to spawn Phantom GUI.", err).into()
            }
        };
    }

    // 2. Fallback to CLI client inside terminal if phantom-gui is missing
    if let Some(cli_path) = find_phantom("phantom") {
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
            "gnome-terminal" => crate::hostenv::host_command("gnome-terminal")
                .args(["--", "bash", "-c", &cli_str])
                .spawn(),
            _ => crate::hostenv::host_command(&term)
                .args(["-e", "bash", "-c", &cli_str])
                .spawn(),
        };

        return match result {
            Ok(child) => match watch_launch(child, LAUNCH_GRACE) {
                Ok(()) => {
                    CommandResult::with_message(&format!("Launched Phantom Keymapper via {term}."))
                }
                Err(status) => AppError::command_failed(format!(
                    "{term} exited right after launch ({status}) while starting Phantom."
                ))
                .into(),
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Live: `ARCHDROID_KEYMAP_PKG=<package> cargo test live_apply_keymap -- --ignored`
    /// with the package's keymap saved and a polkit agent: Save & Apply's path.
    #[test]
    #[ignore]
    fn live_apply_keymap() {
        let package = std::env::var("ARCHDROID_KEYMAP_PKG").expect("ARCHDROID_KEYMAP_PKG");
        let phantom = find_phantom("phantom").expect("phantom");
        let sha256 = phantom_daemon::sha256_of(&phantom).unwrap();
        let result = apply_keymap(&package, Some(&sha256));
        assert_eq!(result.status, "success", "{:?}", result.message);
    }

    #[test]
    fn a_missing_phantom_daemon_says_how_to_start_it_without_colour_codes() {
        let output = "\"/home/u/.local/bin/phantom\" failed with exit code 1: \u{1b}[2m2026-11-18T11:52:54.464955Z\u{1b}[0m \u{1b}[31mERROR\u{1b}[0m IPC error: cannot connect to daemon: No such file or directory (os error 2)";
        let message = phantom_load_failure(output);
        assert!(message.starts_with("The Phantom daemon started but did not take"));
        assert!(message.contains("Save & Apply again"));

        let other = phantom_load_failure("\u{1b}[31mERROR\u{1b}[0m profile has 11 slots");
        assert!(other.contains("ERROR profile has 11 slots"));
        assert!(!other.contains('\u{1b}'));
    }
}
