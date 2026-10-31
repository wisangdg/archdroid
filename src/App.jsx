import React, { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { PackagePlus } from "lucide-react";

import Sidebar from "./components/Sidebar";
import Header from "./components/Header";
import MyAppsView from "./components/apps/MyAppsView";
import InstallToolsView from "./components/tools/InstallToolsView";
import KeymapView from "./components/keymap/KeymapView";
import SettingsView from "./components/settings/SettingsView";
import SetupWizard from "./components/setup/SetupWizard";
import TerminalModal from "./components/terminal/TerminalModal";
import ToastContainer from "./components/ui/ToastContainer";
import ConfirmModal from "./components/ui/ConfirmModal";
import { useToasts } from "./hooks/useToasts";
import { useStreamingTask } from "./hooks/useStreamingTask";

export default function App() {
  const [status, setStatus] = useState("offline");
  const [isInitialized, setIsInitialized] = useState(true);
  const [activeTab, setActiveTab] = useState("home");
  const [apps, setApps] = useState([]);
  const [gappsStatus, setGappsStatus] = useState("unknown");
  const [houdiniStatus, setHoudiniStatus] = useState("unknown");
  const [multitouchStatus, setMultitouchStatus] = useState("unknown");
  const [isOperating, setIsOperating] = useState(false);
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  const prevStatusRef = useRef("offline");
  const { toasts, addToast, removeToast } = useToasts();
  const {
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
  } = useStreamingTask({
    addToast,
    onTaskSuccess: () => {
      checkGappsStatus();
      checkHoudiniStatus();
      loadApps();
    },
  });

  // Confirm Modal State
  const [confirmModal, setConfirmModal] = useState({
    isOpen: false,
    title: "",
    description: "",
    confirmText: "Confirm",
    confirmVariant: "danger",
    iconType: "warning",
    onConfirm: () => {},
  });

  const statusCheckInFlight = useRef(false);

  // Config State
  const [multiWindow, setMultiWindow] = useState(false);
  const [invertColors, setInvertColors] = useState(false);
  const [displayRes, setDisplayRes] = useState({
    width: "",
    height: "",
    dpi: "",
  });

  // Ask the backend which distro packages the action would install. The modal
  // shows this list and the exact command; the same list is sent back as the
  // approval token, and the backend re-validates it before requesting root.
  const previewPackageInstall = async (action) => {
    try {
      return await invoke("preview_package_install", { action });
    } catch (err) {
      addToast({
        type: "error",
        title: "Package Preview Failed",
        message: err?.message || String(err),
      });
      return null;
    }
  };

  useEffect(() => {
    checkStatus();
    loadApps();
    checkGappsStatus();
    checkHoudiniStatus();
    loadConfigs();

    // Poll only the session state. GApps/Houdini probes go through adb and are
    // refreshed when the session comes online and after each task instead.
    // A hidden window skips the tick; checkStatus never overlaps itself.
    const interval = setInterval(() => {
      if (document.hidden) return;
      checkStatus();
    }, 5000);

    return () => {
      clearInterval(interval);
    };
  }, [addToast]);

  useEffect(() => {
    prevStatusRef.current = status;
  }, [status]);

  const checkGappsStatus = async () => {
    try {
      const res = await invoke("check_gapps_status");
      setGappsStatus(res.status);
    } catch (e) {
      console.error("Failed to check GApps status:", e);
      setGappsStatus("unknown");
    }
  };

  const checkHoudiniStatus = async () => {
    try {
      const res = await invoke("check_houdini_status");
      setHoudiniStatus(res.status);
    } catch (e) {
      console.error("Failed to check Houdini status:", e);
    }
  };

  const checkMultitouchStatus = async () => {
    try {
      const res = await invoke("check_multitouch_status");
      setMultitouchStatus(res.status);
    } catch (e) {
      console.error("Failed to check touch feature status:", e);
    }
  };

  const loadApps = useCallback(async () => {
    try {
      const waydroidApps = await invoke("get_waydroid_apps");
      setApps(waydroidApps || []);
    } catch (e) {
      console.error("Failed to load apps:", e);
    }
  }, []);

  const checkStatus = useCallback(async () => {
    if (statusCheckInFlight.current) return;
    statusCheckInFlight.current = true;
    try {
      const res = await invoke("waydroid_status");
      if (
        res.status === "success" &&
        res.data &&
        res.data.includes("RUNNING")
      ) {
        setStatus("online");
        setIsInitialized(true);
        if (prevStatusRef.current !== "online") {
          checkGappsStatus();
          checkHoudiniStatus();
          checkMultitouchStatus();
        }
      } else {
        setStatus("offline");
        if (res.code === "not_found" || res.code === "not_initialized") {
          setIsInitialized(false);
        } else {
          setIsInitialized(true);
        }
      }
    } catch (e) {
      console.error("Status check failed:", e);
      setStatus("offline");
    } finally {
      statusCheckInFlight.current = false;
    }
  }, []);

  const pollStatusUntil = useCallback(
    async (targetStatus, maxAttempts = 15, intervalMs = 400) => {
      for (let attempt = 0; attempt < maxAttempts; attempt++) {
        await new Promise((resolve) => setTimeout(resolve, intervalMs));
        const res = await invoke("waydroid_status");
        const isRunning =
          res.status === "success" && res.data && res.data.includes("RUNNING");
        const current = isRunning ? "online" : "offline";
        if (current === targetStatus) {
          await checkStatus();
          return true;
        }
      }
      await checkStatus();
      return false;
    },
    [checkStatus]
  );

  const handleStart = async () => {
    setIsOperating(true);
    addToast({
      type: "info",
      title: "Starting Engine",
      message: "Initializing Waydroid Android session...",
    });
    try {
      const res = await invoke("waydroid_start");
      if (res?.status === "error") {
        setIsOperating(false);
        addToast({
          type: "error",
          title: "Start Failed",
          message: res.message || "Failed to start Waydroid session.",
        });
        return;
      }
      const online = await pollStatusUntil("online", 15);
      setIsOperating(false);
      if (!online) {
        addToast({
          type: "warning",
          title: "Session Slow to Start",
          message:
            "Start command was sent, but the session is not yet reporting online.",
        });
      }
    } catch (e) {
      console.error("Failed to start Waydroid:", e);
      addToast({
        type: "error",
        title: "Start Failed",
        message: String(e),
      });
      setIsOperating(false);
    }
  };

  const handleStop = async () => {
    setIsOperating(true);
    addToast({
      type: "info",
      title: "Stopping Engine",
      message: "Halting Waydroid session...",
    });
    try {
      const res = await invoke("waydroid_stop");
      if (res?.status === "error") {
        setIsOperating(false);
        addToast({
          type: "error",
          title: "Stop Failed",
          message: res.message || "Failed to stop Waydroid session.",
        });
        return;
      }
      await pollStatusUntil("offline", 10);
      setIsOperating(false);
    } catch (e) {
      console.error("Failed to stop Waydroid:", e);
      addToast({
        type: "error",
        title: "Stop Failed",
        message: String(e),
      });
      setIsOperating(false);
    }
  };

  const handleRestart = async () => {
    setIsOperating(true);
    addToast({
      type: "info",
      title: "Restarting Engine",
      message: "Cycling Waydroid session...",
    });
    try {
      const stopRes = await invoke("waydroid_stop");
      if (stopRes?.status === "error") {
        setIsOperating(false);
        addToast({
          type: "error",
          title: "Restart Failed",
          message:
            stopRes.message ||
            "Failed to stop Waydroid session during restart.",
        });
        return;
      }
      await pollStatusUntil("offline", 8);

      const startRes = await invoke("waydroid_start");
      if (startRes?.status === "error") {
        setIsOperating(false);
        addToast({
          type: "error",
          title: "Restart Failed",
          message: startRes.message || "Failed to restart Waydroid session.",
        });
        return;
      }
      const online = await pollStatusUntil("online", 15);
      setIsOperating(false);
      if (online) {
        addToast({
          type: "success",
          title: "Engine Restarted",
          message: "Waydroid session is back online.",
        });
      } else {
        addToast({
          type: "warning",
          title: "Restart Pending",
          message:
            "Restart was triggered, but the session is not yet reporting online.",
        });
      }
    } catch (e) {
      console.error("Failed to restart Waydroid:", e);
      addToast({
        type: "error",
        title: "Restart Failed",
        message: String(e),
      });
      setIsOperating(false);
    }
  };

  const installApkFile = useCallback(
    async (filePath) => {
      if (!filePath) return;
      // The backend would reject this with `busy`, but dropping a file must not
      // even look like it was accepted while a wipe/installer owns the slot.
      if (operationInFlight.current) {
        addToast({
          type: "warning",
          title: "Operation In Progress",
          message:
            "Wait for the current operation to finish before installing an APK.",
        });
        return;
      }
      const fileName = filePath.split("/").pop() || "APK";
      try {
        addToast({
          type: "info",
          title: "Installing APK",
          message: `Transferring and installing ${fileName}...`,
        });
        const res = await invoke("waydroid_install_apk", { path: filePath });
        if (res.status === "success") {
          addToast({
            type: "success",
            title: "APK Installed",
            message: `${fileName} installed successfully!`,
          });
          loadApps();
          // Waydroid writes the new desktop entry asynchronously; reload once
          // more so the card appears without restarting the app.
          setTimeout(loadApps, 1000);
        } else {
          addToast({
            type: "error",
            title: "Install Failed",
            message: res.message || `Failed to install ${fileName}.`,
          });
        }
      } catch (e) {
        console.error("APK Install error:", e);
        addToast({
          type: "error",
          title: "Install Error",
          message: String(e),
        });
      }
    },
    [addToast, loadApps]
  );

  const handleInstallApk = async () => {
    try {
      const file = await open({
        multiple: false,
        filters: [
          {
            name: "Android Package",
            extensions: ["apk"],
          },
        ],
      });
      if (file) {
        await installApkFile(file);
      }
    } catch (e) {
      console.error("APK Install error:", e);
      addToast({
        type: "error",
        title: "File Picker Error",
        message: String(e),
      });
    }
  };

  const handleDroppedFiles = useCallback(
    async (paths) => {
      if (!paths || paths.length === 0) return;
      const apkFiles = paths.filter(
        (p) => typeof p === "string" && p.toLowerCase().endsWith(".apk")
      );
      if (apkFiles.length === 0) {
        addToast({
          type: "warning",
          title: "Unsupported File",
          message: "Only .apk packages can be installed via drag & drop.",
          duration: 4000,
        });
        return;
      }

      for (const apk of apkFiles) {
        await installApkFile(apk);
      }
    },
    [addToast, installApkFile]
  );

  // Tauri v2 Native Webview Drag-and-Drop Listener
  useEffect(() => {
    let unlisten;
    const setupDragDropListener = async () => {
      try {
        const webview = getCurrentWebview();
        unlisten = await webview.onDragDropEvent((event) => {
          if (event.payload.type === "enter" || event.payload.type === "over") {
            setIsDraggingOver(true);
          } else if (event.payload.type === "leave") {
            setIsDraggingOver(false);
          } else if (event.payload.type === "drop") {
            setIsDraggingOver(false);
            const droppedPaths = event.payload.paths || [];
            handleDroppedFiles(droppedPaths);
          }
        });
      } catch (err) {
        console.warn("Failed to attach Tauri drag-drop listener:", err);
      }
    };

    setupDragDropListener();

    return () => {
      if (typeof unlisten === "function") {
        unlisten();
      }
    };
  }, [handleDroppedFiles]);

  const handleLaunchApp = async (pkg) => {
    try {
      const res = await invoke("waydroid_launch_app", { package: pkg });
      if (res?.status === "success") {
        addToast({
          type: "info",
          title: "Launching",
          message: `Opening ${pkg}...`,
          duration: 2500,
        });
      } else {
        addToast({
          type: "error",
          title: "Launch Failed",
          message: res?.message || `Failed to launch ${pkg}.`,
        });
      }
    } catch (e) {
      console.error("Launch app error:", e);
      addToast({
        type: "error",
        title: "Launch Failed",
        message: String(e),
      });
    }
  };

  const handleCopyPackage = async (pkg) => {
    try {
      await navigator.clipboard.writeText(pkg);
      addToast({
        type: "success",
        title: "Package ID Copied",
        message: `Copied "${pkg}" to clipboard.`,
        duration: 2500,
      });
    } catch (err) {
      console.error("Copy failed:", err);
    }
  };

  const handleUninstallApp = (eOrPkg, pkgOrName, maybeName) => {
    // Support invocation from both AppCard (e, pkg, name) and ContextMenu (pkg, name)
    let pkg = "";
    let name = "";
    if (typeof eOrPkg === "object" && eOrPkg.stopPropagation) {
      eOrPkg.stopPropagation();
      pkg = pkgOrName;
      name = maybeName || pkgOrName;
    } else {
      pkg = eOrPkg;
      name = pkgOrName || eOrPkg;
    }

    setConfirmModal({
      isOpen: true,
      title: `Uninstall ${name}?`,
      description: `Are you sure you want to uninstall ${name} (${pkg})? All application data, cache, and save files will be permanently removed from Waydroid.`,
      confirmText: "Uninstall App",
      confirmVariant: "danger",
      iconType: "trash",
      onConfirm: async () => {
        setConfirmModal((prev) => ({ ...prev, isOpen: false }));
        try {
          addToast({
            type: "info",
            title: "Uninstalling",
            message: `Removing ${name}...`,
          });
          const res = await invoke("waydroid_uninstall_app", { package: pkg });
          if (res.status === "success") {
            addToast({
              type: "success",
              title: "App Removed",
              message: `${name} was uninstalled successfully.`,
            });
            await loadApps();
            // Waydroid removes the desktop entry asynchronously after the
            // package manager finishes, so refresh once more after a short delay.
            setTimeout(loadApps, 1000);
          } else {
            addToast({
              type: "error",
              title: "Uninstall Failed",
              message: res.message || "Failed to remove the application.",
            });
          }
        } catch (err) {
          console.error("Uninstall error:", err);
          addToast({
            type: "error",
            title: "Uninstall Error",
            message: String(err),
          });
        }
      },
    });
  };

  const executeInstallGApps = async (approvedPackages = []) => {
    try {
      await runStreamingTask("install_gapps", {
        taskType: "gapps",
        title: "Installing Google Play Services (GApps)",
        args: { approvedPackages },
      });
    } catch (e) {
      console.error("GApps install error:", e);
      addToast({
        type: "error",
        title: "GApps Error",
        message: String(e),
      });
    }
  };

  const handleInstallGApps = async () => {
    const preview = await previewPackageInstall("gapps");
    if (!preview) return;

    setConfirmModal({
      isOpen: true,
      title: "Install Google Play Services (Root Required)",
      description:
        "This installer modifies the Waydroid Android system image (/var/lib/waydroid/overlay/system) to inject Google Play Store and Services framework.\n\n" +
        "• Source: waydroid_script (pinned commit 48dbfaf) • MindTheGapps / OpenGApps\n" +
        "• Privilege: Requires Administrator (Root) access via Polkit or sudo\n" +
        "• Consequence: Adds Google proprietary services to Android rootfs. Waydroid must be restarted after completion.\n\n" +
        "Do you want to proceed with root installation?",
      confirmText: "Proceed & Authorize Root",
      confirmVariant: "primary",
      iconType: "shield",
      packagePreview: preview,
      onConfirm: async () => {
        setConfirmModal((prev) => ({ ...prev, isOpen: false }));
        await executeInstallGApps(preview.packages);
      },
    });
  };

  const executeInstallHoudini = async (approvedPackages = []) => {
    try {
      await runStreamingTask("install_houdini", {
        taskType: "houdini",
        title: "Installing ARM Translation (libhoudini)",
        args: { approvedPackages },
      });
    } catch (e) {
      console.error("Houdini install error:", e);
      addToast({
        type: "error",
        title: "ARM Translation Error",
        message: String(e),
      });
    }
  };

  const handleInstallHoudini = async () => {
    const preview = await previewPackageInstall("houdini");
    if (!preview) return;

    setConfirmModal({
      isOpen: true,
      title: "Install ARM Translation Layer (Root Required)",
      description:
        "This installer modifies the Waydroid Android system image (/var/lib/waydroid/overlay/system) and waydroid.cfg to inject Intel libhoudini ARM translation libraries.\n\n" +
        "• Source: waydroid_script (pinned commit 48dbfaf) • Intel proprietary prebuilts\n" +
        "• Privilege: Requires Administrator (Root) access via Polkit or sudo\n" +
        "• Consequence: Injects binary translation bridge (binfmt_misc) to execute ARM/ARM64 games on x86_64 CPUs.\n\n" +
        "Do you want to proceed with root installation?",
      confirmText: "Proceed & Authorize Root",
      confirmVariant: "primary",
      iconType: "shield",
      packagePreview: preview,
      onConfirm: async () => {
        setConfirmModal((prev) => ({ ...prev, isOpen: false }));
        await executeInstallHoudini(preview.packages);
      },
    });
  };

  const executeInstallMultitouch = async () => {
    try {
      const result = await runStreamingTask("install_multitouch", {
        taskType: "multitouch",
        title: "Declaring Touch Features for Google Play",
      });
      if (result?.status === "success") {
        addToast({
          type: "info",
          title: "Restart Required",
          message:
            "Android reads declared features at boot. Restart Waydroid, then check Google Play again.",
        });
      }
    } catch (e) {
      console.error("Touch feature install error:", e);
      addToast({
        type: "error",
        title: "Touch Feature Error",
        message: String(e),
      });
    }
  };

  const handleInstallMultitouch = () => {
    setConfirmModal({
      isOpen: true,
      title: "Declare Touch Features (Root Required)",
      description:
        "This operation writes Android touchscreen hardware capability declarations to the Waydroid vendor overlay (/var/lib/waydroid/overlay).\n\n" +
        "• Source: ArchDroid internal hardware profile\n" +
        "• Privilege: Requires Administrator (Root) access via Polkit or sudo\n" +
        "• Consequence: Declares multitouch hardware features so Google Play Store permits downloading touch-dependent games. Requires restarting Waydroid.\n\n" +
        "Do you want to proceed with root installation?",
      confirmText: "Proceed & Authorize Root",
      confirmVariant: "primary",
      iconType: "shield",
      onConfirm: async () => {
        setConfirmModal((prev) => ({ ...prev, isOpen: false }));
        await executeInstallMultitouch();
      },
    });
  };

  const handleViewLogs = async () => {
    try {
      await runStreamingTask("waydroid_log", {
        taskType: "logs",
        title: "Waydroid Diagnostic Logs",
      });
    } catch (e) {
      console.error("View logs error:", e);
      addToast({
        type: "error",
        title: "Log Read Error",
        message: String(e),
      });
    }
  };

  const handleOpenSharedFolder = async () => {
    try {
      const res = await invoke("open_shared_folder");
      if (res?.status === "success") {
        addToast({
          type: "info",
          title: "Shared Folder Opened",
          message: "Opened Android shared media storage in file manager.",
          duration: 3000,
        });
      } else {
        addToast({
          type: "error",
          title: "Failed to Open Folder",
          message: res?.message || "Could not open shared media folder.",
        });
      }
    } catch (e) {
      console.error("Open shared folder error:", e);
      addToast({
        type: "error",
        title: "Failed to Open Folder",
        message: String(e),
      });
    }
  };

  const loadConfigs = async () => {
    try {
      const mw = await invoke("waydroid_prop_get", {
        key: "persist.waydroid.multi_windows",
      });
      setMultiWindow(mw.status === "success" && mw.data === "true");

      const inv = await invoke("waydroid_prop_get", {
        key: "persist.waydroid.invert_colors",
      });
      setInvertColors(inv.status === "success" && inv.data === "true");

      const widthRes = await invoke("waydroid_prop_get", {
        key: "persist.waydroid.width",
      });
      const heightRes = await invoke("waydroid_prop_get", {
        key: "persist.waydroid.height",
      });
      const dpiRes = await invoke("waydroid_prop_get", {
        key: "ro.sf.lcd_density",
      });

      setDisplayRes({
        width: widthRes.status === "success" ? widthRes.data || "" : "",
        height: heightRes.status === "success" ? heightRes.data || "" : "",
        dpi: dpiRes.status === "success" ? dpiRes.data || "" : "",
      });
    } catch (e) {
      console.error("Failed to load configs:", e);
    }
  };

  const handleApplyResolution = async ({ width, height, dpi }) => {
    try {
      if (width) {
        await invoke("waydroid_prop_set", {
          key: "persist.waydroid.width",
          value: width,
        });
      }
      if (height) {
        await invoke("waydroid_prop_set", {
          key: "persist.waydroid.height",
          value: height,
        });
      }
      if (dpi) {
        await invoke("waydroid_prop_set", {
          key: "ro.sf.lcd_density",
          value: dpi,
        });
      }
      await loadConfigs();
      addToast({
        type: "success",
        title: "Display Profile Updated",
        message: `Applied ${width} × ${height} (${dpi} DPI). Restart Waydroid session to apply changes.`,
        duration: 5000,
      });
    } catch (err) {
      console.error("Failed to apply resolution:", err);
      addToast({
        type: "error",
        title: "Display Config Failed",
        message: String(err),
      });
    }
  };

  const toggleConfig = async (key, currentValue, setter, label) => {
    const newValue = currentValue ? "false" : "true";
    try {
      const res = await invoke("waydroid_prop_set", { key, value: newValue });
      if (res?.status === "success") {
        setter(!currentValue);
        addToast({
          type: "info",
          title: "Config Updated",
          message: `${label || key} set to ${!currentValue ? "ON" : "OFF"}.`,
          duration: 2500,
        });
      } else {
        addToast({
          type: "error",
          title: "Config Toggle Failed",
          message: res?.message || `Failed to update ${label || key}.`,
        });
      }
    } catch (e) {
      console.error(`Failed to toggle config ${key}:`, e);
      addToast({
        type: "error",
        title: "Config Toggle Failed",
        message: String(e),
      });
    }
  };

  const handlePropSet = async (e) => {
    e.preventDefault();
    const key = e.target.elements.propKey.value;
    const val = e.target.elements.propVal.value;
    try {
      const res = await invoke("waydroid_prop_set", { key: key, value: val });
      if (res.status === "success") {
        addToast({
          type: "success",
          title: "Property Applied",
          message: `Set ${key} = ${val}`,
        });
        e.target.reset();
        await loadConfigs();
      } else {
        addToast({
          type: "error",
          title: "Property Error",
          message: res.message || "Failed to set property.",
        });
      }
    } catch (err) {
      console.error("Prop set error:", err);
      addToast({
        type: "error",
        title: "Prop Set Error",
        message: String(err),
      });
    }
  };

  const handleInstallPhantom = async () => {
    try {
      await runStreamingTask("install_phantom", {
        taskType: "phantom",
        title: "Installing / Updating Phantom Keymapper",
      });
    } catch (e) {
      console.error("Install Phantom error:", e);
      addToast({
        type: "error",
        title: "Phantom Error",
        message: String(e),
      });
    }
  };

  const handleRunPhantom = async () => {
    try {
      const res = await invoke("run_phantom");
      if (res.status === "success") {
        addToast({
          type: "info",
          title: "Keymapper Launched",
          message: res.message || "Phantom Keymapper started.",
        });
      } else {
        addToast({
          type: "error",
          title: "Keymapper Error",
          message: res.message || "Could not launch the Phantom keymapper.",
        });
      }
    } catch (e) {
      console.error("Run Phantom error:", e);
      addToast({
        type: "error",
        title: "Keymapper Error",
        message: String(e),
      });
    }
  };

  const executeInitWaydroid = async (approvedPackages = []) => {
    try {
      await runStreamingTask("waydroid_init", {
        taskType: "init",
        title: "Initializing Waydroid System Image",
        args: { approvedPackages },
      });
    } catch (e) {
      console.error("Init Waydroid error:", e);
      addToast({
        type: "error",
        title: "Init Error",
        message: String(e),
      });
    }
  };

  const handleInitWaydroid = async () => {
    const preview = await previewPackageInstall("init");
    if (!preview) return;

    setConfirmModal({
      isOpen: true,
      title: "Initialize Waydroid Container (Root Required)",
      description:
        "Initializing Waydroid will download the official LineageOS container images (~1.2 GB) and configure the LXC Android container under /var/lib/waydroid.\n\n" +
        "• Source: Official Waydroid / LineageOS system images\n" +
        "• Privilege: Requires Administrator (Root) access via Polkit or sudo\n" +
        "• Consequence: Sets up root container files and system partitions on disk.\n\n" +
        "Do you want to proceed?",
      confirmText: "Initialize & Authorize Root",
      confirmVariant: "primary",
      iconType: "shield",
      packagePreview: preview,
      onConfirm: async () => {
        setConfirmModal((prev) => ({ ...prev, isOpen: false }));
        await executeInitWaydroid(preview.packages);
      },
    });
  };

  const handleWipeData = () => {
    setConfirmModal({
      isOpen: true,
      title: "Factory Reset Waydroid?",
      description:
        "WARNING: This will completely destroy all Waydroid container data, installed games, GApps modules, and configurations. Back up anything you need first (use Settings → Android Shared Storage to copy files out). You will need to re-initialize the emulator from scratch. This cannot be undone!",
      confirmText: "Wipe Everything",
      confirmVariant: "danger",
      iconType: "danger",
      onConfirm: async () => {
        setConfirmModal((prev) => ({ ...prev, isOpen: false }));
        try {
          await runStreamingTask("waydroid_wipe", {
            taskType: "wipe",
            title: "Factory Reset in Progress...",
          });
        } catch (e) {
          console.error("Wipe data error:", e);
          addToast({
            type: "error",
            title: "Wipe Failed",
            message: String(e),
          });
        }
      },
    });
  };

  const handleRetryTask = () => {
    // Retrying must go back through the confirmation flow. The package plan or
    // the root scope may have changed since the first attempt, so a retry must
    // never call an installer command directly.
    if (
      ["gapps", "houdini", "multitouch", "init", "wipe"].includes(
        terminalTaskType
      )
    ) {
      setTerminalOpen(false);
    }

    switch (terminalTaskType) {
      case "gapps":
        handleInstallGApps();
        break;
      case "houdini":
        handleInstallHoudini();
        break;
      case "multitouch":
        handleInstallMultitouch();
        break;
      case "init":
        handleInitWaydroid();
        break;
      case "phantom":
        handleInstallPhantom();
        break;
      case "logs":
        handleViewLogs();
        break;
      case "wipe":
        handleWipeData();
        break;
      default:
        break;
    }
  };

  const handleDragOver = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (!isDraggingOver) setIsDraggingOver(true);
  };

  const handleDragLeave = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.currentTarget.contains(e.relatedTarget)) return;
    setIsDraggingOver(false);
  };

  const handleDrop = (e) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDraggingOver(false);
  };

  return (
    <div
      className="app-container"
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
    >
      {/* Sidebar */}
      <Sidebar
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        appCount={apps.length}
        status={status}
        onOpenSharedFolder={handleOpenSharedFolder}
      />

      {/* Main Content Area */}
      <main className="main-content">
        <Header
          activeTab={activeTab}
          status={status}
          isInitialized={isInitialized}
          appCount={apps.length}
          onStart={handleStart}
          onStop={handleStop}
          onRestart={handleRestart}
          isOperating={isOperating}
          busy={installing}
        />

        <div className="content-scrollable">
          {!isInitialized ? (
            <SetupWizard
              onInitWaydroid={handleInitWaydroid}
              installing={installing}
            />
          ) : (
            <>
              {activeTab === "home" && (
                <MyAppsView
                  apps={apps}
                  onLaunchApp={handleLaunchApp}
                  onInstallApk={handleInstallApk}
                  onUninstallApp={handleUninstallApp}
                  onCopyPackage={handleCopyPackage}
                  busy={installing}
                />
              )}

              {activeTab === "keymap" && (
                <KeymapView
                  onRunPhantom={handleRunPhantom}
                  onInstallPhantom={handleInstallPhantom}
                  installing={installing}
                />
              )}

              {activeTab === "install" && (
                <InstallToolsView
                  gappsStatus={gappsStatus}
                  houdiniStatus={houdiniStatus}
                  multitouchStatus={multitouchStatus}
                  installing={installing}
                  onInstallGApps={handleInstallGApps}
                  onInstallHoudini={handleInstallHoudini}
                  onInstallMultitouch={handleInstallMultitouch}
                />
              )}

              {activeTab === "settings" && (
                <SettingsView
                  multiWindow={multiWindow}
                  invertColors={invertColors}
                  onToggleMultiWindow={() =>
                    toggleConfig(
                      "persist.waydroid.multi_windows",
                      multiWindow,
                      setMultiWindow,
                      "Multi-Window Mode"
                    )
                  }
                  onToggleInvertColors={() =>
                    toggleConfig(
                      "persist.waydroid.invert_colors",
                      invertColors,
                      setInvertColors,
                      "Invert Colors"
                    )
                  }
                  onPropSet={handlePropSet}
                  onOpenSharedFolder={handleOpenSharedFolder}
                  onViewLogs={handleViewLogs}
                  onWipeData={handleWipeData}
                  currentWidth={displayRes.width}
                  currentHeight={displayRes.height}
                  currentDpi={displayRes.dpi}
                  onApplyResolution={handleApplyResolution}
                  engineRunning={status === "online"}
                  onRestartEngine={handleRestart}
                  busy={installing}
                />
              )}
            </>
          )}
        </div>
      </main>

      {/* Process Output Modal */}
      <TerminalModal
        isOpen={terminalOpen}
        title={terminalTitle}
        taskType={terminalTaskType}
        logs={terminalLogs}
        taskResult={terminalTaskResult}
        installing={installing}
        terminalRef={terminalRef}
        onClose={() => {
          setTerminalOpen(false);
          setTerminalTaskResult(null);
        }}
        onRetry={handleRetryTask}
        onCancel={handleCancelTask}
      />

      {/* Confirmation Modal */}
      <ConfirmModal
        isOpen={confirmModal.isOpen}
        title={confirmModal.title}
        description={confirmModal.description}
        confirmText={confirmModal.confirmText}
        confirmVariant={confirmModal.confirmVariant}
        iconType={confirmModal.iconType}
        packagePreview={confirmModal.packagePreview}
        onConfirm={confirmModal.onConfirm}
        onCancel={() => setConfirmModal((prev) => ({ ...prev, isOpen: false }))}
      />

      {/* Drag & Drop Visual Overlay */}
      {isDraggingOver && (
        <div className="dropzone-overlay">
          <div className="dropzone-card">
            <div className="dropzone-icon-orb">
              <PackagePlus size={44} className="dropzone-icon" />
            </div>
            <h3 className="dropzone-title">Drop APK to Install</h3>
            <p className="dropzone-desc">
              Release your <code>.apk</code> file anywhere to automatically
              install it into Waydroid.
            </p>
            <div className="dropzone-pills">
              <span className="dropzone-pill">Instant Android Sideload</span>
              <span className="dropzone-pill">Auto-verification</span>
            </div>
          </div>
        </div>
      )}

      {/* In-App Toast System */}
      <ToastContainer toasts={toasts} onDismiss={removeToast} />
    </div>
  );
}
