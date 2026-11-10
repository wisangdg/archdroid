import { test } from "node:test";
import assert from "node:assert/strict";
import { TASKS } from "./tasks.js";
import { TASK_STEP_CONFIGS } from "../components/terminal/stepConfigs.js";

// Commands that reach pkexec/sudo. Each needs consent before the root prompt.
const ROOT_COMMANDS = [
  "install_gapps",
  "install_houdini",
  "install_multitouch",
  "install_touch_mode_access",
  "waydroid_init",
  "waydroid_wipe",
];

test("every task can be started, titled, and reported", () => {
  for (const [type, task] of Object.entries(TASKS)) {
    assert.match(task.command, /^[a-z_]+$/, type);
    assert.ok(task.title, `${type} has a terminal title`);
    assert.ok(task.errorTitle, `${type} has an error title`);
  }
});

test("every root task asks for consent first", () => {
  for (const command of ROOT_COMMANDS) {
    const task = Object.values(TASKS).find((t) => t.command === command);
    assert.ok(task, `${command} is a task`);
    assert.ok(task.confirm?.title && task.confirm?.description, command);
  }
});

test("every task with a stepper is a task Retry can restart", () => {
  for (const type of Object.keys(TASK_STEP_CONFIGS)) {
    assert.ok(TASKS[type], `${type} has stepper stages but no task entry`);
  }
});

test("every root consent dialog is structured: one sentence, then rows", () => {
  for (const command of ROOT_COMMANDS) {
    const { confirm } = Object.values(TASKS).find((t) => t.command === command);
    // A single sentence, not the old wall of text with bullets in it.
    assert.doesNotMatch(confirm.description, /\n|•/, command);
    const labels = (confirm.details ?? []).map((row) => row.label);
    assert.ok(labels.includes("Undo"), `${command} says how to undo it`);
    assert.ok(
      labels.includes("Changes") || labels.includes("Deletes"),
      `${command} says what it changes`
    );
    for (const row of confirm.details) assert.ok(row.text, command);
  }
});
