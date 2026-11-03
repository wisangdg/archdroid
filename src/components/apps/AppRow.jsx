import React from "react";
import AppIcon from "./AppIcon";

/**
 * One row of the Control Center app list. Click (or Enter/Space) selects the
 * app for the detail panel, double-click launches it, Up/Down move through the
 * list, and the context-menu key or Shift+F10 opens the options menu.
 */
export default function AppRow({
  app,
  system,
  selected,
  onSelect,
  onLaunch,
  onMove,
  onContextMenu,
  disabled = false,
}) {
  const openMenuFromKeyboard = (e) => {
    const rect = e.currentTarget.getBoundingClientRect();
    onContextMenu(
      {
        clientX: rect.left + Math.min(rect.width / 2, 160),
        clientY: rect.bottom,
        preventDefault: () => {},
      },
      app
    );
  };

  const handleKeyDown = (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      onMove?.(e.key === "ArrowDown" ? 1 : -1);
    } else if (
      (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10")) &&
      onContextMenu
    ) {
      e.preventDefault();
      openMenuFromKeyboard(e);
    }
  };

  return (
    <button
      type="button"
      className={`app-row ${selected ? "selected" : ""}`}
      data-package={app.package}
      aria-label={`${app.name}, ${app.package}, ${system ? "system" : "user"} app`}
      aria-current={selected ? "true" : undefined}
      onClick={() => onSelect(app.package)}
      onDoubleClick={disabled ? undefined : () => onLaunch(app.package)}
      onKeyDown={handleKeyDown}
      onContextMenu={(e) => {
        if (onContextMenu) {
          e.preventDefault();
          onContextMenu(e, app);
        }
      }}
      title="Click for details, double-click to launch, right-click for options"
    >
      <AppIcon key={app.package} app={app} size={38} />
      <span className="app-row-text">
        <span className="app-row-name">{app.name}</span>
        <span className="app-row-package">{app.package}</span>
      </span>
      <span className={`app-tag ${system ? "system" : "user"}`}>
        {system ? "System" : "User"}
      </span>
    </button>
  );
}
