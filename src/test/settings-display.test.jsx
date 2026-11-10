import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ResolutionPresets from "../components/settings/ResolutionPresets";
import SettingsView from "../components/settings/SettingsView";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => ({ status: "success", data: "" })),
}));

const current = {
  currentWidth: "1920",
  currentHeight: "1080",
  currentDpi: "280",
};

describe("Display profile: restart after Apply", () => {
  it("offers Restart only after a new resolution was saved", async () => {
    const onApplyResolution = vi.fn(async () => true);
    const { rerender } = render(
      <ResolutionPresets
        {...current}
        engineRunning
        onApplyResolution={onApplyResolution}
        onRestartEngine={() => {}}
      />
    );
    // Nothing to restart for: the saved profile is the running one.
    expect(screen.queryByRole("button", { name: /Restart Engine/ })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /Standard 720p/ }));
    // Selecting is not saving; a restart would change nothing yet.
    expect(screen.queryByRole("button", { name: /Restart Engine/ })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /^Apply 1280 × 720/ }));
    await waitFor(() => expect(onApplyResolution).toHaveBeenCalled());
    rerender(
      <ResolutionPresets
        currentWidth="1280"
        currentHeight="720"
        currentDpi="240"
        engineRunning
        onApplyResolution={onApplyResolution}
        onRestartEngine={() => {}}
      />
    );
    expect(screen.getByRole("button", { name: /Restart Engine/ })).toBeTruthy();
    expect(screen.getByRole("status").textContent).toMatch(
      /Restart the engine/
    );
    expect(screen.queryByText("Saved & Active")).toBeNull();

    // The session went down and came back: the resolution is in use now.
    rerender(
      <ResolutionPresets
        currentWidth="1280"
        currentHeight="720"
        currentDpi="240"
        engineRunning={false}
        onApplyResolution={onApplyResolution}
      />
    );
    rerender(
      <ResolutionPresets
        currentWidth="1280"
        currentHeight="720"
        currentDpi="240"
        engineRunning
        onApplyResolution={onApplyResolution}
        onRestartEngine={() => {}}
      />
    );
    expect(screen.queryByRole("button", { name: /Restart Engine/ })).toBeNull();
    expect(screen.getByText("Saved & Active")).toBeTruthy();
  });

  it("asks for no restart when the write failed", async () => {
    const onApplyResolution = vi.fn(async () => false);
    render(
      <ResolutionPresets
        {...current}
        engineRunning
        onApplyResolution={onApplyResolution}
        onRestartEngine={() => {}}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: /Standard 720p/ }));
    fireEvent.click(screen.getByRole("button", { name: /^Apply 1280 × 720/ }));
    await waitFor(() => expect(onApplyResolution).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: /Restart Engine/ })).toBeNull();
  });
});

describe("Advanced property form", () => {
  it("is disabled until Android has booted", () => {
    const { rerender } = render(<SettingsView engineRunning={false} />);
    expect(
      screen.getByRole("button", { name: "Apply Property" }).disabled
    ).toBe(true);
    rerender(<SettingsView engineRunning />);
    expect(
      screen.getByRole("button", { name: "Apply Property" }).disabled
    ).toBe(false);
  });
});
