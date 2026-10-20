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
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
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

/// How long `session start` may take to either fail (fast exit) or prove it is
/// up (still running) before we report success and hand it to a reaper thread.
pub const SESSION_START_GRACE_SECS: u64 = 5;

// ponytail: satu flag global untuk semua operasi panjang — UI menjalankan satu
// operasi panjang pada satu waktu (guard isOperating). Registry Child per-task
// hanya kalau operasi paralel nanti benar-benar dibutuhkan.
static CANCEL: AtomicBool = AtomicBool::new(false);

/// Ask the running long operation (`app install|remove`) to stop; its child is
/// killed within one poll tick (~20ms).
pub fn request_cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Every cancellable run clears the flag first, so a stale request from a
/// previous operation cannot kill a new one.
fn cancel_requested() -> bool {
    CANCEL.load(Ordering::SeqCst)
}

/// Does this output mean "the image was never initialised"?
pub fn looks_uninitialized(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains(&NOT_INITIALIZED_MARKER.to_ascii_lowercase())
        || lower.contains(&NOT_INITIALIZED_LEGACY_MARKER.to_ascii_lowercase())
}

/// `waydroid status` prints `Session:\tRUNNING` when the session is up and
/// `Session:\tSTOPPED` (plus only the vendor type) when it is not.
pub fn session_running(status: &str) -> bool {
    status.lines().any(|line| {
        line.strip_prefix("Session:")
            .is_some_and(|value| value.trim() == "RUNNING")
    })
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

/// `waydroid adb connect` points the host adb server at the container IP.
pub fn argv_adb_connect() -> Vec<String> {
    argv(&["adb", "connect"])
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
        run_program(&self.program, args, timeout_secs, false)
    }

    /// Same as [`run`], but a [`request_cancel`] call kills the child.
    pub fn run_cancellable(&self, args: &[String], timeout_secs: u64) -> Result<String> {
        CANCEL.store(false, Ordering::SeqCst);
        run_program(&self.program, args, timeout_secs, true)
    }

    pub fn version(&self) -> Result<String> {
        self.run(&argv_version(), TIMEOUT_QUICK)
            .map(|text| text.trim().to_string())
    }

    /// Raw `waydroid status` output, which is what the UI already parses.
    pub fn status(&self) -> Result<String> {
        self.run(&argv_status(), TIMEOUT_QUICK)
    }

    /// `waydroid session start` is the session manager itself: a foreground
    /// daemon (`tools/actions/session_manager.py` ends in `looper.run()`), so it
    /// never exits while the session is up. Detached handling therefore differs
    /// from every other command: wait a short grace period — exited at once
    /// means it failed (or the session was already running, which also exits 0),
    /// still alive means the session started. A background thread reaps the
    /// child afterwards so it never becomes a zombie.
    pub fn session_start(&self, grace_secs: u64) -> Result<()> {
        self.session_start_argv(&argv_session_start(), grace_secs)
    }

    /// The argv is a parameter only so the unit tests can run a stub instead of
    /// the real daemon (same reason as `with_program`).
    fn session_start_argv(&self, args: &[String], grace_secs: u64) -> Result<()> {
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
        let mut stdout_reader = Some(thread::spawn(move || read_lossy(stdout)));
        let mut stderr_reader = Some(thread::spawn(move || read_lossy(stderr)));

        let deadline = Instant::now() + Duration::from_secs(grace_secs);
        let mut still_running = false;
        let outcome = loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let out = stdout_reader
                        .take()
                        .and_then(|handle| handle.join().ok())
                        .unwrap_or_default();
                    let err = stderr_reader
                        .take()
                        .and_then(|handle| handle.join().ok())
                        .unwrap_or_default();
                    break interpret(&program, status, out, err).map(|_| ());
                }
                Ok(None) => {
                    if Instant::now() >= deadline {
                        still_running = true;
                        break Ok(());
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Err(error) => break Err(AppError::Io(error)),
            }
        };

        // Still alive after the grace period: the daemon is the session, leave
        // it running and hand it to a reaper (drain pipes, then wait()).
        if still_running {
            thread::spawn(move || {
                if let Some(handle) = stdout_reader.take() {
                    let _ = handle.join();
                }
                if let Some(handle) = stderr_reader.take() {
                    let _ = handle.join();
                }
                let _ = child.wait();
            });
        }
        outcome
    }

    pub fn session_stop(&self) -> Result<String> {
        self.run(&argv_session_stop(), TIMEOUT_NORMAL)
    }

    pub fn app_list(&self) -> Result<String> {
        self.run(&argv_app_list(), TIMEOUT_NORMAL)
    }

    pub fn app_install(&self, path: &str) -> Result<String> {
        self.run_cancellable(&argv_app_install(path), TIMEOUT_LONG)
    }

    pub fn app_remove(&self, package: &str) -> Result<String> {
        self.run_cancellable(&argv_app_remove(package), TIMEOUT_LONG)
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

    /// Point the host `adb` server at the container. No root needed (only
    /// `shell`, `container`, `logcat`, `init` and `upgrade` call `actionNeedRoot`).
    pub fn adb_connect(&self) -> Result<String> {
        self.run(&argv_adb_connect(), TIMEOUT_QUICK)
    }
}

/// Run a child with a deadline, draining both pipes on reader threads.
///
/// Shared with `adb.rs`: `adb shell` gets the same timeout and "exit 0 is not
/// success" handling as every waydroid action. `cancellable` opts into the
/// [`request_cancel`] flag (long operations only).
pub(crate) fn run_program(
    program_path: &Path,
    args: &[String],
    timeout_secs: u64,
    cancellable: bool,
) -> Result<String> {
    let program = program_path.to_string_lossy().to_string();
    let mut command = Command::new(program_path);
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
                if (cancellable && cancel_requested()) || Instant::now() >= deadline {
                    let cancelled = cancellable && cancel_requested();
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return if cancelled {
                        Err(AppError::CancelRequested { program })
                    } else {
                        Err(AppError::Timeout {
                            program,
                            seconds: timeout_secs,
                        })
                    };
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

    interpret(&program, status, stdout, stderr)
}

/// Decide what a finished child means. Zero exit is not success on its own:
/// waydroid prints "not initialized" and still exits 0 before `waydroid init`
/// has run, so the output is consulted before the exit code.
fn interpret(program: &str, status: ExitStatus, stdout: String, stderr: String) -> Result<String> {
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
        program: program.to_string(),
        code: status.code(),
        output: if stderr.trim().is_empty() {
            stdout.trim_end().to_string()
        } else {
            stderr.trim_end().to_string()
        },
    })
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
        assert_eq!(argv_adb_connect(), vec!["adb", "connect"]);
        assert_eq!(argv_version(), vec!["--version"]);
    }

    /// Format checked against `tools/actions/status.py` in waydroid 1.6.3:
    /// running prints several tab separated lines, stopped only two.
    #[test]
    fn status_output_is_parsed_for_the_session_state() {
        let running = "Session:\tRUNNING\nContainer:\tRUNNING\nVendor type:\tMAINLINE\nIP address:\t192.168.240.112\nSession user:\twdg(1000)\nWayland display:\twayland-0";
        assert!(session_running(running));

        let stopped = "Session:\tSTOPPED\nVendor type:\tMAINLINE";
        assert!(!session_running(stopped));
        assert!(!session_running(""));
        assert!(!session_running("Session:\tUNKNOWN"));
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

    /// The Cancel button contract: a pending cancel request kills the child
    /// within a poll tick instead of letting it run to its deadline.
    #[test]
    fn a_cancel_request_kills_a_running_child() {
        let tool = Waydroid::with_program("/bin/sleep");
        let killer = thread::spawn(|| {
            thread::sleep(Duration::from_millis(200));
            request_cancel();
        });
        let started = Instant::now();
        let error = tool
            .run_cancellable(&["30".to_string()], TIMEOUT_LONG)
            .unwrap_err();
        killer.join().unwrap();
        assert_eq!(error.code(), code::CANCELLED, "{error}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "cancel did not stop the child promptly"
        );
    }

    /// A stale cancel request must not kill the next operation.
    #[test]
    fn a_cancellable_run_ignores_a_stale_cancel_request() {
        request_cancel();
        let tool = Waydroid::with_program("/bin/echo");
        let output = tool.run_cancellable(&["ok".to_string()], TIMEOUT_QUICK);
        assert_eq!(output.unwrap().trim(), "ok");
    }

    /// `session start` stays alive: that is the daemon working, not a hang.
    #[test]
    fn a_session_that_outlives_the_grace_period_has_started() {
        let tool = Waydroid::with_program("/bin/sleep");
        tool.session_start_argv(&argv(&["30"]), 1).unwrap();
    }

    #[test]
    fn a_session_that_exits_cleanly_during_grace_is_success() {
        let tool = Waydroid::with_program("/bin/echo");
        tool.session_start_argv(&argv(&["session", "start"]), 5)
            .unwrap();
    }

    #[test]
    fn a_session_that_fails_during_grace_reports_its_error() {
        // `/bin/false` exits 1 at once, like a session start that failed.
        let tool = Waydroid::with_program("/bin/false");
        let error = tool.session_start_argv(&[], 5).unwrap_err();
        assert_eq!(error.code(), code::EXIT, "{error}");
    }
}
