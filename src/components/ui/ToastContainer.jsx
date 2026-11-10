import React, { useEffect, useRef, useState } from "react";
import {
  CheckCircle2,
  AlertCircle,
  AlertTriangle,
  Info,
  X,
  Copy,
  Check,
} from "lucide-react";

const ICONS = {
  success: CheckCircle2,
  error: AlertCircle,
  warning: AlertTriangle,
  info: Info,
};

function Toast({ toast, onDismiss }) {
  const type = ICONS[toast.type] ? toast.type : "info";
  const Icon = ICONS[type];
  const [paused, setPaused] = useState(false);
  const [copied, setCopied] = useState(false);
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  // The timer restarts when the pointer or the focus leaves: a message being
  // read (or copied) must not vanish.
  useEffect(() => {
    if (paused || !toast.duration) return undefined;
    const timer = setTimeout(
      () => onDismissRef.current(toast.id),
      toast.duration
    );
    return () => clearTimeout(timer);
  }, [paused, toast.id, toast.duration]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(
        [toast.title, toast.message].filter(Boolean).join(": ")
      );
      setCopied(true);
    } catch (err) {
      console.error("Copy failed:", err);
    }
  };

  return (
    <div
      className={`toast-item toast-${type}`}
      role="status"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
    >
      <div className="toast-icon-wrapper">
        <Icon size={18} className={`toast-icon ${type}`} aria-hidden="true" />
      </div>

      <div className="toast-content">
        {toast.title && <h5 className="toast-title">{toast.title}</h5>}
        <p className="toast-message">{toast.message}</p>
      </div>

      {(type === "error" || type === "warning") && (
        <button
          type="button"
          className="toast-close-btn"
          onClick={copy}
          aria-label="Copy message"
          title="Copy message"
        >
          {copied ? (
            <Check size={14} aria-hidden="true" />
          ) : (
            <Copy size={14} aria-hidden="true" />
          )}
        </button>
      )}
      <button
        type="button"
        className="toast-close-btn"
        onClick={() => onDismiss(toast.id)}
        aria-label="Dismiss notification"
      >
        <X size={14} aria-hidden="true" />
      </button>
    </div>
  );
}

/**
 * The live region is always in the page: a region that appears together with
 * its first message is not announced by screen readers.
 */
export default function ToastContainer({ toasts, onDismiss }) {
  return (
    <div className="toast-container" aria-live="polite">
      {(toasts || []).map((toast) => (
        <Toast key={toast.id} toast={toast} onDismiss={onDismiss} />
      ))}
    </div>
  );
}
