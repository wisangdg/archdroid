import React from "react";
import pkg from "../../package.json";
import {
  MonitorPlay,
  Gamepad2,
  Download,
  Settings,
  FolderOpen,
  Play,
  Square,
  RotateCw,
} from "lucide-react";

const NAV_ITEMS = [
  { id: "home", label: "My Apps", icon: MonitorPlay },
  { id: "keymap", label: "Keymapping", icon: Gamepad2 },
  { id: "install", label: "Install Tools", icon: Download },
  { id: "settings", label: "Settings", icon: Settings },
];

/**
 * Control Center top bar: brand, the four sections, the shared folder, and the
 * engine state with its controls, which stay visible on every page.
 */
export default function TopBar({
  activeTab,
  setActiveTab,
  appCount,
  status,
  isInitialized,
  isOperating,
  booting = false,
  busy = false,
  onStart,
  onStop,
  onRestart,
  onOpenSharedFolder,
}) {
  const online = status === "online";
  const locked = isOperating || busy;

  return (
    <header className="topbar">
      <div className="brand">
        <div className="brand-icon-wrapper" aria-hidden="true">
          <MonitorPlay size={18} />
        </div>
        <div className="brand-text">
          <span className="brand-title">ArchDroid</span>
          <span className="brand-version">v{pkg.version}</span>
        </div>
      </div>

      <nav className="topbar-nav" aria-label="Main">
        {NAV_ITEMS.map((item) => {
          const Icon = item.icon;
          const active = activeTab === item.id;
          return (
            <button
              key={item.id}
              type="button"
              className={`menu-item ${active ? "active" : ""}`}
              aria-current={active ? "page" : undefined}
              onClick={() => setActiveTab(item.id)}
            >
              <Icon size={16} aria-hidden="true" />
              <span className="menu-label">{item.label}</span>
              {item.id === "home" && appCount > 0 && (
                <span className="menu-badge">{appCount}</span>
              )}
            </button>
          );
        })}
      </nav>

      <div className="topbar-right">
        <button
          type="button"
          className="btn-icon"
          onClick={onOpenSharedFolder}
          aria-label="Open Android shared folder"
          title="Open Android shared folder"
        >
          <FolderOpen size={17} aria-hidden="true" />
        </button>

        <div className={`engine-pill ${online ? "online" : "offline"}`}>
          <div
            className={`status-badge ${online ? "status-online" : "status-offline"}`}
            role="status"
          >
            <span className="status-dot" aria-hidden="true"></span>
            <span className="status-text">
              {online
                ? booting
                  ? "Android Booting…"
                  : "Engine Running"
                : "Engine Stopped"}
            </span>
          </div>

          {isInitialized &&
            (online ? (
              <>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  onClick={onRestart}
                  disabled={locked}
                  title="Restart Waydroid session"
                >
                  <RotateCw size={14} aria-hidden="true" />
                  <span>Restart</span>
                </button>
                <button
                  type="button"
                  className="btn btn-danger btn-sm"
                  onClick={onStop}
                  disabled={locked}
                  title="Stop Waydroid session"
                >
                  <Square size={13} fill="currentColor" aria-hidden="true" />
                  <span>Stop Engine</span>
                </button>
              </>
            ) : (
              <button
                type="button"
                className="btn btn-primary btn-sm"
                onClick={onStart}
                disabled={locked}
                title="Start Waydroid session"
              >
                <Play size={14} fill="currentColor" aria-hidden="true" />
                <span>Start Engine</span>
              </button>
            ))}
        </div>
      </div>
    </header>
  );
}
