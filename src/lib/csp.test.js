import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const read = (path) => readFileSync(new URL(path, import.meta.url), "utf8");

const { app } = JSON.parse(read("../../src-tauri/tauri.conf.json"));
const csp = app.security.csp;
const devCsp = app.security.devCsp;

const REQUIRED_DIRECTIVES = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data:",
  "font-src 'self'",
  "connect-src ipc: http://ipc.localhost",
  "object-src 'none'",
  "frame-src 'none'",
];

function hostsIn(policy) {
  return [...policy.matchAll(/\b(?:https?|wss?):\/\/([^\s;]+)/g)].map(
    (match) => match[1]
  );
}

test("the production CSP is set and keeps every required directive", () => {
  assert.equal(typeof csp, "string", "app.security.csp must not be null");
  for (const directive of REQUIRED_DIRECTIVES) {
    assert.ok(csp.includes(directive), `missing CSP directive: ${directive}`);
  }
});

test("the production CSP only allows Tauri IPC, not internet hosts", () => {
  assert.deepEqual(hostsIn(csp), ["ipc.localhost"]);
  assert.ok(!csp.includes("'unsafe-eval'"), "production must not allow eval");
});

test("dev CSP adds Vite and HMR without weakening production", () => {
  assert.notEqual(devCsp, csp);
  assert.ok(devCsp.includes("ws://localhost:5173"), "HMR needs its WebSocket");
  assert.ok(
    devCsp.includes("http://localhost:5173"),
    "dev assets come from Vite"
  );
  assert.deepEqual(hostsIn(devCsp), [
    "ipc.localhost",
    "localhost:5173",
    "localhost:5173",
  ]);
});

test("no remote font or stylesheet is imported", () => {
  const css = read("../index.css");
  assert.ok(
    !/@import\s+url\(\s*['"]?https?:/i.test(css),
    "remote @import found"
  );
  assert.ok(
    !/fonts\.googleapis|fonts\.gstatic/i.test(css),
    "Google Fonts found"
  );
});
