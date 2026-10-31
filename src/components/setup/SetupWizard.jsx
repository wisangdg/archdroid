import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  MonitorPlay,
  Download,
  HardDrive,
  Cpu,
  ShieldCheck,
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  Info,
  RotateCw,
  Wrench,
} from "lucide-react";

function statusIcon(status) {
  switch (status) {
    case "ok":
      return CheckCircle2;
    case "warning":
      return AlertTriangle;
    case "error":
      return AlertCircle;
    default:
      return Info;
  }
}

function badgeClass(status) {
  switch (status) {
    case "ok":
      return "badge-ok";
    case "warning":
      return "badge-warn";
    case "error":
      return "badge-err";
    default:
      return "badge-info";
  }
}

function badgeLabel(status) {
  switch (status) {
    case "ok":
      return "Ready";
    case "warning":
      return "Warning";
    case "error":
      return "Action Needed";
    default:
      return "Notice";
  }
}

/**
 * Init needs elevation, a working network, enough disk, and either a Waydroid
 * binary or a package manager that can install it. Everything else is advisory
 * for initialization and is still shown with its remediation.
 */
function blocksInitialization(item, report) {
  switch (item.key) {
    case "elevation":
      return item.status === "error";
    case "network":
      return item.status !== "ok";
    case "disk":
      return item.status === "warning" || item.status === "error";
    case "waydroid":
      return item.status === "error" && !report.package_manager;
    default:
      return false;
  }
}

export default function SetupWizard({ onInitWaydroid, installing }) {
  const [report, setReport] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [showAll, setShowAll] = useState(false);

  const fetchPreflight = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await invoke("check_host_diagnostics");
      setReport(res);
    } catch (err) {
      console.error("Failed to load preflight diagnostics:", err);
      setReport(null);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchPreflight();
  }, [fetchPreflight]);

  const items = report?.items ?? [];
  // The wizard exists to initialize the image, so that item is the action, not
  // a prerequisite. Keep it visible in "show all" and in Settings diagnostics.
  const issues = items.filter(
    (item) => item.status !== "ok" && item.key !== "waydroid_image"
  );
  const visible = showAll ? items : issues;
  const blocking = report
    ? items.filter((item) => blocksInitialization(item, report))
    : [];
  const blocked = blocking.length > 0;
  const canInitialize = Boolean(report) && !loading && !blocked && !installing;

  return (
    <div className="setup-wizard-container">
      <div className="glass-panel setup-wizard-card">
        <div className="setup-glow-orb">
          <MonitorPlay size={72} className="icon-glow setup-hero-icon" />
        </div>

        <h2 className="setup-title">Android System Image Required</h2>
        <p className="setup-description">
          Your Linux machine is ready to host Waydroid, but the base Android
          system container has not been initialized yet. Click below to download
          the official LineageOS image (~1 GB) and configure your emulator
          automatically.
        </p>

        <div className="setup-specs-list">
          <div className="setup-spec-item">
            <HardDrive size={18} />
            <span>~1.2 GB Storage Required</span>
          </div>
          <div className="setup-spec-item">
            <Cpu size={18} />
            <span>Native Kernel LXC Container</span>
          </div>
          <div className="setup-spec-item">
            <ShieldCheck size={18} />
            <span>Root Elevation via Polkit</span>
          </div>
        </div>

        <div className="setup-preflight">
          <div className="setup-preflight-head">
            <div className="setup-preflight-heading">
              <Wrench size={18} />
              <div>
                <h3>Preflight Checks</h3>
                <p>
                  Host dependencies are verified before root access is
                  requested.
                </p>
              </div>
            </div>
            <button
              type="button"
              className="btn btn-outline btn-sm"
              onClick={fetchPreflight}
              disabled={loading || installing}
              title="Refresh preflight checks"
            >
              <RotateCw size={13} className={loading ? "spinner" : ""} />
              <span>{loading ? "Checking..." : "Re-check"}</span>
            </button>
          </div>

          {loading && !report ? (
            <div className="setup-preflight-placeholder">
              <RotateCw size={16} className="spinner" />
              <span>Checking Linux host dependencies...</span>
            </div>
          ) : error ? (
            <div className="setup-preflight-placeholder setup-preflight-error">
              <AlertCircle size={16} />
              <span>Preflight scan failed: {error}</span>
            </div>
          ) : visible.length === 0 ? (
            <div className="setup-preflight-ok">
              <CheckCircle2 size={16} />
              <span>All preflight checks passed.</span>
            </div>
          ) : (
            <ul className="setup-preflight-list">
              {visible.map((item) => {
                const StatusIcon = statusIcon(item.status);
                return (
                  <li
                    key={item.key}
                    className={`setup-preflight-item status-${item.status || "info"}`}
                  >
                    <div className="setup-preflight-item-head">
                      <span className="setup-preflight-item-title">
                        <StatusIcon size={14} />
                        {item.title}
                      </span>
                      <span className={`diag-badge ${badgeClass(item.status)}`}>
                        {badgeLabel(item.status)}
                      </span>
                    </div>
                    <span className="setup-preflight-item-value">
                      {item.value}
                    </span>
                    <p className="setup-preflight-item-detail">{item.detail}</p>
                    {item.remediation && (
                      <code className="setup-preflight-fix">
                        {item.remediation}
                      </code>
                    )}
                  </li>
                );
              })}
            </ul>
          )}

          {report && items.length > 0 && (
            <button
              type="button"
              className="setup-preflight-toggle"
              onClick={() => setShowAll((value) => !value)}
            >
              {showAll ? "Show only issues" : `Show all ${items.length} checks`}
            </button>
          )}

          {blocked && (
            <p className="setup-preflight-blocked">
              <AlertCircle size={15} />
              Resolve the blocking items above before initializing.
            </p>
          )}
        </div>

        <button
          className="btn btn-primary btn-glow btn-lg"
          onClick={onInitWaydroid}
          disabled={!canInitialize}
          title={
            blocked
              ? "Resolve the blocking preflight items first"
              : "Download and initialize the Android image"
          }
        >
          <Download size={20} />
          <span>
            {installing
              ? "Initializing System..."
              : "Initialize Android Emulator"}
          </span>
        </button>
      </div>
    </div>
  );
}
