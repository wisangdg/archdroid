/**
 * Streaming events are global to the webview, so the UI must prove that an
 * event belongs to the task it is currently showing. A late `cmd-finished`
 * from the previous task must not reset the new task's terminal.
 */
export function isEventForTask(payload, activeTaskId) {
  return Boolean(activeTaskId) && payload?.task_id === activeTaskId;
}

/** Most lines the terminal keeps; older output is dropped. */
export const MAX_TERMINAL_LINES = 2000;

export function appendLogLine(logs, line) {
  const next = [...logs, line];
  return next.length > MAX_TERMINAL_LINES
    ? next.slice(next.length - MAX_TERMINAL_LINES)
    : next;
}
