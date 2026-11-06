import React from "react";
import { Check, Hand, ShieldCheck } from "lucide-react";
import { touchModeState } from "../../hooks/useTouchMode";

const BADGES = {
  on: { className: "badge-success", label: "On" },
  off: { className: "badge-neutral", label: "Off" },
  stopped: { className: "badge-warning", label: "Stopped" },
  needs_access: { className: "badge-warning", label: "Needs access" },
  unsupported: { className: "badge-neutral", label: "Hyprland only" },
  unknown: { className: "badge-neutral", label: "Checking…" },
};

function StateHint({ state, status }) {
  switch (state.kind) {
    case "unsupported":
      return (
        <p className="touch-mode-hint">
          Touch mode reads the cursor position from Hyprland. Other compositors
          do not offer that yet; Phantom&apos;s menu-touch mode below works
          there.
        </p>
      );
    case "needs_access":
      return (
        <p className="touch-mode-hint">
          Your user cannot{" "}
          {status.uinput_access ? "read a mouse" : "create virtual devices"}{" "}
          yet. &quot;Enable access&quot; installs a udev rule that gives the
          logged-in user /dev/uinput and mice (never keyboards).
        </p>
      );
    case "stopped":
      return (
        <p className="touch-mode-hint touch-mode-error" role="alert">
          Touch mode stopped{state.error ? `: ${state.error}` : "."} Switch it
          on to try again.
        </p>
      );
    case "on":
      return (
        <p className="touch-mode-hint">
          Active for {status.mice.join(", ")}. Clicks outside Android windows
          stay normal mouse clicks.
        </p>
      );
    default:
      return null;
  }
}

export default function TouchModePanel({
  status,
  changing,
  installing,
  onToggle,
  onEnableAccess,
}) {
  const state = touchModeState(status);
  const badge = BADGES[state.kind];
  const active = state.kind === "on";
  const canToggle =
    state.kind === "on" || state.kind === "off" || state.kind === "stopped";

  return (
    <section
      className="glass-panel touch-mode-panel"
      aria-labelledby="touch-mode-title"
    >
      <div className="config-toggle-item">
        <div className="feature-icon-badge" aria-hidden="true">
          <Hand size={22} />
        </div>
        <div className="toggle-info touch-mode-info">
          <h4 id="touch-mode-title">
            Touch Mode{" "}
            <span className={`badge-pill ${badge.className}`}>
              {badge.label}
            </span>
          </h4>
          <p>
            Makes a left click or drag over an Android window act like a finger,
            the way other Android emulators do. Some games and web pages inside
            apps ignore mouse clicks entirely.
          </p>
          <StateHint state={state} status={status} />
        </div>
        {state.kind === "needs_access" ? (
          <button
            type="button"
            className="btn btn-outline"
            onClick={onEnableAccess}
            disabled={installing}
          >
            <ShieldCheck size={18} />
            <span>Enable access</span>
          </button>
        ) : (
          <button
            type="button"
            role="switch"
            aria-checked={active}
            aria-label="Touch mode"
            className={`toggle-switch ${active ? "active" : ""}`}
            onClick={() => onToggle(!active)}
            disabled={!canToggle || changing}
          >
            <div className="toggle-switch-handle">
              {active && <Check size={12} />}
            </div>
          </button>
        )}
      </div>
    </section>
  );
}
