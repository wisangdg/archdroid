import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/**
 * APK installation from the file picker and from drag & drop. Dropped files
 * come from the native webview event (real paths); the DOM handlers only keep
 * the browser from navigating to a dropped file and track the hover overlay.
 * `operationInFlight` is the streaming-task flag: an APK must not look accepted
 * while a wipe or installer owns the single-flight slot.
 */
export function useApkInstall({ addToast, operationInFlight, loadApps }) {
  const [isDraggingOver, setIsDraggingOver] = useState(false);

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
    handleInstallApk,
    handleDragOver,
    handleDragLeave,
    handleDrop,
  };
}
