import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import Sidebar from "./components/Sidebar";
import Header from "./components/Header";
import MyAppsView from "./components/apps/MyAppsView";
import InstallToolsView from "./components/tools/InstallToolsView";
import KeymapView from "./components/keymap/KeymapView";
import SettingsView from "./components/settings/SettingsView";
import SetupWizard from "./components/setup/SetupWizard";
import TerminalModal from "./components/terminal/TerminalModal";

export default function App() {
  const [status, setStatus] = useState("offline");
  const [isInitialized, setIsInitialized] = useState(true);
  const [activeTab, setActiveTab] = useState("home");
  const [apps, setApps] = useState([]);
  const [gappsStatus, setGappsStatus] = useState("unknown");
  const [houdiniStatus, setHoudiniStatus] = useState("unknown");
  const [installing, setInstalling] = useState(false);
  const [isOperating, setIsOperating] = useState(false);
  const prevStatusRef = useRef("offline");

  // Terminal Modal State
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [terminalLogs, setTerminalLogs] = useState([]);
  const [terminalTitle, setTerminalTitle] = useState("");
  const terminalRef = useRef(null);

  // Config State
  const [multiWindow, setMultiWindow] = useState(false);
  const [invertColors, setInvertColors] = useState(false);

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
    });

    return () => {
      clearInterval(interval);
      unlistenOutput.then((f) => f());
      unlistenFinished.then((f) => f());
    };
  }, []);

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
    try {
      await invoke("waydroid_start");
      setTimeout(() => {
        checkStatus();
        setIsOperating(false);
      }, 2000);
    } catch (e) {
      console.error("Failed to start Waydroid:", e);
      setIsOperating(false);
    }
  };

  const handleStop = async () => {
    setIsOperating(true);
    try {
      await invoke("waydroid_stop");
      setTimeout(() => {
        checkStatus();
        setIsOperating(false);
      }, 2000);
    } catch (e) {
      console.error("Failed to stop Waydroid:", e);
      setIsOperating(false);
    }
  };

  const handleRestart = async () => {
    setIsOperating(true);
    try {
      await invoke("waydroid_stop");
      setTimeout(async () => {
        await invoke("waydroid_start");
        setTimeout(() => {
          checkStatus();
          setIsOperating(false);
        }, 2000);
      }, 1500);
    } catch (e) {
      console.error("Failed to restart Waydroid:", e);
      setIsOperating(false);
    }
  };

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
        alert("Installing APK, this may take a moment...");
        const res = await invoke("waydroid_install_apk", { path: file });
        if (res.status === "success") {
          alert("APK Installed successfully!");
          loadApps();
        } else {
          alert("Failed to install APK: " + (res.message || "Unknown error"));
        }
      }
    } catch (e) {
      console.error("APK Install error:", e);
    }
  };

  const handleLaunchApp = async (pkg) => {
    try {
      await invoke("waydroid_launch_app", { package: pkg });
    } catch (e) {
      console.error("Launch app error:", e);
    }
  };

  const handleUninstallApp = async (e, pkg, name) => {
    e.stopPropagation();
    if (window.confirm(`Are you sure you want to uninstall ${name || pkg}?`)) {
      try {
        await invoke("waydroid_uninstall_app", { package: pkg });
        await loadApps();
      } catch (err) {
        console.error("Uninstall error:", err);
      }
    }
  };

  const handleInstallGApps = async () => {
    try {
      setInstalling(true);
      setTerminalTitle("Installing Google Play Services (GApps)");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("install_gapps");
    } catch (e) {
      console.error("GApps install error:", e);
      setInstalling(false);
    }
  };

  const handleInstallHoudini = async () => {
    try {
      setInstalling(true);
      setTerminalTitle("Installing ARM Translation (libhoudini)");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("install_houdini");
    } catch (e) {
      console.error("Houdini install error:", e);
      setInstalling(false);
    }
  };

  const handleViewLogs = async () => {
    try {
      setInstalling(true);
      setTerminalTitle("Waydroid Diagnostic Logs");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("waydroid_log");
    } catch (e) {
      console.error("View logs error:", e);
      setInstalling(false);
    }
  };

  const handleOpenSharedFolder = async () => {
    try {
      await invoke("open_shared_folder");
    } catch (e) {
      console.error("Open shared folder error:", e);
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
    } catch (e) {
      console.error("Failed to load configs:", e);
    }
  };

  const toggleConfig = async (key, currentValue, setter) => {
    const newValue = currentValue ? "false" : "true";
    try {
      await invoke("waydroid_prop_set", { key, value: newValue });
      setter(!currentValue);
    } catch (e) {
      console.error(`Failed to toggle config ${key}:`, e);
    }
  };

  const handlePropSet = async (e) => {
    e.preventDefault();
    const key = e.target.elements.propKey.value;
    const val = e.target.elements.propVal.value;
    try {
      await invoke("waydroid_prop_set", { key, value: val });
      e.target.reset();
      await loadConfigs();
    } catch (err) {
      console.error("Prop set error:", err);
    }
  };

  const handleInstallPhantom = async () => {
    try {
      setInstalling(true);
      setTerminalTitle("Installing / Updating Phantom Keymapper");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("install_phantom");
    } catch (e) {
      console.error("Install Phantom error:", e);
      setInstalling(false);
    }
  };

  const handleRunPhantom = async () => {
    try {
      await invoke("run_phantom");
    } catch (e) {
      console.error("Run Phantom error:", e);
    }
  };

  const handleInitWaydroid = async () => {
    try {
      setInstalling(true);
      setTerminalTitle("Initializing Waydroid System Image");
      setTerminalLogs([]);
      setTerminalOpen(true);
      await invoke("waydroid_init");
    } catch (e) {
      console.error("Init Waydroid error:", e);
      setInstalling(false);
    }
  };

  const handleWipeData = async () => {
    if (
      window.confirm(
        "Are you sure you want to completely wipe ALL Waydroid data, apps, and GApps? This action cannot be undone and will require a full re-initialization!"
      )
    ) {
      try {
        setInstalling(true);
        setTerminalTitle("Factory Reset in Progress...");
        setTerminalLogs([]);
        setTerminalOpen(true);
        await invoke("waydroid_wipe");
      } catch (e) {
        console.error("Wipe data error:", e);
        setInstalling(false);
      }
    }
  };

  return (
    <div className="app-container">
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
                  installing={installing}
                  onInstallGApps={handleInstallGApps}
                  onInstallHoudini={handleInstallHoudini}
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
                      setMultiWindow
                    )
                  }
                  onToggleInvertColors={() =>
                    toggleConfig(
                      "persist.waydroid.invert_colors",
                      invertColors,
                      setInvertColors
                    )
                  }
                  onPropSet={handlePropSet}
                  onOpenSharedFolder={handleOpenSharedFolder}
                  onViewLogs={handleViewLogs}
                  onWipeData={handleWipeData}
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
        logs={terminalLogs}
        installing={installing}
        terminalRef={terminalRef}
        onClose={() => setTerminalOpen(false)}
      />
    </div>
  );
}
