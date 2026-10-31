import { test } from "node:test";
import assert from "node:assert/strict";
import {
  TASK_STEP_CONFIGS,
  detectStepProgress,
} from "../components/terminal/stepConfigs.js";

test("stepConfigs exports valid steps for each supported task", () => {
  for (const task of ["gapps", "houdini", "init", "phantom", "wipe"]) {
    const steps = TASK_STEP_CONFIGS[task];
    assert.ok(Array.isArray(steps), `steps for ${task} must be an array`);
    assert.ok(steps.length >= 3, `${task} must have at least 3 steps`);
    for (const step of steps) {
      assert.ok(step.id, "step must have id");
      assert.ok(step.label, "step must have label");
      assert.ok(step.description, "step must have description");
    }
  }
});

test("detectStepProgress derives completion strictly from taskResult.success", () => {
  const result = detectStepProgress("gapps", ["[SYSTEM] Done"], {
    success: true,
    code: null,
    message: null,
  });
  assert.equal(result.isCompleted, true);
  assert.equal(result.isFailed, false);
  assert.equal(result.isCancelled, false);
  assert.equal(result.activeStepIndex, TASK_STEP_CONFIGS.gapps.length - 1);
});

test("detectStepProgress derives cancellation strictly from taskResult.code (F6 fix)", () => {
  // Live finding F6: cancel resulted in 'cancelled' code. Stepper must show isCancelled = true
  // and must NOT show failed or root auth error.
  const logs = [
    "[SYSTEM] Elevated shell started",
    "[SYSTEM] Operation stopped by user",
  ];
  const result = detectStepProgress("gapps", logs, {
    success: false,
    code: "cancelled",
    message: "Operation was cancelled by user.",
  });
  assert.equal(result.isCancelled, true);
  assert.equal(result.isFailed, false);
  assert.equal(result.isCompleted, false);
  assert.equal(result.errorMessage, "Operation was cancelled by user.");
  assert.equal(result.currentMessage, "Operation was cancelled by user.");
});

test("detectStepProgress does not falsely fail on words like failed in third-party logs if task succeeded", () => {
  // E.g., harmless log line containing 'failed to find optional cache, falling back...'
  const logs = [
    "[SYSTEM] checking cache...",
    "warning: failed to find optional cache, using default",
    "[SUCCESS] completed",
  ];
  const result = detectStepProgress("gapps", logs, {
    success: true,
    code: null,
    message: null,
  });
  assert.equal(result.isCompleted, true);
  assert.equal(result.isFailed, false);
  assert.equal(result.isCancelled, false);
});

test("detectStepProgress accurately tracks phantom compilation step", () => {
  const logs = [
    "[SYSTEM] Cloning pinned Phantom source...",
    "[SYSTEM] Running Phantom installer as the desktop user...",
  ];
  const result = detectStepProgress("phantom", logs, null);
  assert.equal(result.activeStepIndex, 1);
  assert.equal(result.isCompleted, false);
  assert.equal(result.isFailed, false);
});
