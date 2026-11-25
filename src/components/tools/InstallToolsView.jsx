import React from "react";
import {
  Check,
  Download,
  CheckCircle2,
  AlertTriangle,
  HelpCircle,
  Layers,
  Cpu,
  Puzzle,
  ShieldBan,
  RotateCw,
} from "lucide-react";

export default function InstallToolsView({
  gappsStatus,
  houdiniStatus,
  multitouchStatus,
  installing,
  onInstallGApps,
  onInstallHoudini,
  onInstallMultitouch,
  adblockStatus,
  onToggleAdblock,
  adguardDns = { state: "unknown" },
  engineRunning,
  onRestartEngine,
}) {
  // The overlay file decides on/off; "partial" and "pending" mean it is on
  // but Android has not loaded or confirmed it yet.
  const adblockOn = ["installed", "partial", "pending"].includes(adblockStatus);
  // Android's Private DNS (useAdguardDns): set over adb, so only once Android
  // has booted, and never over a server ArchDroid did not set.
  const dnsOn = adguardDns.state === "on";
  const adblockBadge = {
    installed: ["badge-success", "On"],
    partial: ["badge-warning", "On after restart"],
    pending: ["badge-warning", "On, unverified"],
    not_installed: ["badge-neutral", "Off"],
  }[adblockStatus] ?? ["badge-neutral", "Checking..."];

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
      case "pending":
        return (
          <span
            className="badge-pill badge-warning"
            title="Installed, but not verified yet. Start Waydroid to verify."
          >
            <AlertTriangle size={14} /> Unverified
          </span>
        );
      case "not_installed":
        return <span className="badge-pill badge-neutral">Not Installed</span>;
      default:
        return <span className="badge-pill badge-neutral">Checking...</span>;
    }
  };

  return (
    <div className="tools-view-container">
      {/* Intro Info Banner */}
      <div className="info-banner">
        <HelpCircle size={20} className="info-icon" />
        <div className="info-text">
          <strong>Add-ons change the Android system image</strong>, so each one
          asks for administrator rights (polkit or sudo) and shows what it will
          do first.
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
                <h3>Google Play Services (GApps)</h3>
                {renderStatusBadge(gappsStatus)}
              </div>
              <p>
                Injects Google Play Store, Play Services, and Google Services
                Framework into the Android container to allow downloading apps
                directly.
              </p>
            </div>
          </div>

          <div className="tool-card-footer">
            <span className="tool-requirement-hint">
              Root required • waydroid_script (48dbfaf)
            </span>
            <button
              className={
                gappsStatus === "installed"
                  ? "btn btn-outline"
                  : "btn btn-primary"
              }
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
                Translates ARM and ARM64 native binaries on x86_64 processors.
                Crucial for playing modern Android 3D mobile games (PUBG,
                Genshin Impact, Roblox, etc.) that do not provide native x86
                libraries.
              </p>
            </div>
          </div>

          <div className="tool-card-footer">
            <span className="tool-requirement-hint">
              Root required • waydroid_script (48dbfaf)
            </span>
            <button
              className={
                houdiniStatus === "installed"
                  ? "btn btn-outline"
                  : "btn btn-primary"
              }
              onClick={onInstallHoudini}
              disabled={installing || houdiniStatus === "installed"}
            >
              {houdiniStatus === "installed" ? (
                <>
                  <CheckCircle2 size={17} />
                  <span>Installed</span>
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
                Declares the multitouch capabilities of Waydroid's touchscreen
                in the vendor partition. Google Play hides every app that
                requires a feature the device never declares — Plants vs Zombies
                2 requires
                <code>android.hardware.touchscreen.multitouch</code> — and it
                reports that with the same &ldquo;isn&apos;t compatible with
                this version&rdquo; message an ARM translation problem produces.
              </p>
            </div>
          </div>

          <div className="tool-card-footer">
            <span className="tool-requirement-hint">
              {multitouchStatus === "partial"
                ? "Written to disk — restart Waydroid to activate"
                : "Root required • Vendor overlay"}
            </span>
            <button
              className={
                multitouchStatus === "installed"
                  ? "btn btn-outline"
                  : "btn btn-primary"
              }
              onClick={onInstallMultitouch}
              disabled={installing || multitouchStatus === "installed"}
            >
              {multitouchStatus === "installed" ? (
                <>
                  <CheckCircle2 size={17} />
                  <span>Installed</span>
                </>
              ) : (
                <>
                  <Download size={17} />
                  <span>
                    {multitouchStatus === "partial"
                      ? "Reinstall"
                      : "Declare Touch Features"}
                  </span>
                </>
              )}
            </button>
          </div>
        </div>

        {/* Ad blocking (hosts file in the system overlay) Card */}
        <div className="glass-panel tool-card" data-testid="adblock-card">
          <div className="tool-card-header">
            <div className="tool-icon-wrap gapps-icon-wrap">
              <ShieldBan size={26} />
            </div>
            <div className="tool-header-text">
              <div className="tool-title-row">
                <h3>Block Ads</h3>
                <span className={`badge-pill ${adblockBadge[0]}`}>
                  {adblockBadge[1]}
                </span>
              </div>
              <p>
                Stops games and apps from reaching mobile ad servers (AdMob,
                AppLovin, ironSource, Unity Ads, Facebook Audience Network and
                others), so ads between levels never load. The blocklist is
                built in; nothing is downloaded. Ads you choose to watch for a
                reward stop loading too.
              </p>
            </div>
          </div>

          <div
            className="config-toggle-item"
            data-testid="adguard-dns"
            data-dns-state={adguardDns.state}
          >
            <div className="toggle-info">
              <h4 id="adguard-dns-title">Also block with AdGuard DNS</h4>
              <p>
                For ads the built-in list misses. Sets Android&apos;s Private
                DNS to AdGuard DNS, which blocks ad servers by name. While it is
                on, every DNS query Android makes (the name of each server an
                app connects to) goes to AdGuard at{" "}
                <code>dns.adguard-dns.com</code> instead of your network&apos;s
                DNS. If your network blocks DNS over TLS (port 853), Android
                cannot look up any name until you turn it off. Off by default;
                no root, no restart.
              </p>
              {adguardDns.state === "foreign" && (
                <p className="dns-note" role="status">
                  {adguardDns.message}
                </p>
              )}
              {!engineRunning && (
                <p className="dns-note">Available once Android has booted.</p>
              )}
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={dnsOn}
              aria-labelledby="adguard-dns-title"
              className={`toggle-switch ${dnsOn ? "active" : ""}`}
              onClick={() => adguardDns.setEnabled(!dnsOn)}
              disabled={
                !engineRunning ||
                installing ||
                adguardDns.changing ||
                adguardDns.state === "foreign"
              }
            >
              <div className="toggle-switch-handle">
                {dnsOn && <Check size={12} />}
              </div>
            </button>
          </div>

          <div className="tool-card-footer">
            <span className="tool-requirement-hint">
              {adblockStatus === "partial"
                ? "Restart Waydroid to apply"
                : "Root required • System overlay"}
            </span>
            <div className="tool-card-actions">
              {adblockStatus === "partial" && engineRunning && (
                <button
                  className="btn btn-outline"
                  onClick={onRestartEngine}
                  disabled={installing}
                >
                  <RotateCw size={17} />
                  <span>Restart Waydroid</span>
                </button>
              )}
              <button
                className={adblockOn ? "btn btn-outline" : "btn btn-primary"}
                onClick={() => onToggleAdblock(!adblockOn)}
                disabled={installing || adblockStatus === "unknown"}
              >
                <ShieldBan size={17} />
                <span>{adblockOn ? "Turn Off" : "Block Ads"}</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
