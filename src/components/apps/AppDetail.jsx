import React from "react";
import {
  Play,
  Square,
  Gamepad2,
  Copy,
  Trash2,
  Download,
  MousePointerClick,
} from "lucide-react";
import AppIcon from "./AppIcon";

/**
 * Right-hand panel of the Control Center: the selected app and every action
 * on it, always visible instead of hidden behind a right-click.
 */
export default function AppDetail({
  app,
  system,
  engineOnline,
  booting = false,
  busy = false,
  onLaunch,
  onForceStop,
  onOpenKeymap,
  onCopyPackage,
  onBackupApk,
  savingApk = false,
  onUninstall,
}) {
  if (!app) {
    return (
      <section className="app-detail app-detail-empty" aria-label="App details">
        <MousePointerClick size={36} aria-hidden="true" />
        <p>Select an app in the list to see its actions.</p>
      </section>
    );
  }

  return (
    <section className="app-detail" aria-labelledby="app-detail-title">
      <div className="app-detail-head">
        <AppIcon
          key={app.package}
          app={app}
          size={88}
          className="app-detail-icon"
        />
        <div className="app-detail-titles">
          <h2 id="app-detail-title">{app.name}</h2>
          <span className="app-detail-package">{app.package}</span>
          <span className={`app-tag ${system ? "system" : "user"}`}>
            {system ? "System app" : "User app"}
          </span>
        </div>
      </div>

      <div className="app-detail-actions">
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => onLaunch(app.package)}
          disabled={busy || booting}
          title={booting ? "Android is still booting" : undefined}
        >
          <Play size={16} fill="currentColor" aria-hidden="true" />
          <span>Launch</span>
        </button>
        <button
          type="button"
          className="btn btn-outline"
          onClick={() => onForceStop(app.package, app.name)}
          disabled={busy || !engineOnline}
          title={
            engineOnline
              ? `Force-stop ${app.name}`
              : "Start the engine to stop running apps"
          }
        >
          <Square size={14} aria-hidden="true" />
          <span>Force stop</span>
        </button>
        <button
          type="button"
          className="btn btn-outline"
          onClick={onOpenKeymap}
        >
          <Gamepad2 size={16} aria-hidden="true" />
          <span>Keymapping</span>
        </button>
        <button
          type="button"
          className="btn btn-outline"
          onClick={() => onCopyPackage(app.package)}
        >
          <Copy size={15} aria-hidden="true" />
          <span>Copy package ID</span>
        </button>
        {onBackupApk && (
          <button
            type="button"
            className="btn btn-outline"
            onClick={() => onBackupApk(app.package, app.name)}
            disabled={busy || !engineOnline || booting || savingApk}
            aria-busy={savingApk}
            title={
              engineOnline
                ? "Save the app's APKs to Downloads (app data is not included)"
                : "Start the engine to save the app's APKs"
            }
          >
            <Download size={15} aria-hidden="true" />
            <span>{savingApk ? "Saving APK…" : "Save APK"}</span>
          </button>
        )}
        <button
          type="button"
          className="btn btn-outline-danger"
          onClick={() => onUninstall(app.package, app.name)}
          disabled={busy}
        >
          <Trash2 size={15} aria-hidden="true" />
          <span>Uninstall</span>
        </button>
      </div>

      <dl className="app-detail-facts">
        <div className="app-fact">
          <dt>Source</dt>
          <dd>{system ? "Preinstalled with Android" : "Installed by you"}</dd>
        </div>
        <div className="app-fact">
          <dt>Engine</dt>
          <dd>{booting ? "Booting" : engineOnline ? "Running" : "Stopped"}</dd>
        </div>
      </dl>

      {!system && (
        <p className="app-detail-note">
          Games with strict anti-cheat may detect Waydroid or the keymapper.
          ArchDroid does not modify the APK.
        </p>
      )}
    </section>
  );
}
