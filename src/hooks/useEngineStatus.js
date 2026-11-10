import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { sessionRunning } from "../lib/waydroidStatus";

const BOOT_POLL_MS = 2000;
const BOOT_POLL_ATTEMPTS = 45; // ~90 s

/**
 * Waydroid session state: polling, and the Start/Stop/Restart handlers that
 * wait for the session to actually reach the target state. `onOnline` runs when
 * the session transitions to online so the caller can refresh live probes.
 */
export function useEngineStatus({ addToast, onOnline }) {
  const [status, setStatus] = useState("offline");
  // `null` until the first status reply, so a first run does not flash the
  // (empty) app library before the setup screen.
  const [isInitialized, setIsInitialized] = useState(null);
  const [isOperating, setIsOperating] = useState(false);
  // The session runs but Android has not finished booting yet.
  const [booting, setBooting] = useState(false);
  const bootWatchRef = useRef(0);
  const prevStatusRef = useRef("offline");
  const statusCheckInFlight = useRef(false);
  const onOnlineRef = useRef(onOnline);
  onOnlineRef.current = onOnline;
  const addToastRef = useRef(addToast);
  addToastRef.current = addToast;
  // The last status error shown, so a poll every 5 s reports it once.
  const lastStatusErrorRef = useRef(null);

  // `waydroid status` failing is not the same as a stopped session: the 0.1.4
  // AppImage showed "Engine Stopped" while every waydroid call failed with an
  // ImportError. Say so once per distinct error.
  const reportStatusError = useCallback((message) => {
    const text = message || "unknown error";
    if (lastStatusErrorRef.current === text) return;
    lastStatusErrorRef.current = text;
    addToastRef.current?.({
      type: "warning",
      title: "Waydroid Not Responding",
      message: `ArchDroid could not read the Waydroid status, so the engine is shown as stopped: ${text} A Diagnostic Bundle (Settings) includes the full error.`,
    });
  }, []);

  // After the session comes online, poll sys.boot_completed until Android is
  // up. Without adb (an error reply) the answer is unknowable, and the cap
  // keeps a stuck probe from showing "booting" forever.
  const watchBoot = useCallback(async () => {
    const watch = ++bootWatchRef.current;
    setBooting(true);
    for (let attempt = 0; attempt < BOOT_POLL_ATTEMPTS; attempt++) {
      let res;
      try {
        res = await invoke("waydroid_boot_state");
      } catch {
        break;
      }
      if (watch !== bootWatchRef.current) return;
      if (!(res?.status === "success" && res.data === "booting")) break;
      await new Promise((resolve) => setTimeout(resolve, BOOT_POLL_MS));
      if (watch !== bootWatchRef.current) return;
    }
    if (watch === bootWatchRef.current) setBooting(false);
  }, []);

  const checkStatus = useCallback(async () => {
    if (statusCheckInFlight.current) return;
    statusCheckInFlight.current = true;
    try {
      const res = await invoke("waydroid_status");
      if (sessionRunning(res)) {
        setStatus("online");
        setIsInitialized(true);
        lastStatusErrorRef.current = null;
        if (prevStatusRef.current !== "online") {
          onOnlineRef.current?.();
          watchBoot();
        }
      } else {
        setStatus("offline");
        bootWatchRef.current++;
        setBooting(false);
        const missing =
          res.code === "not_found" || res.code === "not_initialized";
        setIsInitialized(!missing);
        if (res.status === "error" && !missing) {
          reportStatusError(res.message);
        } else {
          lastStatusErrorRef.current = null;
        }
      }
    } catch (e) {
      console.error("Status check failed:", e);
      setStatus("offline");
      // The status is unreadable, not missing: show the app, not the setup.
      setIsInitialized((known) => known ?? true);
      reportStatusError(String(e));
    } finally {
      statusCheckInFlight.current = false;
    }
  }, [reportStatusError, watchBoot]);

  const pollStatusUntil = useCallback(
    async (targetStatus, maxAttempts = 15, intervalMs = 400) => {
      for (let attempt = 0; attempt < maxAttempts; attempt++) {
        await new Promise((resolve) => setTimeout(resolve, intervalMs));
        const res = await invoke("waydroid_status");
        const current = sessionRunning(res) ? "online" : "offline";
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

  /** Starts the session; resolves to whether it reports online. */
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
        return false;
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
      return online;
    } catch (e) {
      console.error("Failed to start Waydroid:", e);
      addToast({
        type: "error",
        title: "Start Failed",
        message: String(e),
      });
      setIsOperating(false);
      return false;
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
    booting,
    checkStatus,
    handleStart,
    handleStop,
    handleRestart,
  };
}
