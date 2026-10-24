import React, { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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

export default function App() {
  const [status, setStatus] = useState("offline");
  const [isInitialized, setIsInitialized] = useState(true);
  const [activeTab, setActiveTab] = useState("home");
  const [apps, setApps] = useState([]);
  const [gappsStatus, setGappsStatus] = useState("unknown");
  const [houdiniStatus, setHoudiniStatus] = useState("unknown");
  const [multitouchStatus, setMultitouchStatus] = useState("unknown");
  const [installing, setInstalling] = useState(false);
  const [isOperating, setIsOperating] = useState(false);
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  const prevStatusRef = useRef("offline");

  // In-App Toast State
  const [toasts, setToasts] = useState([]);

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

  // Terminal Modal State
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [terminalLogs, setTerminalLogs] = useState([]);
  const [terminalTitle, setTerminalTitle] = useState("");
  const [terminalTaskType, setTerminalTaskType] = useState("logs");
  const terminalRef = useRef(null);

  // Config State
  const [multiWindow, setMultiWindow] = useState(false);
  const [invertColors, setInvertColors] = useState(false);
  const [displayRes, setDisplayRes] = useState({ width: "", height: "", dpi: "" });

  const addToast = useCallback(({ type = "info", title, message, duration = 4000 }) => {
    const id = Date.now().toString() + Math.random().toString(36).substring(2, 6);
    setToasts((prev) => [...prev, { id, type, title, message }]);

    if (duration > 0) {
      setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
      }, duration);
    }
  }, []);

  const removeToast = useCallback((id) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  useEffect(() => {
    checkStatus();
    loadApps();
    checkGappsStatus();
    checkHoudiniStatus();
    loadConfigs();

    const interval = setInterval(() => {
      checkStatus();
      checkGappsStatus();
      checkHoudiniStatus();
    }, 5000);

    const unlistenOutput = listen("cmd-output", (event) => {
      setTerminalLogs((prev) => [...prev, event.payload]);
      if (terminalRef.current) {
        terminalRef.current.scrollTop = terminalRef.current.scrollHeight;
      }
    });

    const unlistenFinished = listen("cmd-finished", (event) => {
      const success = event.payload;
      setTerminalLogs((prev) => [
        ...prev,
        success
          ? "\n[PROCESS COMPLETED SUCCESSFULLY]"
          : "\n[PROCESS FINISHED WITH ERRORS]",
      ]);
      setInstalling(false);
      if (success) {
        addToast({
          type: "success",
          title: "Process Finished",
          message: "Operation completed successfully.",
        });
        checkGappsStatus();
        checkHoudiniStatus();
        loadApps();
      } else {
        addToast({
          type: "error",
          title: "Process Failed",
          message: "Check the terminal output for details.",
        });
      }
    });

    return () => {
      clearInterval(interval);
      unlistenOutput.then((f) => f());
      unlistenFinished.then((f) => f());
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

  const loadApps = async () => {
    try {
      const waydroidApps = await invoke("get_waydroid_apps");
      setApps(waydroidApps || []);
    } catch (e) {
      console.error("Failed to load apps:", e);
    }
  };

  const checkStatus = async () => {
    try {
      const res = await invoke("waydroid_status");
      if (res.status === "success" && res.data && res.data.includes("RUNNING")) {
        setStatus("online");
        setIsInitialized(true);
        if (prevStatusRef.current !== "online") {
          checkGappsStatus();
          checkHoudiniStatus();
          checkMultitouchStatus();
        }
      } else {
        setStatus("offline");
        if (
          res.message &&
          (res.message.includes("Cannot locate waydroid config") ||
            res.message.includes("command not found") ||
            res.message.includes("No such file"))
        ) {
          setIsInitialized(false);
        } else {
          setIsInitialized(true);
        }
      }
    } catch (e) {
      console.error("Status check failed:", e);
      setStatus("offline");
    }
  };

  const handleStart = async () => {
    setIsOperating(true);
    addToast({
      type: "info",
      title: "Starting Engine",
      message: "Initializing Waydroid Android session...",
    });
    try {
      await invoke("waydroid_start");
      setTimeout(() => {
        checkStatus();
        setIsOperating(false);
      }, 2000);
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
      await invoke("waydroid_stop");
      setTimeout(() => {
        checkStatus();
        setIsOperating(false);
      }, 2000);
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
      await invoke("waydroid_stop");
      setTimeout(async () => {
        await invoke("waydroid_start");
        setTimeout(() => {
          checkStatus();
          setIsOperating(false);
          addToast({
            type: "success",
            title: "Engine Restarted",
            message: "Waydroid session is back online.",
          });
        }, 2000);
      }, 1500);
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
      await invoke("waydroid_launch_app", { package: pkg });
      addToast({
        type: "info",
        title: "Launching",
        message: `Opening ${pkg}...`,
        duration: 2500,
      });
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

  const handleInstallGApps = async () => {
    try {
      setInstalling(true);
      setTerminalTaskType("gapps");
      setTerminalTitle("Installing Google Play Services (GApps)");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("install_gapps");
    } catch (e) {
      console.error("GApps install error:", e);
      addToast({
        type: "error",
        title: "GApps Error",
        message: String(e),
      });
      setInstalling(false);
    }
  };

  const handleInstallHoudini = async () => {
    try {
      setInstalling(true);
      setTerminalTaskType("houdini");
      setTerminalTitle("Installing ARM Translation (libhoudini)");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("install_houdini");
    } catch (e) {
      console.error("Houdini install error:", e);
      addToast({
        type: "error",
        title: "ARM Translation Error",
        message: String(e),
      });
      setInstalling(false);
    }
  };

  const handleInstallMultitouch = async () => {
    try {
      setInstalling(true);
      setTerminalTaskType("multitouch");
      setTerminalTitle("Declaring Touch Features for Google Play");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("install_multitouch");
      addToast({
        type: "info",
        title: "Restart Required",
        message:
          "Android reads declared features at boot. Restart Waydroid, then check Google Play again.",
      });
    } catch (e) {
      console.error("Touch feature install error:", e);
      addToast({
        type: "error",
        title: "Touch Feature Error",
        message: String(e),
      });
      setInstalling(false);
    }
  };

  const handleViewLogs = async () => {
    try {
      setInstalling(true);
      setTerminalTaskType("logs");
      setTerminalTitle("Waydroid Diagnostic Logs");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("waydroid_log");
    } catch (e) {
      console.error("View logs error:", e);
      addToast({
        type: "error",
        title: "Log Read Error",
        message: String(e),
      });
      setInstalling(false);
    }
  };

  const handleOpenSharedFolder = async () => {
    try {
      await invoke("open_shared_folder");
      addToast({
        type: "info",
        title: "Shared Folder Opened",
        message: "Opened Android shared media storage in file manager.",
        duration: 3000,
      });
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
        await invoke("waydroid_prop_set", { key: "persist.waydroid.width", value: width });
      }
      if (height) {
        await invoke("waydroid_prop_set", { key: "persist.waydroid.height", value: height });
      }
      if (dpi) {
        await invoke("waydroid_prop_set", { key: "ro.sf.lcd_density", value: dpi });
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

  const handleCancelTask = async () => {
    try {
      addToast({
        type: "warning",
        title: "Cancelling Operation",
        message: "Stopping current background operation...",
      });
      await invoke("waydroid_cancel");
    } catch (err) {
      console.error("Cancel task failed:", err);
      addToast({
        type: "error",
        title: "Cancellation Error",
        message: String(err),
      });
    }
  };

  const toggleConfig = async (key, currentValue, setter, label) => {
    const newValue = currentValue ? "false" : "true";
    try {
      await invoke("waydroid_prop_set", { key, value: newValue });
      setter(!currentValue);
      addToast({
        type: "info",
        title: "Config Updated",
        message: `${label || key} set to ${!currentValue ? "ON" : "OFF"}.`,
        duration: 2500,
      });
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
      setInstalling(true);
      setTerminalTaskType("phantom");
      setTerminalTitle("Installing / Updating Phantom Keymapper");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("install_phantom");
    } catch (e) {
      console.error("Install Phantom error:", e);
      addToast({
        type: "error",
        title: "Phantom Error",
        message: String(e),
      });
      setInstalling(false);
    }
  };

  const handleRunPhantom = async () => {
    try {
      await invoke("run_phantom");
      addToast({
        type: "info",
        title: "Keymapper Launched",
        message: "Phantom keymapper started in terminal window.",
      });
    } catch (e) {
      console.error("Run Phantom error:", e);
      addToast({
        type: "error",
        title: "Keymapper Error",
        message: String(e),
      });
    }
  };

  const handleInitWaydroid = async () => {
    try {
      setInstalling(true);
      setTerminalTaskType("init");
      setTerminalTitle("Initializing Waydroid System Image");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("waydroid_init");
    } catch (e) {
      console.error("Init Waydroid error:", e);
      addToast({
        type: "error",
        title: "Init Error",
        message: String(e),
      });
      setInstalling(false);
    }
  };

  const handleWipeData = () => {
    setConfirmModal({
      isOpen: true,
      title: "Factory Reset Waydroid?",
      description:
        "WARNING: This will completely destroy all Waydroid container data, installed games, GApps modules, and configurations. You will need to re-initialize the emulator from scratch. This cannot be undone!",
      confirmText: "Wipe Everything",
      confirmVariant: "danger",
      iconType: "danger",
      onConfirm: async () => {
        setConfirmModal((prev) => ({ ...prev, isOpen: false }));
        try {
          setInstalling(true);
          setTerminalTaskType("wipe");
          setTerminalTitle("Factory Reset in Progress...");
          setTerminalLogs([]);
          setTerminalOpen(true);
          await invoke("waydroid_wipe");
        } catch (e) {
          console.error("Wipe data error:", e);
          addToast({
            type: "error",
            title: "Wipe Failed",
            message: String(e),
          });
          setInstalling(false);
        }
      },
    });
  };

  const handleRetryTask = () => {
    switch (terminalTaskType) {
      case "gapps":
        handleInstallGApps();
        break;
      case "houdini":
        handleInstallHoudini();
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
        installing={installing}
        terminalRef={terminalRef}
        onClose={() => setTerminalOpen(false)}
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
              Release your <code>.apk</code> file anywhere to automatically install it into Waydroid.
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
