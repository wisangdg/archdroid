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

  it("reports a failing status command once instead of only showing stopped", async () => {
    const broken = {
      status: "error",
      code: "exit",
      message: "ImportError: undefined symbol: g_once_init_leave_pointer",
    };
    const { result, addToast } = setup([broken, broken, STOPPED, broken]);
    const notResponding = () =>
      addToast.mock.calls.filter(
        ([t]) => t.title === "Waydroid Not Responding"
      );
    await waitFor(() => expect(notResponding()).toHaveLength(1));
    expect(notResponding()[0][0].message).toMatch(/g_once_init_leave_pointer/);
    expect(result.current.status).toBe("offline");
    await act(() => result.current.checkStatus()); // same error again: no repeat
    expect(notResponding()).toHaveLength(1);
    await act(() => result.current.checkStatus()); // status readable again
    await act(() => result.current.checkStatus()); // fails again: reported
    expect(notResponding()).toHaveLength(2);
  });

  it("does not report a missing or uninitialized Waydroid as not responding", async () => {
    const { addToast } = setup([
      { status: "error", code: "not_initialized", message: "init first" },
    ]);
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(addToast).not.toHaveBeenCalledWith(
      expect.objectContaining({ title: "Waydroid Not Responding" })
    );
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

  it("says starting while a start is in flight and ignores a second start", async () => {
    let release;
    const { result } = setup([STOPPED], {});
    invoke.mockImplementation(async (cmd) => {
      if (cmd === "waydroid_start") {
        await new Promise((resolve) => (release = resolve));
        return { status: "success" };
      }
      if (cmd === "waydroid_status") return STOPPED;
      return { status: "success" };
    });
    await waitFor(() => expect(result.current.phase).toBe("stopped"));
    let first;
    act(() => {
      first = result.current.handleStart();
    });
    expect(result.current.phase).toBe("starting");
    expect(result.current.operation).toBe("start");
    await act(async () => {
      expect(await result.current.handleStop()).toBeUndefined();
      expect(await result.current.handleStart()).toBe(false);
    });
    expect(
      invoke.mock.calls.filter(([cmd]) => cmd === "waydroid_start")
    ).toHaveLength(1);
    expect(invoke).not.toHaveBeenCalledWith("waydroid_stop");
    release();
    await act(() => first);
    expect(result.current.operation).toBe(null);
  }, 15000);

  it("shows an unreadable status as unreachable, not stopped", async () => {
    const { result } = setup([
      { status: "error", code: "command_failed", message: "ImportError" },
    ]);
    await waitFor(() => expect(result.current.phase).toBe("unreachable"));
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
  it("shows booting until Android reports sys.boot_completed", async () => {
    let boot = { status: "success", data: "booting" };
    invoke.mockImplementation(async (cmd) => {
      if (cmd === "waydroid_status") return RUNNING;
      if (cmd === "waydroid_boot_state") return boot;
      return { status: "success" };
    });
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { result } = renderHook(() =>
        useEngineStatus({ addToast: vi.fn(), onOnline: vi.fn() })
      );
      await waitFor(() => expect(result.current.status).toBe("online"));
      await waitFor(() => expect(result.current.booting).toBe(true));

      boot = { status: "success", data: "booted" };
      await act(async () => {
        await vi.advanceTimersByTimeAsync(2500);
      });
      await waitFor(() => expect(result.current.booting).toBe(false));
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not wait for a boot it cannot observe (no adb)", async () => {
    const { result } = setup([RUNNING], {
      waydroid_boot_state: { status: "error", code: "not_found" },
    });
    await waitFor(() => expect(result.current.status).toBe("online"));
    await waitFor(() => expect(result.current.booting).toBe(false));
    expect(invoke).toHaveBeenCalledWith("waydroid_boot_state");
  });
});
