import React from "react";
import { Gamepad2, Play, Download, Keyboard, MousePointer, ShieldCheck } from "lucide-react";

export default function KeymapView({ onRunPhantom, onInstallPhantom, installing }) {
  return (
    <div className="keymap-view-container">
      {/* Hero Showcase Card */}
      <div className="glass-panel keymap-hero-card">
        <div className="keymap-hero-visual">
          <div className="keymap-icon-ring">
            <Gamepad2 size={64} className="icon-glow keymap-hero-icon" />
          </div>
        </div>

        <div className="keymap-hero-content">
          <h2>Phantom Keymapper for Linux</h2>
          <p>
            Experience PC-grade controls for mobile games. <strong>Phantom</strong> is an ultra-fast, native Wayland keymapper designed specifically for Waydroid that translates keyboard strokes and mouse movements directly into Android multi-touch gestures.
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
          <p>Assign discrete touch coordinates or directional joysticks to any keyboard keys.</p>
        </div>

        <div className="glass-panel keymap-feature-item">
          <div className="feature-icon-badge">
            <MousePointer size={22} />
          </div>
          <h4>FPS Mouse Look & Aim</h4>
          <p>Lock cursor to enable 360-degree camera panning for shooter games like Free Fire & PUBG.</p>
        </div>

        <div className="glass-panel keymap-feature-item">
          <div className="feature-icon-badge">
            <ShieldCheck size={22} />
          </div>
          <h4>Zero Ban Risk</h4>
          <p>Operates outside the Android container via native Linux uinput without modifying APK binaries.</p>
        </div>
      </div>
    </div>
  );
}
