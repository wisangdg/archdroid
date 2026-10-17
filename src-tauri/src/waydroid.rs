//! The single place that talks to the `waydroid` CLI.
//!
//! Argument lists are built by free functions so they can be unit tested without
//! a running container, and every child process goes through [`Waydroid::run`],
//! which gives it a deadline and turns a failed exit into an [`AppError`].
//!
//! The action surface below was checked against waydroid 1.6.3
//! (`tools/helpers/arguments.py`): `status`, `session start|stop`,
//! `container start|stop|restart|freeze|unfreeze`, `app install|remove|launch|intent|list`,
//! `prop get|set`, `show-full-ui`, `shell`, `logcat`, `adb connect|disconnect`, `init`.
//! `-V/--version` is a top level flag.
//!
//! Two behaviours from `tools/__main__.py` shape this module:
//!
//! * Before `waydroid init` has run, every action except `init`, `container`,
//!   `first-launch`, `log` and `bugreport` prints
//!   `Waydroid is not initialized, run "waydroid init"` and then **exits 0**.
//!   A zero exit code therefore does not mean "it worked", so the output is
//!   inspected as well.
//! * `container` and `shell` call `actionNeedRoot()`, so they only work as root.
//!   `status`, `session`, `app`, `prop` and `show-full-ui` do not need root.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{AppError, Result};

pub const BINARY: &str = "waydroid";

/// Shown when `waydroid` cannot be found on PATH.
const HINT: &str = "Install it with your package manager, then run the setup wizard.";

/// Deadlines per command class. Waydroid's own default timeout is 1800s
/// (`prep_args`), so an install that legitimately takes minutes is not cut short.
pub const TIMEOUT_QUICK: u64 = 30;
pub const TIMEOUT_NORMAL: u64 = 120;
pub const TIMEOUT_LONG: u64 = 1800;

/// Printed on stdout, with exit code 0, before `waydroid init` has ever run.
pub const NOT_INITIALIZED_MARKER: &str = "Waydroid is not initialized";
/// Raised by waydroid releases up to 1.3 when waydroid.cfg was missing. Waydroid
/// 1.6.3 no longer produces it, but the UI still greps for it.
pub const NOT_INITIALIZED_LEGACY_MARKER: &str = "Cannot locate waydroid config";

/// Does this output mean "the image was never initialised"?
pub fn looks_uninitialized(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains(&NOT_INITIALIZED_MARKER.to_ascii_lowercase())
        || lower.contains(&NOT_INITIALIZED_LEGACY_MARKER.to_ascii_lowercase())
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_string()).collect()
}

/// `waydroid -V` prints the version.
pub fn argv_version() -> Vec<String> {
    argv(&["--version"])
}

pub fn argv_status() -> Vec<String> {
    argv(&["status"])
}

pub fn argv_session_start() -> Vec<String> {
    argv(&["session", "start"])
}

pub fn argv_session_stop() -> Vec<String> {
    argv(&["session", "stop"])
}

pub fn argv_app_list() -> Vec<String> {
    argv(&["app", "list"])
}

/// `path` is the local path of an `.apk`.
pub fn argv_app_install(path: &str) -> Vec<String> {
    argv(&["app", "install", path])
}

/// `package` is an Android package name such as `org.fdroid.fdroid`.
pub fn argv_app_remove(package: &str) -> Vec<String> {
    argv(&["app", "remove", package])
}

pub fn argv_app_launch(package: &str) -> Vec<String> {
    argv(&["app", "launch", package])
}

pub fn argv_prop_get(key: &str) -> Vec<String> {
    argv(&["prop", "get", key])
}

pub fn argv_prop_set(key: &str, value: &str) -> Vec<String> {
    argv(&["prop", "set", key, value])
}

/// `command` is the Android side command, already split into words.
///
/// The words stay separate argv entries: nothing is handed to a shell, so a
/// package name or path containing `;`, backticks or spaces cannot turn into a
/// second command. Note that `waydroid shell` itself requires root.
pub fn argv_shell(command: &[&str]) -> Vec<String> {
    let mut args = argv(&["shell"]);
    args.extend(command.iter().map(|word| (*word).to_string()));
    args
}

pub struct Waydroid {
    program: PathBuf,
}

impl Waydroid {
    /// Resolve `waydroid` on PATH.
    pub fn detect() -> Result<Self> {
        let program =
            crate::resolve_program(BINARY).ok_or_else(|| AppError::not_found(BINARY, HINT))?;
        Ok(Self { program })
    }

    /// Use an explicit program path. `#[cfg(test)]` because only the unit tests
    /// need to point the runner at a known binary today.
    #[cfg(test)]
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    /// Run `args`, wait at most `timeout_secs`, and return stdout.
    ///
    /// Both pipes are drained on reader threads so a chatty command cannot
    /// deadlock on a full pipe while we wait for it to exit.
    pub fn run(&self, args: &[String], timeout_secs: u64) -> Result<String> {
        let program = self.program.to_string_lossy().to_string();
        let mut command = Command::new(&self.program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child: Child = command
            .spawn()
            .map_err(|error| AppError::from_spawn(&program, HINT, error))?;

        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");
        let stdout_reader = thread::spawn(move || read_lossy(stdout));
        let stderr_reader = thread::spawn(move || read_lossy(stderr));

        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        let _ = stdout_reader.join();
                        let _ = stderr_reader.join();
                        return Err(AppError::Timeout {
                            program,
                            seconds: timeout_secs,
                        });
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AppError::Io(error));
                }
            }
        };

        let stdout = stdout_reader.join().unwrap_or_default();
        let stderr = stderr_reader.join().unwrap_or_default();

        // Waydroid reports "not initialized" on stdout and still exits 0, so this
        // check has to happen before the exit code is consulted.
        if looks_uninitialized(&stderr) {
            return Err(AppError::not_initialized(stderr.trim()));
        }
        if looks_uninitialized(&stdout) {
            return Err(AppError::not_initialized(stdout.trim()));
        }

        if status.success() {
            return Ok(stdout);
        }

        Err(AppError::Exit {
            program,
            code: status.code(),
            output: if stderr.trim().is_empty() {
                stdout.trim_end().to_string()
            } else {
                stderr.trim_end().to_string()
            },
        })
    }

    pub fn version(&self) -> Result<String> {
        self.run(&argv_version(), TIMEOUT_QUICK)
            .map(|text| text.trim().to_string())
    }

    /// Raw `waydroid status` output, which is what the UI already parses.
    pub fn status(&self) -> Result<String> {
        self.run(&argv_status(), TIMEOUT_QUICK)
    }

    pub fn session_start(&self) -> Result<String> {
        self.run(&argv_session_start(), TIMEOUT_NORMAL)
    }

    pub fn session_stop(&self) -> Result<String> {
        self.run(&argv_session_stop(), TIMEOUT_NORMAL)
    }

    pub fn app_list(&self) -> Result<String> {
        self.run(&argv_app_list(), TIMEOUT_NORMAL)
    }

    pub fn app_install(&self, path: &str) -> Result<String> {
        self.run(&argv_app_install(path), TIMEOUT_LONG)
    }

    pub fn app_remove(&self, package: &str) -> Result<String> {
        self.run(&argv_app_remove(package), TIMEOUT_LONG)
    }

    pub fn app_launch(&self, package: &str) -> Result<String> {
        self.run(&argv_app_launch(package), TIMEOUT_NORMAL)
    }

    /// Existing callers expect the value without the trailing newline.
    pub fn prop_get(&self, key: &str) -> Result<String> {
        self.run(&argv_prop_get(key), TIMEOUT_QUICK)
            .map(|text| text.trim().to_string())
    }

    pub fn prop_set(&self, key: &str, value: &str) -> Result<String> {
        self.run(&argv_prop_set(key, value), TIMEOUT_QUICK)
    }

    pub fn shell(&self, command: &[&str]) -> Result<String> {
        self.run(&argv_shell(command), TIMEOUT_NORMAL)
    }
}

fn read_lossy(mut reader: impl Read) -> String {
    let mut buffer = Vec::new();
    let _ = reader.read_to_end(&mut buffer);
    String::from_utf8_lossy(&buffer).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::code;

    #[test]
    fn status_and_session_argv_match_the_upstream_actions() {
        assert_eq!(argv_status(), vec!["status"]);
        assert_eq!(argv_session_start(), vec!["session", "start"]);
        assert_eq!(argv_session_stop(), vec!["session", "stop"]);
        assert_eq!(argv_version(), vec!["--version"]);
    }

    #[test]
    fn app_argv_match_the_upstream_subactions() {
        assert_eq!(argv_app_list(), vec!["app", "list"]);
        assert_eq!(
            argv_app_install("/tmp/app.apk"),
            vec!["app", "install", "/tmp/app.apk"]
        );
        assert_eq!(
            argv_app_remove("org.fdroid.fdroid"),
            vec!["app", "remove", "org.fdroid.fdroid"]
        );
        assert_eq!(
            argv_app_launch("org.fdroid.fdroid"),
            vec!["app", "launch", "org.fdroid.fdroid"]
        );
        assert_eq!(argv_prop_get("ro.x"), vec!["prop", "get", "ro.x"]);
        assert_eq!(argv_prop_set("ro.x", "1"), vec!["prop", "set", "ro.x", "1"]);
    }

    /// The words must stay separate argv entries: something that looks like a
    /// shell payload has to arrive as a single, inert argument.
    #[test]
    fn a_package_name_never_becomes_a_second_command() {
        let hostile = "org.fdroid.fdroid; rm -rf /tmp/x";
        assert_eq!(argv_app_remove(hostile), vec!["app", "remove", hostile]);
        assert_eq!(
            argv_shell(&["pm", "list", "packages"]),
            vec!["shell", "pm", "list", "packages"]
        );
        assert_eq!(argv_shell(&[hostile]).len(), 2);
    }

    #[test]
    fn uninitialized_output_is_recognised_in_both_wordings() {
        assert!(looks_uninitialized(
            "Waydroid is not initialized, run \"waydroid init\""
        ));
        assert!(looks_uninitialized(
            "Cannot locate waydroid config file /var/lib/waydroid/waydroid.cfg, is waydroid initialized?"
        ));
        assert!(!looks_uninitialized("Session:\tRUNNING"));
    }

    #[test]
    fn a_normal_exit_returns_stdout() {
        let tool = Waydroid::with_program("/bin/echo");
        let output = tool.run(&["hello".to_string()], TIMEOUT_QUICK).unwrap();
        assert_eq!(output.trim(), "hello");
    }

    /// The regression this whole module exists for: exit code 0 is not success.
    #[test]
    fn exit_zero_with_the_not_initialized_notice_is_an_error() {
        let tool = Waydroid::with_program("/bin/echo");
        let error = tool
            .run(
                &argv(&["Waydroid is not initialized", "run waydroid init"]),
                TIMEOUT_QUICK,
            )
            .unwrap_err();
        assert_eq!(error.code(), code::NOT_INITIALIZED);
        assert!(error.to_string().contains("Waydroid is not initialized"));
    }

    #[test]
    fn a_non_zero_exit_becomes_an_exit_error_with_its_output() {
        let tool = Waydroid::with_program("/bin/sh");
        let error = tool
            .run(&argv(&["-c", "echo boom >&2; exit 3"]), TIMEOUT_QUICK)
            .unwrap_err();
        assert_eq!(error.code(), code::EXIT);
        assert!(error.to_string().contains("exit code 3"), "{error}");
        assert!(error.to_string().contains("boom"), "{error}");
    }

    #[test]
    fn a_missing_binary_is_reported_as_not_installed() {
        let tool = Waydroid::with_program("/nonexistent/waydroid-does-not-exist");
        let error = tool.run(&argv_status(), TIMEOUT_QUICK).unwrap_err();
        assert_eq!(error.code(), code::NOT_FOUND);
        assert!(error.to_string().contains("No such file"), "{error}");
    }

    #[test]
    fn a_command_that_outlives_its_deadline_is_killed() {
        let tool = Waydroid::with_program("/bin/sleep");
        let started = Instant::now();
        let error = tool.run(&["30".to_string()], 1).unwrap_err();
        assert_eq!(error.code(), code::TIMEOUT);
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the deadline was not enforced"
        );
    }
}
