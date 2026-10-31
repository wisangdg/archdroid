import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const projectRoot = new URL("../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, projectRoot), "utf8");

test("package-installing actions preview the distro packages before confirming", () => {
  const app = read("src/App.jsx");

  for (const [handler, action] of [
    ["handleInstallGApps", "gapps"],
    ["handleInstallHoudini", "houdini"],
    ["handleInitWaydroid", "init"],
  ]) {
    const start = app.indexOf(`const ${handler} =`);
    assert.ok(start > 0, `${handler} not found`);
    const body = app.slice(start, app.indexOf("const handle", start + 1));

    assert.match(
      body,
      new RegExp(`previewPackageInstall\\(\\s*"${action}"\\s*\\)`),
      `${handler} does not preview the ${action} package install`
    );
    assert.match(
      body,
      /packagePreview:\s*preview/,
      `${handler} drops the preview`
    );
    assert.match(
      body,
      /executeInstall\w+\(preview\.packages\)|executeInitWaydroid\(preview\.packages\)/,
      `${handler} does not forward the approved package list`
    );
  }
});

test("installer commands receive the approved package list", () => {
  const app = read("src/App.jsx");

  for (const command of ["install_gapps", "install_houdini", "waydroid_init"]) {
    const index = app.indexOf(`runStreamingTask("${command}"`);
    assert.ok(index > 0, `${command} is never streamed`);
    const body = app.slice(index, index + 400);
    assert.match(
      body,
      /args:\s*\{\s*approvedPackages\s*\}/,
      `${command} sends no approval`
    );
  }
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
