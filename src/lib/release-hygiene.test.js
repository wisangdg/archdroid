import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const root = new URL("../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, root), "utf8");

test("no private key material is tracked in the repository", () => {
  const files = execFileSync("git", ["ls-files", "-z"], { cwd: root })
    .toString()
    .split("\0")
    .filter(Boolean);
  for (const file of files) {
    if (/\.(png|ico|icns|zst|lock)$/.test(file)) continue;
    let text;
    try {
      text = read(file);
    } catch {
      continue;
    }
    assert.ok(
      !/BEGIN (PGP|RSA|EC|OPENSSH) PRIVATE KEY|BEGIN PGP PRIVATE KEY BLOCK/.test(
        text
      ),
      `${file} contains private key material`
    );
  }
});

test("there is no in-app updater, so no unsigned artefact is ever executed", () => {
  assert.doesNotMatch(read("src-tauri/Cargo.toml"), /updater/);
  assert.doesNotMatch(read("src-tauri/tauri.conf.json"), /updater/);
  assert.doesNotMatch(read("package.json"), /plugin-updater/);
});
