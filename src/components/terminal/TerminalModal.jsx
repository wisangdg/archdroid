import React, {
  useState,
  useMemo,
  useEffect,
  useLayoutEffect,
  useRef,
} from "react";
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
  ArrowDownToLine,
} from "lucide-react";
import ProcessStepper from "./ProcessStepper";
import {
  detectStepProgress,
  terminalOutcome,
  TERMINAL_OUTCOMES,
} from "./stepConfigs";
import { useFocusTrap } from "../../lib/useFocusTrap";
import { progressPercent } from "../../lib/taskEvents";

/** Within this many pixels of the end, new output keeps the view at the end. */
const STICK_TO_END_PX = 40;

export default function TerminalModal({
  isOpen,
  title,
  taskType = "logs", // 'gapps' | 'houdini' | 'init' | 'phantom' | 'wipe' | 'logs'
  logs = [],
  dropped = 0,
  currentStep = null,
  runId = 0,
  taskResult = null,
  installing,
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
  const consoleRef = useRef(null);
  // Follow new output only while the reader is at the end; scrolling up to
  // read something earlier must not be undone by the next line.
  const [atEnd, setAtEnd] = useState(true);
  const atEndRef = useRef(true);

  const handleConsoleScroll = (e) => {
    const el = e.currentTarget;
    const end =
      el.scrollHeight - el.scrollTop - el.clientHeight <= STICK_TO_END_PX;
    atEndRef.current = end;
    setAtEnd(end);
  };

  const jumpToEnd = () => {
    const el = consoleRef.current;
    if (el) el.scrollTop = el.scrollHeight;
    atEndRef.current = true;
    setAtEnd(true);
  };

  // After the lines are in the DOM, so the last one is really visible.
  useLayoutEffect(() => {
    const el = consoleRef.current;
    if (el && atEndRef.current) el.scrollTop = el.scrollHeight;
  }, [logs, showConsole]);

  // A new task starts with the console closed (stepper tasks) and at the end.
  useEffect(() => {
    setShowConsole(false);
    atEndRef.current = true;
    setAtEnd(true);
  }, [runId]);

  // Compute step progress from logs and structured taskResult
  const progress = useMemo(() => {
    return detectStepProgress(taskType, logs, taskResult, currentStep);
  }, [taskType, logs, taskResult, currentStep]);

  const percent = installing ? progressPercent(logs[logs.length - 1]) : null;
  const outcome = terminalOutcome(progress, installing);
  const view = TERMINAL_OUTCOMES[outcome];

  // If viewing logs or if process failed/cancelled, ensure console is visible
  useEffect(() => {
    if (taskType === "logs" || progress.isFailed || progress.isCancelled) {
      setShowConsole(true);
    }
  }, [taskType, runId, progress.isFailed, progress.isCancelled]);

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
        data-task-type={taskType}
        data-task-outcome={outcome}
      >
        {/* Modal Header */}
        <div className="terminal-header">
          <div className="terminal-title-wrap">
            <div className={`terminal-icon-badge ${view.badge}`}>
              <OutcomeIcon outcome={outcome} />
            </div>

            <div>
              <h3 id="terminal-modal-title" className="terminal-title">
                {title}
              </h3>
              <span className="terminal-status-text">{view.status}</span>
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
            <div className={`stepper-status-card ${view.card}`}>
              <div className="status-card-header">
                <span className="status-card-phase">
                  {view.phase ??
                    `Stage ${progress.activeStepIndex + 1} of ${progress.steps.length}: ${
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
              {percent !== null && (
                <progress
                  className="status-card-progress"
                  max="100"
                  value={percent}
                  aria-label={`Download progress: ${percent}%`}
                />
              )}

              {(outcome === "failed" || outcome === "cancelled") && (
                <div className="status-failure-advice">
                  {outcome === "cancelled" ? (
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
          <div
            ref={consoleRef}
            className="terminal-console-body"
            onScroll={handleConsoleScroll}
          >
            {logs.length === 0 ? (
              <div className="terminal-empty-text">
                <Loader2 size={16} className="spinner" />
                <span>Awaiting elevated process stream...</span>
              </div>
            ) : (
              logs.map((line, idx) => (
                <div
                  key={dropped + idx}
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
              {logs.length} lines
              {dropped > 0 && ` (${dropped} earlier lines not kept)`}
            </span>
            {(showConsole || !hasStepper) && !atEnd && (
              <button
                type="button"
                className="btn btn-outline btn-sm"
                onClick={jumpToEnd}
              >
                <ArrowDownToLine size={14} aria-hidden="true" />
                <span>Latest</span>
              </button>
            )}
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
                    <span>Cancelling...</span>
                  </>
                ) : (
                  <>
                    <Ban size={14} />
                    <span>Cancel</span>
                  </>
                )}
              </button>
            )}

            {outcome === "failed" && onRetry && (
              <button className="btn btn-outline btn-sm" onClick={onRetry}>
                <RotateCw size={14} />
                <span>Retry</span>
              </button>
            )}
            {!installing && (
              <button className="btn btn-primary btn-sm" onClick={onClose}>
                {outcome === "succeeded" ? "Done" : "Close"}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

const OUTCOME_ICONS = {
  running: Loader2,
  succeeded: Check,
  failed: AlertTriangle,
  cancelled: Ban,
  ended: Terminal,
};

function OutcomeIcon({ outcome }) {
  const Icon = OUTCOME_ICONS[outcome];
  return (
    <Icon
      size={18}
      className={outcome === "running" ? "spinner" : undefined}
      aria-hidden="true"
    />
  );
}
