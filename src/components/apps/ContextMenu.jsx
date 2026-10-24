import React, { useEffect, useRef } from "react";
import { Play, Copy, Trash2 } from "lucide-react";

export default function ContextMenu({
  position,
  app,
  onClose,
  onLaunch,
  onCopyPackage,
  onUninstall,
}) {
  const menuRef = useRef(null);

  useEffect(() => {
    const handleClickOutside = (e) => {
      if (menuRef.current && !menuRef.current.contains(e.target)) {
        onClose();
      }
    };

    const handleKeyDown = (e) => {
      if (e.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  if (!position || !app) return null;

  // Ensure menu stays within screen boundaries
  const screenW = window.innerWidth;
  const screenH = window.innerHeight;
  const menuW = 210;
  const menuH = 140;

  const left = Math.min(position.x, screenW - menuW - 10);
  const top = Math.min(position.y, screenH - menuH - 10);

  return (
    <div
      ref={menuRef}
      className="custom-context-menu"
      style={{ left: `${left}px`, top: `${top}px` }}
      onClick={(e) => e.stopPropagation()}
    >
      <div className="context-menu-header">
        <span className="context-menu-app-name">{app.name}</span>
      </div>

      <div className="context-menu-divider" />

      <button
        type="button"
        className="context-menu-item"
        onClick={() => {
          onLaunch(app.package);
          onClose();
        }}
      >
        <Play size={15} fill="currentColor" />
        <span>Launch App</span>
      </button>

      <button
        type="button"
        className="context-menu-item"
        onClick={() => {
          onCopyPackage(app.package);
          onClose();
        }}
      >
        <Copy size={15} />
        <span>Copy Package ID</span>
      </button>

      <div className="context-menu-divider" />

      <button
        type="button"
        className="context-menu-item danger"
        onClick={() => {
          onUninstall(app.package, app.name);
          onClose();
        }}
      >
        <Trash2 size={15} />
        <span>Uninstall App</span>
      </button>
    </div>
  );
}
