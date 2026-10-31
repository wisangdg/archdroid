import { test } from "node:test";
import assert from "node:assert/strict";
import { setupStage } from "./setupStage.js";

const report = (waydroid, image) => ({
  items: [
    { key: "waydroid", status: waydroid },
    { key: "waydroid_image", status: image },
  ],
});

test("setupStage distinguishes missing, uninitialized, initializing, ready", () => {
  assert.equal(setupStage(report("error", "warning"), false), "missing");
  assert.equal(setupStage(report("ok", "warning"), false), "uninitialized");
  assert.equal(setupStage(report("ok", "warning"), true), "initializing");
  assert.equal(setupStage(report("ok", "ok"), false), "ready");
  assert.equal(setupStage(null, false), "uninitialized");
});
