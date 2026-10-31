export const TASK_STEP_CONFIGS = {
  gapps: [
    { id: "elevate", label: "Root Access", description: "Polkit elevation" },
    { id: "deps", label: "Dependencies", description: "squashfs & lzip" },
    { id: "repo", label: "Download Script", description: "waydroid_script" },
    { id: "env", label: "Python Venv", description: "Virtual environment" },
    { id: "inject", label: "Injecting GApps", description: "Writing to rootfs" },
    { id: "done", label: "Ready", description: "Installation complete" },
  ],
  houdini: [
    { id: "elevate", label: "Root Access", description: "Polkit elevation" },
    { id: "deps", label: "Dependencies", description: "squashfs & lzip" },
    { id: "repo", label: "Download Script", description: "waydroid_script" },
    { id: "env", label: "Python Venv", description: "Virtual environment" },
    { id: "inject", label: "Installing Houdini", description: "ARM translation bridge" },
    { id: "done", label: "Ready", description: "ARM support active" },
  ],
  init: [
    { id: "elevate", label: "Root Access", description: "Polkit elevation" },
    { id: "pkg", label: "Packages", description: "System repositories" },
    { id: "download", label: "Download Image", description: "LineageOS rootfs (~1GB)" },
    { id: "init", label: "Initialize", description: "Setting up container" },
    { id: "done", label: "Ready", description: "Ready to launch" },
  ],
  phantom: [
    { id: "elevate", label: "Root Access", description: "Polkit elevation" },
    { id: "repo", label: "Clone / Pull", description: "GitHub repository" },
    { id: "build", label: "Install Script", description: "Compiling keymapper" },
    { id: "done", label: "Ready", description: "Phantom ready" },
  ],
  wipe: [
    { id: "elevate", label: "Root Access", description: "Polkit elevation" },
    { id: "stop", label: "Stop Session", description: "Halting container" },
    { id: "wipe", label: "Wiping Storage", description: "Cleaning /var/lib/waydroid" },
    { id: "done", label: "Wipe Complete", description: "Clean state restored" },
  ],
};

export function detectStepProgress(taskType, logs = []) {
  const steps = TASK_STEP_CONFIGS[taskType] || [];
  if (steps.length === 0) {
    return {
      steps: [],
      activeStepIndex: 0,
      isFailed: false,
      isCompleted: false,
      currentMessage: logs[logs.length - 1] || "",
      errorMessage: null,
    };
  }

  const logText = logs.join("\n");
  const isCancelled =
    logText.includes("[CANCELLED]") ||
    logText.includes("was cancelled") ||
    logText.includes("Operation cancelled");

  const isFailed =
    !isCancelled &&
    (logText.includes("[ERROR]") ||
      logText.includes("failed!") ||
      logText.includes("Failed to install") ||
      logText.includes("[PROCESS FINISHED WITH ERRORS]"));

  const isCompleted =
    !isCancelled &&
    (logText.includes("[SUCCESS]") ||
      logText.includes("[PROCESS COMPLETED SUCCESSFULLY]") ||
      logText.includes("Wipe complete!"));

  let activeIndex = 0;
  let currentMessage = "Initiating task...";
  let errorMessage = null;

  if (isCancelled) {
    currentMessage = "Operation was cancelled by user.";
    errorMessage = "Operation was cancelled by user.";
  } else if (isFailed) {
    const errorLine = logs.find(
      (l) => l.includes("[ERROR]") || l.includes("failed") || l.includes("Failed")
    );
    errorMessage = errorLine || "The process terminated with errors.";
  }

  // Check from latest step backwards
  if (isCompleted) {
    activeIndex = steps.length - 1;
    currentMessage = "Task finished successfully!";
  } else {
    for (let i = logs.length - 1; i >= 0; i--) {
      const line = logs[i];
      if (!line) continue;

      if (
        line.includes("Injecting GApps") ||
        line.includes("Installing ARM Translation") ||
        line.includes("Initializing Waydroid") ||
        line.includes("Running install script") ||
        line.includes("Wiping Waydroid data")
      ) {
        activeIndex = Math.min(steps.length - 2, 4);
        currentMessage = line;
        break;
      }

      if (
        line.includes("Installing Python requirements") ||
        line.includes("Setting up Python") ||
        line.includes("pip install")
      ) {
        activeIndex = 3;
        currentMessage = line;
        break;
      }

      if (
        line.includes("Downloading Waydroid Script") ||
        line.includes("Cloning waydroid_script") ||
        line.includes("Cloning Phantom") ||
        line.includes("Updating Phantom")
      ) {
        activeIndex = 2;
        currentMessage = line;
        break;
      }

      if (
        line.includes("Installing required dependencies") ||
        line.includes("Installing required extraction tools") ||
        line.includes("pacman -S") ||
        line.includes("apt-get install") ||
        line.includes("dnf install") ||
        line.includes("zypper") ||
        line.includes("Stopping Waydroid")
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
