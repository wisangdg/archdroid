import { test } from "node:test";
import assert from "node:assert/strict";
import { parseWaydroidStatus, sessionRunning } from "./waydroidStatus.js";

test("a running session, frozen or not, is running", () => {
  const out =
    "Session:\tRUNNING\nContainer:\tFROZEN\nVendor type:\tMAINLINE\nIP address:\t192.168.240.112\n";
  assert.deepEqual(parseWaydroidStatus(out), { running: true, frozen: true });
  assert.deepEqual(
    parseWaydroidStatus("Session:\tRUNNING\nContainer:\tRUNNING"),
    {
      running: true,
      frozen: false,
    }
  );
});

test("only the Session line decides, not any RUNNING in the text", () => {
  assert.equal(
    parseWaydroidStatus("Session:\tSTOPPED\nVendor type:\tMAINLINE").running,
    false
  );
  // A substring match on "RUNNING" got this wrong.
  assert.equal(
    parseWaydroidStatus("Session:\tSTOPPED\nContainer:\tRUNNING").running,
    false
  );
  assert.equal(
    parseWaydroidStatus("Session user: RUNNING_USER").running,
    false
  );
});

test("an error reply or missing data is not running", () => {
  assert.equal(
    sessionRunning({ status: "error", data: "Session:\tRUNNING" }),
    false
  );
  assert.equal(sessionRunning({ status: "success", data: null }), false);
  assert.equal(sessionRunning(undefined), false);
  assert.equal(
    sessionRunning({ status: "success", data: "Session: RUNNING\n" }),
    true
  );
});
