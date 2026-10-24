//! Typed errors shared by the backend commands.
//!
//! Commands used to build a `status`/`message` pair by hand, which made "waydroid
//! is not installed", "the image was never initialised" and "the command exited
//! non-zero" indistinguishable to the UI. `AppError` names the failure once, and
//! the wire type the UI reads (`CommandResult`) is derived from it in one place.

use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, AppError>;

/// Stable identifiers the UI can branch on instead of matching prose.
pub mod code {
    pub const NOT_FOUND: &str = "not_found";
    pub const NOT_INITIALIZED: &str = "not_initialized";
    pub const ELEVATION_DENIED: &str = "elevation_denied";
    pub const TIMEOUT: &str = "timeout";
    pub const CANCELLED: &str = "cancelled";
    pub const INVALID_INPUT: &str = "invalid_input";
    pub const EXIT: &str = "exit";
    pub const IO: &str = "io";
    pub const SESSION_NOT_RUNNING: &str = "session_not_running";
    pub const SESSION_FROZEN: &str = "session_frozen";
    pub const ADB_UNAUTHORIZED: &str = "adb_unauthorized";
    pub const COMMAND_FAILED: &str = "command_failed";
}

#[derive(Debug)]
pub enum AppError {
    /// A program the backend needs is not on PATH.
    NotFound { program: String, hint: String },
    /// Waydroid is installed but its image was never initialised.
    NotInitialized { detail: String },
    /// polkit/sudo could not elevate, or no helper is available at all.
    ElevationDenied { detail: String },
    /// The child was killed because it outlived its deadline.
    Timeout { program: String, seconds: u64 },
    /// The child was killed because the user pressed cancel.
    CancelRequested { program: String },
    /// The caller sent something that must not reach a child process.
    InvalidInput { message: String },
    /// The child ran to completion and exited non-zero.
    Exit {
        program: String,
        code: Option<i32>,
        output: String,
    },
    /// No Android session is running, so there is no device to talk to.
    SessionNotRunning { detail: String },
    /// The session is running but Waydroid suspended the container. ADB
    /// commands hang while an LXC container is frozen, so callers must resume
    /// it or report this state instead of waiting for a timeout.
    SessionFrozen { detail: String },
    /// The device is reachable but has not accepted this host's ADB key.
    AdbUnauthorized { detail: String },
    /// A command reported failure in its output even though it exited 0.
    CommandFailed { detail: String },
    /// Anything that came out of `std::io`.
    Io(io::Error),
}

impl AppError {
    pub fn not_found(program: &str, hint: &str) -> Self {
        Self::NotFound {
            program: program.to_string(),
            hint: hint.to_string(),
        }
    }

    pub fn not_initialized(detail: impl Into<String>) -> Self {
        Self::NotInitialized {
            detail: detail.into(),
        }
    }

    pub fn elevation_denied(detail: impl Into<String>) -> Self {
        Self::ElevationDenied {
            detail: detail.into(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    pub fn session_not_running(detail: impl Into<String>) -> Self {
        Self::SessionNotRunning {
            detail: detail.into(),
        }
    }

    pub fn session_frozen(detail: impl Into<String>) -> Self {
        Self::SessionFrozen {
            detail: detail.into(),
        }
    }

    pub fn adb_unauthorized(detail: impl Into<String>) -> Self {
        Self::AdbUnauthorized {
            detail: detail.into(),
        }
    }

    pub fn command_failed(detail: impl Into<String>) -> Self {
        Self::CommandFailed {
            detail: detail.into(),
        }
    }

    /// Map a failed `Command::spawn` onto the right variant, so a missing binary
    /// is reported as "not installed" instead of a bare I/O error.
    pub fn from_spawn(program: &str, hint: &str, error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::NotFound {
            Self::not_found(program, hint)
        } else {
            Self::Io(error)
        }
    }

    /// Machine readable reason, mirrored into `CommandResult::code`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => code::NOT_FOUND,
            Self::NotInitialized { .. } => code::NOT_INITIALIZED,
            Self::ElevationDenied { .. } => code::ELEVATION_DENIED,
            Self::Timeout { .. } => code::TIMEOUT,
            Self::CancelRequested { .. } => code::CANCELLED,
            Self::InvalidInput { .. } => code::INVALID_INPUT,
            Self::Exit { .. } => code::EXIT,
            Self::SessionNotRunning { .. } => code::SESSION_NOT_RUNNING,
            Self::SessionFrozen { .. } => code::SESSION_FROZEN,
            Self::AdbUnauthorized { .. } => code::ADB_UNAUTHORIZED,
            Self::CommandFailed { .. } => code::COMMAND_FAILED,
            Self::Io(_) => code::IO,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The "No such file or directory" wording is deliberate: the UI
            // recognises it as "waydroid is missing" and it is also what a
            // plain `Command::new` failure used to print.
            Self::NotFound { program, hint } => write!(
                f,
                "cannot find \"{program}\" on PATH: No such file or directory. {hint}"
            ),
            // Waydroid's own wording is authoritative when we have it; it exits 0
            // and prints the notice on stdout, so the text is all we got.
            Self::NotInitialized { detail } => {
                if detail.trim().is_empty() {
                    write!(f, "Waydroid is not initialized, run \"waydroid init\".")
                } else {
                    write!(f, "{}", detail.trim())
                }
            }
            Self::ElevationDenied { detail } => write!(f, "root access was refused: {detail}"),
            Self::Timeout { program, seconds } => write!(
                f,
                "\"{program}\" did not finish within {seconds}s and was terminated"
            ),
            Self::CancelRequested { program } => write!(f, "\"{program}\" was cancelled"),
            Self::InvalidInput { message } => write!(f, "{message}"),
            Self::Exit {
                program,
                code,
                output,
            } => match code {
                Some(code) => write!(f, "\"{program}\" failed with exit code {code}: {output}"),
                None => write!(f, "\"{program}\" was terminated by a signal: {output}"),
            },
            Self::SessionNotRunning { detail } => {
                if detail.trim().is_empty() {
                    write!(f, "Waydroid is not running. Start it first.")
                } else {
                    write!(f, "Waydroid is not running. {}", detail.trim())
                }
            }
            Self::SessionFrozen { detail } => {
                if detail.trim().is_empty() {
                    write!(f, "Waydroid is suspended (container FROZEN).")
                } else {
                    write!(
                        f,
                        "Waydroid is suspended (container FROZEN). {}",
                        detail.trim()
                    )
                }
            }
            Self::AdbUnauthorized { detail } => {
                if detail.trim().is_empty() {
                    write!(f, "ADB is not authorized for the Waydroid device.")
                } else {
                    write!(
                        f,
                        "ADB is not authorized for the Waydroid device. {}",
                        detail.trim()
                    )
                }
            }
            Self::CommandFailed { detail } => write!(f, "{detail}"),
            Self::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_keeps_the_wording_the_ui_looks_for() {
        let error = AppError::not_found("waydroid", "Install it with your package manager.");
        let text = error.to_string();
        assert!(text.contains("No such file"), "{text}");
        assert!(text.contains("waydroid"), "{text}");
        assert_eq!(error.code(), code::NOT_FOUND);
    }

    #[test]
    fn not_initialized_prefers_waydroids_own_wording() {
        let error = AppError::not_initialized("Waydroid is not initialized, run \"waydroid init\"");
        assert_eq!(
            error.to_string(),
            "Waydroid is not initialized, run \"waydroid init\""
        );
        assert_eq!(error.code(), code::NOT_INITIALIZED);
    }

    #[test]
    fn not_initialized_falls_back_to_our_own_text() {
        let error = AppError::not_initialized("   ");
        assert!(error.to_string().contains("waydroid init"));
        assert_eq!(error.code(), code::NOT_INITIALIZED);
    }

    #[test]
    fn exit_quotes_the_captured_output_verbatim() {
        let error = AppError::Exit {
            program: "waydroid".to_string(),
            code: Some(1),
            output: "Cannot locate waydroid config file".to_string(),
        };
        let text = error.to_string();
        assert!(text.contains("exit code 1"), "{text}");
        // The UI still greps the raw text, so it must survive untouched.
        assert!(
            text.contains("Cannot locate waydroid config file"),
            "{text}"
        );
        assert_eq!(error.code(), code::EXIT);
    }

    #[test]
    fn signal_deaths_do_not_claim_an_exit_code() {
        let error = AppError::Exit {
            program: "waydroid".to_string(),
            code: None,
            output: String::new(),
        };
        assert!(error.to_string().contains("signal"), "{error}");
    }

    #[test]
    fn timeout_reports_the_deadline_it_enforced() {
        let error = AppError::Timeout {
            program: "waydroid".to_string(),
            seconds: 30,
        };
        assert!(error.to_string().contains("30s"), "{error}");
        assert_eq!(error.code(), code::TIMEOUT);
    }

    #[test]
    fn a_cancelled_child_is_not_a_timeout() {
        let error = AppError::CancelRequested {
            program: "waydroid".to_string(),
        };
        assert!(error.to_string().contains("cancelled"), "{error}");
        assert_eq!(error.code(), code::CANCELLED);
    }

    #[test]
    fn invalid_input_is_its_own_code() {
        let error = AppError::invalid_input("That is not a valid package name.");
        assert_eq!(error.to_string(), "That is not a valid package name.");
        assert_eq!(error.code(), code::INVALID_INPUT);
    }

    #[test]
    fn session_errors_have_stable_codes_and_actionable_text() {
        let not_running = AppError::session_not_running("Start the session from the dashboard.");
        assert_eq!(not_running.code(), code::SESSION_NOT_RUNNING);
        assert!(not_running.to_string().contains("not running"));

        let frozen = AppError::session_frozen("Open the Waydroid window to resume it.");
        assert_eq!(frozen.code(), code::SESSION_FROZEN);
        assert!(frozen.to_string().contains("FROZEN"));
        assert!(frozen.to_string().contains("resume"));
    }

    #[test]
    fn adb_authorization_and_command_failures_do_not_look_like_exit_codes() {
        let unauthorized = AppError::adb_unauthorized("The host key was not accepted.");
        assert_eq!(unauthorized.code(), code::ADB_UNAUTHORIZED);
        assert!(unauthorized.to_string().contains("not authorized"));

        let failed = AppError::command_failed("waydroid prop set did not take effect.");
        assert_eq!(failed.code(), code::COMMAND_FAILED);
        assert_eq!(failed.to_string(), "waydroid prop set did not take effect.");
        assert!(!failed.to_string().contains("exit code"));
    }

    #[test]
    fn io_errors_convert_and_keep_their_message() {
        let error: AppError = io::Error::new(io::ErrorKind::PermissionDenied, "nope").into();
        assert_eq!(error.to_string(), "nope");
        assert_eq!(error.code(), code::IO);
    }

    #[test]
    fn a_missing_binary_is_not_an_io_error() {
        let error = AppError::from_spawn(
            "waydroid",
            "Install it.",
            io::Error::new(io::ErrorKind::NotFound, "No such file or directory"),
        );
        assert_eq!(error.code(), code::NOT_FOUND);
    }

    #[test]
    fn a_run_failure_that_is_not_missing_stays_io() {
        let error = AppError::from_spawn(
            "waydroid",
            "Install it.",
            io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        );
        assert_eq!(error.code(), code::IO);
    }
}
