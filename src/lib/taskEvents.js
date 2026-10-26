/**
 * Streaming events are global to the webview, so the UI must prove that an
 * event belongs to the task it is currently showing. A late `cmd-finished`
 * from the previous task must not reset the new task's terminal.
 */
export function isEventForTask(payload, activeTaskId) {
  return Boolean(activeTaskId) && payload?.task_id === activeTaskId;
}
