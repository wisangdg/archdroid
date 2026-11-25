import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useAdguardDns } from "../hooks/useAdguardDns";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const ok = (data, message = null) => ({ status: "success", data, message });

describe("useAdguardDns", () => {
  beforeEach(() => invoke.mockReset());

  it("applies the stored choice only once Android has booted", async () => {
    invoke.mockResolvedValue(ok("on"));
    const addToast = vi.fn();
    const { result, rerender } = renderHook(
      ({ androidReady }) => useAdguardDns({ androidReady, addToast }),
      { initialProps: { androidReady: false } }
    );
    expect(result.current.state).toBe("unknown");
    expect(invoke).not.toHaveBeenCalled();

    rerender({ androidReady: true });
    await waitFor(() => expect(result.current.state).toBe("on"));
    expect(invoke).toHaveBeenCalledWith("private_dns_sync");
  });

  it("shows what Android uses after a change, and keeps it on an error", async () => {
    const addToast = vi.fn();
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === "private_dns_sync") return ok("off");
      if (cmd === "private_dns_set" && args.enabled)
        return ok("foreign", "Android already uses Private DNS dns.example");
      return {
        status: "error",
        code: "session_frozen",
        message: "Open the Waydroid window and try again.",
      };
    });
    const { result } = renderHook(() =>
      useAdguardDns({ androidReady: true, addToast })
    );
    await waitFor(() => expect(result.current.state).toBe("off"));

    await act(() => result.current.setEnabled(true));
    expect(invoke).toHaveBeenCalledWith("private_dns_set", { enabled: true });
    expect(result.current.state).toBe("foreign");
    expect(result.current.message).toBe(
      "Android already uses Private DNS dns.example"
    );

    await act(() => result.current.setEnabled(false));
    expect(result.current.state).toBe("foreign");
    expect(result.current.changing).toBe(false);
    expect(addToast).toHaveBeenLastCalledWith(
      expect.objectContaining({
        type: "error",
        message: "Open the Waydroid window and try again.",
      })
    );
  });
});
