import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

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
    <div className="log-row">
      <div className="log-row-text">
        <h4>ArchDroid Log</h4>
        <p>
          This app's own log. Home paths and secrets are masked when shown.
          Location: <code>{location || "…"}</code>
        </p>
        {note && <p role="status">{note}</p>}
        {text !== null && <pre className="app-log-view">{text}</pre>}
      </div>
      <div className="log-row-actions">
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
