import React from "react";
import {
  AlertTriangle,
  AlertOctagon,
  Package,
  Trash2,
  ShieldAlert,
  X,
} from "lucide-react";
import { useFocusTrap } from "../../lib/useFocusTrap";

export default function ConfirmModal({
  isOpen,
  title,
  description,
  confirmText = "Confirm",
  cancelText = "Cancel",
  confirmVariant = "danger",
  iconType = "danger",
  packagePreview = null,
  onConfirm,
  onCancel,
}) {
  const modalRef = useFocusTrap(isOpen, onCancel);

  if (!isOpen) return null;

  const renderIcon = () => {
    switch (iconType) {
      case "trash":
        return (
          <Trash2
            size={24}
            className="confirm-icon danger"
            aria-hidden="true"
          />
        );
      case "danger":
        return (
          <AlertOctagon
            size={24}
            className="confirm-icon danger"
            aria-hidden="true"
          />
        );
      case "shield":
        return (
          <ShieldAlert
            size={24}
            className="confirm-icon warning"
            aria-hidden="true"
          />
        );
      case "warning":
      default:
        return (
          <AlertTriangle
            size={24}
            className="confirm-icon warning"
            aria-hidden="true"
          />
        );
    }
  };

  return (
    <div className="confirm-modal-overlay" onClick={onCancel}>
      <div
        ref={modalRef}
        className="glass-panel confirm-modal-card"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="confirm-modal-title"
      >
        <div className="confirm-modal-header">
          <div className="confirm-icon-wrap">{renderIcon()}</div>
          <button
            type="button"
            className="btn-icon-close"
            onClick={onCancel}
            aria-label="Close dialog"
          >
            <X size={18} aria-hidden="true" />
          </button>
        </div>

        <div className="confirm-modal-body">
          <h3 id="confirm-modal-title" className="confirm-modal-title">
            {title}
          </h3>
          <p className="confirm-modal-desc">{description}</p>
          {packagePreview && (
            <div className="confirm-package-preview">
              <div className="confirm-package-header">
                <Package size={16} />
                <span>System packages this action will install</span>
              </div>
              {packagePreview.packages.length > 0 ? (
                <>
                  <ul className="confirm-package-list">
                    {packagePreview.packages.map((name) => (
                      <li key={name}>{name}</li>
                    ))}
                  </ul>
                  {packagePreview.command ? (
                    <pre className="confirm-package-command">
                      {packagePreview.command}
                    </pre>
                  ) : (
                    <p className="confirm-package-note">
                      No supported package manager was detected. Install these
                      packages manually before continuing.
                    </p>
                  )}
                </>
              ) : (
                <p className="confirm-package-note">
                  No system packages will be installed. The action only uses
                  tools that are already present.
                </p>
              )}
            </div>
          )}
        </div>

        <div className="confirm-modal-actions">
          <button type="button" className="btn btn-outline" onClick={onCancel}>
            {cancelText}
          </button>
          <button
            type="button"
            className={`btn ${confirmVariant === "danger" ? "btn-danger" : "btn-primary"}`}
            onClick={onConfirm}
            data-autofocus="true"
          >
            {confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}
