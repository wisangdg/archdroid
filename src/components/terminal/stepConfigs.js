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
    {
      id: "download",
      label: "Download Image",
      description: "Android system image",
    },
    { id: "init", label: "Initialize", description: "Configuring container" },
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

export function detectStepProgress(taskType, logs = [], taskResult = null) {
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

  if (isCompleted) {
    activeIndex = steps.length - 1;
    currentMessage = "Task finished successfully!";
  } else if (isCancelled) {
    currentMessage = "Operation was cancelled by user.";
  } else if (lastStepMarker(logs, steps)) {
    // Scripts announce each phase with "[STEP] <id>"; trust that over guessing.
    const marker = lastStepMarker(logs, steps);
    activeIndex = marker.index;
    currentMessage = marker.message;
  } else {
    // Scan logs backwards to find the current active step based on taskType
    for (let i = logs.length - 1; i >= 0; i--) {
      const line = logs[i];
      if (!line) continue;

      if (taskType === "gapps" || taskType === "houdini") {
        if (
          line.includes("Injecting GApps") ||
          line.includes("Installing ARM Translation") ||
          line.includes("libhoudini")
        ) {
          activeIndex = 4;
          currentMessage = line;
          break;
        }
        if (
          line.includes("Setting up isolated Python venv") ||
          line.includes("Installing Python requirements") ||
          line.includes("pip install")
        ) {
          activeIndex = 3;
          currentMessage = line;
          break;
        }
        if (
          line.includes("Cloning pinned") ||
          line.includes("waydroid_script")
        ) {
          activeIndex = 2;
          currentMessage = line;
          break;
        }
        if (
          line.includes("Installing dependencies") ||
          line.includes("pacman") ||
          line.includes("apt") ||
          line.includes("dnf") ||
          line.includes("zypper")
        ) {
          activeIndex = 1;
          currentMessage = line;
          break;
        }
        if (
          line.includes("Requesting root access") ||
          line.includes("Elevated shell started")
        ) {
          activeIndex = 0;
          currentMessage = line;
          break;
        }
      } else if (taskType === "phantom") {
        if (
          line.includes("Running Phantom installer") ||
          line.includes("Compiling") ||
          line.includes("cargo") ||
          line.includes("Building")
        ) {
          activeIndex = 1;
          currentMessage = line;
          break;
        }
        if (line.includes("Cloning pinned") || line.includes("git")) {
          activeIndex = 0;
          currentMessage = line;
          break;
        }
      } else if (taskType === "wipe") {
        if (line.includes("Wiping Waydroid")) {
          activeIndex = 2;
          currentMessage = line;
          break;
        }
        if (line.includes("Stopping Waydroid")) {
          activeIndex = 1;
          currentMessage = line;
          break;
        }
        if (
          line.includes("Requesting root access") ||
          line.includes("Elevated shell started")
        ) {
          activeIndex = 0;
          currentMessage = line;
          break;
        }
      } else if (taskType === "init") {
        if (
          line.includes("Initializing Waydroid") ||
          line.includes("waydroid init")
        ) {
          activeIndex = 3;
          currentMessage = line;
          break;
        }
        if (line.includes("Downloading") || line.includes("system.img")) {
          activeIndex = 2;
          currentMessage = line;
          break;
        }
        if (line.includes("Preflight") || line.includes("package")) {
          activeIndex = 1;
          currentMessage = line;
          break;
        }
        if (
          line.includes("Requesting root access") ||
          line.includes("Elevated shell started")
        ) {
          activeIndex = 0;
          currentMessage = line;
          break;
        }
      }
    }
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
