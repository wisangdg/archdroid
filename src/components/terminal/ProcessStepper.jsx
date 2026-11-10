import React, { useEffect, useRef } from "react";
import { Check, Loader2, AlertCircle } from "lucide-react";

export default function ProcessStepper({
  steps,
  activeStepIndex,
  isFailed,
  isCancelled,
  isCompleted,
}) {
  // Seven stages do not fit the default window: keep the current one in view.
  const activeRef = useRef(null);
  useEffect(() => {
    activeRef.current?.scrollIntoView?.({ inline: "center", block: "nearest" });
  }, [activeStepIndex]);

  if (!steps || steps.length === 0) return null;

  return (
    <div className="stepper-container">
      <div className="stepper-track">
        {steps.map((step, idx) => {
          let stepStatus = "pending";
          if (isFailed && idx === activeStepIndex) {
            stepStatus = "failed";
          } else if (idx < activeStepIndex || isCompleted) {
            stepStatus = "completed";
          } else if (idx === activeStepIndex && !isCancelled) {
            stepStatus = "active";
          }

          return (
            <React.Fragment key={step.id}>
              {/* Step Node */}
              <div
                ref={idx === activeStepIndex ? activeRef : undefined}
                className={`stepper-node stepper-node-${stepStatus}`}
              >
                <div className="stepper-circle">
                  {stepStatus === "completed" ? (
                    <Check size={14} className="stepper-icon-check" />
                  ) : stepStatus === "failed" ? (
                    <AlertCircle size={14} className="stepper-icon-failed" />
                  ) : stepStatus === "active" ? (
                    <Loader2
                      size={14}
                      className="stepper-icon-spinner spinner"
                    />
                  ) : (
                    <span className="stepper-number">{idx + 1}</span>
                  )}
                </div>
                <div className="stepper-text">
                  <span className="stepper-label">{step.label}</span>
                  {step.description && (
                    <span className="stepper-desc">{step.description}</span>
                  )}
                </div>
              </div>

              {/* Connecting Line */}
              {idx < steps.length - 1 && (
                <div
                  className={`stepper-line ${
                    idx < activeStepIndex || isCompleted ? "completed" : ""
                  }`}
                />
              )}
            </React.Fragment>
          );
        })}
      </div>
    </div>
  );
}
