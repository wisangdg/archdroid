/**
 * The engine's one visible state, derived from what `useEngineStatus` tracks.
 *
 *   checking ──► uninitialized          (no Waydroid image yet: setup screen)
 *      │
 *      ▼
 *   stopped ──start──► starting ──► booting ──► running
 *      ▲                                          │
 *      └──────── stopping ◄──────stop─────────────┤
 *                                                 │
 *               restarting ◄────restart───────────┘ (then booting)
 *
 *   unreachable: `waydroid status` itself fails, so whether the session runs
 *   is unknown (it was shown as "Engine Stopped" before).
 *
 * An operation in flight wins over the polled status: during a start the
 * session reports offline for a few seconds, and that is "starting", not
 * "stopped". Each phase has one label and one CSS tone, so the top bar and any
 * other view cannot disagree about it; the badge carries the phase as
 * `data-engine-phase` for tests and automation.
 */
export function enginePhase({
  status,
  isInitialized,
  operation = null,
  booting = false,
  unreachable = false,
}) {
  if (operation === "start") return "starting";
  if (operation === "stop") return "stopping";
  if (operation === "restart") return "restarting";
  if (isInitialized === null) return "checking";
  if (isInitialized === false) return "uninitialized";
  if (status === "online") return booting ? "booting" : "running";
  return unreachable ? "unreachable" : "stopped";
}

/**
 * Top bar text and tone per phase. The tone is the badge class
 * (`status-online`, `status-offline`, `status-pending`): green only while the
 * session runs, amber while Start/Stop/Restart is in flight. `transient`
 * phases end by themselves, so only they may animate (a steady animation kept
 * WebKitGTK repainting).
 */
export const ENGINE_PHASES = {
  checking: { label: "Checking Engine…", tone: "offline", transient: true },
  uninitialized: { label: "Not Initialized", tone: "offline" },
  stopped: { label: "Engine Stopped", tone: "offline" },
  unreachable: { label: "Waydroid Not Responding", tone: "offline" },
  starting: { label: "Starting Engine…", tone: "pending", transient: true },
  booting: { label: "Android Booting…", tone: "online", transient: true },
  running: { label: "Engine Running", tone: "online" },
  stopping: { label: "Stopping Engine…", tone: "pending", transient: true },
  restarting: { label: "Restarting Engine…", tone: "pending", transient: true },
};
