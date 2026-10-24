import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ShieldCheck,
  CheckCircle2,
  AlertTriangle,
  AlertCircle,
  Info,
  RotateCw,
  Monitor,
  Cpu,
  Layers,
  Gamepad2,
  Server,
} from "lucide-react";

export default function HostDiagnostics() {
  const [report, setReport] = useState(null);
  const [loading, setLoading] = useState(false);

  const fetchDiagnostics = async () => {
    setLoading(true);
    try {
      const res = await invoke("check_host_diagnostics");
      setReport(res);
    } catch (err) {
      console.error("Failed to load host diagnostics:", err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchDiagnostics();
  }, []);

  const getIcon = (key) => {
    switch (key) {
      case "session":
        return Monitor;
      case "kvm":
        return Cpu;
      case "binder":
        return Layers;
      case "gpu":
        return Gamepad2;
      case "service":
        return Server;
      default:
        return ShieldCheck;
    }
  };

  const renderStatusBadge = (status) => {
    switch (status) {
      case "ok":
        return (
          <span className="diag-badge badge-ok">
            <CheckCircle2 size={13} />
            Ready
          </span>
        );
      case "warning":
        return (
          <span className="diag-badge badge-warn">
            <AlertTriangle size={13} />
            Warning
          </span>
        );
      case "error":
        return (
          <span className="diag-badge badge-err">
            <AlertCircle size={13} />
            Action Needed
          </span>
        );
      case "info":
      default:
        return (
          <span className="diag-badge badge-info">
            <Info size={13} />
            Notice
          </span>
        );
    }
  };

  return (
    <div className="glass-panel host-diagnostics-panel">
      {/* Header */}
      <div className="section-title-wrap">
        <ShieldCheck size={22} className="section-title-icon" />
        <div style={{ flex: 1 }}>
          <div className="diag-title-row">
            <h3>Linux Host System Diagnostics</h3>
            <button
              type="button"
              className="btn btn-outline btn-sm btn-diag-refresh"
              onClick={fetchDiagnostics}
              disabled={loading}
              title="Refresh diagnostic status"
            >
              <RotateCw size={13} className={loading ? "spinner" : ""} />
              <span>{loading ? "Checking..." : "Re-check"}</span>
            </button>
          </div>
          <p>
            Verifies Linux kernel modules, display protocol, GPU passthrough, and virtualization compatibility for Waydroid.
          </p>
        </div>
      </div>

      {/* Diagnostics Cards Grid */}
      <div className="diag-grid">
        {report && report.items ? (
          report.items.map((item) => {
            const Icon = getIcon(item.key);
            return (
              <div
                key={item.key}
                className={`diag-card diag-status-${item.status || "info"}`}
              >
                <div className="diag-card-top">
                  <div className="diag-icon-wrapper">
                    <Icon size={18} />
                  </div>
                  {renderStatusBadge(item.status)}
                </div>

                <div className="diag-card-body">
                  <h4 className="diag-item-title">{item.title}</h4>
                  <span className="diag-item-value">{item.value}</span>
                  <p className="diag-item-detail">{item.detail}</p>
                </div>
              </div>
            );
          })
        ) : (
          <div className="diag-loading-placeholder">
            <RotateCw size={18} className="spinner" />
            <span>Scanning Linux host environment...</span>
          </div>
        )}
      </div>
    </div>
  );
}
