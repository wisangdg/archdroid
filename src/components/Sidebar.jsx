import React from "react";
import pkg from "../../package.json";
import {
  MonitorPlay,
  Gamepad2,
  Download,
  Settings,
  FolderOpen,
  Cpu,
} from "lucide-react";

export default function Sidebar({
  activeTab,
  setActiveTab,
  appCount,
  status,
  onOpenSharedFolder,
}) {
  const navItems = [
    {
      id: "home",
      label: "My Apps",
      icon: MonitorPlay,
      badge: appCount > 0 ? appCount : null,
    },
    { id: "keymap", label: "Keymapping", icon: Gamepad2 },
    { id: "install", label: "Install Tools", icon: Download },
    { id: "settings", label: "Settings", icon: Settings },
  ];

  return (
    <aside className="sidebar">
      {/* Brand Header */}
      <div className="brand">
        <div className="brand-icon-wrapper">
          <MonitorPlay size={26} className="icon-glow" />
        </div>
        <div className="brand-text">
          <span className="brand-title">ArchDroid</span>
          <span className="brand-subtitle">Waydroid Manager</span>
        </div>
      </div>

      {/* Navigation List */}
      <nav className="nav-menu">
        <div className="nav-label">MENU</div>
        {navItems.map((item) => {
          const Icon = item.icon;
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              className={`menu-item ${isActive ? "active" : ""}`}
              onClick={() => setActiveTab(item.id)}
            >
              <div className="menu-icon-wrap">
                <Icon size={19} />
              </div>
              <span className="menu-label">{item.label}</span>
              {item.badge !== null && (
                <span className="menu-badge">{item.badge}</span>
              )}
            </button>
          );
        })}
      </nav>

      {/* Sidebar Footer Widget */}
      <div className="sidebar-footer">
        <div className="engine-status-card">
          <div className="engine-status-row">
            <span className="engine-status-label">Engine Status</span>
            <span
              className={`status-pill-mini ${status === "online" ? "online" : "offline"}`}
            >
              <span className="status-dot"></span>
              {status === "online" ? "Online" : "Offline"}
            </span>
          </div>
          <button
            className="btn-sidebar-quick-action"
            onClick={onOpenSharedFolder}
            title="Open Android Shared Folder"
          >
            <FolderOpen size={15} />
            <span>Shared Folder</span>
          </button>
        </div>

        <div className="sidebar-version-tag">
          <Cpu size={13} />
          <span>{`ArchDroid v${pkg.version} • Linux`}</span>
        </div>
      </div>
    </aside>
  );
}
