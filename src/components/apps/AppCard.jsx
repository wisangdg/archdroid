import React, { useState } from "react";
import { Trash2, Play } from "lucide-react";

export default function AppCard({
  app,
  onLaunch,
  onUninstall,
  onContextMenu,
  disabled = false,
}) {
  const [imgError, setImgError] = useState(false);

  const handleKeyDown = (e) => {
    if (disabled) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onLaunch(app.package);
    } else if (
      (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) &&
      onContextMenu
    ) {
      e.preventDefault();
      const rect = e.currentTarget.getBoundingClientRect();
      onContextMenu(
        {
          clientX: rect.left + rect.width / 2,
          clientY: rect.bottom,
          preventDefault: () => {},
        },
        app
      );
    }
  };

  return (
    <div
      className="app-card"
      role="button"
      tabIndex={disabled ? -1 : 0}
      aria-disabled={disabled}
      aria-label={`Launch ${app.name}`}
      onClick={disabled ? undefined : () => onLaunch(app.package)}
      onKeyDown={handleKeyDown}
      onContextMenu={(e) => {
        if (onContextMenu) {
          e.preventDefault();
          onContextMenu(e, app);
        }
      }}
      title="Left-click to launch, right-click for options"
    >
      {/* Icon with glowing backdrop */}
      <div className="app-icon-container">
        <div className="app-icon">
          {app.icon && !imgError ? (
            <img
              src={app.icon}
              alt={app.name}
              className="app-icon-img"
              loading="lazy"
              onError={() => setImgError(true)}
            />
          ) : (
            <span className="app-icon-fallback" role="img" aria-label="App">
              🤖
            </span>
          )}
        </div>
        <div className="app-launch-overlay">
          <Play size={20} fill="currentColor" aria-hidden="true" />
        </div>
      </div>

      {/* App Meta */}
      <div className="app-meta">
        <span className="app-name" title={app.name}>
          {app.name}
        </span>
        <span className="app-package" title={app.package}>
          {app.package}
        </span>
      </div>

      {/* Quick Uninstall Overlay Button */}
      <button
        type="button"
        className="btn-uninstall-overlay"
        onClick={(e) => {
          e.stopPropagation();
          onUninstall(e, app.package, app.name);
        }}
        disabled={disabled}
        title={`Uninstall ${app.name}`}
        aria-label={`Uninstall ${app.name}`}
      >
        <Trash2 size={15} aria-hidden="true" />
      </button>
    </div>
  );
}
