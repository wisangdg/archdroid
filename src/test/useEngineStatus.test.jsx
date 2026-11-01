import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { useEngineStatus } from "../hooks/useEngineStatus";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const RUNNING = { status: "success", data: "Session:\tRUNNING" };
const STOPPED = { status: "success", data: "Session:\tSTOPPED" };

function setup(statusReplies, commands = {}) {
  let i = 0;
  invoke.mockImplementation(async (cmd) => {
    if (cmd === "waydroid_status") {
      const reply = statusReplies[Math.min(i, statusReplies.length - 1)];
      i += 1;
      return reply;
    }
    return commands[cmd] ?? { status: "success" };
  });
  const addToast = vi.fn();
  const onOnline = vi.fn();
  const hook = renderHook(() => useEngineStatus({ addToast, onOnline }));
  return { ...hook, addToast, onOnline };
}

describe("useEngineStatus", () => {
  beforeEach(() => invoke.mockReset());

  it("reports online and runs onOnline once on the transition", async () => {
    const { result, onOnline } = setup([RUNNING]);
    await waitFor(() => expect(result.current.status).toBe("online"));
    expect(result.current.isInitialized).toBe(true);
    expect(onOnline).toHaveBeenCalledTimes(1);
    await act(() => result.current.checkStatus());
    expect(onOnline).toHaveBeenCalledTimes(1);
  });

  it("marks the engine uninitialized from the error code, not the message", async () => {
    const { result } = setup([
      { status: "error", code: "not_initialized", message: "anything" },
    ]);
    await waitFor(() => expect(result.current.isInitialized).toBe(false));
    expect(result.current.status).toBe("offline");
  });

  it("keeps isInitialized true for other offline errors", async () => {
    const { result } = setup([
      { status: "error", code: "timeout", message: "slow" },
    ]);
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    await waitFor(() => expect(result.current.status).toBe("offline"));
    expect(result.current.isInitialized).toBe(true);
  });

  it("surfaces a failed start and releases isOperating", async () => {
    const { result, addToast } = setup([STOPPED], {
      waydroid_start: { status: "error", message: "binder missing" },
    });
    await act(() => result.current.handleStart());
    expect(result.current.isOperating).toBe(false);
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Start Failed",
        message: expect.stringMatching(/^binder missing .*Diagnostic Bundle/),
      })
    );
  });

  it("waits until the session reports online after a successful start", async () => {
    // first reply is the mount probe; later ones are the post-start polls
    const { result, addToast } = setup([STOPPED, STOPPED, RUNNING]);
    await act(() => result.current.handleStart());
    expect(result.current.status).toBe("online");
    expect(result.current.isOperating).toBe(false);
    expect(addToast).not.toHaveBeenCalledWith(
      expect.objectContaining({ title: "Session Slow to Start" })
    );
  });

  it("does not claim a restart when the stop step fails", async () => {
    const { result, addToast } = setup([RUNNING], {
      waydroid_stop: { status: "error", message: "busy" },
    });
    await waitFor(() => expect(result.current.status).toBe("online"));
    await act(() => result.current.handleRestart());
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({ title: "Restart Failed" })
    );
    expect(addToast).not.toHaveBeenCalledWith(
      expect.objectContaining({ title: "Engine Restarted" })
    );
    expect(invoke).not.toHaveBeenCalledWith("waydroid_start");
    expect(result.current.isOperating).toBe(false);
  });
});
