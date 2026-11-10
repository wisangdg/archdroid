import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useApkInstall } from "../hooks/useApkInstall";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

let startedListener = null;
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((event, cb) => {
    if (event === "cmd-started") startedListener = cb;
    return Promise.resolve(() => {});
  }),
}));

let dropHandler = null;
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: (cb) => {
      dropHandler = cb;
      return Promise.resolve(() => {});
    },
  }),
}));

function setup({ busy = false, install = { status: "success" } } = {}) {
  invoke.mockImplementation(async (cmd) =>
    cmd === "waydroid_install_apk" ? install : { status: "success" }
  );
  const addToast = vi.fn();
  const loadApps = vi.fn();
  const operationInFlight = { current: busy };
  const hook = renderHook(() =>
    useApkInstall({ addToast, operationInFlight, loadApps })
  );
  return { ...hook, addToast, loadApps };
}

const installCalls = () =>
  invoke.mock.calls.filter(([cmd]) => cmd === "waydroid_install_apk");

describe("useApkInstall", () => {
  beforeEach(() => {
    invoke.mockReset();
    open.mockReset();
    dropHandler = null;
  });

  it("installs the picked file and refreshes the app list", async () => {
    const { result, addToast, loadApps } = setup();
    open.mockResolvedValue("/tmp/game.apk");
    await act(() => result.current.handleInstallApk());
    expect(installCalls()).toEqual([
      ["waydroid_install_apk", { path: "/tmp/game.apk" }],
    ]);
    expect(loadApps).toHaveBeenCalled();
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({ title: "APK Installed" })
    );
  });

  it("does nothing when the picker is cancelled", async () => {
    const { result } = setup();
    open.mockResolvedValue(null);
    await act(() => result.current.handleInstallApk());
    expect(installCalls()).toHaveLength(0);
  });

  it("reports a rejected install without refreshing the list", async () => {
    const { result, addToast, loadApps } = setup({
      install: { status: "error", message: "not a valid APK" },
    });
    open.mockResolvedValue("/tmp/bad.apk");
    await act(() => result.current.handleInstallApk());
    expect(loadApps).not.toHaveBeenCalled();
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Install Failed",
        message: "not a valid APK",
      })
    );
  });

  it("refuses to install while a streaming task owns the slot", async () => {
    const { result, addToast } = setup({ busy: true });
    open.mockResolvedValue("/tmp/game.apk");
    await act(() => result.current.handleInstallApk());
    expect(installCalls()).toHaveLength(0);
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({ title: "Operation In Progress" })
    );
  });

  it("installs only the .apk files from a drop and tracks the overlay", async () => {
    const { result } = setup();
    await waitFor(() => expect(dropHandler).not.toBeNull());
    act(() => dropHandler({ payload: { type: "enter" } }));
    expect(result.current.isDraggingOver).toBe(true);
    await act(async () =>
      dropHandler({
        payload: {
          type: "drop",
          paths: [
            "/a/one.APK",
            "/a/notes.txt",
            "/a/game.xapk",
            "/a/x.apkm.txt",
          ],
        },
      })
    );
    await waitFor(() => expect(installCalls()).toHaveLength(2));
    expect(installCalls().map(([, args]) => args.path)).toEqual([
      "/a/one.APK",
      "/a/game.xapk",
    ]);
    expect(result.current.isDraggingOver).toBe(false);
  });

  it("warns instead of installing when nothing dropped is an APK", async () => {
    const { addToast } = setup();
    await waitFor(() => expect(dropHandler).not.toBeNull());
    await act(async () =>
      dropHandler({ payload: { type: "drop", paths: ["/a/readme.txt"] } })
    );
    expect(installCalls()).toHaveLength(0);
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({ title: "Unsupported File" })
    );
  });

  it("shows the file while it installs and cancels by task ID", async () => {
    let finish;
    invoke.mockImplementation((cmd) =>
      cmd === "waydroid_install_apk"
        ? new Promise((resolve) => (finish = resolve))
        : Promise.resolve({ status: "success" })
    );
    open.mockResolvedValue("/home/u/Downloads/game.apks");
    const addToast = vi.fn();
    const { result } = renderHook(() =>
      useApkInstall({
        addToast,
        operationInFlight: { current: false },
        loadApps: vi.fn(),
      })
    );
    act(() => {
      result.current.handleInstallApk();
    });
    await waitFor(() => expect(result.current.installingApk).toBe("game.apks"));
    act(() => startedListener({ payload: { task_id: "task-7" } }));
    await act(() => result.current.cancelInstall());
    expect(invoke).toHaveBeenCalledWith("cancel_task", { taskId: "task-7" });
    await act(async () =>
      finish({ status: "error", code: "cancelled", message: "cancelled" })
    );
    expect(result.current.installingApk).toBe(null);
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({ title: "Install Cancelled" })
    );
  });
});
