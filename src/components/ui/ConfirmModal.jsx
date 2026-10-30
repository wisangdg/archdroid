import React, { useEffect } from "react";
import { AlertTriangle, AlertOctagon, Trash2, ShieldAlert, X } from "lucide-react";

export default function ConfirmModal({
  isOpen,
  title,
  description,
  confirmText = "Confirm",
  cancelText = "Cancel",
  confirmVariant = "danger",
  iconType = "danger",
  onConfirm,
  onCancel,
}) {
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e) => {
      if (e.key === "Escape") {
        onCancel();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onCancel]);

  if (!isOpen) return null;

  const renderIcon = () => {
    switch (iconType) {
      case "trash":
        return <Trash2 size={24} className="confirm-icon danger" />;
      case "danger":
        return <AlertOctagon size={24} className="confirm-icon danger" />;
      case "shield":
        return <ShieldAlert size={24} className="confirm-icon warning" />;
      case "warning":
      default:
        return <AlertTriangle size={24} className="confirm-icon warning" />;
    }
  };

  return (
    <div className="confirm-modal-overlay" onClick={onCancel}>
      <div
        className="glass-panel confirm-modal-card"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-modal-title"
      >
        <div className="confirm-modal-header">
          <div className="confirm-icon-wrap">
            {renderIcon()}
          </div>
          <button
            type="button"
            className="btn-icon-close"
            onClick={onCancel}
            aria-label="Close dialog"
          >
            <X size={18} />
          </button>
        </div>

        <div className="confirm-modal-body">
          <h3 id="confirm-modal-title" className="confirm-modal-title">{title}</h3>
          <p className="confirm-modal-desc">{description}</p>
        </div>

        <div className="confirm-modal-actions">
          <button
            type="button"
            className="btn btn-outline"
            onClick={onCancel}
          >
            {cancelText}
          </button>
          <button
            type="button"
            className={`btn ${confirmVariant === "danger" ? "btn-danger" : "btn-primary"}`}
            onClick={onConfirm}
            autoFocus
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}
