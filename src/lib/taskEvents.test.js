import test from "node:test";
import assert from "node:assert/strict";

import { isEventForTask } from "./taskEvents.js";

test("events from the active task are accepted", () => {
  assert.equal(isEventForTask({ task_id: "task-2" }, "task-2"), true);
});

test("events from another task are ignored", () => {
  assert.equal(isEventForTask({ task_id: "task-1" }, "task-2"), false);
});

test("events are ignored when no task is active", () => {
  assert.equal(isEventForTask({ task_id: "task-2" }, null), false);
  assert.equal(isEventForTask({ task_id: "task-2" }, undefined), false);
});

test("malformed payloads are ignored instead of throwing", () => {
  assert.equal(isEventForTask(null, "task-2"), false);
  assert.equal(isEventForTask({}, "task-2"), false);
});
