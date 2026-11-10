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
import AppLogPanel from "./AppLogPanel";
import DiagnosticBundlePanel from "./DiagnosticBundlePanel";

/** The page is long; these jump to its parts. */
const SECTIONS = [
  { id: "settings-display", label: "Display" },
  { id: "settings-android", label: "Android" },
  { id: "settings-diagnostics", label: "Diagnostics" },
  { id: "settings-logs", label: "Logs & support" },
  { id: "settings-reset", label: "Reset" },
];

const jumpTo = (id) =>
  document
    .getElementById(id)
    ?.scrollIntoView?.({ behavior: "smooth", block: "start" });

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
  busy = false,
}) {
  return (
    <div className="settings-view-container">
      <nav className="settings-jump" aria-label="Settings sections">
        {SECTIONS.map((section) => (
          <button
            key={section.id}
            type="button"
            className="segmented-btn"
            onClick={() => jumpTo(section.id)}
          >
            {section.label}
          </button>
        ))}
      </nav>

      {/* Display & Resolution Presets */}
      <span id="settings-display" className="settings-anchor" />
      <ResolutionPresets
        currentWidth={currentWidth}
        currentHeight={currentHeight}
        currentDpi={currentDpi}
        onApplyResolution={onApplyResolution}
        engineRunning={engineRunning}
        onRestartEngine={onRestartEngine}
        disabled={busy}
      />

      <span id="settings-android" className="settings-anchor" />
      {/* Shared Folder */}
      <div className="glass-panel settings-card">
        <div className="settings-card-content">
          <div className="settings-card-icon">
            <FolderOpen size={24} />
          </div>
          <div className="settings-card-text">
            <h3>Android Shared Storage</h3>
            <p>
              Direct bridge to <code>/sdcard/</code> (internal shared storage).
              Transfer APKs, ROMs, OBB game data, and screenshots between Linux
              and Android seamlessly.
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
            <h3>Android Display Options</h3>
            <p>How Android windows appear on your desktop</p>
          </div>
        </div>

        {!engineRunning && (
          <p className="config-offline-note" role="note">
            Start the engine and wait for Android to boot to read and change
            these settings.
          </p>
        )}
        <div className="config-toggle-list">
          {/* Multi Window Mode */}
          <div className="config-toggle-item">
            <div className="toggle-info">
              <h4>Multi-Window Mode</h4>
              <p>
                Runs each Android app in its own free-floating native desktop
                window (ideal for multitasking) instead of a single mobile
                screen frame.
              </p>
            </div>
            <button
              type="button"
              className={`toggle-switch ${multiWindow ? "active" : ""}`}
              onClick={onToggleMultiWindow}
              disabled={busy || !engineRunning}
              role="switch"
              aria-checked={multiWindow}
              aria-label="Multi-Window Mode"
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
                Inverts Android container surface color channels. Can be used to
                force dark mode on stubborn light-theme apps.
              </p>
            </div>
            <button
              type="button"
              className={`toggle-switch ${invertColors ? "active" : ""}`}
              onClick={onToggleInvertColors}
              disabled={busy || !engineRunning}
              role="switch"
              aria-checked={invertColors}
              aria-label="Invert Colors"
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
        <div className="section-title-wrap">
          <Sliders size={20} className="section-title-icon" />
          <div>
            <h3>Android Properties</h3>
            <p>
              Set any Android system property with{" "}
              <code>waydroid prop set &lt;key&gt; &lt;val&gt;</code>.
            </p>
          </div>
        </div>

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
            <button
              type="submit"
              className="btn btn-primary btn-apply"
              disabled={busy || !engineRunning}
            >
              Apply Property
            </button>
          </div>
          <span className="prop-form-hint">
            Tip: Common keys include <code>persist.waydroid.width</code>,{" "}
            <code>persist.waydroid.height</code>,{" "}
            <code>persist.waydroid.multi_windows</code>
          </span>
        </form>
      </div>

      {/* Linux Host System Diagnostics Checkup */}
      <span id="settings-diagnostics" className="settings-anchor" />
      <HostDiagnostics />

      <span id="settings-logs" className="settings-anchor" />
      {/* One section for every log: Waydroid's, ArchDroid's, and the bundle. */}
      <section className="glass-panel logs-panel" aria-labelledby="logs-title">
        <div className="section-title-wrap">
          <Terminal size={20} className="section-title-icon" />
          <div>
            <h3 id="logs-title">Logs &amp; Support</h3>
            <p>
              Stored on this computer only; ArchDroid never uploads logs or
              telemetry.
            </p>
          </div>
        </div>

        <div className="log-row">
          <div className="log-row-text">
            <h4>Waydroid Log</h4>
            <p>
              <code>/var/lib/waydroid/waydroid.log</code>: container boot
              failures, crashes, and graphics driver errors.
            </p>
          </div>
          <div className="log-row-actions">
            <button
              className="btn btn-outline"
              onClick={onViewLogs}
              disabled={busy}
            >
              <Terminal size={17} />
              <span>View Logs</span>
            </button>
          </div>
        </div>

        <AppLogPanel />

        <DiagnosticBundlePanel />
      </section>

      <span id="settings-reset" className="settings-anchor" />
      {/* Danger Zone */}
      <div className="glass-panel danger-card">
        <div className="danger-card-header">
          <AlertOctagon size={24} className="danger-icon" />
          <div>
            <h3>Factory Reset</h3>
            <p>
              Completely wipe all Waydroid container data, downloaded games,
              GApps modules, and user configurations. This resets Waydroid to a
              clean initial state.
            </p>
          </div>
        </div>
        <button
          className="btn btn-danger btn-wipe"
          onClick={onWipeData}
          disabled={busy}
          title="Factory Reset Waydroid"
        >
          <RotateCcw size={17} />
          <span>Factory Reset (Wipe All Data)</span>
        </button>
      </div>
    </div>
  );
}
