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

test("appendLogLine keeps only the newest MAX_TERMINAL_LINES", async () => {
  const { appendLogLine, MAX_TERMINAL_LINES } = await import("./taskEvents.js");
  let logs = [];
  for (let i = 0; i < MAX_TERMINAL_LINES + 5; i++)
    logs = appendLogLine(logs, `l${i}`);
  assert.equal(logs.length, MAX_TERMINAL_LINES);
  assert.equal(logs[0], "l5");
});

test("a progress redraw replaces the redraw before it", async () => {
  const { applyOutput, EMPTY_TERMINAL } = await import("./taskEvents.js");
  let state = applyOutput(EMPTY_TERMINAL, [
    { line: "[STEP] download" },
    { line: " 1%|a", progress: true },
    { line: " 2%|b", progress: true },
  ]);
  assert.deepEqual(state.lines, ["[STEP] download", " 2%|b"]);
  // The bar's final state ends in a newline: it stays, and so does the redraw
  // before it; the next ordinary line is appended.
  state = applyOutput(state, [{ line: "100%|c" }, { line: "unpacking" }]);
  assert.deepEqual(state.lines, [
    "[STEP] download",
    " 2%|b",
    "100%|c",
    "unpacking",
  ]);
});

test("a long download stays far below the line cap", async () => {
  const { applyOutput, EMPTY_TERMINAL } = await import("./taskEvents.js");
  let state = applyOutput(EMPTY_TERMINAL, [{ line: "[STEP] download" }]);
  for (let i = 0; i < 5000; i++)
    state = applyOutput(state, [{ line: ` ${i % 100}%|x`, progress: true }]);
  assert.equal(state.lines.length, 2);
  assert.equal(state.dropped, 0);
});

test("the cap counts what it dropped", async () => {
  const { applyOutput, EMPTY_TERMINAL, MAX_TERMINAL_LINES } =
    await import("./taskEvents.js");
  const entries = Array.from({ length: MAX_TERMINAL_LINES + 7 }, (_, i) => ({
    line: `l${i}`,
  }));
  const state = applyOutput(EMPTY_TERMINAL, entries);
  assert.equal(state.lines.length, MAX_TERMINAL_LINES);
  assert.equal(state.dropped, 7);
  assert.equal(state.lines[0], "l7");
});

test("lastStepId finds the newest marker of a batch", async () => {
  const { lastStepId } = await import("./taskEvents.js");
  assert.equal(
    lastStepId([
      { line: "[STEP] deps" },
      { line: "x" },
      { line: "[STEP] repo" },
    ]),
    "repo"
  );
  assert.equal(lastStepId([{ line: "no marker" }]), null);
});

test("progressPercent reads tqdm and Waydroid download lines", async () => {
  const { progressPercent } = await import("./taskEvents.js");
  assert.equal(
    progressPercent(" 43%|████▎     | 84.1M/198M [00:05<00:07]"),
    43
  );
  assert.equal(
    progressPercent("[Downloading]  512.0 MB/1024.0 MB    5.2 MB/s(approx.)"),
    50
  );
  assert.equal(progressPercent("Unpacking 43% of the files"), null);
  assert.equal(progressPercent(undefined), null);
});
