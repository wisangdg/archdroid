import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import DiagnosticBundlePanel from "../components/settings/DiagnosticBundlePanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("DiagnosticBundlePanel", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("creates a bundle, shows where it was saved, and copies its text", async () => {
    invoke.mockResolvedValue({
      path: "~/Downloads/archdroid-diagnostics-20261101010203.txt",
      content: "ArchDroid diagnostic bundle\nVersion: archdroid 0.1.1",
    });
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    render(<DiagnosticBundlePanel />);
    expect(screen.getByText("Copy").disabled).toBe(true);

    fireEvent.click(screen.getByText("Create Bundle"));

    await waitFor(() => {
      expect(
        screen.getByText(/Saved to ~\/Downloads\/archdroid-diagnostics-/)
      ).toBeTruthy();
    });
    expect(invoke).toHaveBeenCalledWith("diagnostic_bundle_create");
    expect(
      screen.getByLabelText("Diagnostic bundle preview").textContent
    ).toContain("Version: archdroid 0.1.1");

    fireEvent.click(screen.getByText("Copy"));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(
        "ArchDroid diagnostic bundle\nVersion: archdroid 0.1.1"
      );
    });
  });

  it("reports a backend rejection instead of a stale success", async () => {
    invoke.mockRejectedValue({
      status: "error",
      message: "Could not find a Downloads or home folder.",
    });

    render(<DiagnosticBundlePanel />);
    fireEvent.click(screen.getByText("Create Bundle"));

    await waitFor(() => {
      expect(
        screen.getByText(
          "Could not create the bundle: Could not find a Downloads or home folder."
        )
      ).toBeTruthy();
    });
    expect(screen.getByText("Copy").disabled).toBe(true);
  });
});
