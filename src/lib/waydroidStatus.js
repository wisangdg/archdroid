/**
 * Parse `waydroid status` output (the `data` of the `waydroid_status` reply).
 * Mirrors `session_running`/`container_frozen` in src-tauri/src/waydroid.rs:
 * one `Key:\tVALUE` pair per line, and only `Session:` decides whether the
 * session runs.
 */
export function parseWaydroidStatus(text) {
  const values = {};
  for (const line of String(text ?? "").split("\n")) {
    const colon = line.indexOf(":");
    if (colon < 0) continue;
    const value = line.slice(colon + 1).trim();
    if (value) values[line.slice(0, colon).trim()] = value;
  }
  return {
    running: values.Session === "RUNNING",
    frozen: values.Container === "FROZEN",
  };
}

/** Whether a `waydroid_status` reply says the session runs. */
export function sessionRunning(res) {
  return res?.status === "success" && parseWaydroidStatus(res.data).running;
}
