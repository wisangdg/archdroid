import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = new URL("../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, projectRoot), "utf8");

const DEV_COMMANDS = [
  "waydroid_status_json",
  "waydroid_is_running",
  "waydroid_list_instances",
  "profile_capabilities",
  "profile_get",
  "profile_apply",
  "waydroid_reboot",
  "waydroid_kill_app",
  "waydroid_getprop_all",
  "waydroid_adb_cmd",
  "waydroid_version",
];

function frontendFiles() {
  const dir = fileURLToPath(new URL("src/", projectRoot));
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile() && /\.(?:jsx?|tsx?)$/.test(entry.name))
    .filter((entry) => !entry.name.endsWith(".test.js"))
    .map((entry) => join(entry.parentPath ?? entry.path, entry.name));
}

function uiCommands() {
  const commands = new Set();
  for (const file of frontendFiles()) {
    const source = readFileSync(file, "utf8");
    for (const match of source.matchAll(/\binvoke\(\s*"([a-z_0-9]+)"/g)) {
      commands.add(match[1]);
    }
    // runStreamingTask("cmd", ...) forwards its first argument to invoke().
    for (const match of source.matchAll(
      /\brunStreamingTask\(\s*"([a-z_0-9]+)"/g
    )) {
      commands.add(match[1]);
    }
  }
  return [...commands].sort();
}

function handlerCommands() {
  const rust = read("src-tauri/src/lib.rs");
  const start = rust.indexOf("generate_handler![");
  const end = rust.indexOf("])", start);
  assert.ok(start > 0 && end > start, "could not find the invoke handler");

  const registered = [];
  const gated = [];
  let attributes = "";
  for (const rawLine of rust.slice(start, end).split("\n")) {
    const line = rawLine.trim();
    if (line.startsWith("#[")) {
      attributes += line;
      continue;
    }
    const entry = line.match(/^([a-z_][a-z0-9_]*),?$/);
    if (entry) {
      (attributes.includes('feature = "dev-commands"')
        ? gated
        : registered
      ).push(entry[1]);
      attributes = "";
    } else if (line !== "" && !line.startsWith("//")) {
      attributes = "";
    }
  }
  return { registered: registered.sort(), gated: gated.sort() };
}

test("every registered command has a UI caller and vice versa", () => {
  const { registered } = handlerCommands();
  assert.deepEqual(registered, uiCommands());
});

test("UI-less commands stay behind the dev-commands feature", () => {
  const { registered, gated } = handlerCommands();
  for (const command of DEV_COMMANDS) {
    assert.ok(gated.includes(command), `${command} must be gated`);
    assert.ok(!registered.includes(command), `${command} must not ship`);
  }
  for (const command of ["waydroid_start", "waydroid_wipe", "cancel_task"]) {
    assert.ok(registered.includes(command), `${command} must stay registered`);
    assert.ok(!gated.includes(command), `${command} must not be gated`);
  }
});

test("the arbitrary Android shell command is not in the default build", () => {
  const { registered } = handlerCommands();
  assert.ok(!registered.includes("waydroid_adb_cmd"));
});

test("root-altering operations require explicit confirmation before execution", () => {
  const appSource = read("src/App.jsx");

  const rootOperations = [
    { handler: "handleInstallGApps", command: "install_gapps" },
    { handler: "handleInstallHoudini", command: "install_houdini" },
    { handler: "handleInstallMultitouch", command: "install_multitouch" },
    { handler: "handleInitWaydroid", command: "waydroid_init" },
    { handler: "handleWipeData", command: "waydroid_wipe" },
  ];

  for (const { handler } of rootOperations) {
    const handlerIndex = appSource.indexOf(`const ${handler} =`);
    assert.ok(handlerIndex > 0, `${handler} not found in App.jsx`);

    // Verify the handler opens a confirmation modal before running the streaming task
    const nextFunctionIndex = appSource.indexOf(
      "const handle",
      handlerIndex + 1
    );
    const handlerBody = appSource.slice(
      handlerIndex,
      nextFunctionIndex > 0 ? nextFunctionIndex : handlerIndex + 1500
    );

    assert.ok(
      handlerBody.includes("setConfirmModal"),
      `${handler} must open a confirmation dialog`
    );
    assert.ok(
      handlerBody.includes("Root Required") || handlerBody.includes("WARNING:"),
      `${handler} confirmation must explicitly warn the user`
    );
  }
});
