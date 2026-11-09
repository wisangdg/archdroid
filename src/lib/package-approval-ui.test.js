import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { TASKS } from "./tasks.js";

const projectRoot = new URL("../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, projectRoot), "utf8");

test("package-installing actions preview the distro packages before confirming", () => {
  for (const type of ["gapps", "houdini", "init"]) {
    assert.equal(TASKS[type].preview, type, `${type} has no package preview`);
    assert.ok(TASKS[type].confirm, `${type} is not confirmed`);
  }

  const app = read("src/App.jsx");
  const start = app.indexOf("const startTask =");
  assert.ok(start > 0, "startTask not found");
  const body = app.slice(start, app.indexOf("const handle", start));
  assert.match(body, /previewPackageInstall\(task\.preview\)/);
  assert.match(
    body,
    /packagePreview:\s*preview/,
    "startTask drops the preview"
  );
  // The reviewed list is the approval token the backend re-validates.
  assert.match(
    body,
    /approvedPackages:\s*preview\.packages/,
    "startTask does not forward the approved package list"
  );
});

test("installer commands receive the approved package list", () => {
  const app = read("src/App.jsx");
  const start = app.indexOf("const runTask =");
  assert.ok(start > 0, "runTask not found");
  const body = app.slice(start, app.indexOf("const startTask", start));
  assert.match(
    body,
    /runStreamingTask\(task\.command,\s*\{[\s\S]*?\bargs,?\s*\}/,
    "runTask does not pass the approval to the command"
  );
});

test("the app forwards the reviewed package preview to the modal", () => {
  const app = read("src/App.jsx");
  assert.match(
    app,
    /<ConfirmModal[\s\S]*?packagePreview=\{confirmModal\.packagePreview\}/,
    "ConfirmModal is rendered without packagePreview"
  );
});

test("confirmation modal shows the package list and exact command", () => {
  const modal = read("src/components/ui/ConfirmModal.jsx");
  assert.match(
    modal,
    /packagePreview\.packages/,
    "package list is never rendered"
  );
  assert.match(
    modal,
    /packagePreview\.command/,
    "exact command is never rendered"
  );
  assert.match(
    modal,
    /No system packages will be installed/,
    "empty plan is not explained"
  );
});

test("installers refuse to run without an approved package list", () => {
  const rust = read("src-tauri/src/lib.rs");
  for (const command of ["waydroid_init", "install_gapps", "install_houdini"]) {
    const index = rust.indexOf(`async fn ${command}(`);
    assert.ok(index > 0, `${command} not found`);
    const body = rust.slice(
      index,
      rust.indexOf("#[tauri::command]", index + 1)
    );
    assert.match(
      body,
      /approved_install_packages\(/,
      `${command} does not validate the approved packages`
    );
  }
});
