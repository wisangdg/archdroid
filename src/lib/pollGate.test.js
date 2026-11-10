import { test } from "node:test";
import assert from "node:assert/strict";
import { pollDue, UNFOCUSED_POLL_MS } from "./pollGate.js";

test("a focused window polls on every tick", () => {
  assert.equal(pollDue(1000, 1001, { hidden: false, focused: true }), true);
});

test("a hidden window never polls", () => {
  assert.equal(pollDue(0, 10 ** 9, { hidden: true, focused: true }), false);
});

test("an unfocused window polls once per UNFOCUSED_POLL_MS", () => {
  const bg = { hidden: false, focused: false };
  assert.equal(pollDue(0, 5000, bg), false);
  assert.equal(pollDue(0, UNFOCUSED_POLL_MS - 1, bg), false);
  assert.equal(pollDue(0, UNFOCUSED_POLL_MS, bg), true);
});
