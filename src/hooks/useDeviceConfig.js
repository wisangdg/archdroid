import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

const PROP = {
  multiWindow: "persist.waydroid.multi_windows",
  invertColors: "persist.waydroid.invert_colors",
  width: "persist.waydroid.width",
  height: "persist.waydroid.height",
  dpi: "ro.sf.lcd_density",
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
 * Waydroid properties shown in Settings: the two boolean toggles, the display
 * profile, and the free-form property setter. State only changes after the
 * backend confirmed the write.
 */
export function useDeviceConfig({ addToast }) {
  const [multiWindow, setMultiWindow] = useState(false);
  const [invertColors, setInvertColors] = useState(false);
  const [displayRes, setDisplayRes] = useState({
    width: "",
    height: "",
    dpi: "",
  });

  const loadConfigs = useCallback(async () => {
    try {
      const get = (key) => invoke("waydroid_prop_get", { key });
      const mw = await get(PROP.multiWindow);
      setMultiWindow(mw.status === "success" && mw.data === "true");
      const inv = await get(PROP.invertColors);
      setInvertColors(inv.status === "success" && inv.data === "true");

      const [widthRes, heightRes, dpiRes] = [
        await get(PROP.width),
        await get(PROP.height),
        await get(PROP.dpi),
      ];
      const value = (res) => (res.status === "success" ? res.data || "" : "");
      setDisplayRes({
        width: value(widthRes),
        height: value(heightRes),
        dpi: value(dpiRes),
      });
    } catch (e) {
      console.error("Failed to load configs:", e);
    }
  }, []);

  useEffect(() => {
    loadConfigs();
  }, [loadConfigs]);

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

  const handleApplyResolution = async ({ width, height, dpi }) => {
    try {
      if (width) await setProp(PROP.width, width);
      if (height) await setProp(PROP.height, height);
      if (dpi) await setProp(PROP.dpi, dpi);
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
        message: err?.message || String(err),
      });
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

  return {
    multiWindow,
    invertColors,
    displayRes,
    toggleMultiWindow,
    toggleInvertColors,
    handleApplyResolution,
    handlePropSet,
  };
}
