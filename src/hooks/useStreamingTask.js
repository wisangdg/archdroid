import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isEventForTask, appendLogLine } from "../lib/taskEvents";

const PROCESS_ERROR_TITLES = {
  elevation_denied: "Root Access Denied",
  timeout: "Operation Timed Out",
  cancelled: "Operation Cancelled",
};

/**
 * Owns the single streaming task shown in the terminal modal: the backend's
 * `cmd-started`/`cmd-output`/`cmd-finished` events, the single-flight flag, and
 * Cancel by task ID. `onTaskSuccess` runs after a task finishes successfully so
 * the caller can refresh whatever the task may have changed.
 */
export function useStreamingTask({ addToast, onTaskSuccess }) {
  const [installing, setInstalling] = useState(false);
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [terminalLogs, setTerminalLogs] = useState([]);
  const [terminalTitle, setTerminalTitle] = useState("");
  const [terminalTaskType, setTerminalTaskType] = useState("logs");
  const [terminalTaskResult, setTerminalTaskResult] = useState(null);
  const terminalRef = useRef(null);
  const operationInFlight = useRef(false);
  // ID of the streaming task currently shown in the terminal. Events from any
  // other task (a late finish, or a non-modal background write) are ignored.
  const activeTaskId = useRef(null);
  // Whether the backend actually emitted `cmd-started` for the current request.
  // A preflight error returns before any event, so the terminal must be closed
  // and the error shown instead of leaving it stuck on "Awaiting...".
  const taskStartedRef = useRef(false);
  const onTaskSuccessRef = useRef(onTaskSuccess);
  onTaskSuccessRef.current = onTaskSuccess;

  const runStreamingTask = async (command, { taskType, title, args } = {}) => {
    if (operationInFlight.current) {
      addToast({
        type: "warning",
        title: "Operation In Progress",
        message:
          "Wait for the current operation to finish before starting another.",
      });
      return null;
    }

    operationInFlight.current = true;
    activeTaskId.current = null;
    taskStartedRef.current = false;
    setInstalling(true);
    setTerminalTaskType(taskType);
    setTerminalTitle(title);
    setTerminalLogs([]);
    setTerminalTaskResult(null);
    setTerminalOpen(true);

    try {
      const result = await invoke(command, args);
      if (result?.status === "error" && result.code === "busy") {
        operationInFlight.current = false;
        activeTaskId.current = null;
        setInstalling(false);
        addToast({
          type: "warning",
          title: "Operation In Progress",
          message: result.message || "Another operation is already running.",
        });
      } else if (result?.status === "error" && !taskStartedRef.current) {
        operationInFlight.current = false;
        activeTaskId.current = null;
        setInstalling(false);
        setTerminalOpen(false);
        addToast({
          type: "error",
          title: "Operation Failed",
          message: result.message || `${title} could not start.`,
        });
      }
      return result;
    } catch (error) {
      operationInFlight.current = false;
      activeTaskId.current = null;
      setInstalling(false);
      throw error;
    }
  };

  useEffect(() => {
    const unlistenStarted = listen("cmd-started", (event) => {
      if (operationInFlight.current) {
        taskStartedRef.current = true;
        activeTaskId.current = event.payload.task_id;
      }
    });

    const unlistenOutput = listen("cmd-output", (event) => {
      if (!isEventForTask(event.payload, activeTaskId.current)) return;
      setTerminalLogs((prev) => appendLogLine(prev, event.payload.line));
      if (terminalRef.current) {
        terminalRef.current.scrollTop = terminalRef.current.scrollHeight;
      }
    });

    const unlistenFinished = listen("cmd-finished", (event) => {
      const { success, code, message } = event.payload;
      // A late event from the previous task must not close the terminal or
      // clear the single-flight flag of the task that replaced it.
      if (!isEventForTask(event.payload, activeTaskId.current)) return;
      activeTaskId.current = null;
      setTerminalTaskResult({ success, code, message });
      setTerminalLogs((prev) => [
        ...prev,
        success
          ? "\n[PROCESS COMPLETED SUCCESSFULLY]"
          : `\n[PROCESS FINISHED WITH ERRORS${code ? ` (${code})` : ""}]`,
      ]);
      operationInFlight.current = false;
      setInstalling(false);
      if (success) {
        addToast({
          type: "success",
          title: "Process Finished",
          message: "Operation completed successfully.",
        });
        onTaskSuccessRef.current?.();
      } else {
        addToast({
          type: "error",
          title: PROCESS_ERROR_TITLES[code] || "Process Failed",
          message: message || "Check the terminal output for details.",
        });
      }
    });

    return () => {
      unlistenStarted.then((f) => f());
      unlistenOutput.then((f) => f());
      unlistenFinished.then((f) => f());
    };
  }, [addToast]);

  const handleCancelTask = useCallback(async () => {
    const taskId = activeTaskId.current;
    if (!taskId) {
      addToast({
        type: "warning",
        title: "Nothing to Cancel",
        message: "No streaming operation is active.",
      });
      return;
    }

    try {
      addToast({
        type: "warning",
        title: "Cancelling Operation",
        message: "Stopping the current operation...",
      });
      // Send the ID, not a global flag: a stale click must not stop the next
      // task, and the backend only cancels the operation that owns this ID.
      await invoke("cancel_task", { taskId });
    } catch (err) {
      console.error("Cancel task failed:", err);
      addToast({
        type: "error",
        title: "Cancellation Error",
        message: String(err),
      });
    }
  }, [addToast]);

  return {
    installing,
    operationInFlight,
    terminalRef,
    terminalOpen,
    terminalLogs,
    terminalTitle,
    terminalTaskType,
    terminalTaskResult,
    setTerminalOpen,
    setTerminalTaskResult,
    runStreamingTask,
    handleCancelTask,
  };
}
