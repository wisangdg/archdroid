import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";

/** Single APKs and split-APK bundles; the backend installs bundles with adb. */
export const APK_EXTENSIONS = ["apk", "apks", "xapk", "apkm"];

export const isInstallable = (path) =>
  typeof path === "string" &&
  APK_EXTENSIONS.some((ext) => path.toLowerCase().endsWith(`.${ext}`));

/**
 * APK installation from the file picker and from drag & drop. Dropped files
 * come from the native webview event (real paths); the DOM handlers only keep
 * the browser from navigating to a dropped file and track the hover overlay.
 * `operationInFlight` is the streaming-task flag: an APK must not look accepted
 * while a wipe or installer owns the single-flight slot.
 */
export function useApkInstall({ addToast, operationInFlight, loadApps }) {
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  // The file being installed. A big bundle takes minutes, so the page shows
  // it for the whole time (with Cancel) instead of a toast that is gone in 4 s.
  const [installingApk, setInstallingApk] = useState(null);
  // Cancel was pressed for the current install. Shown on the button, and
  // remembered when the backend has not announced the task ID yet.
  const [cancelling, setCancelling] = useState(false);
  const installActive = useRef(false);
  const installTaskId = useRef(null);
  const cancelRequested = useRef(false);
  // Set by Cancel, read by a multi-file drop to skip the files after it.
  const cancelledBatch = useRef(false);

  const sendCancel = useCallback(async (taskId) => {
    try {
      await invoke("cancel_task", { taskId });
    } catch (e) {
      console.error("Cancel install failed:", e);
    }
  }, []);

  // The backend announces the install's task ID; Cancel needs it. A Cancel
  // pressed before this arrives is sent now instead of being lost.
  useEffect(() => {
    const unlisten = listen("cmd-started", (event) => {
      if (!installActive.current) return;
      installTaskId.current = event.payload.task_id;
      if (cancelRequested.current) sendCancel(event.payload.task_id);
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, [sendCancel]);

  const cancelInstall = useCallback(async () => {
    if (!installActive.current || cancelRequested.current) return;
    cancelRequested.current = true;
    cancelledBatch.current = true;
    setCancelling(true);
    if (installTaskId.current) await sendCancel(installTaskId.current);
  }, [sendCancel]);

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
      if (installActive.current) {
        addToast({
          type: "warning",
          title: "Operation In Progress",
          message: "Wait for the current install to finish.",
        });
        return;
      }
      installActive.current = true;
      installTaskId.current = null;
      cancelRequested.current = false;
      setCancelling(false);
      setInstallingApk(fileName);
      try {
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
        } else if (res.code === "cancelled") {
          addToast({
            type: "info",
            title: "Install Cancelled",
            message: `${fileName} was not installed.`,
          });
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
      } finally {
        installActive.current = false;
        installTaskId.current = null;
        cancelRequested.current = false;
        setCancelling(false);
        setInstallingApk(null);
      }
    },
    [addToast, loadApps, operationInFlight]
  );

  const handleInstallApk = async () => {
    try {
      const file = await open({
        multiple: false,
        filters: [
          {
            name: "Android Package",
            extensions: APK_EXTENSIONS,
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
      const apkFiles = paths.filter(isInstallable);
      if (apkFiles.length === 0) {
        addToast({
          type: "warning",
          title: "Unsupported File",
          message:
            "Only .apk files and split APK bundles (.apks, .xapk, .apkm) can be installed via drag & drop.",
          duration: 4000,
        });
        return;
      }

      cancelledBatch.current = false;
      for (const apk of apkFiles) {
        await installApkFile(apk);
        // Cancel stops the batch, not only the file that was installing.
        if (cancelledBatch.current) break;
      }
      cancelledBatch.current = false;
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

  return {
    isDraggingOver,
    installingApk,
    cancellingInstall: cancelling,
    cancelInstall,
    handleInstallApk,
    handleDragOver,
    handleDragLeave,
    handleDrop,
  };
}
