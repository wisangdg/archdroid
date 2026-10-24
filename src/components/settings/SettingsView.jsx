import React from "react";
import {
  FolderOpen,
  Sliders,
  Terminal,
  AlertOctagon,
  Check,
  RotateCcw,
} from "lucide-react";
import ResolutionPresets from "./ResolutionPresets";
import HostDiagnostics from "./HostDiagnostics";

export default function SettingsView({
  multiWindow,
  invertColors,
  onToggleMultiWindow,
  onToggleInvertColors,
  onPropSet,
  onOpenSharedFolder,
  onViewLogs,
  onWipeData,
  currentWidth = "",
  currentHeight = "",
  currentDpi = "",
  onApplyResolution,
  engineRunning = false,
  onRestartEngine,
}) {
  return (
    <div className="settings-view-container">
      {/* Display & Resolution Presets */}
      <ResolutionPresets
        currentWidth={currentWidth}
        currentHeight={currentHeight}
        currentDpi={currentDpi}
        onApplyResolution={onApplyResolution}
        engineRunning={engineRunning}
        onRestartEngine={onRestartEngine}
      />

      {/* Shared Folder */}
      <div className="glass-panel settings-card">
        <div className="settings-card-content">
          <div className="settings-card-icon">
            <FolderOpen size={24} />
          </div>
          <div className="settings-card-text">
            <h3>Android Shared Storage</h3>
            <p>
              Direct bridge to <code>/sdcard/</code> (internal shared storage). Transfer APKs, ROMs, OBB game data, and screenshots between Linux and Android seamlessly.
            </p>
          </div>
        </div>
        <button
          className="btn btn-outline"
          onClick={onOpenSharedFolder}
          title="Open in your default file manager"
        >
          <FolderOpen size={17} />
          <span>Open Folder</span>
        </button>
      </div>

      {/* Visual Config Manager */}
      <div className="glass-panel">
        <div className="section-title-wrap">
          <Sliders size={20} className="section-title-icon" />
          <div>
            <h3>Visual Configuration Toggles</h3>
            <p>Customize how Waydroid renders on your desktop</p>
          </div>
        </div>

        <div className="config-toggle-list">
          {/* Multi Window Mode */}
          <div className="config-toggle-item">
            <div className="toggle-info">
              <h4>Multi-Window Mode</h4>
              <p>
                Runs each Android app in its own free-floating native desktop window (ideal for multitasking) instead of a single mobile screen frame.
              </p>
            </div>
            <button
              type="button"
              className={`toggle-switch ${multiWindow ? "active" : ""}`}
              onClick={onToggleMultiWindow}
              aria-label="Toggle Multi-Window Mode"
            >
              <div className="toggle-switch-handle">
                {multiWindow && <Check size={12} />}
              </div>
            </button>
          </div>

          {/* Invert Colors */}
          <div className="config-toggle-item">
            <div className="toggle-info">
              <h4>Invert Display Colors</h4>
              <p>
                Inverts Android container surface color channels. Can be used to force dark mode on stubborn light-theme apps.
              </p>
            </div>
            <button
              type="button"
              className={`toggle-switch ${invertColors ? "active" : ""}`}
              onClick={onToggleInvertColors}
              aria-label="Toggle Invert Colors"
            >
              <div className="toggle-switch-handle">
                {invertColors && <Check size={12} />}
              </div>
            </button>
          </div>
        </div>
      </div>

      {/* Advanced System Properties */}
      <div className="glass-panel">
        <h3>Advanced Properties Manager</h3>
        <p>
          Set custom Android system properties using <code>waydroid prop set &lt;key&gt; &lt;val&gt;</code>.
        </p>

        <form onSubmit={onPropSet} className="prop-form">
          <div className="prop-inputs-row">
            <input
              name="propKey"
              type="text"
              className="prop-input-key"
              placeholder="Property Key (e.g. persist.waydroid.width)"
              required
            />
            <input
              name="propVal"
              type="text"
              className="prop-input-val"
              placeholder="Value (e.g. 1920)"
              required
            />
            <button type="submit" className="btn btn-primary btn-apply">
              Apply Property
            </button>
          </div>
          <span className="prop-form-hint">
            Tip: Common keys include <code>persist.waydroid.width</code>, <code>persist.waydroid.height</code>, <code>ro.sf.lcd_density</code>
          </span>
        </form>
      </div>

      {/* Linux Host System Diagnostics Checkup */}
      <HostDiagnostics />

      {/* Diagnostic Logs */}
      <div className="glass-panel settings-card">
        <div className="settings-card-content">
          <div className="settings-card-icon">
            <Terminal size={24} />
          </div>
          <div className="settings-card-text">
            <h3>Diagnostic & System Logs</h3>
            <p>
              Inspect the live <code>/var/lib/waydroid/waydroid.log</code> file to diagnose crashes, container boot failures, or graphic driver errors.
            </p>
          </div>
        </div>
        <button className="btn btn-outline" onClick={onViewLogs}>
          <Terminal size={17} />
          <span>View Logs</span>
        </button>
      </div>

      {/* Danger Zone */}
      <div className="glass-panel danger-card">
        <div className="danger-card-header">
          <AlertOctagon size={24} className="danger-icon" />
          <div>
            <h3>Danger Zone: Factory Reset</h3>
            <p>
              Completely wipe all Waydroid container data, downloaded games, GApps modules, and user configurations. This resets Waydroid to a clean initial state.
            </p>
          </div>
        </div>
        <button
          className="btn btn-danger btn-wipe"
          onClick={onWipeData}
          title="Factory Reset Waydroid"
        >
          <RotateCcw size={17} />
          <span>Factory Reset (Wipe All Data)</span>
        </button>
      </div>
    </div>
  );
}
