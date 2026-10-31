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
const MAX_CAPTURED_OUTPUT: usize = 4096;

/// How long a terminated script gets to run its `trap ... EXIT` cleanup before
/// the whole process group is killed outright. `elevate_env_for` relies on that
/// trap to remove `$ARCHDROID_WORK`, so SIGKILL must not be the first signal.
const KILL_GRACE: Duration = Duration::from_secs(5);

/// The single-flight slot. Streaming operations share one terminal, so only
/// one of them may own the slot at a time.
static ACTIVE_TASK: Mutex<Option<ActiveTask>> = Mutex::new(None);
static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

struct ActiveTask {
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
struct StreamStarted {
    task_id: String,
}

/// Payload for `cmd-output`. The task ID lets the UI ignore output that does
/// not belong to the operation it is currently showing.
#[derive(Clone, Serialize)]
struct StreamOutput {
    task_id: String,
    line: String,
}

/// Final payload for `cmd-finished`. The UI branches on `code`, so it never
/// has to parse the human-readable `message`.
#[derive(Clone, Serialize)]
struct StreamFinished {
    task_id: String,
    success: bool,
    code: Option<&'static str>,
    message: Option<String>,
}

pub(crate) struct StreamResult {
    outcome: Result<(), AppError>,
    /// True when the elevated shell actually started, i.e. elevation worked.
    started: bool,
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

    fn timed_out(&self) -> bool {
        matches!(&self.outcome, Err(AppError::Timeout { .. }))
    }

    pub(crate) fn cancelled(&self) -> bool {
        matches!(&self.outcome, Err(AppError::CancelRequested { .. }))
    }

    fn finished_event(&self, task_id: &str) -> StreamFinished {
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

    fn exit_code(&self) -> Option<i32> {
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

fn emit_output(app: &AppHandle, task_id: &str, line: impl Into<String>) {
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
fn failure_hint(text: &str) -> Option<&'static str> {
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

fn flush_line(app: &AppHandle, task_id: &str, buffer: &mut String, captured: &Arc<Mutex<String>>) {
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
fn spawn_line_reader(
    app: AppHandle,
    task_id: &str,
    reader: impl Read + Send + 'static,
    captured: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    let task_id = task_id.to_string();
    thread::spawn(move || {
        read_lines(reader, |mut line| {
            flush_line(&app, &task_id, &mut line, &captured)
        });
    })
}

/// Split a byte stream on `\n` and `\r`, decoding each line as UTF-8 only once
/// it is complete: progress bars (tqdm, pip) draw with multi-byte characters
/// such as `█`, which byte-by-byte decoding would turn into mojibake.
fn read_lines(reader: impl Read, mut on_line: impl FnMut(String)) {
    let mut reader = BufReader::new(reader);
    let mut line = Vec::new();
    let mut byte = [0; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if byte[0] == b'\n' || byte[0] == b'\r' {
            on_line(String::from_utf8_lossy(&line).into_owned());
            line.clear();
        } else {
            line.push(byte[0]);
        }
    }
    on_line(String::from_utf8_lossy(&line).into_owned());
}

/// Why a child is no longer being waited for.
enum WaitOutcome {
    Exited(ExitStatus),
    TimedOut,
    Cancelled,
}

/// Send `signal` to the process group led by `pid`.
///
/// `kill` is resolved without PATH, like every other helper that runs with more
/// privilege than the caller. `--` stops the negative group ID from being read
/// as an option.
fn signal_process_group(pid: u32, signal: &str) -> bool {
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
fn terminate_process_group(child: &mut Child) {
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
fn request_privileged_cancel(cancel_path: Option<&Path>) {
    if let Some(path) = cancel_path {
        let _ = std::fs::write(path, "cancel\n");
    }
}

/// Wait for `child`, but stop its process group when the deadline elapses or
/// the task-scoped cancel flag is raised.
fn wait_with_deadline(
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
fn stream_outcome(
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
fn run_and_stream(
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
fn apply_completion(attempt: &StreamResult, completion: Completion) -> Option<String> {
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

fn finish_stream(app: &AppHandle, task_id: &str, attempt: &StreamResult, completion: Completion) {
    if let Some(warning) = apply_completion(attempt, completion) {
        emit_output(app, task_id, warning);
    }
    let _ = app.emit("cmd-finished", attempt.finished_event(task_id));
}

/// Run a user-level script. Phantom installs into `$HOME/.local`, so it does
/// not need root at all; keeping it out of the privileged path also avoids
/// running a user-controlled Rust toolchain as root.
fn run_user_script(
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
enum ElevationStep {
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
fn pkexec_step(attempt: &mut StreamResult, forced: &str) -> ElevationStep {
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
fn sudo_failure(attempt: StreamResult) -> Result<StreamResult, AppError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use std::os::unix::process::CommandExt;
    use std::process::{ExitStatus, Stdio};
    use std::time::Instant;

    #[test]
    fn success_reports_no_error_code() {
        let result = CommandResult::with_data("Session:\tRUNNING".to_string());
        assert_eq!(result.status, "success");
        assert_eq!(result.code, None);
        assert_eq!(result.data.as_deref(), Some("Session:\tRUNNING"));
    }

    #[test]
    fn a_successful_stream_maps_to_a_success_result_only_after_it_exits() {
        let result = StreamResult::success(true).into_command_result_with_message("written");
        assert_eq!(result.status, "success");
        assert_eq!(result.message.as_deref(), Some("written"));
        assert_eq!(result.code, None);
    }

    #[test]
    fn a_failed_stream_maps_to_a_typed_error_result() {
        let result = StreamResult::failure(
            AppError::elevation_denied("The polkit authentication dialog was dismissed."),
            false,
        )
        .into_command_result();
        assert_eq!(result.status, "error");
        assert_eq!(result.code.as_deref(), Some(error::code::ELEVATION_DENIED));
        assert!(result.message.unwrap().contains("dismissed"));
    }

    #[test]
    fn stream_exit_codes_stay_available_for_the_polkit_dismissal_path() {
        let attempt = StreamResult::failure(
            AppError::Exit {
                program: "pkexec".to_string(),
                code: Some(126),
                output: String::new(),
            },
            false,
        );
        assert!(!attempt.is_success());
        assert_eq!(attempt.exit_code(), Some(126));
    }

    /// `ACTIVE_TASK` is process-global, so tests that manipulate the slot must
    /// not run concurrently with each other.
    static TASK_SLOT_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn lock_task_slot_for_test() -> std::sync::MutexGuard<'static, ()> {
        TASK_SLOT_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn single_flight_allows_only_one_active_task() {
        let _slot = lock_task_slot_for_test();
        let first = TaskGuard::start("first task").expect("the slot must start empty");
        let first_id = first.id().to_string();
        assert!(first_id.starts_with("task-"));

        let error = match TaskGuard::start("second task") {
            Ok(_) => panic!("a second task must not acquire the active slot"),
            Err(error) => error,
        };
        assert_eq!(error.code(), error::code::BUSY);
        assert!(error.to_string().contains("first task"), "{error}");

        drop(first);
        let next = TaskGuard::start("third task").expect("dropping must release the slot");
        assert_ne!(first_id, next.id());
    }

    /// Every non-streaming command that changes Waydroid state goes through
    /// `run_exclusive`, so a wipe/init/installer holding the slot makes them
    /// fail with `busy` instead of racing the image.
    #[test]
    fn state_changing_commands_are_rejected_while_a_privileged_task_runs() {
        let _slot = lock_task_slot_for_test();
        let privileged =
            TaskGuard::start("privileged operation").expect("the slot must start empty");

        for label in [
            "session start",
            "session stop",
            "APK install",
            "app uninstall",
            "property update",
            "app launch",
        ] {
            let result =
                tauri::async_runtime::block_on(run_exclusive(label, |_| CommandResult::success()));
            assert_eq!(result.status, "error", "{label} must be rejected");
            assert_eq!(
                result.code.as_deref(),
                Some(error::code::BUSY),
                "{label} must report busy"
            );
            assert!(
                result
                    .message
                    .as_deref()
                    .unwrap_or("")
                    .contains("privileged operation"),
                "{label} must name the operation that owns the slot"
            );
        }

        drop(privileged);
        let result = tauri::async_runtime::block_on(run_exclusive("session start", |_| {
            CommandResult::success()
        }));
        assert_eq!(result.status, "success", "the slot must be free again");
        assert_eq!(result.code, None);
    }

    #[test]
    fn cancel_only_matches_the_task_that_owns_the_slot() {
        let _slot = lock_task_slot_for_test();
        assert!(!cancel_active_task("task-none"), "no task must be a no-op");

        let task = TaskGuard::start("cancellable task").expect("the slot must start empty");
        let task_id = task.id().to_string();
        assert!(!task.is_cancelled());

        assert!(!cancel_active_task("task-999999"));
        assert!(
            !task.is_cancelled(),
            "a stale ID must not cancel the active task"
        );

        assert!(cancel_active_task(&task_id));
        assert!(task.is_cancelled(), "the matching task must be cancelled");
    }

    #[test]
    fn read_lines_keeps_utf8_progress_bars_intact() {
        let input = " 42%|████▏     | 82.5M\r 43%|████▎     | 84.1M\nlast";
        let mut lines = Vec::new();
        read_lines(input.as_bytes(), |line| lines.push(line));
        assert_eq!(
            lines,
            [" 42%|████▏     | 82.5M", " 43%|████▎     | 84.1M", "last"]
        );
    }

    #[test]
    fn a_stale_cancel_cannot_reach_the_next_task() {
        let _slot = lock_task_slot_for_test();
        let first = TaskGuard::start("first task").expect("the slot must start empty");
        let stale_id = first.id().to_string();
        drop(first);

        let second = TaskGuard::start("second task").expect("the slot must be reusable");
        assert!(!cancel_active_task(&stale_id));
        assert!(
            !second.is_cancelled(),
            "the replacement task must stay untouched"
        );
        assert!(cancel_active_task(second.id()));
        assert!(second.is_cancelled());
    }

    fn wait_for_pid_file(path: &Path) -> i32 {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(pid) = text.trim().parse() {
                    return pid;
                }
            }
            assert!(Instant::now() < deadline, "pid file was never written");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn process_is_gone(pid: i32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let alive = Command::new("kill")
                .args(["-0", &pid.to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if !alive {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// A deadline must stop the shell, the child it spawned, and let the EXIT
    /// trap clean up before the group is SIGKILLed.
    #[test]
    fn a_script_past_its_deadline_is_killed_with_its_children_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!(
            "archdroid-deadline-{}-{}",
            std::process::id(),
            NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let work = dir.join("work");
        let pid_file = dir.join("grandchild.pid");
        std::fs::create_dir_all(&work).expect("test workdir must be creatable");

        let mut command = Command::new("bash");
        command
            .args([
                "--noprofile",
                "--norc",
                "-c",
                r#"trap 'rm -rf "$WORK"' EXIT TERM
sleep 300 &
echo $! > "$PID_FILE"
wait
"#,
            ])
            .env("WORK", &work)
            .env("PID_FILE", &pid_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = command.spawn().expect("bash must start");

        let grandchild = wait_for_pid_file(&pid_file);
        let started = Instant::now();
        let cancel = AtomicBool::new(false);
        let outcome =
            wait_with_deadline(&mut child, 1, &cancel, None).expect("waiting must not fail");

        assert!(matches!(outcome, WaitOutcome::TimedOut));
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the deadline was not enforced"
        );
        assert!(!work.exists(), "the EXIT trap must run before SIGKILL");
        assert!(
            process_is_gone(grandchild),
            "the child spawned by the script must not be orphaned"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Cancel uses the same group termination as a deadline, so it must also
    /// stop grandchildren and let the EXIT trap clean the work directory.
    #[test]
    fn a_cancelled_script_is_killed_with_its_children_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!(
            "archdroid-cancel-{}-{}",
            std::process::id(),
            NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let work = dir.join("work");
        let pid_file = dir.join("grandchild.pid");
        std::fs::create_dir_all(&work).expect("test workdir must be creatable");

        let mut command = Command::new("bash");
        command
            .args([
                "--noprofile",
                "--norc",
                "-c",
                r#"trap 'rm -rf "$WORK"' EXIT TERM
sleep 300 &
echo $! > "$PID_FILE"
wait
"#,
            ])
            .env("WORK", &work)
            .env("PID_FILE", &pid_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = command.spawn().expect("bash must start");

        let grandchild = wait_for_pid_file(&pid_file);
        let cancel = AtomicBool::new(true);
        let started = Instant::now();
        let outcome =
            wait_with_deadline(&mut child, 30, &cancel, None).expect("waiting must not fail");

        assert!(matches!(outcome, WaitOutcome::Cancelled));
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "cancel was not enforced"
        );
        assert!(!work.exists(), "the EXIT trap must run before SIGKILL");
        assert!(
            process_is_gone(grandchild),
            "the child spawned by the script must not be orphaned"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_script_that_finishes_before_its_deadline_keeps_its_exit_code() {
        let mut command = Command::new("bash");
        command
            .args(["--noprofile", "--norc", "-c", "exit 7"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let mut child = command.spawn().expect("bash must start");

        let cancel = AtomicBool::new(false);
        match wait_with_deadline(&mut child, 30, &cancel, None).expect("waiting must not fail") {
            WaitOutcome::Exited(status) => assert_eq!(status.code(), Some(7)),
            WaitOutcome::TimedOut => panic!("a quick script must not time out"),
            WaitOutcome::Cancelled => panic!("a quick script must not be cancelled"),
        }
    }

    /// Run a script for real so the mapping is tested against a genuine
    /// `ExitStatus`, not a hand-written one.
    fn run_bash(script: &str) -> ExitStatus {
        Command::new("bash")
            .args(["--noprofile", "--norc", "-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("bash must start")
    }

    fn exit_failure(program: &str, code: i32, started: bool) -> StreamResult {
        StreamResult::failure(
            AppError::Exit {
                program: program.to_string(),
                code: Some(code),
                output: String::new(),
            },
            started,
        )
    }

    #[test]
    fn an_installer_that_exits_1_becomes_an_exit_error_not_a_success() {
        let status = run_bash("echo 'installer exploded' >&2; exit 1");
        let attempt = stream_outcome(
            "bash",
            Ok(WaitOutcome::Exited(status)),
            "installer exploded".to_string(),
            30,
            |_| panic!("an exit code is already visible; no extra error line is needed"),
        );

        assert!(!attempt.is_success());
        assert!(!attempt.started);
        assert_eq!(attempt.exit_code(), Some(1));
        let event = attempt.finished_event("task-exit");
        assert_eq!(event.code, Some(error::code::EXIT));
        assert!(
            event.message.unwrap_or_default().contains("exit code 1"),
            "the exit code must be in the message"
        );
    }

    #[test]
    fn an_exit_after_the_privileged_shell_started_is_still_an_exit() {
        let status = run_bash("exit 1");
        let attempt = stream_outcome(
            "bash",
            Ok(WaitOutcome::Exited(status)),
            format!("{ELEVATED_SENTINEL}\nsomething failed\n"),
            30,
            |_| {},
        );

        assert!(attempt.started, "the sentinel proves the body ran as root");
        assert_eq!(attempt.exit_code(), Some(1));
        assert_eq!(
            attempt.finished_event("task-root").code,
            Some(error::code::EXIT)
        );
    }

    #[test]
    fn a_dismissed_polkit_dialog_is_terminal_and_never_falls_back_to_sudo() {
        let mut attempt = exit_failure("pkexec", 126, false);

        assert_eq!(pkexec_step(&mut attempt, ""), ElevationStep::Stop);
        let event = attempt.finished_event("task-126");
        assert_eq!(event.code, Some(error::code::ELEVATION_DENIED));
        assert!(
            event.message.unwrap_or_default().contains("dismissed"),
            "the user must be told the dialog was dismissed"
        );
        assert_eq!(attempt.exit_code(), None, "126 is no longer an exit code");
    }

    #[test]
    fn a_polkit_failure_that_never_started_may_fall_back_to_sudo() {
        let mut attempt = exit_failure("pkexec", 1, false);

        assert_eq!(pkexec_step(&mut attempt, ""), ElevationStep::FallBackToSudo);
        assert_eq!(
            attempt.exit_code(),
            Some(1),
            "the attempt must stay untouched for the next step"
        );
    }

    #[test]
    fn forcing_pkexec_never_falls_back_even_when_it_never_started() {
        let mut attempt = exit_failure("pkexec", 1, false);

        assert_eq!(pkexec_step(&mut attempt, "pkexec"), ElevationStep::Stop);
        assert_eq!(
            attempt.finished_event("task-forced").code,
            Some(error::code::ELEVATION_DENIED)
        );
    }

    #[test]
    fn a_script_that_started_and_failed_does_not_open_a_second_prompt() {
        let mut attempt = exit_failure("bash", 1, true);

        assert_eq!(pkexec_step(&mut attempt, ""), ElevationStep::Stop);
        assert_eq!(
            attempt.finished_event("task-body").code,
            Some(error::code::EXIT)
        );
    }

    #[test]
    fn a_hung_or_cancelled_polkit_prompt_never_opens_a_sudo_prompt() {
        let mut timed_out = StreamResult::failure(
            AppError::Timeout {
                program: "pkexec".to_string(),
                seconds: 1,
            },
            false,
        );
        assert_eq!(pkexec_step(&mut timed_out, ""), ElevationStep::Stop);
        assert_eq!(
            timed_out.finished_event("task-hang").code,
            Some(error::code::TIMEOUT)
        );

        let mut cancelled = StreamResult::failure(
            AppError::CancelRequested {
                program: "pkexec".to_string(),
            },
            false,
        );
        assert_eq!(pkexec_step(&mut cancelled, ""), ElevationStep::Stop);
        assert_eq!(
            cancelled.finished_event("task-cancel").code,
            Some(error::code::CANCELLED)
        );
    }

    #[test]
    fn a_sudo_that_never_elevated_becomes_elevation_denied() {
        let error = match sudo_failure(exit_failure("sudo", 1, false)) {
            Ok(_) => panic!("a sudo failure that never started must be replaced"),
            Err(error) => error,
        };
        assert_eq!(error.code(), error::code::ELEVATION_DENIED);
        assert!(
            error.to_string().contains("polkit authentication agent"),
            "{error}"
        );
    }

    #[test]
    fn a_sudo_script_that_started_and_failed_keeps_its_exit_code() {
        let attempt = sudo_failure(exit_failure("bash", 2, true))
            .expect("a script that ran as root keeps its result");
        assert_eq!(attempt.exit_code(), Some(2));
        assert_eq!(
            attempt.finished_event("task-sudo").code,
            Some(error::code::EXIT)
        );
    }

    #[test]
    fn a_hanging_command_reports_timeout_and_streams_the_reason() {
        let mut emitted = None;
        let attempt = stream_outcome(
            "bash",
            Ok(WaitOutcome::TimedOut),
            String::new(),
            5,
            |line| emitted = Some(line),
        );

        assert_eq!(
            attempt.finished_event("task-timeout").code,
            Some(error::code::TIMEOUT)
        );
        assert!(
            emitted.unwrap_or_default().contains("within 5s"),
            "the timeout line must tell the user the deadline"
        );
    }

    #[test]
    fn a_timeout_after_elevation_stays_started_and_is_terminal() {
        let attempt = StreamResult::failure(
            AppError::Timeout {
                program: "bash".to_string(),
                seconds: 1,
            },
            true,
        );
        assert!(attempt.started);
        assert!(attempt.timed_out());
        assert!(!attempt.is_success());
        assert_eq!(attempt.exit_code(), None);
        assert_eq!(
            attempt.finished_event("task-1").code,
            Some(error::code::TIMEOUT)
        );
    }

    #[test]
    fn a_cancelled_stream_is_terminal_and_not_an_elevation_failure() {
        let attempt = StreamResult::failure(
            AppError::CancelRequested {
                program: "bash".to_string(),
            },
            false,
        );
        assert!(attempt.cancelled());
        assert!(!attempt.timed_out());
        assert!(!attempt.is_success());
        assert_eq!(attempt.exit_code(), None);
        assert_eq!(
            attempt.finished_event("task-2").code,
            Some(error::code::CANCELLED)
        );
    }

    #[test]
    fn a_cancelled_stream_never_writes_its_completion_marker() {
        let marker = marker_file("cancel_test_marker");
        let _ = std::fs::remove_file(&marker);

        apply_completion(
            &StreamResult::failure(
                AppError::CancelRequested {
                    program: "installer".to_string(),
                },
                true,
            ),
            Completion::WriteMarker("cancel_test_marker"),
        );
        assert!(
            !marker.exists(),
            "a cancel must not leave an installed marker"
        );
    }

    #[test]
    fn a_timed_out_stream_never_writes_its_completion_marker() {
        let marker = marker_file("timeout_test_marker");
        let _ = std::fs::remove_file(&marker);

        apply_completion(
            &StreamResult::failure(
                AppError::Timeout {
                    program: "installer".to_string(),
                    seconds: 1,
                },
                true,
            ),
            Completion::WriteMarker("timeout_test_marker"),
        );
        assert!(
            !marker.exists(),
            "a timeout must not leave an installed marker"
        );

        apply_completion(
            &StreamResult::success(true),
            Completion::WriteMarker("timeout_test_marker"),
        );
        assert!(marker.exists(), "the gate must still apply a real success");
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn a_failed_installer_never_writes_an_installed_marker() {
        let marker = marker_file("exit_test_marker");
        let _ = std::fs::remove_file(&marker);

        apply_completion(
            &exit_failure("installer", 1, true),
            Completion::WriteMarker("exit_test_marker"),
        );
        assert!(
            !marker.exists(),
            "a non-zero exit must not look like a completed install"
        );
    }

    #[test]
    fn installer_markers_follow_reinstall_success_failure_and_removal() {
        let marker = marker_file("reinstall_test_marker");
        let _ = std::fs::remove_file(&marker);
        let completion = Completion::WriteMarker("reinstall_test_marker");

        // Fresh success: pending until a live check confirms it.
        apply_completion(&StreamResult::success(true), completion);
        assert_eq!(marker_state(&marker), MarkerState::Pending);
        assert_eq!(
            offline_marker_report(&marker, "X").unwrap().status,
            "pending"
        );

        // Live check confirms; a second successful install is idempotent.
        refresh_marker(&marker);
        assert_eq!(marker_state(&marker), MarkerState::Verified);
        apply_completion(&StreamResult::success(true), completion);
        assert_eq!(marker_state(&marker), MarkerState::Pending);
        refresh_marker(&marker);

        // A refused prompt touched nothing: the verified marker survives.
        apply_completion(
            &StreamResult::failure(AppError::elevation_denied("dismissed"), false),
            completion,
        );
        assert_eq!(marker_state(&marker), MarkerState::Verified);

        // Failing half-way as root invalidates the old marker.
        apply_completion(&exit_failure("installer", 1, true), completion);
        assert_eq!(marker_state(&marker), MarkerState::Absent);
        assert!(offline_marker_report(&marker, "X").is_none());

        // A legacy "installed" marker is not trusted as verified.
        std::fs::write(&marker, "installed\n").unwrap();
        assert_eq!(marker_state(&marker), MarkerState::Pending);
        let _ = std::fs::remove_file(&marker);
    }

    #[test]
    fn failure_hints_name_disk_and_network_problems() {
        assert!(
            failure_hint("cp: error writing 'x': No space left on device")
                .unwrap()
                .contains("disk filled up")
        );
        assert!(failure_hint(
            "fatal: unable to access 'https://github.com/x': Could not resolve host: github.com"
        )
        .unwrap()
        .contains("network dropped"));
        assert!(failure_hint("ERROR: Failed to establish a new connection").is_some());
        assert!(failure_hint("[ERROR] Failed to install GApps.").is_none());
    }

    #[test]
    fn a_denied_elevation_never_writes_an_installed_marker() {
        let marker = marker_file("elevation_test_marker");
        let _ = std::fs::remove_file(&marker);

        apply_completion(
            &StreamResult::failure(AppError::elevation_denied("dismissed"), false),
            Completion::WriteMarker("elevation_test_marker"),
        );
        assert!(
            !marker.exists(),
            "a refused password prompt must not look like a completed install"
        );
    }

    #[test]
    fn clear_install_markers_only_clears_on_success() {
        let m1 = marker_file("gapps_installed");
        let m2 = marker_file("houdini_installed");
        let m3 = marker_file("multitouch_installed");

        write_marker(&m1).unwrap();
        write_marker(&m2).unwrap();
        write_marker(&m3).unwrap();

        assert!(m1.exists());
        assert!(m2.exists());
        assert!(m3.exists());

        // A failed attempt must NOT clear markers
        apply_completion(
            &exit_failure("wipe", 1, true),
            Completion::ClearInstallMarkers,
        );
        assert!(m1.exists(), "failure must not clear markers");
        assert!(m2.exists(), "failure must not clear markers");
        assert!(m3.exists(), "failure must not clear markers");

        // A cancelled attempt must NOT clear markers
        apply_completion(
            &StreamResult::failure(
                AppError::CancelRequested {
                    program: "wipe".to_string(),
                },
                true,
            ),
            Completion::ClearInstallMarkers,
        );
        assert!(m1.exists(), "cancel must not clear markers");
        assert!(m2.exists(), "cancel must not clear markers");
        assert!(m3.exists(), "cancel must not clear markers");

        // A successful attempt MUST clear markers
        apply_completion(
            &StreamResult::success(true),
            Completion::ClearInstallMarkers,
        );
        assert!(!m1.exists(), "successful wipe must clear gapps marker");
        assert!(!m2.exists(), "successful wipe must clear houdini marker");
        assert!(!m3.exists(), "successful wipe must clear multitouch marker");
    }

    #[test]
    fn finished_event_is_structured_even_for_success() {
        let value =
            serde_json::to_value(StreamResult::success(true).finished_event("task-7")).unwrap();
        assert_eq!(value["task_id"], "task-7");
        assert_eq!(value["success"], true);
        assert!(value["code"].is_null());
        assert!(value["message"].is_null());
    }

    #[test]
    fn started_event_carries_the_task_id() {
        let value = serde_json::to_value(StreamStarted {
            task_id: "task-11".to_string(),
        })
        .unwrap();
        assert_eq!(value["task_id"], "task-11");
        assert_eq!(value.as_object().unwrap().len(), 1);
    }

    #[test]
    fn output_event_carries_the_task_id_with_the_line() {
        let value = serde_json::to_value(StreamOutput {
            task_id: "task-9".to_string(),
            line: "installing".to_string(),
        })
        .unwrap();
        assert_eq!(value["task_id"], "task-9");
        assert_eq!(value["line"], "installing");
    }

    #[test]
    fn finished_event_carries_the_error_code_instead_of_only_a_boolean() {
        for (error, expected) in [
            (
                AppError::elevation_denied("dismissed"),
                error::code::ELEVATION_DENIED,
            ),
            (
                AppError::Timeout {
                    program: "installer".to_string(),
                    seconds: 60,
                },
                error::code::TIMEOUT,
            ),
            (
                AppError::CancelRequested {
                    program: "installer".to_string(),
                },
                error::code::CANCELLED,
            ),
        ] {
            let value =
                serde_json::to_value(StreamResult::failure(error, false).finished_event("task-8"))
                    .unwrap();
            assert_eq!(value["task_id"], "task-8");
            assert_eq!(value["success"], false);
            assert_eq!(value["code"], expected);
            assert!(value["message"].is_string());
        }
    }
}
