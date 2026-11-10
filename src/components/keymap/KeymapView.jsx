import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Gamepad2,
  Play,
  Download,
  CheckCircle2,
  AlertTriangle,
} from "lucide-react";
import TouchModePanel from "./TouchModePanel";

const PHANTOM_BADGES = {
  installed: (
    <span className="badge-pill badge-success">
      <CheckCircle2 size={14} /> Installed
    </span>
  ),
  partial: (
    <span
      className="badge-pill badge-warning"
      title="Only the terminal client was found; Launch opens it in a terminal."
    >
      <AlertTriangle size={14} /> Terminal only
    </span>
  ),
  not_installed: (
    <span className="badge-pill badge-neutral">Not Installed</span>
  ),
};

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

  // null while the first check runs, so Launch is not offered on a guess.
  const [phantom, setPhantom] = useState(null);
  const refreshPhantom = useCallback(async () => {
    try {
      const res = await invoke("phantom_status");
      setPhantom(res?.status ?? "not_installed");
    } catch (e) {
      console.error("Phantom status error:", e);
      setPhantom("not_installed");
    }
  }, []);

  // Read on open and again when a task (the Phantom install among them) ends.
  const wasInstalling = useRef(installing);
  useEffect(() => {
    if (wasInstalling.current && !installing) refreshPhantom();
    wasInstalling.current = installing;
  }, [installing, refreshPhantom]);
  useEffect(() => {
    refreshPhantom();
  }, [refreshPhantom]);

  const available = phantom === "installed" || phantom === "partial";

  return (
    <div className="keymap-view-container">
      <TouchModePanel
        status={touchMode.status}
        changing={touchMode.changing}
        installing={installing}
        onToggle={onToggleTouchMode}
        onEnableAccess={onEnableTouchAccess}
      />

      <div className="glass-panel keymap-hero-card">
        <div className="keymap-hero-visual">
          <div className="keymap-icon-ring">
            <Gamepad2 size={48} className="keymap-hero-icon" />
          </div>
        </div>

        <div className="keymap-hero-content">
          <div className="tool-title-row">
            <h2>Phantom Keymapper</h2>
            {PHANTOM_BADGES[phantom] ?? (
              <span className="badge-pill badge-neutral">Checking…</span>
            )}
          </div>
          <p>
            Phantom maps keyboard keys and mouse movement to Android touch
            gestures, so games made for a touchscreen can be played with the
            keyboard and mouse. Launch it next to the game.
          </p>
          <ul className="keymap-facts">
            <li>Keys to taps and joysticks; mouse look for shooters.</li>
            <li>
              Runs on the Linux side through uinput and does not change any APK.
            </li>
            <li>Games with strict anti-cheat may still detect or block it.</li>
          </ul>

          <div className="keymap-actions">
            <button
              className="btn btn-primary"
              onClick={onRunPhantom}
              disabled={!available}
              aria-describedby={available ? undefined : "phantom-launch-hint"}
            >
              <Play size={18} fill="currentColor" />
              <span>Launch Keymapper</span>
            </button>
            <button
              className={`btn ${phantom === "not_installed" ? "btn-primary" : "btn-outline"}`}
              onClick={onInstallPhantom}
              disabled={installing}
              title="Build Phantom from its pinned source"
            >
              <Download size={18} />
              <span>
                {phantom === "not_installed"
                  ? "Install Phantom"
                  : "Update Phantom"}
              </span>
            </button>
          </div>
          {phantom === "not_installed" && (
            <p id="phantom-launch-hint" className="keymap-hint">
              Install Phantom first. It builds as your user (no root) and needs
              about 1 GB of free space.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
