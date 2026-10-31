import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useDeviceConfig } from "../hooks/useDeviceConfig";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const ok = (data) => ({ status: "success", data });

function setup({ props = {}, set = () => ok() } = {}) {
  invoke.mockImplementation(async (cmd, args) => {
    if (cmd === "waydroid_prop_get") return ok(props[args.key] ?? "");
    if (cmd === "waydroid_prop_set") return set(args);
    return ok();
  });
  const addToast = vi.fn();
  const hook = renderHook(() => useDeviceConfig({ addToast }));
  return { ...hook, addToast };
}

const setCalls = () =>
  invoke.mock.calls.filter(([cmd]) => cmd === "waydroid_prop_set");

describe("useDeviceConfig", () => {
  beforeEach(() => invoke.mockReset());

  it("loads toggles and the display profile from the current properties", async () => {
    const { result } = setup({
      props: {
        "persist.waydroid.multi_windows": "true",
        "persist.waydroid.width": "1080",
        "persist.waydroid.height": "1920",
        "ro.sf.lcd_density": "420",
      },
    });
    await waitFor(() => expect(result.current.multiWindow).toBe(true));
    expect(result.current.invertColors).toBe(false);
    expect(result.current.displayRes).toEqual({
      width: "1080",
      height: "1920",
      dpi: "420",
    });
  });

  it("flips a toggle only after the backend confirmed the write", async () => {
    const { result, addToast } = setup();
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    await act(() => result.current.toggleInvertColors());
    expect(result.current.invertColors).toBe(true);
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({ title: "Config Updated" })
    );
  });

  it("leaves the toggle alone when the backend rejects the write", async () => {
    const { result, addToast } = setup({
      set: () => ({ status: "error", message: "not allowed" }),
    });
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    await act(() => result.current.toggleMultiWindow());
    expect(result.current.multiWindow).toBe(false);
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Config Toggle Failed",
        message: "not allowed",
      })
    );
  });

  it("applies the display profile and reports success", async () => {
    const { result, addToast } = setup();
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    await act(() =>
      result.current.handleApplyResolution({
        width: "720",
        height: "1280",
        dpi: "320",
      })
    );
    expect(setCalls().map(([, a]) => a.key)).toEqual([
      "persist.waydroid.width",
      "persist.waydroid.height",
      "ro.sf.lcd_density",
    ]);
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({ title: "Display Profile Updated" })
    );
  });

  it("stops and reports an error instead of claiming the profile was applied", async () => {
    const { result, addToast } = setup({
      set: (args) =>
        args.key === "persist.waydroid.height"
          ? { status: "error", message: "session not running" }
          : ok(),
    });
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    await act(() =>
      result.current.handleApplyResolution({
        width: "720",
        height: "1280",
        dpi: "320",
      })
    );
    expect(setCalls()).toHaveLength(2); // dpi is never attempted
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Display Config Failed",
        message: "session not running",
      })
    );
    expect(addToast).not.toHaveBeenCalledWith(
      expect.objectContaining({ title: "Display Profile Updated" })
    );
  });
});
