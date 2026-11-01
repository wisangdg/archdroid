import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * Waydroid session state: polling, and the Start/Stop/Restart handlers that
 * wait for the session to actually reach the target state. `onOnline` runs when
 * the session transitions to online so the caller can refresh live probes.
 */
export function useEngineStatus({ addToast, onOnline }) {
  const [status, setStatus] = useState("offline");
  const [isInitialized, setIsInitialized] = useState(true);
  const [isOperating, setIsOperating] = useState(false);
  const prevStatusRef = useRef("offline");
  const statusCheckInFlight = useRef(false);
  const onOnlineRef = useRef(onOnline);
  onOnlineRef.current = onOnline;

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
          onOnlineRef.current?.();
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
          message: `${res.message || "Failed to start Waydroid session."} A Diagnostic Bundle (Settings) includes the Waydroid log.`,
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
            "The session is not reporting online yet. If it stays offline, create a Diagnostic Bundle in Settings: it includes the Waydroid log with the reason.",
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

  useEffect(() => {
    checkStatus();
    // Poll only the session state. A hidden window skips the tick;
    // checkStatus never overlaps itself.
    const interval = setInterval(() => {
      if (document.hidden) return;
      checkStatus();
    }, 5000);
    return () => clearInterval(interval);
  }, [checkStatus]);

  useEffect(() => {
    prevStatusRef.current = status;
  }, [status]);

  return {
    status,
    isInitialized,
    isOperating,
    checkStatus,
    handleStart,
    handleStop,
    handleRestart,
  };
}
