import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

const PROP = {
  multiWindow: "persist.waydroid.multi_windows",
  invertColors: "persist.waydroid.invert_colors",
  width: "persist.waydroid.width",
  height: "persist.waydroid.height",
};

/** `waydroid_prop_set` that throws on a backend error result instead of ignoring it. */
async function setProp(key, value) {
  const res = await invoke("waydroid_prop_set", { key, value });
  if (res?.status === "error") {
    throw new Error(res.message || `Failed to set ${key}.`);
  }
  return res;
}

/**
 * Set the display density through `wm density` (the backend reads it back).
 * Not `ro.sf.lcd_density`: Android sets it at boot and Waydroid rejects a
 * runtime write, so the preset always failed.
 */
async function setDensity(dpi) {
  const res = await invoke("waydroid_density_set", { dpi: Number(dpi) });
  if (res?.status === "error") {
    throw new Error(res.message || `Failed to set DPI ${dpi}.`);
  }
  return res;
}

/**
 * Waydroid properties shown in Settings: the two boolean toggles, the display
 * profile, and the free-form property setter. State only changes after the
 * backend confirmed the write.
 */
export function useDeviceConfig({ addToast }) {
  const [multiWindow, setMultiWindow] = useState(false);
  const [invertColors, setInvertColors] = useState(false);
  // Android never dims or sleeps while this is on (see src-tauri/src/awake.rs).
  const [keepAwake, setKeepAwake] = useState(true);
  // The Super key does not open Google Assistant (src-tauri/src/assistant.rs).
  const [superKeyBlocked, setSuperKeyBlocked] = useState(true);
  const [displayRes, setDisplayRes] = useState({
    width: "",
    height: "",
    dpi: "",
  });

  const loadConfigs = useCallback(async () => {
    // Applies the stored choice too, so a fresh Android is kept awake. Apart
    // from the reads below: a failure here must not hide the other settings.
    invoke("keep_awake_sync")
      .then((res) => {
        if (res?.status === "success") setKeepAwake(res.data === "true");
      })
      .catch((e) => console.error("Failed to sync keep awake:", e));
    invoke("super_key_sync")
      .then((res) => {
        if (res?.status === "success") setSuperKeyBlocked(res.data === "true");
      })
      .catch((e) => console.error("Failed to sync the Super key:", e));
    const show = ([mw, inv, width, height, dpi]) => {
      setMultiWindow(mw === "true");
      setInvertColors(inv === "true");
      setDisplayRes({ width, height, dpi });
    };
    try {
      // One adb shell for all five values (in src-tauri/src/adb.rs, in the
      // order of PROP); `waydroid prop get` is a Python process per key.
      const { props, density } = await invoke("settings_values");
      show([...props, density ? String(density) : ""]);
      return;
    } catch {
      // No adb, or a frozen container: read them the slower way below.
    }
    try {
      const get = (key) => invoke("waydroid_prop_get", { key });
      // Independent reads: together they take as long as the slowest one.
      const value = (res) => (res.status === "success" ? res.data || "" : "");
      const results = await Promise.all([
        get(PROP.multiWindow),
        get(PROP.invertColors),
        get(PROP.width),
        get(PROP.height),
        invoke("waydroid_density_get"),
      ]);
      show(results.map(value));
    } catch (e) {
      console.error("Failed to load configs:", e);
    }
  }, []);

  // No read on mount: with the engine stopped every one of these calls fails
  // (and starts a Python process to find that out). App.jsx calls
  // `loadConfigs` once Android has booted.

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

  const toggleMultiWindow = () =>
    toggleConfig(
      PROP.multiWindow,
      multiWindow,
      setMultiWindow,
      "Multi-Window Mode"
    );

  const toggleInvertColors = () =>
    toggleConfig(
      PROP.invertColors,
      invertColors,
      setInvertColors,
      "Invert Colors"
    );

  /** Applies the profile; resolves to whether it was written. */
  const handleApplyResolution = async ({ width, height, dpi }) => {
    const outOfRange = (value) =>
      value !== "" && !(Number(value) >= 320 && Number(value) <= 7680);
    if (outOfRange(width) || outOfRange(height)) {
      addToast({
        type: "error",
        title: "Display Config Failed",
        message: "Width and height must be between 320 and 7680 pixels.",
      });
      return false;
    }
    try {
      if (width) await setProp(PROP.width, width);
      if (height) await setProp(PROP.height, height);
      if (dpi) await setDensity(dpi);
      await loadConfigs();
      addToast({
        type: "success",
        title: "Display Profile Updated",
        message: `Applied ${width} × ${height} (${dpi} DPI). The DPI applies now; restart the Waydroid session for the resolution.`,
        duration: 5000,
      });
      return true;
    } catch (err) {
      console.error("Failed to apply resolution:", err);
      addToast({
        type: "error",
        title: "Display Config Failed",
        message: err?.message || String(err),
      });
      return false;
    }
  };

  const handlePropSet = async (e) => {
    e.preventDefault();
    const key = e.target.elements.propKey.value;
    const val = e.target.elements.propVal.value;
    try {
      const res = await invoke("waydroid_prop_set", { key, value: val });
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

  const toggleKeepAwake = async () => {
    const enabled = !keepAwake;
    try {
      const res = await invoke("keep_awake_set", { enabled });
      if (res?.status !== "success") {
        throw new Error(res?.message || "Android did not take the change.");
      }
      setKeepAwake(res.data === "true");
      addToast({
        type: "info",
        title: "Config Updated",
        message: enabled
          ? "Android stays awake: games no longer dim or freeze when left alone."
          : "Android dims and sleeps again after its screen timeout.",
        duration: 3000,
      });
    } catch (e) {
      addToast({
        type: "error",
        title: "Config Toggle Failed",
        message: e?.message || String(e),
      });
    }
  };

  const toggleSuperKey = async () => {
    const blocked = !superKeyBlocked;
    try {
      const res = await invoke("super_key_set", { blocked });
      if (res?.status !== "success") {
        throw new Error(res?.message || "Android did not take the change.");
      }
      setSuperKeyBlocked(res.data === "true");
      addToast({
        type: "info",
        title: "Config Updated",
        message: blocked
          ? "The Super key no longer opens Google Assistant over your game."
          : "Google Assistant is back; the Super key opens it again.",
        duration: 3000,
      });
    } catch (e) {
      addToast({
        type: "error",
        title: "Config Toggle Failed",
        message: e?.message || String(e),
      });
    }
  };

  return {
    keepAwake,
    toggleKeepAwake,
    superKeyBlocked,
    toggleSuperKey,
    multiWindow,
    invertColors,
    displayRes,
    loadConfigs,
    toggleMultiWindow,
    toggleInvertColors,
    handleApplyResolution,
    handlePropSet,
  };
}
