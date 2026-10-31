import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = new URL("../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, projectRoot), "utf8");

const capabilities = JSON.parse(read("src-tauri/capabilities/default.json"));
const cargo = read("src-tauri/Cargo.toml");
const pkg = JSON.parse(read("package.json"));

const ALLOWED_PERMISSIONS = [
  "core:event:allow-listen",
  "core:event:allow-unlisten",
  "dialog:allow-open",
];

const FORBIDDEN_PLUGINS = [
  "tauri-plugin-shell",
  "tauri-plugin-fs",
  "tauri-plugin-http",
  "tauri-plugin-process",
];

const FORBIDDEN_PACKAGES = [
  "@tauri-apps/plugin-shell",
  "@tauri-apps/plugin-fs",
  "@tauri-apps/plugin-http",
  "@tauri-apps/plugin-process",
];

const FORBIDDEN_SINKS = [
  "dangerouslySetInnerHTML",
  ".innerHTML",
  "outerHTML",
  "insertAdjacentHTML",
  "document.write",
  "createContextualFragment",
  "new Function",
  /\beval\s*\(/,
  "srcdoc",
  "javascript:",
];

function sourceFiles() {
  const dir = fileURLToPath(new URL("src/", projectRoot));
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter(
      (entry) => entry.isFile() && /\.(?:jsx?|tsx?|html)$/.test(entry.name)
    )
    .filter((entry) => !entry.name.endsWith(".test.js"))
    .map((entry) => join(entry.parentPath ?? entry.path, entry.name));
}

test("the webview capability only grants the IPC the UI calls", () => {
  assert.deepEqual(
    [...capabilities.permissions].sort(),
    [...ALLOWED_PERMISSIONS].sort()
  );
  assert.deepEqual(capabilities.windows, ["main"]);
});

test("no shell, filesystem, HTTP, or process plugin is exposed", () => {
  for (const name of FORBIDDEN_PLUGINS) {
    assert.ok(!cargo.includes(name), `${name} is a backend dependency`);
  }
  for (const name of FORBIDDEN_PACKAGES) {
    assert.ok(
      !(name in (pkg.dependencies ?? {})),
      `${name} is a frontend dependency`
    );
  }
  for (const permission of capabilities.permissions) {
    assert.ok(
      !/^(?:shell|fs|http|process):/.test(permission),
      `dangerous permission exposed: ${permission}`
    );
  }
});

test("the webview never renders HTML from strings", () => {
  for (const file of sourceFiles()) {
    const source = readFileSync(file, "utf8");
    for (const sink of FORBIDDEN_SINKS) {
      if (typeof sink === "string") {
        assert.ok(!source.includes(sink), `${file} uses ${sink}`);
      } else {
        assert.ok(!sink.test(source), `${file} uses ${sink}`);
      }
    }
  }
});

test("the UI loads no remote content", () => {
  const files = [
    ...sourceFiles(),
    fileURLToPath(new URL("index.html", projectRoot)),
  ];
  for (const file of files) {
    const source = readFileSync(file, "utf8");
    assert.ok(!/https?:\/\//.test(source), `${file} references a remote URL`);
  }
});
