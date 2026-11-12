import { test } from "node:test";
import assert from "node:assert/strict";
import { enginePhase, ENGINE_PHASES } from "./enginePhase.js";

const base = { status: "offline", isInitialized: true };

test("the first status reply decides between setup and the engine", () => {
  assert.equal(enginePhase({ ...base, isInitialized: null }), "checking");
  assert.equal(enginePhase({ ...base, isInitialized: false }), "uninitialized");
  assert.equal(enginePhase(base), "stopped");
});

test("a running session is booting until Android reports boot_completed", () => {
  assert.equal(
    enginePhase({ ...base, status: "online", booting: true }),
    "booting"
  );
  assert.equal(enginePhase({ ...base, status: "online" }), "running");
});

test("an operation in flight wins over the polled status", () => {
  // During a start the session still reports offline for seconds: that is
  // "starting", not "stopped" (the bar used to say "Engine Stopped").
  assert.equal(enginePhase({ ...base, operation: "start" }), "starting");
  assert.equal(
    enginePhase({ ...base, status: "online", operation: "stop" }),
    "stopping"
  );
  assert.equal(
    enginePhase({ ...base, status: "offline", operation: "restart" }),
    "restarting"
  );
});

test("an unreadable status is not reported as a stopped engine", () => {
  assert.equal(enginePhase({ ...base, unreachable: true }), "unreachable");
  // A later successful reply that says RUNNING wins.
  assert.equal(
    enginePhase({ ...base, status: "online", unreachable: true }),
    "running"
  );
});

test("every phase has a label and a known tone; only transient ones animate", () => {
  for (const [phase, spec] of Object.entries(ENGINE_PHASES)) {
    assert.ok(spec.label, `${phase} has a label`);
    assert.ok(["online", "offline", "pending"].includes(spec.tone), phase);
  }
  // The steady states must not animate (CPU, see ui-styles.test.js).
  for (const steady of ["running", "stopped", "unreachable", "uninitialized"]) {
    assert.ok(!ENGINE_PHASES[steady].transient, steady);
  }
});
