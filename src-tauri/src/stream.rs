//! Streaming task machinery: the single-flight slot, `cmd-*` events, child
//! process supervision (deadline, cancel, process group), and the user and
//! privileged script runners built on top of them.

use serde::Serialize;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::error::AppError;
use crate::*;

/// Upper bound for the output kept around for inspection.
pub(crate) const MAX_CAPTURED_OUTPUT: usize = 4096;

/// How long a terminated script gets to run its `trap ... EXIT` cleanup before
/// the whole process group is killed outright. `elevate_env_for` relies on that
/// trap to remove `$ARCHDROID_WORK`, so SIGKILL must not be the first signal.
pub(crate) const KILL_GRACE: Duration = Duration::from_secs(5);

/// The single-flight slot. Streaming operations share one terminal, so only
/// one of them may own the slot at a time.
pub(crate) static ACTIVE_TASK: Mutex<Option<ActiveTask>> = Mutex::new(None);
pub(crate) static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) struct ActiveTask {
    id: String,
    label: &'static str,
    cancel: Arc<AtomicBool>,
}

/// Holds the single-flight slot until the operation finishes, including on
/// every early return or panic.
pub(crate) struct TaskGuard {
    id: String,
    cancel: Arc<AtomicBool>,
}

impl TaskGuard {
    pub(crate) fn start(label: &'static str) -> Result<Self, AppError> {
        let mut active = ACTIVE_TASK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(current) = active.as_ref() {
            return Err(AppError::busy(current.label));
        }
        let id = format!("task-{}", NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed));
        let cancel = Arc::new(AtomicBool::new(false));
        active.replace(ActiveTask {
            id: id.clone(),
            label,
            cancel: cancel.clone(),
        });
        Ok(Self { id, cancel })
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    #[cfg(test)]
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// Ask the active task to stop, but only when `task_id` still owns the slot.
///
/// A stale ID (the operation finished between the click and this call, or the
/// UI kept an old ID) is a no-op: it must never cancel the task that replaced
/// it. Returns whether a live task matched.
pub(crate) fn cancel_active_task(task_id: &str) -> bool {
    let active = ACTIVE_TASK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match active.as_ref() {
        Some(task) if task.id == task_id => {
            task.cancel.store(true, Ordering::SeqCst);
            true
        }
        _ => false,
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        let mut active = ACTIVE_TASK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if active.as_ref().is_some_and(|task| task.id == self.id) {
            *active = None;
        }
    }
}

/// Payload for `cmd-started`. The UI stores this ID and sends it back to
/// `cancel_task`, so Cancel can only stop the operation it is showing.
#[derive(Clone, Serialize)]
pub(crate) struct StreamStarted {
    pub(crate) task_id: String,
}

/// Payload for `cmd-output`. The task ID lets the UI ignore output that does
/// not belong to the operation it is currently showing.
#[derive(Clone, Serialize)]
pub(crate) struct StreamOutput {
    pub(crate) task_id: String,
    pub(crate) line: String,
}

/// Final payload for `cmd-finished`. The UI branches on `code`, so it never
/// has to parse the human-readable `message`.
#[derive(Clone, Serialize)]
pub(crate) struct StreamFinished {
    pub(crate) task_id: String,
    pub(crate) success: bool,
    pub(crate) code: Option<&'static str>,
    pub(crate) message: Option<String>,
}

pub(crate) struct StreamResult {
    pub(crate) outcome: Result<(), AppError>,
    /// True when the elevated shell actually started, i.e. elevation worked.
    pub(crate) started: bool,
}

impl StreamResult {
    pub(crate) fn success(started: bool) -> Self {
        Self {
            outcome: Ok(()),
            started,
        }
    }

    pub(crate) fn failure(error: AppError, started: bool) -> Self {
        Self {
            outcome: Err(error),
            started,
        }
    }

    pub(crate) fn is_success(&self) -> bool {
        self.outcome.is_ok()
    }

    pub(crate) fn timed_out(&self) -> bool {
        matches!(&self.outcome, Err(AppError::Timeout { .. }))
    }

    pub(crate) fn cancelled(&self) -> bool {
        matches!(&self.outcome, Err(AppError::CancelRequested { .. }))
    }

    pub(crate) fn finished_event(&self, task_id: &str) -> StreamFinished {
        match &self.outcome {
            Ok(()) => StreamFinished {
                task_id: task_id.to_string(),
                success: true,
                code: None,
                message: None,
            },
            Err(error) => StreamFinished {
                task_id: task_id.to_string(),
                success: false,
                code: Some(error.code()),
                message: Some(error.to_string()),
            },
        }
    }

    pub(crate) fn exit_code(&self) -> Option<i32> {
        match &self.outcome {
            Err(AppError::Exit { code, .. }) => *code,
            _ => None,
        }
    }

    pub(crate) fn into_command_result(self) -> CommandResult {
        match self.outcome {
            Ok(()) => CommandResult::success(),
            Err(error) => error.into(),
        }
    }

    pub(crate) fn into_command_result_with_message(self, message: &str) -> CommandResult {
        match self.outcome {
            Ok(()) => CommandResult::with_message(message),
            Err(error) => error.into(),
        }
    }
}

pub(crate) fn emit_started(app: &AppHandle, task_id: &str) {
    let _ = app.emit(
        "cmd-started",
        StreamStarted {
            task_id: task_id.to_string(),
        },
    );
}

pub(crate) fn emit_output(app: &AppHandle, task_id: &str, line: impl Into<String>) {
    let _ = app.emit(
        "cmd-output",
        StreamOutput {
            task_id: task_id.to_string(),
            line: line.into(),
        },
    );
}

/// Emit a completed line and remember the beginning of the output.
/// Recognise host conditions that make an installer fail half-way and say what
/// to do, instead of leaving the user with the raw tail of pip/git/cp output.
pub(crate) fn failure_hint(text: &str) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("no space left on device") || lower.contains("disk quota exceeded") {
        Some("The disk filled up during the operation. Free at least 1.5 GB on the filesystem holding /var/lib/waydroid and /tmp, then retry; the add-on is not marked installed.")
    } else if [
        "could not resolve host",
        "temporary failure in name resolution",
        "network is unreachable",
        "connection timed out",
        "connection reset",
        "failed to establish a new connection",
        "unable to access 'https://",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        Some("The network dropped during the download. Check the connection and retry; nothing was marked installed and the retry starts from a clean work directory.")
    } else {
        None
    }
}

pub(crate) fn flush_line(
    app: &AppHandle,
    task_id: &str,
    buffer: &mut String,
    captured: &Arc<Mutex<String>>,
) {
    if buffer.is_empty() {
        return;
    }
    emit_output(app, task_id, buffer.clone());
    if let Ok(mut text) = captured.lock() {
        // Past the head budget, still keep lines that explain a failure so the
        // hint survives long installer output.
        if text.len() < MAX_CAPTURED_OUTPUT
            || (text.len() < 2 * MAX_CAPTURED_OUTPUT && failure_hint(buffer).is_some())
        {
            text.push_str(buffer);
            text.push('\n');
        }
    }
    buffer.clear();
}

/// Read a pipe byte by byte so progress bars that use carriage returns (pacman,
/// curl) are streamed as their own lines, and flush the trailing line at EOF.
pub(crate) fn spawn_line_reader(
    app: AppHandle,
    task_id: &str,
    reader: impl Read + Send + 'static,
    captured: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    let task_id = task_id.to_string();
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
                flush_line(&app, &task_id, &mut buffer, &captured);
            } else {
                buffer.push(c);
            }
        }
        flush_line(&app, &task_id, &mut buffer, &captured);
    })
}

/// Why a child is no longer being waited for.
pub(crate) enum WaitOutcome {
    Exited(ExitStatus),
    TimedOut,
    Cancelled,
}

/// Send `signal` to the process group led by `pid`.
///
/// `kill` is resolved without PATH, like every other helper that runs with more
/// privilege than the caller. `--` stops the negative group ID from being read
/// as an option.
pub(crate) fn signal_process_group(pid: u32, signal: &str) -> bool {
    let Some(kill) = trusted_program("kill") else {
        return false;
    };
    Command::new(kill)
        .args([signal, "--", &format!("-{pid}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Stop the child and everything it spawned.
///
/// SIGTERM goes to the whole group first so the elevated shell can run its EXIT
/// trap (`rm -rf "$ARCHDROID_WORK"`); a group that ignores it is SIGKILLed
/// after [`KILL_GRACE`].
// ponytail: process group only; a child that calls setsid() (sudo with
// use_pty, a daemonising installer) escapes. Move to a cgroup/PID namespace
// if that ever becomes a real failure.
pub(crate) fn terminate_process_group(child: &mut Child) {
    let pid = child.id();
    let term_sent = signal_process_group(pid, "-TERM");
    let deadline = Instant::now() + KILL_GRACE;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) => thread::sleep(Duration::from_millis(20)),
        }
    }

    // No trusted `kill`: at least stop the direct child instead of leaking it.
    if !term_sent {
        let _ = child.kill();
    }
    // Sweep the rest of the group too: a child may ignore SIGTERM or outlive
    // the shell that led the group.
    let _ = signal_process_group(pid, "-KILL");
    let _ = child.wait();
}

/// Ask the root-side watcher to stop its own process group. The app cannot
/// signal a root-owned group directly (EPERM).
pub(crate) fn request_privileged_cancel(cancel_path: Option<&Path>) {
    if let Some(path) = cancel_path {
        let _ = std::fs::write(path, "cancel\n");
    }
}

/// Wait for `child`, but stop its process group when the deadline elapses or
/// the task-scoped cancel flag is raised.
pub(crate) fn wait_with_deadline(
    child: &mut Child,
    timeout_secs: u64,
    cancel: &AtomicBool,
    cancel_path: Option<&Path>,
) -> std::io::Result<WaitOutcome> {
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(WaitOutcome::Exited(status)),
            Ok(None) if cancel.load(Ordering::SeqCst) => {
                // The root-side watcher handles the elevated group; the local
                // kill is the only option for user-level scripts.
                request_privileged_cancel(cancel_path);
                terminate_process_group(child);
                return Ok(WaitOutcome::Cancelled);
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                request_privileged_cancel(cancel_path);
                terminate_process_group(child);
                return Ok(WaitOutcome::TimedOut);
            }
            Err(error) => {
                request_privileged_cancel(cancel_path);
                terminate_process_group(child);
                return Err(error);
            }
        }
    }
}

/// Map a finished wait onto the typed result the UI sees.
///
/// `started` comes from the sentinel line the elevated shell prints first, so
/// "elevation failed" and "the script ran and failed" stay distinguishable
/// without parsing pkexec/sudo messages.
pub(crate) fn stream_outcome(
    program: &str,
    wait: std::io::Result<WaitOutcome>,
    captured_text: String,
    timeout_secs: u64,
    emit_error: impl FnOnce(String),
) -> StreamResult {
    let started = captured_text.contains(ELEVATED_SENTINEL);
    match wait {
        Ok(WaitOutcome::Exited(status)) if status.success() => StreamResult::success(started),
        Ok(WaitOutcome::Exited(status)) => {
            if let Some(hint) = failure_hint(&captured_text) {
                emit_error(format!("[HINT] {hint}"));
            }
            StreamResult::failure(
                AppError::Exit {
                    program: program.to_string(),
                    code: status.code(),
                    output: captured_text,
                },
                started,
            )
        }
        Ok(WaitOutcome::TimedOut) => {
            let error = AppError::Timeout {
                program: program.to_string(),
                seconds: timeout_secs,
            };
            emit_error(format!("[ERROR] {error}"));
            StreamResult::failure(error, started)
        }
        Ok(WaitOutcome::Cancelled) => {
            let error = AppError::CancelRequested {
                program: program.to_string(),
            };
            emit_error(format!("[ERROR] {error}"));
            StreamResult::failure(error, started)
        }
        Err(error) => {
            emit_error(format!(
                "[ERROR] Lost track of the running command: {error}"
            ));
            StreamResult::failure(AppError::Io(error), started)
        }
    }
}

/// Spawn a command, stream both of its pipes to the UI and report how it went.
pub(crate) fn run_and_stream(
    app: &AppHandle,
    task_id: &str,
    mut command: Command,
    timeout_secs: u64,
    cancel: &AtomicBool,
    cancel_path: Option<&Path>,
) -> StreamResult {
    let program = command.get_program().to_string_lossy().to_string();
    // The UI can press Cancel between `cmd-started` and this spawn. Do not open
    // a polkit/sudo prompt for a task the user already stopped.
    if cancel.load(Ordering::SeqCst) {
        let error = AppError::CancelRequested {
            program: program.clone(),
        };
        emit_output(app, task_id, format!("[ERROR] {error}"));
        return StreamResult::failure(error, false);
    }
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Own process group: a deadline can then stop pkexec/sudo, bash and
        // everything they spawned together instead of just the direct child.
        .process_group(0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            let error =
                AppError::from_spawn(&program, "Install the missing program and try again.", e);
            log::error!(
                "{program} could not start: {}",
                redact_text(&error.to_string())
            );
            emit_output(app, task_id, format!("[ERROR] {error}"));
            return StreamResult::failure(error, false);
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let captured = Arc::new(Mutex::new(String::new()));
    let stdout_reader = spawn_line_reader(app.clone(), task_id, stdout, captured.clone());
    let stderr_reader = spawn_line_reader(app.clone(), task_id, stderr, captured.clone());

    let wait = wait_with_deadline(&mut child, timeout_secs, cancel, cancel_path);
    if let Some(path) = cancel_path {
        let _ = std::fs::remove_file(path);
    }

    let _ = stdout_reader.join();
    let _ = stderr_reader.join();

    let captured_text = captured.lock().map(|text| text.clone()).unwrap_or_default();
    let outcome = stream_outcome(&program, wait, captured_text, timeout_secs, |line| {
        emit_output(app, task_id, line)
    });
    if outcome.is_success() {
        log::info!("{program} finished");
    } else if let Err(error) = &outcome.outcome {
        log::warn!("{program} failed: {}", redact_text(&error.to_string()));
    }
    outcome
}

/// Markers are written by the unprivileged app process *after* a root script
/// exits successfully. Root never writes into the user cache, so a symlink
/// planted under `~/.cache` cannot redirect a root-owned write.
#[derive(Clone, Copy)]
pub(crate) enum Completion {
    None,
    WriteMarker(&'static str),
    ClearInstallMarkers,
}

impl Completion {
    /// Returns a user-visible warning if the marker could not be written.
    pub(crate) fn apply(self) -> Option<String> {
        match self {
            Self::None => None,
            Self::WriteMarker(name) => write_marker(&marker_file(name)).err().map(|error| {
                format!("[WARNING] Installed, but could not record it in the cache ({error}); status may show \"not installed\" until the next live check.")
            }),
            Self::ClearInstallMarkers => {
                for name in [
                    "gapps_installed",
                    "houdini_installed",
                    "multitouch_installed",
                ] {
                    let _ = std::fs::remove_file(marker_file(name));
                }
                None
            }
        }
    }
}

/// Apply a completion marker only after a successful run. Timeout and cancel
/// are failures, so they must never leave an "installed" marker behind.
pub(crate) fn apply_completion(attempt: &StreamResult, completion: Completion) -> Option<String> {
    if attempt.is_success() {
        completion.apply()
    } else {
        // A reinstall that got root and then failed may have left the add-on
        // half-replaced, so an older marker no longer vouches for anything.
        // A refused prompt touched nothing and keeps it.
        if let (true, Completion::WriteMarker(name)) = (attempt.started, completion) {
            let _ = std::fs::remove_file(marker_file(name));
        }
        None
    }
}

pub(crate) fn finish_stream(
    app: &AppHandle,
    task_id: &str,
    attempt: &StreamResult,
    completion: Completion,
) {
    if let Some(warning) = apply_completion(attempt, completion) {
        emit_output(app, task_id, warning);
    }
    let _ = app.emit("cmd-finished", attempt.finished_event(task_id));
}

/// Run a user-level script. Phantom installs into `$HOME/.local`, so it does
/// not need root at all; keeping it out of the privileged path also avoids
/// running a user-controlled Rust toolchain as root.
pub(crate) fn run_user_script(
    app: &AppHandle,
    task_id: &str,
    script: String,
    timeout_secs: u64,
    cancel: &AtomicBool,
) -> StreamResult {
    log::info!("user script requested ({} lines)", script.lines().count());
    let Some(shell) = trusted_program("bash") else {
        let error = AppError::not_found(
            "bash",
            "Install bash or start ArchDroid from a normal desktop session.",
        );
        emit_output(app, task_id, format!("[ERROR] {error}"));
        let attempt = StreamResult::failure(error, false);
        finish_stream(app, task_id, &attempt, Completion::None);
        return attempt;
    };
    let mut command = Command::new(shell);
    command
        .arg("--noprofile")
        .arg("--norc")
        .arg("-c")
        .arg(&script);
    let attempt = run_and_stream(app, task_id, command, timeout_secs, cancel, None);
    finish_stream(app, task_id, &attempt, Completion::None);
    attempt
}

/// Await the user-level script so the command only resolves after it exits.
pub(crate) async fn stream_user_script(
    app: AppHandle,
    script: String,
    timeout_secs: u64,
) -> StreamResult {
    let task = match TaskGuard::start("user script") {
        Ok(task) => task,
        Err(error) => return StreamResult::failure(error, false),
    };
    let task_id = task.id().to_string();
    emit_started(&app, &task_id);
    let cancel = task.cancel_flag();
    let fallback_app = app.clone();
    let fallback_task_id = task_id.clone();
    match tauri::async_runtime::spawn_blocking(move || {
        let _task = task;
        run_user_script(&app, &task_id, script, timeout_secs, &cancel)
    })
    .await
    {
        Ok(attempt) => attempt,
        Err(error) => {
            let error = AppError::Io(std::io::Error::other(format!(
                "user script task failed: {error}"
            )));
            emit_output(&fallback_app, &fallback_task_id, format!("[ERROR] {error}"));
            let attempt = StreamResult::failure(error, false);
            finish_stream(&fallback_app, &fallback_task_id, &attempt, Completion::None);
            attempt
        }
    }
}

/// What to do after the polkit attempt.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ElevationStep {
    /// The attempt is final: report it and never open a sudo prompt.
    Stop,
    /// Elevation never started, so sudo may still work.
    FallBackToSudo,
}

/// Decide whether a failed polkit attempt may still try sudo.
///
/// Cancel, a dismissed dialog (exit 126) and a deadline are terminal. A script
/// that actually started keeps its own exit result. Only an attempt that never
/// started is allowed to fall back.
pub(crate) fn pkexec_step(attempt: &mut StreamResult, forced: &str) -> ElevationStep {
    if attempt.cancelled() {
        return ElevationStep::Stop;
    }

    if attempt.exit_code() == Some(126) {
        let started = attempt.started;
        *attempt = StreamResult::failure(
            AppError::elevation_denied(
                "The polkit authentication dialog was dismissed; nothing was changed.",
            ),
            started,
        );
        return ElevationStep::Stop;
    }

    if attempt.timed_out() {
        return ElevationStep::Stop;
    }

    if attempt.started || forced == "pkexec" {
        if !attempt.started && !attempt.is_success() {
            *attempt = StreamResult::failure(
                AppError::elevation_denied(
                    "polkit could not elevate here; no privileged shell was started.",
                ),
                false,
            );
        }
        return ElevationStep::Stop;
    }

    ElevationStep::FallBackToSudo
}

/// Replace "sudo never elevated" with a typed elevation error.
///
/// A script that started and exited non-zero keeps its exit result, so a failed
/// installer is not misreported as a failed password prompt.
pub(crate) fn sudo_failure(attempt: StreamResult) -> Result<StreamResult, AppError> {
    if attempt.is_success() || attempt.started || attempt.timed_out() || attempt.cancelled() {
        return Ok(attempt);
    }
    Err(AppError::elevation_denied(
        "sudo could not elevate or the privileged shell never started. Install a polkit authentication agent (for example hyprpolkitagent or polkit-gnome), or start ArchDroid from a terminal so sudo can ask for the password.",
    ))
}

/// Run a script that needs root and wait for it to exit.
///
/// The script body is written as if it were already root: there are no `sudo`
/// calls inside. `elevate_env` supplies a fixed PATH, a passwd-derived HOME and
/// a private work directory, regardless of what polkit/sudo passed through.
///
/// Elevation is tried through polkit first, because a polkit password dialog is
/// drawn by a desktop agent and therefore also works when ArchDroid was started
/// from an app launcher. `sudo` remains the fallback because it can still prompt
/// when the app was started from a terminal. Force either one with
/// `ARCHDROID_ELEVATE=pkexec|sudo`.
pub(crate) fn run_privileged_script_for_task(
    app: &AppHandle,
    task_id: &str,
    script: String,
    completion: Completion,
    timeout_secs: u64,
    cancel: &AtomicBool,
) -> StreamResult {
    log::info!("root script requested ({} lines)", script.lines().count());
    // A user-owned path is the only out-of-band signal that works both ways:
    // the app can create it without root, and the root watcher can see it.
    let cancel_path = std::env::temp_dir().join(format!(
        "archdroid-cancel-{}-{}",
        std::process::id(),
        task_id
    ));
    let _ = std::fs::remove_file(&cancel_path);
    let body = match elevate_env(&script) {
        Ok(body) => format!(
            "ARCHDROID_CANCEL_FILE={}\n{}",
            sh_quote(&cancel_path.to_string_lossy()),
            body
        ),
        Err(error) => {
            emit_output(app, task_id, format!("[ERROR] {error}"));
            let attempt = StreamResult::failure(error, false);
            finish_stream(app, task_id, &attempt, completion);
            return attempt;
        }
    };
    let forced = std::env::var("ARCHDROID_ELEVATE").unwrap_or_default();
    let mut pkexec_failed = false;

    if forced != "sudo" {
        if let Some(command) = elevated_command("pkexec", &body) {
            emit_output(
                app,
                task_id,
                "[SYSTEM] Requesting root access via polkit...",
            );
            let mut attempt = run_and_stream(
                app,
                task_id,
                command,
                timeout_secs,
                cancel,
                Some(&cancel_path),
            );
            match pkexec_step(&mut attempt, &forced) {
                ElevationStep::Stop => {
                    finish_stream(app, task_id, &attempt, completion);
                    return attempt;
                }
                ElevationStep::FallBackToSudo => {
                    pkexec_failed = true;
                    emit_output(
                        app,
                        task_id,
                        "[SYSTEM] polkit could not elevate here, falling back to sudo...",
                    );
                }
            }
        } else if forced == "pkexec" {
            let error = AppError::elevation_denied(
                "ARCHDROID_ELEVATE=pkexec but pkexec is not available, so root access cannot be requested.",
            );
            emit_output(app, task_id, format!("[ERROR] {error}"));
            let attempt = StreamResult::failure(error, false);
            finish_stream(app, task_id, &attempt, completion);
            return attempt;
        }
    }

    let Some(command) = elevated_command("sudo", &body) else {
        let detail = if forced == "sudo" {
            "ARCHDROID_ELEVATE=sudo but sudo is not available, so root access cannot be requested."
        } else if pkexec_failed {
            "polkit could not elevate and sudo is not available, so root access cannot be requested."
        } else {
            "Neither pkexec nor sudo is available, so root access cannot be requested."
        };
        let error = AppError::elevation_denied(detail);
        emit_output(app, task_id, format!("[ERROR] {error}"));
        let attempt = StreamResult::failure(error, false);
        finish_stream(app, task_id, &attempt, completion);
        return attempt;
    };

    emit_output(app, task_id, "[SYSTEM] Requesting root access via sudo...");
    let attempt = run_and_stream(
        app,
        task_id,
        command,
        timeout_secs,
        cancel,
        Some(&cancel_path),
    );

    let attempt = match sudo_failure(attempt) {
        Ok(attempt) => attempt,
        Err(error) => {
            emit_output(app, task_id, "[SYSTEM] Root access failed. Install a polkit authentication agent (for example hyprpolkitagent or polkit-gnome), or start ArchDroid from a terminal so sudo can ask for the password.");
            StreamResult::failure(error, false)
        }
    };

    finish_stream(app, task_id, &attempt, completion);
    attempt
}

/// Await the privileged script so the command only resolves after it exits.
pub(crate) async fn stream_privileged_script(
    app: AppHandle,
    script: String,
    completion: Completion,
    timeout_secs: u64,
) -> StreamResult {
    let task = match TaskGuard::start("privileged operation") {
        Ok(task) => task,
        Err(error) => return StreamResult::failure(error, false),
    };
    let task_id = task.id().to_string();
    emit_started(&app, &task_id);
    let cancel = task.cancel_flag();
    let fallback_app = app.clone();
    let fallback_task_id = task_id.clone();
    let fallback_completion = completion;
    match tauri::async_runtime::spawn_blocking(move || {
        let _task = task;
        run_privileged_script_for_task(&app, &task_id, script, completion, timeout_secs, &cancel)
    })
    .await
    {
        Ok(attempt) => attempt,
        Err(error) => {
            let error = AppError::Io(std::io::Error::other(format!(
                "privileged script task failed: {error}"
            )));
            emit_output(&fallback_app, &fallback_task_id, format!("[ERROR] {error}"));
            let attempt = StreamResult::failure(error, false);
            finish_stream(
                &fallback_app,
                &fallback_task_id,
                &attempt,
                fallback_completion,
            );
            attempt
        }
    }
}
