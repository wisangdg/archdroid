import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LifeBuoy } from "lucide-react";

export default function DiagnosticBundlePanel() {
  const [busy, setBusy] = useState(false);
  const [bundle, setBundle] = useState(null);
  const [note, setNote] = useState("");

  const create = async () => {
    setBusy(true);
    setNote("Collecting host diagnostics and logs…");
    try {
      const result = await invoke("diagnostic_bundle_create");
      setBundle(result);
      setNote(`Saved to ${result.path}. Review it before sharing.`);
    } catch (err) {
      setBundle(null);
      setNote(`Could not create the bundle: ${err?.message || String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(bundle?.content ?? "");
      setNote("Copied. Home paths and secrets are already masked.");
    } catch {
      setNote("Copy failed; open the saved file instead.");
    }
  };

  return (
    <div className="glass-panel settings-card">
      <div className="settings-card-content">
        <div className="settings-card-icon">
          <LifeBuoy size={24} aria-hidden="true" />
        </div>
        <div className="settings-card-text">
          <h3>Diagnostic Bundle</h3>
          <p>
            One text file for bug reports: system and Waydroid versions, host
            checks, add-on state, and recent logs. The logs can include Android
            package names and APK file names you have used; home paths and
            secrets are masked. Collected without root, saved to your Downloads
            folder, and never uploaded.
          </p>
          {note && <p role="status">{note}</p>}
          {bundle && (
            <pre
              className="app-log-view"
              aria-label="Diagnostic bundle preview"
              style={{ maxHeight: 240, overflow: "auto" }}
            >
              {bundle.content}
            </pre>
          )}
        </div>
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        <button className="btn btn-outline" onClick={create} disabled={busy}>
          {busy ? "Creating…" : "Create Bundle"}
        </button>
        <button className="btn btn-outline" onClick={copy} disabled={!bundle}>
          Copy
        </button>
      </div>
    </div>
  );
}
