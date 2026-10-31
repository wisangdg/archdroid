import React, { useState, useMemo, useEffect } from "react";
import {
  Terminal,
  Copy,
  Check,
  X,
  Loader2,
  ChevronDown,
  ChevronUp,
  Download,
  AlertTriangle,
  RotateCw,
  FileText,
  Ban,
} from "lucide-react";
import ProcessStepper from "./ProcessStepper";
import { detectStepProgress } from "./stepConfigs";
import { useFocusTrap } from "../../lib/useFocusTrap";

export default function TerminalModal({
  isOpen,
  title,
  taskType = "logs", // 'gapps' | 'houdini' | 'init' | 'phantom' | 'wipe' | 'logs'
  logs = [],
  taskResult = null,
  installing,
  terminalRef,
  onClose,
  onRetry,
  onCancel,
}) {
  const modalRef = useFocusTrap(isOpen, installing ? undefined : onClose);
  const [copied, setCopied] = useState(false);
  const [cancelling, setCancelling] = useState(false);

  // Reset cancelling state when installing changes
  useEffect(() => {
    if (!installing) {
      setCancelling(false);
    }
  }, [installing]);

  const handleCancelClick = async () => {
    setCancelling(true);
    if (onCancel) {
      await onCancel();
    }
  };
  const [showConsole, setShowConsole] = useState(false);

  // Compute step progress from logs and structured taskResult
  const progress = useMemo(() => {
    return detectStepProgress(taskType, logs, taskResult);
  }, [taskType, logs, taskResult]);

  // If viewing logs or if process failed/cancelled, ensure console is visible
  useEffect(() => {
    if (taskType === "logs" || progress.isFailed || progress.isCancelled) {
      setShowConsole(true);
    }
  }, [taskType, progress.isFailed, progress.isCancelled]);

  if (!isOpen) return null;

  const handleCopyLogs = async () => {
    try {
      await navigator.clipboard.writeText(logs.join("\n"));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy logs:", err);
    }
  };

  const handleDownloadLogs = () => {
    try {
      const blob = new Blob([logs.join("\n")], {
        type: "text/plain;charset=utf-8",
      });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      const dateStr = new Date()
        .toISOString()
        .replace(/[:.]/g, "-")
        .slice(0, 19);
      link.href = url;
      link.download = `archdroid-${taskType}-${dateStr}.log`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
    } catch (err) {
      console.error("Download logs failed:", err);
    }
  };

  const hasStepper = progress.steps.length > 0;

  return (
    <div className="terminal-overlay">
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="terminal-modal-title"
        className={`glass-panel terminal-modal ${hasStepper ? "with-stepper" : ""}`}
      >
        {/* Modal Header */}
        <div className="terminal-header">
          <div className="terminal-title-wrap">
            <div
              className={`terminal-icon-badge ${
                progress.isCancelled
                  ? "badge-cancelled"
                  : progress.isFailed
                    ? "badge-failed"
                    : progress.isCompleted
                      ? "badge-done"
                      : "badge-running"
              }`}
            >
              {installing ? (
                <Loader2 size={18} className="spinner" aria-hidden="true" />
              ) : progress.isCancelled ? (
                <Ban size={18} aria-hidden="true" />
              ) : progress.isFailed ? (
                <AlertTriangle size={18} aria-hidden="true" />
              ) : progress.isCompleted ? (
                <Check size={18} aria-hidden="true" />
              ) : (
                <Terminal size={18} aria-hidden="true" />
              )}
            </div>

            <div>
              <h3 id="terminal-modal-title" className="terminal-title">
                {title}
              </h3>
              <span className="terminal-status-text">
                {installing
                  ? "Task running in background..."
                  : progress.isCancelled
                    ? "Operation cancelled"
                    : progress.isFailed
                      ? "Process encountered errors"
                      : progress.isCompleted
                        ? "Finished successfully"
                        : "Task Complete"}
              </span>
            </div>
          </div>

          <div className="terminal-actions">
            {/* Copy Logs */}
            <button
              type="button"
              className="btn btn-outline btn-sm"
              onClick={handleCopyLogs}
              disabled={logs.length === 0}
              title="Copy output to clipboard"
            >
              {copied ? (
                <>
                  <Check size={14} />
                  <span>Copied</span>
                </>
              ) : (
                <>
                  <Copy size={14} />
                  <span>Copy</span>
                </>
              )}
            </button>

            {/* Export / Download Logs */}
            <button
              type="button"
              className="btn btn-outline btn-sm"
              onClick={handleDownloadLogs}
              disabled={logs.length === 0}
              title="Download log file (.log)"
            >
              <Download size={14} />
              <span>Export</span>
            </button>

            {/* Cancel Button in Header */}
            {installing && onCancel && (
              <button
                type="button"
                className="btn btn-danger btn-sm btn-cancel-top"
                onClick={handleCancelClick}
                disabled={cancelling}
                title="Cancel running operation"
              >
                {cancelling ? (
                  <>
                    <Loader2 size={13} className="spinner" />
                    <span>Cancelling...</span>
                  </>
                ) : (
                  <>
                    <Ban size={13} />
                    <span>Cancel</span>
                  </>
                )}
              </button>
            )}

            {!installing && (
              <button
                type="button"
                className="btn-icon-close"
                onClick={onClose}
                title="Close Window (Esc)"
                aria-label="Close dialog"
              >
                <X size={18} aria-hidden="true" />
              </button>
            )}
          </div>
        </div>

        {/* Visual Stepper (if task has defined stages) */}
        {hasStepper && (
          <div className="terminal-stepper-section">
            <ProcessStepper
              steps={progress.steps}
              activeStepIndex={progress.activeStepIndex}
              isFailed={progress.isFailed}
              isCancelled={progress.isCancelled}
              isCompleted={progress.isCompleted}
            />

            {/* Current Active Status Card */}
            <div
              className={`stepper-status-card ${
                progress.isFailed
                  ? "status-failed"
                  : progress.isCompleted
                    ? "status-completed"
                    : "status-active"
              }`}
            >
              <div className="status-card-header">
                <span className="status-card-phase">
                  {progress.isCancelled
                    ? "Operation Cancelled"
                    : progress.isFailed
                      ? "Execution Failed"
                      : progress.isCompleted
                        ? "All Stages Completed"
                        : `Stage ${progress.activeStepIndex + 1} of ${progress.steps.length}: ${
                            progress.steps[progress.activeStepIndex]?.label
                          }`}
                </span>
                {installing && (
                  <span className="status-live-indicator">
                    <span className="status-dot"></span> In Progress
                  </span>
                )}
              </div>

              <p className="status-card-message">
                {progress.errorMessage || progress.currentMessage}
              </p>

              {(progress.isFailed || progress.isCancelled) && (
                <div className="status-failure-advice">
                  {progress.isCancelled ? (
                    <>
                      <strong>Operation Cancelled:</strong> The process was
                      stopped by user request. Stages marked complete stay
                      applied; the interrupted stage may be incomplete, so run
                      the operation again to finish it.
                    </>
                  ) : (
                    <>
                      <strong>Troubleshooting:</strong> Root authentication may
                      have been rejected, or packages could not be reached.
                      Ensure polkit authentication is configured or check
                      internet connectivity.
                    </>
                  )}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Toggle Detailed Console Accordion */}
        {hasStepper && (
          <div className="console-toggle-bar">
            <button
              type="button"
              className="btn-console-toggle"
              onClick={() => setShowConsole(!showConsole)}
            >
              <Terminal size={15} />
              <span>
                {showConsole
                  ? "Hide Detailed Console"
                  : "Show Detailed Console Output"}
              </span>
              <span className="console-lines-badge">{logs.length} lines</span>
              {showConsole ? (
                <ChevronUp size={15} />
              ) : (
                <ChevronDown size={15} />
              )}
            </button>
          </div>
        )}

        {/* Monospace Output Window */}
        {(showConsole || !hasStepper) && (
          <div ref={terminalRef} className="terminal-console-body">
            {logs.length === 0 ? (
              <div className="terminal-empty-text">
                <Loader2 size={16} className="spinner" />
                <span>Awaiting elevated process stream...</span>
              </div>
            ) : (
              logs.map((line, idx) => (
                <div
                  key={idx}
                  className={`terminal-log-line ${
                    line.includes("[ERROR]")
                      ? "log-error"
                      : line.includes("[SUCCESS]")
                        ? "log-success"
                        : line.includes("[SYSTEM]")
                          ? "log-system"
                          : ""
                  }`}
                >
                  {line}
                </div>
              ))
            )}
          </div>
        )}

        {/* Modal Footer */}
        <div className="terminal-footer">
          <div className="terminal-footer-meta">
            <FileText size={14} className="meta-icon" />
            <span className="terminal-lines-count">
              {logs.length} log lines stream captured
            </span>
          </div>

          <div className="terminal-footer-actions">
            {installing && onCancel && (
              <button
                type="button"
                className="btn btn-danger btn-sm"
                onClick={handleCancelClick}
                disabled={cancelling}
                title="Cancel running operation"
              >
                {cancelling ? (
                  <>
                    <Loader2 size={14} className="spinner" />
                    <span>Cancelling Task...</span>
                  </>
                ) : (
                  <>
                    <Ban size={14} />
                    <span>Cancel Task</span>
                  </>
                )}
              </button>
            )}

            {progress.isFailed && onRetry && !installing && (
              <button className="btn btn-outline btn-sm" onClick={onRetry}>
                <RotateCw size={14} />
                <span>Retry</span>
              </button>
            )}
            {!installing && (
              <button className="btn btn-primary btn-sm" onClick={onClose}>
                {progress.isCompleted ? "Done" : "Close"}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
