import React from "react";

const HEADINGS = {
  home: { title: "My Applications" },
  setup: { title: "Waydroid Setup" },
  keymap: {
    title: "Smart Keymapping",
    subtitle: "Hardware keyboard & mouse touch simulation via Phantom",
  },
  install: {
    title: "Addons & Tool Injections",
    subtitle: "Google Play Services & ARM Translation (libhoudini)",
  },
  settings: {
    title: "Emulator Configuration",
    subtitle: "Display settings, system properties & diagnostics",
  },
};

/**
 * The page's h1. My Apps and the setup screen keep it for screen readers
 * only: their own headers already say what the page is.
 */
export default function PageHeading({ activeTab }) {
  const info = HEADINGS[activeTab] ?? { title: "ArchDroid" };
  if (activeTab === "home" || activeTab === "setup") {
    return <h1 className="sr-only">{info.title}</h1>;
  }
  return (
    <div className="page-heading">
      <h1>{info.title}</h1>
      {info.subtitle && <p>{info.subtitle}</p>}
    </div>
  );
}
