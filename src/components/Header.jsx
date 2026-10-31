import React from "react";
import { Play, Square, RotateCw } from "lucide-react";

export default function Header({
  activeTab,
  status,
  isInitialized,
  appCount,
  onStart,
  onStop,
  onRestart,
  isOperating,
  busy = false,
}) {
  const getHeaderInfo = () => {
    switch (activeTab) {
      case "home":
        return {
          title: "My Applications",
          subtitle: `${appCount} application${appCount === 1 ? "" : "s"} ready to launch`,
        };
      case "keymap":
        return {
          title: "Smart Keymapping",
          subtitle: "Hardware keyboard & mouse touch simulation via Phantom",
        };
      case "install":
        return {
          title: "Addons & Tool Injections",
          subtitle: "Google Play Services & ARM Translation (libhoudini)",
        };
      case "settings":
        return {
          title: "Emulator Configuration",
          subtitle: "Display settings, system properties & diagnostics",
        };
      default:
        return { title: "ArchDroid", subtitle: "Waydroid Manager" };
    }
  };

  const info = getHeaderInfo();

  return (
    <header className="header">
      <div className="header-titles">
        <h1 className="header-heading">{info.title}</h1>
        <p className="header-subheading">{info.subtitle}</p>
      </div>

      <div className="header-controls">
        {/* Status Badge */}
        <div className={`status-badge ${status === "online" ? "status-online" : "status-offline"}`}>
          <div className="status-dot"></div>
          <span className="status-text">{status === "online" ? "Engine Running" : "Engine Stopped"}</span>
        </div>

        {/* Engine Control Actions */}
        {isInitialized && (
          <div className="header-actions">
            {status === "offline" ? (
              <button
                className="btn btn-primary btn-glow"
                onClick={onStart}
                disabled={isOperating || busy}
                title="Start Waydroid Session"
              >
                <Play size={17} fill="currentColor" />
                <span>Start Engine</span>
              </button>
            ) : (
              <>
                <button
                  className="btn btn-outline"
                  onClick={onRestart}
                  disabled={isOperating || busy}
                  title="Restart Waydroid Session"
                >
                  <RotateCw size={16} />
                  <span>Restart</span>
                </button>
                <button
                  className="btn btn-danger"
                  onClick={onStop}
                  disabled={isOperating || busy}
                  title="Stop Waydroid Session"
                >
                  <Square size={16} fill="currentColor" />
                  <span>Stop Engine</span>
                </button>
              </>
            )}
          </div>
        )}
      </div>
    </header>
  );
}
