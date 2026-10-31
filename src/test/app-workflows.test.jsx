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
  listen: vi.fn((event, cb) => {
    eventListeners[event] = cb;
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
      expect(screen.getByText("F-Droid")).toBeTruthy();
      expect(screen.getByText("Online")).toBeTruthy();
    });
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
        screen.getByText("Only .apk packages can be installed via drag & drop.")
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
    const toolsTab = screen.getByText("Install Tools");
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

    const toolsTab = screen.getByText("Install Tools");
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

    const toolsTab = screen.getByText("Install Tools");
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

    fireEvent.click(screen.getByText("Install Tools"));

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

    fireEvent.click(screen.getByText("Settings"));

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
      expect(screen.getByText("Factory Reset in Progress...")).toBeTruthy();
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

    fireEvent.click(screen.getByText("Install Tools"));

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
});
