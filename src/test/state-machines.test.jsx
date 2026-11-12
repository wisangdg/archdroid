import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import TopBar from "../components/TopBar";
import TerminalModal from "../components/terminal/TerminalModal";

const bar = {
  activeTab: "home",
  setActiveTab: vi.fn(),
  appCount: 0,
  status: "offline",
  isInitialized: true,
  isOperating: false,
  onStart: vi.fn(),
  onStop: vi.fn(),
  onRestart: vi.fn(),
  onOpenSharedFolder: vi.fn(),
};

describe("engine phase in the top bar", () => {
  it.each([
    ["starting", "Starting Engine…", "status-pending"],
    ["stopping", "Stopping Engine…", "status-pending"],
    ["restarting", "Restarting Engine…", "status-pending"],
    ["unreachable", "Waydroid Not Responding", "status-offline"],
    ["booting", "Android Booting…", "status-online"],
  ])("%s reads %s", (phase, label, tone) => {
    render(<TopBar {...bar} phase={phase} />);
    const badge = screen.getByRole("status");
    expect(badge.textContent).toBe(label);
    expect(badge.dataset.enginePhase).toBe(phase);
    expect(badge.classList.contains(tone)).toBe(true);
  });
});

describe("terminal outcome", () => {
  const modal = {
    isOpen: true,
    title: "Installing GApps",
    taskType: "gapps",
    logs: ["[STEP] deps", "line"],
    onClose: vi.fn(),
    onRetry: vi.fn(),
    onCancel: vi.fn(),
  };

  it("styles a cancelled task as cancelled everywhere, without Retry", () => {
    render(
      <TerminalModal
        {...modal}
        installing={false}
        taskResult={{ success: false, code: "cancelled" }}
      />
    );
    const dialog = screen.getByRole("dialog");
    expect(dialog.dataset.taskOutcome).toBe("cancelled");
    expect(screen.getByText("Operation cancelled")).toBeTruthy();
    expect(
      dialog.querySelector(".stepper-status-card.status-cancelled")
    ).toBeTruthy();
    expect(
      dialog.querySelector(".terminal-icon-badge.badge-cancelled")
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: /retry/i })).toBeNull();
  });

  it("offers Retry only for a failed task", () => {
    render(
      <TerminalModal
        {...modal}
        installing={false}
        taskResult={{ success: false, code: "timeout" }}
      />
    );
    expect(screen.getByRole("dialog").dataset.taskOutcome).toBe("failed");
    expect(screen.getByRole("button", { name: /retry/i })).toBeTruthy();
  });

  it("is running while the task runs, whatever the log says", () => {
    render(
      <TerminalModal
        {...modal}
        logs={["[ERROR] transient, retrying"]}
        installing={true}
      />
    );
    expect(screen.getByRole("dialog").dataset.taskOutcome).toBe("running");
    expect(screen.getByText("Running…")).toBeTruthy();
  });
});
