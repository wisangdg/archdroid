import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * Touch mode: left clicks over Waydroid windows become finger touches.
 *
 * The backend status is cheap (no root, no container), so it is read on mount
 * and after every change instead of being cached.
 */
export function useTouchMode({ addToast }) {
  const [status, setStatus] = useState(null);
  const [changing, setChanging] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await invoke("touch_mode_status"));
    } catch (e) {
      console.error("Touch mode status error:", e);
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const setEnabled = useCallback(
    async (enabled) => {
      setChanging(true);
      try {
        const res = await invoke("touch_mode_set", { enabled });
        if (res.status !== "success") {
          addToast({
            type: "error",
            title: "Touch Mode",
            message: res.message || "Touch mode could not be changed.",
          });
        } else if (enabled) {
          addToast({
            type: "info",
            title: "Touch Mode On",
            message:
              "Left clicks over Android windows are now finger touches. Give Waydroid a few seconds to notice the new touchscreen.",
          });
        }
      } catch (e) {
        console.error("Touch mode error:", e);
        addToast({ type: "error", title: "Touch Mode", message: String(e) });
      } finally {
        setChanging(false);
        await refresh();
      }
    },
    [addToast, refresh]
  );

  return { status, changing, refresh, setEnabled };
}

/** What the panel should say, derived from the backend status alone. */
export function touchModeState(status) {
  if (!status) return { kind: "unknown" };
  if (!status.supported) return { kind: "unsupported" };
  if (!status.uinput_access || status.mice.length === 0) {
    return { kind: "needs_access" };
  }
  if (status.enabled && !status.running) {
    return { kind: "stopped", error: status.error };
  }
  return { kind: status.running ? "on" : "off" };
}
