import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ConfirmModal from "../components/ui/ConfirmModal";
import TerminalModal from "../components/terminal/TerminalModal";
import ProcessStepper from "../components/terminal/ProcessStepper";
import SetupWizard from "../components/setup/SetupWizard";
import InstallToolsView from "../components/tools/InstallToolsView";
import { TASK_STEP_CONFIGS } from "../components/terminal/stepConfigs";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation((cmd) => {
    if (cmd === "check_host_diagnostics") {
      return Promise.resolve({
        items: [
          {
            key: "network",
            title: "Network Connection",
            status: "error",
            value: "No Route",
            detail: "Cannot reach download mirror.",
            remediation: "Check host network or DNS.",
          },
        ],
        package_manager: "pacman",
      });
    }
    return Promise.resolve({ status: "success" });
  }),
}));

describe("ConfirmModal component", () => {
  it("renders wipe confirmation details and handles cancellation", () => {
    const handleCancel = vi.fn();
    const handleConfirm = vi.fn();

    render(
      <ConfirmModal
        isOpen={true}
        title="Wipe All Android Data?"
        description="This will completely reset the Waydroid container."
        confirmText="Wipe Everything"
        confirmVariant="danger"
        iconType="danger"
        onCancel={handleCancel}
        onConfirm={handleConfirm}
      />
    );

    expect(screen.getByText("Wipe All Android Data?")).toBeTruthy();
    expect(
      screen.getByText("This will completely reset the Waydroid container.")
    ).toBeTruthy();

    const cancelBtn = screen.getByText("Cancel");
    fireEvent.click(cancelBtn);
    expect(handleCancel).toHaveBeenCalledTimes(1);
    expect(handleConfirm).not.toHaveBeenCalled();
  });

  it("renders package preview details and handles confirmation", () => {
    const handleCancel = vi.fn();
    const handleConfirm = vi.fn();

    render(
      <ConfirmModal
        isOpen={true}
        title="Install GApps Dependencies"
        description="The following system packages will be installed via root:"
        packagePreview={{
          packageManager: "pacman",
          packages: ["lxc", "python-pip", "curl"],
          command: "pacman -S --needed lxc python-pip curl",
        }}
        confirmText="Authorize & Install"
        confirmVariant="primary"
        iconType="warning"
        onCancel={handleCancel}
        onConfirm={handleConfirm}
      />
    );

    expect(screen.getByText("lxc")).toBeTruthy();
    expect(screen.getByText("python-pip")).toBeTruthy();
    expect(
      screen.getByText("pacman -S --needed lxc python-pip curl")
    ).toBeTruthy();

    const confirmBtn = screen.getByText("Authorize & Install");
    fireEvent.click(confirmBtn);
    expect(handleConfirm).toHaveBeenCalledTimes(1);
  });
});

describe("TerminalModal and ProcessStepper", () => {
  it("renders terminal logs and triggers cancel when running", () => {
    const handleCancel = vi.fn();
    const handleClose = vi.fn();

    render(
      <TerminalModal
        isOpen={true}
        title="Installing GApps"
        logs={[
          "[SYSTEM] Requesting root access...",
          "[SYSTEM] Elevated shell started",
        ]}
        installing={true}
        taskType="gapps"
        taskResult={null}
        onCancel={handleCancel}
        onClose={handleClose}
      />
    );

    expect(screen.getByText("Installing GApps")).toBeTruthy();
    const cancelButtons = screen.getAllByText("Cancel");
    expect(cancelButtons.length).toBeGreaterThan(0);
    fireEvent.click(cancelButtons[0]);
    expect(handleCancel).toHaveBeenCalledTimes(1);
  });

  it("displays Cancelled status badge when taskResult.code is cancelled (F6 fix)", () => {
    render(
      <TerminalModal
        isOpen={true}
        title="Installing GApps"
        logs={[
          "[SYSTEM] Requesting root access...",
          "[SYSTEM] Operation cancelled",
        ]}
        installing={false}
        taskType="gapps"
        taskResult={{
          success: false,
          code: "cancelled",
          message: "Operation was cancelled by user.",
        }}
        onCancel={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByText("Operation cancelled")).toBeTruthy();
    expect(screen.queryByText("EXECUTION FAILED")).toBeNull();
    expect(
      screen.queryByText(/Root authentication may have been rejected/i)
    ).toBeNull();
  });

  it("displays Finished successfully status badge when taskResult.success is true", () => {
    render(
      <TerminalModal
        isOpen={true}
        title="Installing GApps"
        logs={["[SUCCESS] Installation complete!"]}
        installing={false}
        taskType="gapps"
        taskResult={{
          success: true,
          code: null,
          message: null,
        }}
        onCancel={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByText("Finished successfully")).toBeTruthy();
  });

  it("stepper renders configured steps for wipe task", () => {
    render(
      <ProcessStepper
        steps={TASK_STEP_CONFIGS.wipe}
        activeStepIndex={2}
        isFailed={false}
        isCompleted={false}
      />
    );

    expect(screen.getByText("Root Access")).toBeTruthy();
    expect(screen.getByText("Stop Session")).toBeTruthy();
    expect(screen.getByText("Wiping Storage")).toBeTruthy();
    expect(screen.getByText("Wipe Complete")).toBeTruthy();
  });

  it("stepper keeps reached stages and stops spinning after a cancel", () => {
    const { container } = render(
      <ProcessStepper
        steps={TASK_STEP_CONFIGS.wipe}
        activeStepIndex={2}
        isFailed={false}
        isCancelled={true}
        isCompleted={false}
      />
    );

    expect(container.querySelectorAll(".stepper-node-completed")).toHaveLength(
      2
    );
    expect(container.querySelector(".stepper-node-active")).toBeNull();
    expect(container.querySelector(".stepper-icon-spinner")).toBeNull();
  });
});

describe("SetupWizard preflight and diagnostic blocking", () => {
  it("renders host diagnostics and blocks initialization when network/disk fails", async () => {
    const handleInit = vi.fn();
    render(<SetupWizard onInitWaydroid={handleInit} installing={false} />);

    await waitFor(() => {
      expect(screen.getByText("Network Connection")).toBeTruthy();
    });
    expect(screen.getByText("Check host network or DNS.")).toBeTruthy();
    expect(
      screen.getByText("Resolve the blocking items above before initializing.")
    ).toBeTruthy();

    const initBtn = screen.getByRole("button", {
      name: /Initialize Android Emulator/i,
    });
    expect(initBtn.hasAttribute("disabled")).toBe(true);
  });
});

describe("InstallToolsView marker states", () => {
  it("shows an unverified install as reinstallable, not as installed", () => {
    render(
      <InstallToolsView
        gappsStatus="pending"
        houdiniStatus="installed"
        multitouchStatus="not_installed"
        installing={false}
        onInstallGApps={vi.fn()}
        onInstallHoudini={vi.fn()}
        onInstallMultitouch={vi.fn()}
      />
    );
    expect(screen.getByText("Unverified")).toBeTruthy();
    const disabled = screen
      .getAllByRole("button")
      .filter((button) => button.disabled);
    // Only the verified Houdini install locks its button.
    expect(disabled.length).toBe(1);
  });
});
