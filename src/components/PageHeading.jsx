import React from "react";

const HEADINGS = {
  home: { title: "My Applications" },
  setup: { title: "Waydroid Setup" },
  // The h1 is the tab's own name: one name per page.
  keymap: {
    title: "Keymapping",
    subtitle: "Touch mode and the Phantom keymapper",
  },
  install: {
    title: "Install Tools",
    subtitle: "Google Play, ARM translation and Play Store compatibility",
  },
  settings: {
    title: "Settings",
    subtitle: "Display, Android properties, diagnostics and logs",
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
