import React, { useEffect, useRef } from "react";
import { Play, Copy, Trash2 } from "lucide-react";

export default function ContextMenu({
  position,
  app,
  onClose,
  onLaunch,
  onCopyPackage,
  onUninstall,
  disabled = false,
}) {
  const menuRef = useRef(null);
  const previousActiveElement = useRef(null);

  useEffect(() => {
    previousActiveElement.current = document.activeElement;

    // Focus the first menu item initially
    const firstButton = menuRef.current?.querySelector(
      "button:not([disabled])"
    );
    if (firstButton) {
      firstButton.focus();
    }

    const handleClickOutside = (e) => {
      if (menuRef.current && !menuRef.current.contains(e.target)) {
        onClose();
      }
    };

    const handleKeyDown = (e) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
        return;
      }

      const items = Array.from(
        menuRef.current?.querySelectorAll("button:not([disabled])") || []
      );
      if (items.length === 0) return;

      const currentIndex = items.indexOf(document.activeElement);

      if (e.key === "ArrowDown") {
        e.preventDefault();
        const nextIndex = (currentIndex + 1) % items.length;
        items[nextIndex].focus();
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        const prevIndex = (currentIndex - 1 + items.length) % items.length;
        items[prevIndex].focus();
      } else if (e.key === "Home") {
        e.preventDefault();
        items[0].focus();
      } else if (e.key === "End") {
        e.preventDefault();
        items[items.length - 1].focus();
      } else if (e.key === "Tab") {
        onClose();
      }
    };

    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
      if (
        previousActiveElement.current &&
        typeof previousActiveElement.current.focus === "function"
      ) {
        previousActiveElement.current.focus();
      }
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
      role="menu"
      aria-label={`Options for ${app.name}`}
      tabIndex={-1}
    >
      <div className="context-menu-header" role="none">
        <span className="context-menu-app-name">{app.name}</span>
      </div>

      <div className="context-menu-divider" role="separator" />

      <button
        type="button"
        role="menuitem"
        className="context-menu-item"
        onClick={() => {
          onLaunch(app.package);
          onClose();
        }}
        disabled={disabled}
      >
        <Play size={15} fill="currentColor" aria-hidden="true" />
        <span>Launch App</span>
      </button>

      <button
        type="button"
        role="menuitem"
        className="context-menu-item"
        onClick={() => {
          onCopyPackage(app.package);
          onClose();
        }}
      >
        <Copy size={15} aria-hidden="true" />
        <span>Copy Package ID</span>
      </button>

      <div className="context-menu-divider" role="separator" />

      <button
        type="button"
        role="menuitem"
        className="context-menu-item danger"
        onClick={() => {
          onUninstall(app.package, app.name);
          onClose();
        }}
        disabled={disabled}
      >
        <Trash2 size={15} aria-hidden="true" />
        <span>Uninstall App</span>
      </button>
    </div>
  );
}
