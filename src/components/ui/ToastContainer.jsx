import React from "react";
import { CheckCircle2, AlertCircle, AlertTriangle, Info, X } from "lucide-react";

export default function ToastContainer({ toasts, onDismiss }) {
  if (!toasts || toasts.length === 0) return null;

  const renderIcon = (type) => {
    switch (type) {
      case "success":
        return <CheckCircle2 size={18} className="toast-icon success" />;
      case "error":
        return <AlertCircle size={18} className="toast-icon error" />;
      case "warning":
        return <AlertTriangle size={18} className="toast-icon warning" />;
      case "info":
      default:
        return <Info size={18} className="toast-icon info" />;
    }
  };

  return (
    <div className="toast-container" aria-live="polite">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={`toast-item toast-${toast.type || "info"}`}
        >
          <div className="toast-icon-wrapper">
            {renderIcon(toast.type)}
          </div>

          <div className="toast-content">
            {toast.title && <h5 className="toast-title">{toast.title}</h5>}
            <p className="toast-message">{toast.message}</p>
          </div>

          <button
            type="button"
            className="toast-close-btn"
            onClick={() => onDismiss(toast.id)}
            aria-label="Dismiss notification"
          >
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
