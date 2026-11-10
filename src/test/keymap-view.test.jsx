import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import KeymapView from "../components/keymap/KeymapView";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const touchMode = { status: null, changing: false, refresh: vi.fn() };

function renderView(props = {}) {
  return render(
    <KeymapView
      onRunPhantom={vi.fn()}
      onInstallPhantom={vi.fn()}
      installing={false}
      touchMode={touchMode}
      onToggleTouchMode={vi.fn()}
      onEnableTouchAccess={vi.fn()}
      {...props}
    />
  );
}

const launch = () => screen.getByRole("button", { name: /Launch Keymapper/ });

describe("KeymapView Phantom status", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("keeps Launch disabled until the first check answers", () => {
    invoke.mockReturnValue(new Promise(() => {}));
    renderView();
    expect(screen.getAllByText("Checking…").length).toBeGreaterThan(0);
    expect(launch().disabled).toBe(true);
  });

  it("offers Install instead of Launch when Phantom is missing", async () => {
    invoke.mockResolvedValue({ status: "not_installed" });
    renderView();
    await screen.findByText("Not Installed");
    expect(launch().disabled).toBe(true);
    expect(launch().getAttribute("aria-describedby")).toBe(
      "phantom-launch-hint"
    );
    expect(
      screen.getByRole("button", { name: /Install Phantom/ })
    ).toBeTruthy();
  });

  it("enables Launch when Phantom is installed", async () => {
    invoke.mockResolvedValue({ status: "installed" });
    renderView();
    await screen.findByText("Installed");
    expect(launch().disabled).toBe(false);
    expect(screen.getByRole("button", { name: /Update Phantom/ })).toBeTruthy();
  });

  it("checks again when a task ends", async () => {
    invoke.mockResolvedValue({ status: "not_installed" });
    const { rerender } = renderView({ installing: true });
    await screen.findByText("Not Installed");

    invoke.mockResolvedValue({ status: "installed" });
    rerender(
      <KeymapView
        onRunPhantom={vi.fn()}
        onInstallPhantom={vi.fn()}
        installing={false}
        touchMode={touchMode}
        onToggleTouchMode={vi.fn()}
        onEnableTouchAccess={vi.fn()}
      />
    );
    await waitFor(() => expect(launch().disabled).toBe(false));
    expect(invoke).toHaveBeenCalledWith("phantom_status");
  });
});
