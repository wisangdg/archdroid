export const TASK_STEP_CONFIGS = {
  gapps: [
    { id: "elevate", label: "Root Access", description: "Polkit elevation" },
    { id: "deps", label: "Dependencies", description: "System packages" },
    {
      id: "repo",
      label: "Source & Lock",
      description: "waydroid_script checkout",
    },
    {
      id: "env",
      label: "Python Venv",
      description: "Isolated environment & wheels",
    },
    {
      id: "download",
      label: "Verified Download",
      description: "MindTheGapps, SHA-256 pinned",
    },
    {
      id: "inject",
      label: "Injecting GApps",
      description: "Writing to rootfs",
    },
    { id: "done", label: "Ready", description: "Installation complete" },
  ],
  houdini: [
    { id: "elevate", label: "Root Access", description: "Polkit elevation" },
    { id: "deps", label: "Dependencies", description: "System packages" },
    {
      id: "repo",
      label: "Source & Lock",
      description: "waydroid_script checkout",
    },
    {
      id: "env",
      label: "Python Venv",
      description: "Isolated environment & wheels",
    },
    {
      id: "download",
      label: "Verified Download",
      description: "libhoudini, SHA-256 pinned",
    },
    {
      id: "inject",
      label: "Installing Houdini",
      description: "ARM translation bridge",
    },
    { id: "done", label: "Ready", description: "ARM support active" },
  ],
  init: [
    { id: "elevate", label: "Root Access", description: "Polkit elevation" },
    {
      id: "pkg",
      label: "Preflight & Packages",
      description: "System dependencies",
    },
    // `waydroid init` downloads the image and configures the container in
    // one command, so the script can only announce one stage for both.
    {
      id: "download",
      label: "Download & Initialize",
      description: "Android image and container",
    },
    { id: "done", label: "Ready", description: "Ready to launch" },
  ],
  phantom: [
    {
      id: "repo",
      label: "Source Checkout",
      description: "Cloning pinned Phantom repo",
    },
    {
      id: "build",
      label: "Compile & Install",
      description: "Building binaries as user",
    },
    { id: "done", label: "Ready", description: "Phantom ready" },
  ],
  wipe: [
    { id: "elevate", label: "Root Access", description: "Polkit elevation" },
    { id: "stop", label: "Stop Session", description: "Halting container" },
    {
      id: "wipe",
      label: "Wiping Storage",
      description: "Cleaning /var/lib/waydroid",
    },
    { id: "done", label: "Wipe Complete", description: "Clean state restored" },
  ],
};

/**
 * What the terminal says about the task, by outcome. The header badge, the
 * header status and the stage card all read this one table, so they cannot
 * disagree (a cancelled task used to keep the "active" card style).
 *
 * running ──► succeeded | failed | cancelled
 * `ended`: not running and no outcome known (output ended without a result).
 */
export const TERMINAL_OUTCOMES = {
  running: {
    badge: "badge-running",
    card: "status-active",
    status: "Running…",
  },
  succeeded: {
    badge: "badge-done",
    card: "status-completed",
    status: "Finished successfully",
    phase: "All Stages Completed",
  },
  failed: {
    badge: "badge-failed",
    card: "status-failed",
    status: "Process encountered errors",
    phase: "Execution Failed",
  },
  cancelled: {
    badge: "badge-cancelled",
    card: "status-cancelled",
    status: "Operation cancelled",
    phase: "Operation Cancelled",
  },
  ended: {
    badge: "badge-running",
    card: "status-active",
    status: "Task Complete",
  },
};

/** The terminal's outcome from `detectStepProgress` and whether it still runs. */
export function terminalOutcome(progress, running) {
  if (running) return "running";
  if (progress.isCancelled) return "cancelled";
  if (progress.isFailed) return "failed";
  if (progress.isCompleted) return "succeeded";
  return "ended";
}

const STEP_MARKER = /^\s*\[STEP\] (\S+)/;

/** Newest "[STEP] <id>" line that names a known step, plus the line after it. */
function lastStepMarker(logs, steps) {
  for (let i = logs.length - 1; i >= 0; i--) {
    const match = STEP_MARKER.exec(logs[i] || "");
    if (!match) continue;
    const index = steps.findIndex((step) => step.id === match[1]);
    if (index < 0) continue;
    const later = logs.slice(i + 1).filter(Boolean);
    return {
      index,
      message: later[later.length - 1] || steps[index].description,
    };
  }
  return null;
}

/**
 * `currentStep` is the stage id the task announced last. It is tracked outside
 * `logs`, which is capped: the marker line may no longer be in it.
 */
export function detectStepProgress(
  taskType,
  logs = [],
  taskResult = null,
  currentStep = null
) {
  const steps = TASK_STEP_CONFIGS[taskType] || [];
  const logText = logs.join("\n");

  // Prefer structured result from cmd-finished when available
  const isCancelled = taskResult
    ? taskResult.code === "cancelled" || taskResult.code === "cancelled_by_user"
    : logText.includes("[CANCELLED]") ||
      logText.includes("was cancelled") ||
      logText.includes("Operation cancelled");

  const isCompleted = taskResult
    ? taskResult.success === true
    : !isCancelled &&
      (logText.includes("[SUCCESS]") ||
        logText.includes("[PROCESS COMPLETED SUCCESSFULLY]") ||
        logText.includes("Wipe complete!"));

  const isFailed = taskResult
    ? taskResult.success === false && !isCancelled
    : !isCancelled &&
      (logText.includes("[ERROR]") ||
        logText.includes("Failed to install") ||
        logText.includes("[PROCESS FINISHED WITH ERRORS]"));

  if (steps.length === 0) {
    return {
      steps: [],
      activeStepIndex: 0,
      isCancelled,
      isFailed,
      isCompleted,
      currentMessage: logs[logs.length - 1] || "",
      errorMessage: isCancelled
        ? taskResult?.message || "Operation was cancelled by user."
        : isFailed
          ? taskResult?.message || "Operation failed."
          : null,
    };
  }

  let activeIndex = 0;
  let currentMessage = "Initiating task...";
  let errorMessage = null;

  if (isCancelled) {
    currentMessage = "Operation was cancelled by user.";
    errorMessage = taskResult?.message || "Operation was cancelled by user.";
  } else if (isFailed) {
    const errorLine = logs.find(
      (l) => l.includes("[ERROR]") || l.includes("Failed:")
    );
    errorMessage =
      taskResult?.message || errorLine || "The process terminated with errors.";
  }

  // A cancelled task still reports the stage it reached, so the stepper keeps
  // the finished stages instead of jumping back to stage 1.
  if (isCompleted) {
    activeIndex = steps.length - 1;
    currentMessage = "Task finished successfully!";
  } else {
    // Every script announces its phases with "[STEP] <id>" (scripts.rs and
    // waydroid_init in lib.rs). Before the first marker the task is still
    // elevating, which is the first stage.
    const marker = lastStepMarker(logs, steps);
    const known = steps.findIndex((step) => step.id === currentStep);
    if (marker && marker.index >= known) {
      activeIndex = marker.index;
      currentMessage = marker.message;
    } else if (known >= 0) {
      activeIndex = known;
      const shown = logs.filter(Boolean);
      currentMessage = shown[shown.length - 1] || steps[known].description;
    }
  }

  if (isCancelled) {
    currentMessage = "Operation was cancelled by user.";
  }

  return {
    steps,
    activeStepIndex: activeIndex,
    isFailed,
    isCancelled,
    isCompleted,
    currentMessage: currentMessage || "Processing...",
    errorMessage,
  };
}
