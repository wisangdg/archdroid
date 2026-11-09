import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const root = new URL("../../", import.meta.url);
// Built from two pieces so this file does not match its own scan.
const PRIVATE_KEY_MARKER = new RegExp(
  "BEGIN (PGP|RSA|EC|OPENSSH) " + "PRIVATE KEY"
);
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
      !PRIVATE_KEY_MARKER.test(text),
      `${file} contains private key material`
    );
  }
});

test("there is no in-app updater, so no unsigned artefact is ever executed", () => {
  assert.doesNotMatch(read("src-tauri/Cargo.toml"), /updater/);
  assert.doesNotMatch(read("src-tauri/tauri.conf.json"), /updater/);
  assert.doesNotMatch(read("package.json"), /plugin-updater/);
});

test("every file that carries the release version agrees on it", () => {
  const versions = {
    "package.json": JSON.parse(read("package.json")).version,
    "src-tauri/tauri.conf.json": JSON.parse(read("src-tauri/tauri.conf.json"))
      .version,
    "src-tauri/Cargo.toml": read("src-tauri/Cargo.toml").match(
      /^\[package\][^[]*?^version\s*=\s*"([^"]+)"/ms
    )?.[1],
    "packaging/arch/PKGBUILD": read("packaging/arch/PKGBUILD").match(
      /^pkgver=(\S+)$/m
    )?.[1],
  };
  const expected = versions["package.json"];
  assert.match(expected, /^\d+\.\d+\.\d+$/);
  for (const [file, version] of Object.entries(versions)) {
    assert.equal(version, expected, `${file} has version ${version}`);
  }
  // Cargo.lock records the crate version too; a stale lock fails `--locked`.
  assert.match(
    read("src-tauri/Cargo.lock"),
    new RegExp(
      `name = "archdroid"\\nversion = "${expected.replace(/\./g, "\\.")}"`
    )
  );
});
