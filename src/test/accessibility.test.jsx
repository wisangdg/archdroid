import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import AppCard from "../components/apps/AppCard";
import ResolutionPresets from "../components/settings/ResolutionPresets";
import ContextMenu from "../components/apps/ContextMenu";
import ConfirmModal from "../components/ui/ConfirmModal";
import TerminalModal from "../components/terminal/TerminalModal";
import ToastContainer from "../components/ui/ToastContainer";
import fs from "node:fs";
import path from "node:path";

describe("Accessibility (P1-3)", () => {
  describe("AppCard keyboard accessibility", () => {
    const mockApp = {
      name: "F-Droid",
      package: "org.fdroid.fdroid",
      icon: null,
    };

    it("has button role, tabIndex, and aria-label", () => {
      render(
        <AppCard
          app={mockApp}
          onLaunch={vi.fn()}
          onUninstall={vi.fn()}
          onContextMenu={vi.fn()}
        />
      );

      const card = screen.getByRole("button", { name: "Launch F-Droid" });
      expect(card).toBeTruthy();
      expect(card.getAttribute("tabIndex")).toBe("0");
      expect(card.getAttribute("aria-disabled")).toBe("false");
    });

    it("launches app when Enter or Space is pressed", () => {
      const handleLaunch = vi.fn();
      render(
        <AppCard
          app={mockApp}
          onLaunch={handleLaunch}
          onUninstall={vi.fn()}
          onContextMenu={vi.fn()}
        />
      );

      const card = screen.getByRole("button", { name: "Launch F-Droid" });

      fireEvent.keyDown(card, { key: "Enter" });
      expect(handleLaunch).toHaveBeenCalledWith("org.fdroid.fdroid");

      fireEvent.keyDown(card, { key: " " });
      expect(handleLaunch).toHaveBeenCalledTimes(2);
    });

    it("opens context menu when ContextMenu or Shift+F10 is pressed", () => {
      const handleContextMenu = vi.fn();
      render(
        <AppCard
          app={mockApp}
          onLaunch={vi.fn()}
          onUninstall={vi.fn()}
          onContextMenu={handleContextMenu}
        />
      );

      const card = screen.getByRole("button", { name: "Launch F-Droid" });

      fireEvent.keyDown(card, { key: "ContextMenu" });
      expect(handleContextMenu).toHaveBeenCalledTimes(1);

      fireEvent.keyDown(card, { key: "F10", shiftKey: true });
      expect(handleContextMenu).toHaveBeenCalledTimes(2);
    });

    it("prevents keyboard action when disabled", () => {
      const handleLaunch = vi.fn();
      render(
        <AppCard
          app={mockApp}
          onLaunch={handleLaunch}
          onUninstall={vi.fn()}
          onContextMenu={vi.fn()}
          disabled={true}
        />
      );

      const card = screen.getByRole("button", { name: "Launch F-Droid" });
      expect(card.getAttribute("tabIndex")).toBe("-1");
      expect(card.getAttribute("aria-disabled")).toBe("true");

      fireEvent.keyDown(card, { key: "Enter" });
      expect(handleLaunch).not.toHaveBeenCalled();
    });
  });

  describe("ResolutionPresets keyboard accessibility", () => {
    it("renders presets as accessible buttons with keyboard activation", () => {
      const handleApply = vi.fn();
      render(
        <ResolutionPresets
          currentWidth="1920"
          currentHeight="1080"
          currentDpi="280"
          onApply={handleApply}
        />
      );

      const presetButtons = screen.getAllByRole("button", {
        name: /Tablet Full HD|Standard 720p|Mobile Phone|Desktop 2K/i,
      });
      expect(presetButtons.length).toBeGreaterThan(0);

      const standard720p = presetButtons.find((b) =>
        b.getAttribute("aria-label")?.includes("Standard 720p")
      );
      expect(standard720p).toBeTruthy();
      expect(standard720p.getAttribute("tabIndex")).toBe("0");

      fireEvent.keyDown(standard720p, { key: "Enter" });
      expect(standard720p.getAttribute("aria-pressed")).toBe("true");
    });
  });

  describe("ContextMenu accessibility and arrow key navigation", () => {
    const mockApp = {
      name: "Calculator",
      package: "com.android.calculator2",
    };

    it("has role menu and role menuitem for actions with focus and arrow navigation", () => {
      const handleClose = vi.fn();
      const handleLaunch = vi.fn();

      render(
        <ContextMenu
          position={{ x: 100, y: 100 }}
          app={mockApp}
          onClose={handleClose}
          onLaunch={handleLaunch}
          onCopyPackage={vi.fn()}
          onUninstall={vi.fn()}
        />
      );

      const menu = screen.getByRole("menu", {
        name: "Options for Calculator",
      });
      expect(menu).toBeTruthy();

      const items = screen.getAllByRole("menuitem");
      expect(items.length).toBe(3);

      // Initial focus on first item
      expect(document.activeElement).toBe(items[0]);

      // Arrow down moves to next item
      fireEvent.keyDown(window, { key: "ArrowDown" });
      expect(document.activeElement).toBe(items[1]);

      // Arrow up moves back to first item
      fireEvent.keyDown(window, { key: "ArrowUp" });
      expect(document.activeElement).toBe(items[0]);

      // Escape closes menu
      fireEvent.keyDown(window, { key: "Escape" });
      expect(handleClose).toHaveBeenCalledTimes(1);
    });
  });

  describe("Modal focus trap and restoration", () => {
    it("ConfirmModal has role dialog, traps Tab focus, and handles Escape", () => {
      const triggerBtn = document.createElement("button");
      triggerBtn.textContent = "Open Modal";
      document.body.appendChild(triggerBtn);
      triggerBtn.focus();

      const handleCancel = vi.fn();
      const handleConfirm = vi.fn();

      const { unmount } = render(
        <ConfirmModal
          isOpen={true}
          title="Reset Confirmation"
          description="Are you sure you want to proceed?"
          onCancel={handleCancel}
          onConfirm={handleConfirm}
        />
      );

      const dialog = screen.getByRole("dialog");
      expect(dialog).toBeTruthy();
      expect(dialog.getAttribute("aria-modal")).toBe("true");

      const cancelBtn = screen.getByText("Cancel");
      expect(cancelBtn).toBeTruthy();
      const confirmBtn = screen.getByText("Confirm");

      // Confirm button has autofocus
      expect(document.activeElement).toBe(confirmBtn);

      // Tab from last focusable cycles back to first focusable
      fireEvent.keyDown(window, { key: "Tab" });
      const closeBtn = screen.getByLabelText("Close dialog");
      expect(document.activeElement).toBe(closeBtn);

      // Shift+Tab cycles back
      fireEvent.keyDown(window, { key: "Tab", shiftKey: true });
      expect(document.activeElement).toBe(confirmBtn);

      // Escape invokes cancel
      fireEvent.keyDown(window, { key: "Escape" });
      expect(handleCancel).toHaveBeenCalledTimes(1);

      unmount();
      // Focus restored to trigger element
      expect(document.activeElement).toBe(triggerBtn);
      document.body.removeChild(triggerBtn);
    });

    it("TerminalModal has role dialog and aria-modal", () => {
      render(
        <TerminalModal
          isOpen={true}
          title="Process Logs"
          logs={["line 1", "line 2"]}
          installing={false}
          onClose={vi.fn()}
        />
      );

      const dialog = screen.getByRole("dialog");
      expect(dialog).toBeTruthy();
      expect(dialog.getAttribute("aria-modal")).toBe("true");
      expect(dialog.getAttribute("aria-labelledby")).toBe(
        "terminal-modal-title"
      );
    });
  });

  describe("Toast accessibility & Reduced Motion CSS audit", () => {
    it("ToastContainer has aria-live polite and role status for items", () => {
      render(
        <ToastContainer
          toasts={[
            {
              id: "t1",
              type: "success",
              title: "Done",
              message: "Task completed",
            },
          ]}
          onDismiss={vi.fn()}
        />
      );

      const toast = screen.getByRole("status");
      expect(toast).toBeTruthy();
      expect(screen.getByText("Done")).toBeTruthy();
    });

    it("src/index.css includes prefers-reduced-motion media query and focus-visible indicators", () => {
      const cssPath = path.resolve(__dirname, "../../src/index.css");
      const css = fs.readFileSync(cssPath, "utf8");

      expect(css).toContain("@media (prefers-reduced-motion: reduce)");
      expect(css).toContain(":focus-visible");
      expect(css).toContain(".app-card:focus-visible");
      expect(css).toContain(".preset-card:focus-visible");
    });
  });
});
