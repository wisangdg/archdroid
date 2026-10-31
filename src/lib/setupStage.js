/**
 * First-run stage of the Waydroid runtime, derived from the host preflight
 * report (and whether an init task is running).
 */
export function setupStage(report, installing) {
  if (installing) return "initializing";
  const items = report?.items ?? [];
  const byKey = (key) => items.find((item) => item.key === key);
  if (byKey("waydroid")?.status === "error") return "missing";
  if (byKey("waydroid_image")?.status === "ok") return "ready";
  return "uninitialized";
}

export const STAGE_COPY = {
  missing: {
    title: "Waydroid Is Not Installed",
    description:
      "ArchDroid could not find the waydroid command. Install it with the command shown in the preflight checks below, then re-check.",
  },
  uninitialized: {
    title: "Android System Image Required",
    description:
      "Your Linux machine is ready to host Waydroid, but the base Android system container has not been initialized yet. Click below to download the official LineageOS image (~1 GB) and configure your emulator automatically.",
  },
  initializing: {
    title: "Initializing Android System",
    description:
      "Downloading and configuring the Android image. This can take several minutes; keep this window open.",
  },
  ready: {
    title: "Android System Image Is Ready",
    description:
      "The image is already initialized. Re-check to open the dashboard.",
  },
};
