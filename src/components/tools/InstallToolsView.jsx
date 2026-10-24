import React from "react";
import { Download, CheckCircle2, AlertTriangle, HelpCircle, Layers, Cpu, Puzzle } from "lucide-react";

export default function InstallToolsView({
  gappsStatus,
  houdiniStatus,
  multitouchStatus,
  installing,
  onInstallGApps,
  onInstallHoudini,
  onInstallMultitouch,
}) {
  const renderStatusBadge = (status) => {
    switch (status) {
      case "installed":
        return (
          <span className="badge-pill badge-success">
            <CheckCircle2 size={14} /> Installed
          </span>
        );
      case "partial":
        return (
          <span className="badge-pill badge-warning">
            <AlertTriangle size={14} /> Partial
          </span>
        );
      case "not_installed":
        return (
          <span className="badge-pill badge-neutral">
            Not Installed
          </span>
        );
      default:
        return (
          <span className="badge-pill badge-neutral">
            Checking...
          </span>
        );
    }
  };

  return (
    <div className="tools-view-container">
      {/* Intro Info Banner */}
      <div className="info-banner">
        <HelpCircle size={20} className="info-icon" />
        <div className="info-text">
          <strong>Addon Injection System</strong>: These tools inject custom packages directly into the Waydroid Android root image. Administrator (root) privileges will be requested via Polkit/Sudo.
        </div>
      </div>

      {/* Tools Cards */}
      <div className="tools-grid">
        {/* GApps Card */}
        <div className="glass-panel tool-card">
          <div className="tool-card-header">
            <div className="tool-icon-wrap gapps-icon-wrap">
              <Layers size={26} />
            </div>
            <div className="tool-header-text">
              <div className="tool-title-row">
                <h3>Google Play Services (OpenGApps)</h3>
                {renderStatusBadge(gappsStatus)}
              </div>
              <p>
                Injects the official Google Play Store, Play Services, and Google Services Framework
                into the Android container to allow downloading apps directly.
              </p>
            </div>
          </div>

          <div className="tool-card-footer">
            <span className="tool-requirement-hint">Source: waydroid_script • OpenGApps</span>
            <button
              className={gappsStatus === "installed" ? "btn btn-outline" : "btn btn-primary"}
              onClick={onInstallGApps}
              disabled={installing || gappsStatus === "installed"}
            >
              {gappsStatus === "installed" ? (
                <>
                  <CheckCircle2 size={17} />
                  <span>Installed</span>
                </>
              ) : (
                <>
                  <Download size={17} />
                  <span>Install GApps</span>
                </>
              )}
            </button>
          </div>
        </div>

        {/* ARM Translation (libhoudini) Card */}
        <div className="glass-panel tool-card">
          <div className="tool-card-header">
            <div className="tool-icon-wrap arm-icon-wrap">
              <Cpu size={26} />
            </div>
            <div className="tool-header-text">
              <div className="tool-title-row">
                <h3>ARM Translation Layer (libhoudini)</h3>
                {renderStatusBadge(houdiniStatus)}
              </div>
              <p>
                Translates ARM and ARM64 native binaries on x86_64 processors. Crucial for playing modern Android 3D mobile games (PUBG, Genshin Impact, Roblox, etc.) that do not provide native x86 libraries.
              </p>
            </div>
          </div>

          <div className="tool-card-footer">
            <span className="tool-requirement-hint">Native Intel/AMD x86_64 Bridge</span>
            <button
              className={houdiniStatus === "installed" ? "btn btn-outline" : "btn btn-primary"}
              onClick={onInstallHoudini}
              disabled={installing || houdiniStatus === "installed"}
            >
              {houdiniStatus === "installed" ? (
                <>
                  <CheckCircle2 size={17} />
                  <span>Active</span>
                </>
              ) : (
                <>
                  <Download size={17} />
                  <span>Install ARM Support</span>
                </>
              )}
            </button>
          </div>
        </div>

        {/* Play Store compatibility (declared hardware features) Card */}
        <div className="glass-panel tool-card">
          <div className="tool-card-header">
            <div className="tool-icon-wrap gapps-icon-wrap">
              <Puzzle size={26} />
            </div>
            <div className="tool-header-text">
              <div className="tool-title-row">
                <h3>Play Store Compatibility (Touch Features)</h3>
                {renderStatusBadge(multitouchStatus)}
              </div>
              <p>
                Declares the multitouch capabilities of Waydroid's touchscreen in the vendor
                partition. Google Play hides every app that requires a feature the device never
                declares — Plants vs Zombies 2 requires
                <code>android.hardware.touchscreen.multitouch</code> — and it reports that with the
                same &ldquo;isn&apos;t compatible with this version&rdquo; message an ARM
                translation problem produces.
              </p>
            </div>
          </div>

          <div className="tool-card-footer">
            <span className="tool-requirement-hint">
              {multitouchStatus === "partial"
                ? "Written to disk — restart Waydroid to activate"
                : "Same declaration LDPlayer 9 ships"}
            </span>
            <button
              className={multitouchStatus === "installed" ? "btn btn-outline" : "btn btn-primary"}
              onClick={onInstallMultitouch}
              disabled={installing || multitouchStatus === "installed"}
            >
              {multitouchStatus === "installed" ? (
                <>
                  <CheckCircle2 size={17} />
                  <span>Declared</span>
                </>
              ) : (
                <>
                  <Download size={17} />
                  <span>
                    {multitouchStatus === "partial" ? "Reinstall" : "Declare Touch Features"}
                  </span>
                </>
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
