import React, { useEffect } from "react";
import {
  Gamepad2,
  Play,
  Download,
  Keyboard,
  MousePointer,
  ShieldCheck,
} from "lucide-react";
import TouchModePanel from "./TouchModePanel";

export default function KeymapView({
  onRunPhantom,
  onInstallPhantom,
  installing,
  touchMode,
  onToggleTouchMode,
  onEnableTouchAccess,
}) {
  // The backend thread can stop on its own (mouse unplugged, compositor
  // gone); read the state whenever the page is opened, not only at start.
  const refreshTouchMode = touchMode.refresh;
  useEffect(() => {
    refreshTouchMode?.();
  }, [refreshTouchMode]);

  return (
    <div className="keymap-view-container">
      <TouchModePanel
        status={touchMode.status}
        changing={touchMode.changing}
        installing={installing}
        onToggle={onToggleTouchMode}
        onEnableAccess={onEnableTouchAccess}
      />

      {/* Hero Showcase Card */}
      <div className="glass-panel keymap-hero-card">
        <div className="keymap-hero-visual">
          <div className="keymap-icon-ring">
            <Gamepad2 size={64} className="icon-glow keymap-hero-icon" />
          </div>
        </div>

        <div className="keymap-hero-content">
          <h2>Phantom Keymapper</h2>
          <p>
            <strong>Phantom</strong> maps keyboard keys and mouse movement to
            Android touch gestures, so games made for a touchscreen can be
            played with WASD and the mouse. Install it once, then launch it next
            to the game.
          </p>

          <div className="keymap-actions">
            <button
              className="btn btn-primary btn-glow"
              onClick={onRunPhantom}
              title="Launch Phantom Keymapper"
            >
              <Play size={18} fill="currentColor" />
              <span>Launch Keymapper</span>
            </button>
            <button
              className="btn btn-outline"
              onClick={onInstallPhantom}
              disabled={installing}
              title="Clone/update Phantom from GitHub"
            >
              <Download size={18} />
              <span>Install / Update Phantom</span>
            </button>
          </div>
        </div>
      </div>

      {/* Feature Highlights Grid */}
      <div className="keymap-features-grid">
        <div className="glass-panel keymap-feature-item">
          <div className="feature-icon-badge">
            <Keyboard size={22} />
          </div>
          <h4>WASD & Custom Binds</h4>
          <p>
            Assign discrete touch coordinates or directional joysticks to any
            keyboard keys.
          </p>
        </div>

        <div className="glass-panel keymap-feature-item">
          <div className="feature-icon-badge">
            <MousePointer size={22} />
          </div>
          <h4>FPS Mouse Look & Aim</h4>
          <p>
            Lock cursor to enable 360-degree camera panning for shooter games
            like Free Fire & PUBG.
          </p>
        </div>

        <div className="glass-panel keymap-feature-item">
          <div className="feature-icon-badge">
            <ShieldCheck size={22} />
          </div>
          <h4>No APK Changes</h4>
          <p>
            Runs outside the Android container via native Linux uinput and never
            modifies APK binaries. Games with strict anti-cheat may still detect
            or block it.
          </p>
        </div>
      </div>
    </div>
  );
}
