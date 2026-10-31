import React, { useState } from "react";
import { Trash2, Play } from "lucide-react";

export default function AppCard({ app, onLaunch, onUninstall, onContextMenu, disabled = false }) {
  const [imgError, setImgError] = useState(false);

  return (
    <div
      className="app-card"
      onClick={disabled ? undefined : () => onLaunch(app.package)}
      onContextMenu={(e) => {
        if (onContextMenu) {
          e.preventDefault();
          onContextMenu(e, app);
        }
      }}
      title={`Left-click to launch, right-click for options`}
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
            <span className="app-icon-fallback" role="img" aria-label="App">🤖</span>
          )}
        </div>
        <div className="app-launch-overlay">
          <Play size={20} fill="currentColor" />
        </div>
      </div>

      {/* App Meta */}
      <div className="app-meta">
        <span className="app-name" title={app.name}>{app.name}</span>
        <span className="app-package" title={app.package}>{app.package}</span>
      </div>

      {/* Quick Uninstall Overlay Button */}
      <button
        type="button"
        className="btn-uninstall-overlay"
        onClick={(e) => onUninstall(e, app.package, app.name)}
        disabled={disabled}
        title={`Uninstall ${app.name}`}
      >
        <Trash2 size={15} />
      </button>
    </div>
  );
}
