import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  render,
  screen,
  waitFor,
  fireEvent,
  act,
} from "@testing-library/react";
import App from "../App";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

let eventListeners = {};
vi.mock("@tauri-apps/api/event", () => ({
  // Like Tauri: every listener of an event is called, not only the last one.
  listen: vi.fn((event, cb) => {
    const previous = eventListeners[event];
    eventListeners[event] = previous
      ? (payload) => {
          previous(payload);
          cb(payload);
        }
      : cb;
    return Promise.resolve(() => {});
  }),
}));

let dragDropHandler = null;
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: (cb) => {
      dragDropHandler = cb;
      return Promise.resolve(() => {});
    },
  }),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

describe("App state & workflow integration", () => {
  beforeEach(() => {
    eventListeners = {};
    dragDropHandler = null;
    vi.clearAllMocks();
  });

  it("handles status not_initialized by rendering SetupWizard", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "error",
          code: "not_initialized",
          message: "Waydroid is not initialized",
        });
      }
      if (cmd === "check_host_diagnostics") {
        return Promise.resolve({
          items: [],
          package_manager: "pacman",
        });
      }
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "check_gapps_status")
        return Promise.resolve({ status: "not_installed" });
      if (cmd === "check_houdini_status")
        return Promise.resolve({ status: "not_installed" });
      if (cmd === "check_multitouch_status")
        return Promise.resolve({ status: "not_installed" });
      if (cmd === "waydroid_prop_get")
        return Promise.resolve({ status: "success", data: "false" });
      return Promise.resolve({ status: "success" });
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText("Initialize Android Emulator")).toBeTruthy();
    });
  });

  it("handles status online by rendering navigation and main views", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          code: null,
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") {
        return Promise.resolve([
          {
            name: "F-Droid",
            package: "org.fdroid.fdroid",
            icon: null,
          },
        ]);
      }
      if (cmd === "check_gapps_status")
        return Promise.resolve({ status: "installed" });
      if (cmd === "check_houdini_status")
        return Promise.resolve({ status: "installed" });
      if (cmd === "check_multitouch_status")
        return Promise.resolve({ status: "not_installed" });
      if (cmd === "waydroid_prop_get")
        return Promise.resolve({ status: "success", data: "false" });
      return Promise.resolve({ status: "success" });
    });

    render(<App />);

    await waitFor(() => {
      // Control Center: the row in the list and the selected app's heading.
      expect(screen.getByRole("button", { name: /F-Droid/ })).toBeTruthy();
      expect(
        screen.getByRole("heading", { level: 2, name: "F-Droid" })
      ).toBeTruthy();
      expect(screen.getByText("Engine Running")).toBeTruthy();
    });
  });

  it("shows apps that appear while the session runs without any user action", async () => {
    let apps = [];
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          code: null,
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") return Promise.resolve(apps);
      if (cmd === "waydroid_prop_get")
        return Promise.resolve({ status: "success", data: "false" });
      return Promise.resolve({ status: "not_installed" });
    });
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      render(<App />);
      await waitFor(() =>
        expect(screen.getByText("Engine Running")).toBeTruthy()
      );
      expect(screen.queryByText("Settings App")).toBeNull();

      // First boot after init: Waydroid writes the launcher entries later.
      apps = [
        { name: "Settings App", package: "com.android.settings", icon: null },
      ];
      await act(async () => {
        vi.advanceTimersByTime(10000);
      });
      await waitFor(() =>
        expect(
          screen.getByRole("heading", { level: 2, name: "Settings App" })
        ).toBeTruthy()
      );
    } finally {
      vi.useRealTimers();
    }
  });

  it("rejects non-APK files during drag and drop with a warning toast", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      return Promise.resolve({ status: "success" });
    });

    render(<App />);

    await waitFor(() => {
      expect(dragDropHandler).toBeTypeOf("function");
    });

    await act(async () => {
      dragDropHandler({
        payload: {
          type: "drop",
          paths: ["/home/user/document.pdf"],
        },
      });
    });

    await waitFor(() => {
      expect(screen.getByText("Unsupported File")).toBeTruthy();
      expect(
        screen.getByText(/Only \.apk files and split APK bundles/)
      ).toBeTruthy();
    });
  });

  it("handles streaming install task failure with code timeout", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "preview_package_install") {
        return Promise.resolve({
          packageManager: "pacman",
          packages: ["lxc"],
          command: "pacman -S --needed lxc",
        });
      }
      if (cmd === "install_gapps") {
        setTimeout(() => {
          eventListeners["cmd-started"]?.({
            payload: { task_id: "task-1" },
          });
          eventListeners["cmd-finished"]?.({
            payload: {
              task_id: "task-1",
              success: false,
              code: "timeout",
              message: "Operation timed out after 300 seconds.",
            },
          });
        }, 10);
        return new Promise((resolve) => {
          setTimeout(() => {
            resolve({
              status: "error",
              code: "timeout",
              message: "Operation timed out after 300 seconds.",
            });
          }, 30);
        });
      }
      return Promise.resolve({ status: "success" });
    });

    render(<App />);

    // Switch to Install Tools view
    const toolsTab = screen.getByRole("button", { name: "Install Tools" });
    fireEvent.click(toolsTab);

    await waitFor(() => {
      expect(screen.getByText("Google Play Services (GApps)")).toBeTruthy();
    });

    const installGAppsBtn = screen.getByText("Install GApps");
    fireEvent.click(installGAppsBtn);

    // Confirm dialog should appear with package preview
    await waitFor(() => {
      expect(
        screen.getByText("Install Google Play Services (Root Required)")
      ).toBeTruthy();
    });

    const confirmBtn = screen.getByText("Proceed & Authorize Root");
    fireEvent.click(confirmBtn);

    // Terminal modal opens and event starts
    await waitFor(() => {
      expect(
        screen.getByText("Installing Google Play Services (GApps)")
      ).toBeTruthy();
    });

    await waitFor(() => {
      expect(screen.getByText("Operation Timed Out")).toBeTruthy();
    });
  });

  it("handles successful GApps installation and status refresh", async () => {
    let gappsStatus = "not_installed";

    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "check_gapps_status") {
        return Promise.resolve({ status: gappsStatus });
      }
      if (cmd === "preview_package_install") {
        return Promise.resolve({
          packageManager: "pacman",
          packages: ["lxc", "curl"],
          command: "pacman -S --needed lxc curl",
        });
      }
      if (cmd === "install_gapps") {
        gappsStatus = "installed";
        setTimeout(() => {
          eventListeners["cmd-started"]?.({
            payload: { task_id: "task-gapps-ok" },
          });
          eventListeners["cmd-output"]?.({
            payload: {
              task_id: "task-gapps-ok",
              line: "[SUCCESS] GApps installed.",
            },
          });
          eventListeners["cmd-finished"]?.({
            payload: {
              task_id: "task-gapps-ok",
              success: true,
              code: null,
              message: null,
            },
          });
        }, 10);
        return Promise.resolve({ status: "success" });
      }
      return Promise.resolve({ status: "success" });
    });

    render(<App />);

    const toolsTab = screen.getByRole("button", { name: "Install Tools" });
    fireEvent.click(toolsTab);

    await waitFor(() => {
      expect(screen.getByText("Install GApps")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Install GApps"));

    await waitFor(() => {
      expect(screen.getByText("Proceed & Authorize Root")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Proceed & Authorize Root"));

    await waitFor(() => {
      expect(screen.getByText("Finished successfully")).toBeTruthy();
      expect(screen.getByText("Process Finished")).toBeTruthy();
    });
  });

  it("handles Houdini ARM installation elevation_denied error", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "preview_package_install") {
        return Promise.resolve({
          packageManager: "pacman",
          packages: ["tar"],
          command: "pacman -S --needed tar",
        });
      }
      if (cmd === "install_houdini") {
        // Emit cmd-started and cmd-finished while task is running
        eventListeners["cmd-started"]?.({
          payload: { task_id: "task-houdini-denied" },
        });
        eventListeners["cmd-finished"]?.({
          payload: {
            task_id: "task-houdini-denied",
            success: false,
            code: "elevation_denied",
            message: "Root authentication was cancelled or rejected by user.",
          },
        });
        return Promise.resolve({
          status: "error",
          code: "elevation_denied",
          message: "Root authentication was cancelled or rejected by user.",
        });
      }
      return Promise.resolve({ status: "success" });
    });

    render(<App />);

    const toolsTab = screen.getByRole("button", { name: "Install Tools" });
    fireEvent.click(toolsTab);

    await waitFor(() => {
      expect(screen.getByText("Install ARM Support")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Install ARM Support"));

    await waitFor(() => {
      expect(screen.getByText("Proceed & Authorize Root")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Proceed & Authorize Root"));

    await waitFor(() => {
      expect(screen.getByText("Root Access Denied")).toBeTruthy();
    });
  });

  it("triggers cancel_task with the active taskId when Cancel is clicked", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "preview_package_install") {
        return Promise.resolve({
          packageManager: "pacman",
          packages: [],
          command: "",
        });
      }
      if (cmd === "install_gapps") {
        // Emit cmd-started immediately upon task invocation
        eventListeners["cmd-started"]?.({
          payload: { task_id: "task-cancel-me" },
        });
        return new Promise(() => {}); // keeps running until cancelled
      }
      if (cmd === "cancel_task") {
        return Promise.resolve({ status: "success" });
      }
      return Promise.resolve({ status: "success" });
    });

    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Install Tools" }));

    await waitFor(() => {
      expect(screen.getByText("Install GApps")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Install GApps"));

    await waitFor(() => {
      expect(screen.getByText("Proceed & Authorize Root")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Proceed & Authorize Root"));

    // Wait for task to start and cancel button to be available
    await waitFor(() => {
      const cancelButtons = screen.getAllByRole("button", {
        name: /^cancel$/i,
      });
      expect(cancelButtons.length).toBeGreaterThan(0);
    });

    const cancelBtn = screen.getAllByRole("button", {
      name: /^cancel$/i,
    })[0];
    fireEvent.click(cancelBtn);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("cancel_task", {
        taskId: "task-cancel-me",
      });
    });
  });

  it("handles wipe container confirmation and triggers waydroid_wipe", async () => {
    let wipeInvoked = false;

    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "waydroid_wipe") {
        wipeInvoked = true;
        setTimeout(() => {
          eventListeners["cmd-started"]?.({
            payload: { task_id: "task-wipe-1" },
          });
          eventListeners["cmd-finished"]?.({
            payload: {
              task_id: "task-wipe-1",
              success: true,
              code: null,
              message: null,
            },
          });
        }, 10);
        return Promise.resolve({ status: "success" });
      }
      return Promise.resolve({ status: "success" });
    });

    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Settings" }));

    await waitFor(() => {
      expect(screen.getByText("Factory Reset (Wipe All Data)")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Factory Reset (Wipe All Data)"));

    await waitFor(() => {
      expect(screen.getByText("Factory Reset Waydroid?")).toBeTruthy();
      expect(screen.getByText("Wipe Everything")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Wipe Everything"));

    await waitFor(() => {
      expect(wipeInvoked).toBe(true);
      expect(
        screen.getByText("Factory Reset (Wiping Waydroid Data)")
      ).toBeTruthy();
    });
  });

  it("handles busy response without breaking terminal state", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "preview_package_install") {
        return Promise.resolve({
          packageManager: "pacman",
          packages: [],
          command: "",
        });
      }
      if (cmd === "install_gapps") {
        return Promise.resolve({
          status: "error",
          code: "busy",
          message: "Another privileged operation is currently in flight.",
        });
      }
      return Promise.resolve({ status: "success" });
    });

    render(<App />);

    fireEvent.click(screen.getByRole("button", { name: "Install Tools" }));

    await waitFor(() => {
      expect(screen.getByText("Install GApps")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Install GApps"));

    await waitFor(() => {
      expect(screen.getByText("Proceed & Authorize Root")).toBeTruthy();
    });

    fireEvent.click(screen.getByText("Proceed & Authorize Root"));

    await waitFor(() => {
      expect(
        screen.getByText("Another privileged operation is currently in flight.")
      ).toBeTruthy();
    });
  });

  it("Force stop in the app detail stops the selected app", async () => {
    let killReply = { status: "success" };
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") {
        return Promise.resolve({
          status: "success",
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      }
      if (cmd === "get_waydroid_apps") {
        return Promise.resolve([
          { name: "F-Droid", package: "org.fdroid.fdroid", icon: null },
        ]);
      }
      if (cmd === "waydroid_kill_app") return Promise.resolve(killReply);
      if (cmd === "waydroid_prop_get")
        return Promise.resolve({ status: "success", data: "false" });
      return Promise.resolve({ status: "not_installed" });
    });

    render(<App />);
    const forceStop = await screen.findByRole("button", { name: "Force stop" });
    await waitFor(() => expect(forceStop.disabled).toBe(false));

    fireEvent.click(forceStop);
    await waitFor(() => expect(screen.getByText("App Stopped")).toBeTruthy());
    expect(invoke).toHaveBeenCalledWith("waydroid_kill_app", {
      package: "org.fdroid.fdroid",
    });

    killReply = { status: "error", message: "adb is not installed" };
    fireEvent.click(forceStop);
    await waitFor(() =>
      expect(screen.getByText("Force Stop Failed")).toBeTruthy()
    );
    expect(screen.getByText("adb is not installed")).toBeTruthy();
  });
  const RUNNING = {
    status: "success",
    data: "Session: RUNNING\nContainer: RUNNING\n",
  };

  it("Retry after a failed Enable access asks for consent again", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") return Promise.resolve(RUNNING);
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "touch_mode_status")
        return Promise.resolve({
          enabled: false,
          running: false,
          supported: true,
          uinput_access: false,
          mice: [],
          rule_installed: false,
        });
      if (cmd === "install_touch_mode_access") {
        setTimeout(() => {
          eventListeners["cmd-started"]?.({ payload: { task_id: "task-7" } });
          eventListeners["cmd-finished"]?.({
            payload: {
              task_id: "task-7",
              success: false,
              code: "elevation_denied",
              message: "Authentication was cancelled.",
            },
          });
        }, 10);
        return new Promise((resolve) =>
          setTimeout(() => resolve({ status: "error" }), 30)
        );
      }
      if (cmd === "waydroid_prop_get")
        return Promise.resolve({ status: "success", data: "false" });
      return Promise.resolve({ status: "not_installed" });
    });

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Keymapping" }));
    fireEvent.click(await screen.findByText("Enable access"));
    fireEvent.click(await screen.findByText("Proceed & Authorize Root"));

    const retry = await screen.findByRole("button", { name: /Retry/ });
    fireEvent.click(retry);
    await waitFor(() =>
      expect(
        screen.getByText("Enable Touch Mode Access (Root Required)")
      ).toBeTruthy()
    );
  });

  it("re-reads the multitouch status after a task succeeds", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") return Promise.resolve(RUNNING);
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "install_multitouch") {
        setTimeout(() => {
          eventListeners["cmd-started"]?.({ payload: { task_id: "task-8" } });
          eventListeners["cmd-finished"]?.({
            payload: { task_id: "task-8", success: true, code: null },
          });
        }, 10);
        return new Promise((resolve) =>
          setTimeout(() => resolve({ status: "success" }), 30)
        );
      }
      if (cmd === "waydroid_prop_get")
        return Promise.resolve({ status: "success", data: "false" });
      return Promise.resolve({ status: "not_installed" });
    });

    render(<App />);
    await waitFor(() =>
      expect(screen.getByText("Engine Running")).toBeTruthy()
    );
    const calls = () =>
      invoke.mock.calls.filter(([cmd]) => cmd === "check_multitouch_status")
        .length;
    // Once on mount (works while offline), once when the session is online.
    await waitFor(() => expect(calls()).toBe(2));

    fireEvent.click(screen.getByRole("button", { name: "Install Tools" }));
    fireEvent.click(await screen.findByText(/Declare Touch Features/));
    fireEvent.click(await screen.findByText("Proceed & Authorize Root"));
    await waitFor(() => expect(calls()).toBe(3));
  });

  it("reads Settings properties again once the session is online", async () => {
    let status = { status: "success", data: "Session:\tSTOPPED\n" };
    invoke.mockImplementation((cmd, args) => {
      if (cmd === "waydroid_status") return Promise.resolve(status);
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      if (cmd === "waydroid_prop_get") {
        if (status === RUNNING && args.key === "persist.waydroid.multi_windows")
          return Promise.resolve({ status: "success", data: "true" });
        return status === RUNNING
          ? Promise.resolve({ status: "success", data: "false" })
          : Promise.resolve({ status: "error", message: "session stopped" });
      }
      return Promise.resolve({ status: "not_installed" });
    });

    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    const toggle = await screen.findByRole("switch", {
      name: "Multi-Window Mode",
    });
    expect(toggle.disabled).toBe(true);
    expect(
      screen.getByText(/wait for Android to boot to read and change/)
    ).toBeTruthy();

    status = RUNNING;
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /Start/ }));
    });
    await waitFor(
      () => {
        expect(toggle.disabled).toBe(false);
        expect(toggle.className).toContain("active");
      },
      { timeout: 4000 }
    );
  });
  // Mock for a running session whose Android boot finishes when `booted()`
  // returns true; the app list holds one app.
  const bootingSession = (booted) =>
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status") return Promise.resolve(RUNNING);
      if (cmd === "get_waydroid_apps")
        return Promise.resolve([
          { name: "F-Droid", package: "org.fdroid.fdroid", icon: null },
        ]);
      if (cmd === "waydroid_boot_state")
        return Promise.resolve({
          status: "success",
          data: booted() ? "booted" : "booting",
        });
      if (cmd === "waydroid_density_get")
        return Promise.resolve({ status: "success", data: "225" });
      if (cmd === "waydroid_prop_get")
        return Promise.resolve({ status: "success", data: "" });
      return Promise.resolve({ status: "not_installed" });
    });
  const callsOf = (name) =>
    invoke.mock.calls.filter(([cmd]) => cmd === name).length;

  it("reads Settings again only after Android has booted", async () => {
    let booted = false;
    bootingSession(() => booted);
    render(<App />);
    await screen.findByText("Android Booting…");
    // At "RUNNING" adb and the platform service do not answer yet.
    const beforeBoot = callsOf("waydroid_density_get");
    await new Promise((resolve) => setTimeout(resolve, 300));
    expect(callsOf("waydroid_density_get")).toBe(beforeBoot);

    booted = true;
    await waitFor(
      () => expect(callsOf("waydroid_density_get")).toBe(beforeBoot + 1),
      { timeout: 4000 }
    );
    await screen.findByText("Engine Running");
  }, 10000);

  it("does not launch from the list or the menu while Android boots", async () => {
    bootingSession(() => false);
    render(<App />);
    await screen.findByText("Android Booting…");
    const row = (await screen.findAllByText("F-Droid"))[0];

    fireEvent.doubleClick(row);
    await screen.findByText("Android Is Booting");
    fireEvent.contextMenu(row);
    fireEvent.click(await screen.findByText("Launch App"));
    expect(callsOf("waydroid_launch_app")).toBe(0);
  });

  it("launches the app asked for during the boot once Android is up", async () => {
    let booted = false;
    bootingSession(() => booted);
    render(<App />);
    await screen.findByText("Android Booting…");
    fireEvent.doubleClick((await screen.findAllByText("F-Droid"))[0]);
    await screen.findByText("Android Is Booting");
    expect(callsOf("waydroid_launch_app")).toBe(0);
    booted = true;
    await waitFor(() => expect(callsOf("waydroid_launch_app")).toBe(1), {
      timeout: 6000,
    });
  }, 10000);

  it("starts the engine before launching when it is stopped", async () => {
    // `waydroid app launch` against a stopped session becomes the session and
    // never returns, so it must not be sent until the session is up.
    let running = false;
    const order = [];
    invoke.mockImplementation((cmd) => {
      order.push(cmd);
      if (cmd === "waydroid_status")
        return Promise.resolve(
          running ? RUNNING : { status: "success", data: "Session:\tSTOPPED\n" }
        );
      if (cmd === "waydroid_start") {
        running = true;
        return Promise.resolve({ status: "success" });
      }
      if (cmd === "get_waydroid_apps")
        return Promise.resolve([
          { name: "F-Droid", package: "org.fdroid.fdroid", icon: null },
        ]);
      if (cmd === "waydroid_boot_state")
        return Promise.resolve({ status: "success", data: "booted" });
      if (cmd === "waydroid_prop_get" || cmd === "waydroid_density_get")
        return Promise.resolve({ status: "success", data: "" });
      return Promise.resolve({ status: "success" });
    });
    render(<App />);
    await screen.findByText("Engine Stopped");
    fireEvent.doubleClick((await screen.findAllByText("F-Droid"))[0]);
    await screen.findByText("Starting Engine First");
    await waitFor(() => expect(callsOf("waydroid_launch_app")).toBe(1), {
      timeout: 6000,
    });
    expect(order.indexOf("waydroid_start")).toBeGreaterThan(-1);
    expect(order.indexOf("waydroid_start")).toBeLessThan(
      order.indexOf("waydroid_launch_app")
    );
    expect(callsOf("waydroid_start")).toBe(1);
  }, 10000);

  it("keeps the display profile from being applied while the engine is off", async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status")
        return Promise.resolve({
          status: "success",
          data: "Session:\tSTOPPED\n",
        });
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      return Promise.resolve({ status: "error", message: "session stopped" });
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    const apply = await screen.findByRole("button", {
      name: /^Apply \d+ × \d+/,
    });
    expect(apply.disabled).toBe(true);
    expect(screen.getByText(/to apply a display/)).toBeTruthy();
  });

  it("shows neither the library nor the setup before the first status reply", async () => {
    let reply;
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status")
        return new Promise((resolve) => (reply = resolve));
      if (cmd === "check_host_diagnostics")
        return Promise.resolve({ items: [], package_manager: "pacman" });
      if (cmd === "get_waydroid_apps") return Promise.resolve([]);
      return Promise.resolve({ status: "not_installed" });
    });

    render(<App />);
    expect(screen.getByText("Reading Waydroid status…")).toBeTruthy();
    expect(screen.queryByText("Initialize Android Emulator")).toBeNull();

    await act(async () =>
      reply({ status: "error", code: "not_initialized", message: "no" })
    );
    await waitFor(() =>
      expect(screen.getByText("Initialize Android Emulator")).toBeTruthy()
    );
  });

  it("Save APK stores the selected app's bundle and says where", async () => {
    let reply = {
      status: "success",
      data: "~/Downloads/org.fdroid.fdroid-1.apks",
    };
    invoke.mockImplementation((cmd) => {
      if (cmd === "waydroid_status")
        return Promise.resolve({
          status: "success",
          data: "Session: RUNNING\nContainer: RUNNING\n",
        });
      if (cmd === "get_waydroid_apps")
        return Promise.resolve([
          { name: "F-Droid", package: "org.fdroid.fdroid", icon: null },
        ]);
      if (cmd === "waydroid_backup_apk") return Promise.resolve(reply);
      if (cmd === "waydroid_prop_get")
        return Promise.resolve({ status: "success", data: "false" });
      return Promise.resolve({ status: "not_installed" });
    });

    render(<App />);
    const save = await screen.findByRole("button", { name: "Save APK" });
    await waitFor(() => expect(save.disabled).toBe(false));
    fireEvent.click(save);
    await waitFor(() => expect(screen.getByText("APK Saved")).toBeTruthy());
    expect(invoke).toHaveBeenCalledWith("waydroid_backup_apk", {
      package: "org.fdroid.fdroid",
    });
    expect(screen.getByText(/org\.fdroid\.fdroid-1\.apks/)).toBeTruthy();

    reply = { status: "error", message: "adb is not installed" };
    fireEvent.click(save);
    await waitFor(() =>
      expect(screen.getByText("Save APK Failed")).toBeTruthy()
    );
  });

  it("keeps Save APK busy until the copy has finished", async () => {
    let finish;
    bootingSession(() => true);
    const base = invoke.getMockImplementation();
    invoke.mockImplementation((cmd, args) =>
      cmd === "waydroid_backup_apk"
        ? new Promise((resolve) => (finish = resolve))
        : base(cmd, args)
    );
    render(<App />);
    const save = await screen.findByRole("button", { name: "Save APK" });
    await waitFor(() => expect(save.disabled).toBe(false));

    fireEvent.click(save);
    await waitFor(() => expect(save.disabled).toBe(true));
    expect(save.textContent).toContain("Saving APK");
    fireEvent.click(save);
    expect(callsOf("waydroid_backup_apk")).toBe(1);

    await act(async () => finish({ status: "success", data: "~/x.apks" }));
    await waitFor(() => expect(save.disabled).toBe(false));
  });
});
