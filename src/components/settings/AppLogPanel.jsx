import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FileText } from "lucide-react";

export default function AppLogPanel() {
  const [location, setLocation] = useState("");
  const [text, setText] = useState(null);
  const [note, setNote] = useState("");

  useEffect(() => {
    invoke("app_log_location")
      .then((res) => res?.status === "success" && setLocation(res.data))
      .catch(() => {});
  }, []);

  const view = async () => {
    const res = await invoke("app_log_read");
    setText(res?.status === "success" ? res.data || "(log is empty)" : "");
    setNote(res?.status === "success" ? "" : "Could not read the log.");
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text ?? "");
      setNote("Copied. Personal paths and secrets are already masked.");
    } catch {
      setNote("Copy failed; select the text manually.");
    }
  };

  const clear = async () => {
    const res = await invoke("app_log_clear");
    setText(null);
    setNote(res?.status === "success" ? "Log cleared." : "Could not clear.");
  };

  return (
    <div className="glass-panel settings-card">
      <div className="settings-card-content">
        <div className="settings-card-icon">
          <FileText size={24} aria-hidden="true" />
        </div>
        <div className="settings-card-text">
          <h3>Application Log</h3>
          <p>
            Stored locally only; ArchDroid never uploads logs or telemetry. Home
            paths and secrets are masked when shown. Location:{" "}
            <code>{location || "…"}</code>
          </p>
          {note && <p role="status">{note}</p>}
          {text !== null && (
            <pre
              className="app-log-view"
              style={{ maxHeight: 240, overflow: "auto" }}
            >
              {text}
            </pre>
          )}
        </div>
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        <button className="btn btn-outline" onClick={view}>
          View
        </button>
        <button className="btn btn-outline" onClick={copy} disabled={!text}>
          Copy
        </button>
        <button className="btn btn-outline" onClick={clear}>
          Clear
        </button>
      </div>
    </div>
  );
}
