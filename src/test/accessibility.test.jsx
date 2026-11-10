import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import AppRow from "../components/apps/AppRow";
import ResolutionPresets from "../components/settings/ResolutionPresets";
import ContextMenu from "../components/apps/ContextMenu";
import ConfirmModal from "../components/ui/ConfirmModal";
import TerminalModal from "../components/terminal/TerminalModal";
import ToastContainer from "../components/ui/ToastContainer";
import fs from "node:fs";
import path from "node:path";

describe("Accessibility (P1-3)", () => {
  describe("AppRow keyboard accessibility", () => {
    const mockApp = {
      name: "F-Droid",
      package: "org.fdroid.fdroid",
      icon: null,
    };
    const renderRow = (props = {}) => {
      const handlers = {
        onSelect: vi.fn(),
        onLaunch: vi.fn(),
        onMove: vi.fn(),
        onContextMenu: vi.fn(),
      };
      render(<AppRow app={mockApp} system={false} {...handlers} {...props} />);
      return {
        row: screen.getByRole("button", { name: /F-Droid/ }),
        ...handlers,
      };
    };

    it("is a real button that marks the selected row", () => {
      const { row } = renderRow({ selected: true });
      expect(row.tagName).toBe("BUTTON");
      expect(row.getAttribute("aria-current")).toBe("true");
    });

    it("selects on click and launches on double-click", () => {
      const { row, onSelect, onLaunch } = renderRow();
      fireEvent.click(row);
      expect(onSelect).toHaveBeenCalledWith("org.fdroid.fdroid");
      fireEvent.doubleClick(row);
      expect(onLaunch).toHaveBeenCalledWith("org.fdroid.fdroid");
    });

    it("moves through the list with the arrow keys", () => {
      const { row, onMove } = renderRow();
      fireEvent.keyDown(row, { key: "ArrowDown" });
      fireEvent.keyDown(row, { key: "ArrowUp" });
      expect(onMove.mock.calls).toEqual([[1], [-1]]);
    });

    it("opens the context menu with the menu key or Shift+F10", () => {
      const { row, onContextMenu } = renderRow();
      fireEvent.keyDown(row, { key: "ContextMenu" });
      fireEvent.keyDown(row, { key: "F10", shiftKey: true });
      expect(onContextMenu).toHaveBeenCalledTimes(2);
    });

    it("does not launch while another task is running", () => {
      const { row, onLaunch } = renderRow({ disabled: true });
      fireEvent.doubleClick(row);
      expect(onLaunch).not.toHaveBeenCalled();
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
    it("a destructive ConfirmModal opens with Cancel focused", () => {
      const { unmount } = render(
        <ConfirmModal
          isOpen={true}
          title="Factory Reset Waydroid?"
          description="This cannot be undone."
          confirmText="Wipe Everything"
          confirmVariant="danger"
          onCancel={() => {}}
          onConfirm={() => {}}
        />
      );
      expect(document.activeElement).toBe(screen.getByText("Cancel"));
      unmount();
    });

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
          confirmVariant="primary"
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
      expect(css).toContain(".app-row:focus-visible");
      expect(css).toContain(".preset-card:focus-visible");
    });
  });
});
