import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  render,
  screen,
  fireEvent,
  renderHook,
  act,
  waitFor,
} from "@testing-library/react";
import TouchModePanel from "../components/keymap/TouchModePanel";
import { touchModeState, useTouchMode } from "../hooks/useTouchMode";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const READY = {
  enabled: false,
  running: false,
  supported: true,
  uinput_access: true,
  mice: ["Compx Fantech Helios II"],
  rule_installed: true,
};

function renderPanel(status, handlers = {}) {
  const onToggle = handlers.onToggle ?? vi.fn();
  const onEnableAccess = handlers.onEnableAccess ?? vi.fn();
  render(
    <TouchModePanel
      status={status}
      changing={false}
      installing={false}
      onToggle={onToggle}
      onEnableAccess={onEnableAccess}
    />
  );
  return { onToggle, onEnableAccess };
}

describe("touchModeState", () => {
  it("derives every panel state from the backend status", () => {
    expect(touchModeState(null).kind).toBe("unknown");
    expect(touchModeState({ ...READY, supported: false }).kind).toBe(
      "unsupported"
    );
    expect(touchModeState({ ...READY, uinput_access: false }).kind).toBe(
      "needs_access"
    );
    expect(touchModeState({ ...READY, mice: [] }).kind).toBe("needs_access");
    expect(touchModeState(READY).kind).toBe("off");
    expect(
      touchModeState({ ...READY, enabled: true, running: true }).kind
    ).toBe("on");
    expect(
      touchModeState({ ...READY, enabled: true, error: "no mouse" })
    ).toEqual({ kind: "stopped", error: "no mouse" });
  });
});

describe("TouchModePanel", () => {
  it("is an accessible switch that turns touch mode on", () => {
    const { onToggle } = renderPanel(READY);
    const toggle = screen.getByRole("switch", { name: "Touch mode" });
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    fireEvent.click(toggle);
    expect(onToggle).toHaveBeenCalledWith(true);
  });

  it("turns it off again and names the mice it holds", () => {
    const { onToggle } = renderPanel({
      ...READY,
      enabled: true,
      running: true,
    });
    const toggle = screen.getByRole("switch", { name: "Touch mode" });
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    expect(screen.getByText(/Compx Fantech Helios II/)).toBeTruthy();
    fireEvent.click(toggle);
    expect(onToggle).toHaveBeenCalledWith(false);
  });

  it("offers access setup instead of a switch that cannot work", () => {
    const { onEnableAccess } = renderPanel({ ...READY, uinput_access: false });
    expect(screen.queryByRole("switch")).toBeNull();
    expect(screen.getByText(/never keyboards/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Enable access/ }));
    expect(onEnableAccess).toHaveBeenCalled();
  });

  it("says plainly when the compositor is not supported", () => {
    renderPanel({ ...READY, supported: false });
    expect(screen.getByRole("switch").disabled).toBe(true);
    expect(
      screen.getByText(/reads the cursor position from Hyprland/)
    ).toBeTruthy();
  });

  it("surfaces why a remembered touch mode is not running", () => {
    renderPanel({
      ...READY,
      enabled: true,
      error: "no mouse could be grabbed",
    });
    expect(screen.getByRole("alert").textContent).toContain(
      "no mouse could be grabbed"
    );
    expect(screen.getByRole("switch").disabled).toBe(false);
  });
});

describe("useTouchMode", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("reads the status on mount and again after a change", async () => {
    let enabled = false;
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === "touch_mode_status") {
        return { ...READY, enabled, running: enabled };
      }
      if (cmd === "touch_mode_set") {
        enabled = args.enabled;
        return { status: "success" };
      }
      throw new Error(`unexpected ${cmd}`);
    });
    const addToast = vi.fn();
    const { result } = renderHook(() => useTouchMode({ addToast }));
    await waitFor(() => expect(result.current.status?.running).toBe(false));

    await act(() => result.current.setEnabled(true));
    expect(invoke).toHaveBeenCalledWith("touch_mode_set", { enabled: true });
    expect(result.current.status.running).toBe(true);
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({ title: "Touch Mode On" })
    );
  });

  it("shows the backend's reason when touch mode cannot start", async () => {
    invoke.mockImplementation(async (cmd) =>
      cmd === "touch_mode_status"
        ? READY
        : { status: "error", message: "No write access to /dev/uinput." }
    );
    const addToast = vi.fn();
    const { result } = renderHook(() => useTouchMode({ addToast }));
    await act(() => result.current.setEnabled(true));
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({
        type: "error",
        message: "No write access to /dev/uinput.",
      })
    );
  });
});
