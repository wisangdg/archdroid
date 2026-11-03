import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import MyAppsView from "../components/apps/MyAppsView";
import TopBar from "../components/TopBar";

const APPS = [
  { name: "Plants vs Zombies 2", package: "com.ea.game.pvz2_row", icon: null },
  { name: "Settings", package: "com.android.settings", icon: null },
  { name: "Clock", package: "com.android.deskclock", icon: null },
];

function renderView(props = {}) {
  const handlers = {
    onLaunchApp: vi.fn(),
    onForceStopApp: vi.fn(),
    onOpenKeymap: vi.fn(),
    onInstallApk: vi.fn(),
    onUninstallApp: vi.fn(),
    onCopyPackage: vi.fn(),
  };
  render(<MyAppsView apps={APPS} engineOnline {...handlers} {...props} />);
  return handlers;
}

describe("Control Center: My Apps", () => {
  it("shows the first app in the detail panel and the app count", () => {
    renderView();
    expect(
      screen.getByRole("heading", { level: 2, name: "Plants vs Zombies 2" })
    ).toBeTruthy();
    expect(screen.getByText("3 applications ready to launch")).toBeTruthy();
    expect(screen.getByText("User app")).toBeTruthy();
  });

  it("selecting a row moves the detail panel to that app", () => {
    renderView();
    fireEvent.click(screen.getByRole("button", { name: /^Clock,/ }));
    expect(
      screen.getByRole("heading", { level: 2, name: "Clock" })
    ).toBeTruthy();
    expect(screen.getByText("System app")).toBeTruthy();
  });

  it("runs every detail action on the selected app", () => {
    const h = renderView();
    fireEvent.click(screen.getByRole("button", { name: /^Settings,/ }));
    const panel = screen
      .getByRole("heading", { level: 2, name: "Settings" })
      .closest("section");
    const inPanel = within(panel);
    fireEvent.click(inPanel.getByRole("button", { name: "Launch" }));
    fireEvent.click(inPanel.getByRole("button", { name: "Force stop" }));
    fireEvent.click(inPanel.getByRole("button", { name: "Keymapping" }));
    fireEvent.click(inPanel.getByRole("button", { name: "Copy package ID" }));
    fireEvent.click(inPanel.getByRole("button", { name: "Uninstall" }));
    expect(h.onLaunchApp).toHaveBeenCalledWith("com.android.settings");
    expect(h.onForceStopApp).toHaveBeenCalledWith(
      "com.android.settings",
      "Settings"
    );
    expect(h.onOpenKeymap).toHaveBeenCalledTimes(1);
    expect(h.onCopyPackage).toHaveBeenCalledWith("com.android.settings");
    expect(h.onUninstallApp).toHaveBeenCalledWith(
      "com.android.settings",
      "Settings"
    );
  });

  it("disables Force stop while the engine is stopped", () => {
    renderView({ engineOnline: false });
    expect(screen.getByRole("button", { name: "Force stop" }).disabled).toBe(
      true
    );
    expect(screen.getByRole("button", { name: "Launch" }).disabled).toBe(false);
  });

  it("falls back to the first visible app when the selection is filtered out", () => {
    renderView();
    fireEvent.click(screen.getByRole("button", { name: /^Clock,/ }));
    fireEvent.click(screen.getByRole("button", { name: /^User/ }));
    expect(
      screen.getByRole("heading", { level: 2, name: "Plants vs Zombies 2" })
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Clock,/ })).toBeNull();
  });

  it("the arrow keys move the selection through the list", () => {
    renderView();
    const first = screen.getByRole("button", { name: /^Plants vs Zombies 2,/ });
    fireEvent.keyDown(first, { key: "ArrowDown" });
    expect(
      screen.getByRole("heading", { level: 2, name: "Settings" })
    ).toBeTruthy();
  });

  it("explains an empty library", () => {
    renderView({ apps: [] });
    expect(screen.getByText("No Android apps installed yet")).toBeTruthy();
    expect(
      screen.getByText("Select an app in the list to see its actions.")
    ).toBeTruthy();
  });
});

describe("Control Center: top bar", () => {
  const base = {
    activeTab: "home",
    setActiveTab: vi.fn(),
    appCount: 3,
    isInitialized: true,
    isOperating: false,
    onStart: vi.fn(),
    onStop: vi.fn(),
    onRestart: vi.fn(),
    onOpenSharedFolder: vi.fn(),
  };

  it("marks the current section and switches sections", () => {
    const setActiveTab = vi.fn();
    render(<TopBar {...base} status="offline" setActiveTab={setActiveTab} />);
    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(
      within(nav)
        .getByRole("button", { name: /My Apps/ })
        .getAttribute("aria-current")
    ).toBe("page");
    fireEvent.click(within(nav).getByRole("button", { name: "Settings" }));
    expect(setActiveTab).toHaveBeenCalledWith("settings");
  });

  it("offers Start Engine while stopped and Stop/Restart while running", () => {
    const { rerender } = render(<TopBar {...base} status="offline" />);
    expect(screen.getByRole("status").textContent).toBe("Engine Stopped");
    expect(screen.getByRole("button", { name: "Start Engine" })).toBeTruthy();
    rerender(<TopBar {...base} status="online" />);
    expect(screen.getByRole("status").textContent).toBe("Engine Running");
    expect(screen.getByRole("button", { name: "Stop Engine" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Restart" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Start Engine" })).toBeNull();
  });

  it("hides engine controls before Waydroid is initialized", () => {
    render(<TopBar {...base} status="offline" isInitialized={false} />);
    expect(screen.queryByRole("button", { name: "Start Engine" })).toBeNull();
  });
});
