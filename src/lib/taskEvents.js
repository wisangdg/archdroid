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

/** Empty terminal: the kept lines, how many older ones were dropped, and
 *  whether the last line is a progress-bar redraw. */
export const EMPTY_TERMINAL = { lines: [], dropped: 0, progressTail: false };

/**
 * Add a batch of `cmd-output` payloads (`{ line, progress }`).
 *
 * A progress redraw replaces the redraw before it, so a download is one line
 * that changes instead of ten new lines a second. `dropped` counts the lines
 * the cap removed: the UI says so, and it keeps row keys stable.
 */
export function applyOutput(state, entries) {
  if (entries.length === 0) return state;
  const lines = state.lines.slice();
  let { dropped, progressTail } = state;
  for (const { line, progress } of entries) {
    if (progressTail && progress && lines.length > 0) {
      lines[lines.length - 1] = line;
    } else {
      lines.push(line);
    }
    progressTail = Boolean(progress);
  }
  const overflow = lines.length - MAX_TERMINAL_LINES;
  if (overflow > 0) {
    lines.splice(0, overflow);
    dropped += overflow;
  }
  return { lines, dropped, progressTail };
}

const STEP_MARKER = /^\s*\[STEP\] (\S+)/;

/** The id of the newest "[STEP] <id>" in a batch of lines, if any. */
export function lastStepId(entries) {
  for (let i = entries.length - 1; i >= 0; i--) {
    const match = STEP_MARKER.exec(entries[i].line || "");
    if (match) return match[1];
  }
  return null;
}

/**
 * Percentage of a download from a progress line: tqdm (" 43%|████▎ | 84.1M")
 * or Waydroid's own ("[Downloading] 512.0 MB/1024.0 MB ...").
 */
export function progressPercent(line) {
  if (typeof line !== "string") return null;
  const bar = /^\s*(\d{1,3})%\|/.exec(line);
  if (bar) return Math.min(100, Number(bar[1]));
  const sizes = /\[Downloading\]\s+([\d.]+)\s*MB\/([\d.]+)\s*MB/.exec(line);
  if (sizes && Number(sizes[2]) > 0) {
    return Math.min(
      100,
      Math.round((100 * Number(sizes[1])) / Number(sizes[2]))
    );
  }
  return null;
}
