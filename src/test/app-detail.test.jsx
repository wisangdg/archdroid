import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import AppDetail, { formatBytes } from "../components/apps/AppDetail";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const APP = { name: "Etar", package: "org.lineageos.etar", icon: null };

function renderDetail(props = {}) {
  return render(
    <AppDetail
      app={APP}
      system={false}
      engineOnline
      onLaunch={vi.fn()}
      onForceStop={vi.fn()}
      onOpenKeymap={vi.fn()}
      onCopyPackage={vi.fn()}
      onUninstall={vi.fn()}
      {...props}
    />
  );
}

describe("AppDetail facts", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("shows the version, APK size and update date Android reports", async () => {
    invoke.mockResolvedValue({
      version: "13",
      updated: "2026-09-27 20:28:18",
      apk_bytes: 6636491,
    });
    renderDetail();
    expect(await screen.findByText("13")).toBeTruthy();
    expect(screen.getByText("6.6 MB")).toBeTruthy();
    expect(screen.getByText("2026-09-27")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("app_info", {
      package: "org.lineageos.etar",
    });
  });

  it("does not ask Android while the engine is stopped or booting", () => {
    renderDetail({ engineOnline: false });
    expect(screen.getByText("Shown while Android runs")).toBeTruthy();
    renderDetail({ booting: true });
    expect(invoke).not.toHaveBeenCalled();
  });

  it("says so when Android does not answer", async () => {
    invoke.mockRejectedValue(new Error("frozen"));
    renderDetail();
    expect(await screen.findByText("Android did not report them")).toBeTruthy();
  });

  it("formats sizes in decimal units", () => {
    expect(formatBytes(null)).toBe(null);
    expect(formatBytes(512)).toBe("1 KB");
    expect(formatBytes(53_477_376)).toBe("53.5 MB");
    expect(formatBytes(1_250_000_000)).toBe("1.25 GB");
  });
});
