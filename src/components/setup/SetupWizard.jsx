import React from "react";
import { MonitorPlay, Download, HardDrive, Cpu, ShieldCheck } from "lucide-react";

export default function SetupWizard({ onInitWaydroid, installing }) {
  return (
    <div className="setup-wizard-container">
      <div className="glass-panel setup-wizard-card">
        <div className="setup-glow-orb">
          <MonitorPlay size={72} className="icon-glow setup-hero-icon" />
        </div>

        <h2 className="setup-title">Android System Image Required</h2>
        <p className="setup-description">
          Your Linux machine is ready to host Waydroid, but the base Android system container has not been initialized yet.
          Click below to download the official LineageOS image (~1 GB) and configure your emulator automatically.
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

        <button
          className="btn btn-primary btn-glow btn-lg"
          onClick={onInitWaydroid}
          disabled={installing}
        >
          <Download size={20} />
          <span>{installing ? "Initializing System..." : "Initialize Android Emulator"}</span>
        </button>
      </div>
    </div>
  );
}
