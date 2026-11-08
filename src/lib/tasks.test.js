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
