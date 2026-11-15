import { test } from "node:test";
import assert from "node:assert/strict";
import {
  phantomKey,
  mouseKey,
  newNode,
  moveNode,
  nodePoint,
  problems,
  toProfile,
  fromProfile,
} from "./keymapProfile.js";

test("pressed keys map to the names Phantom parses", () => {
  const cases = {
    KeyW: "W",
    Digit1: "1",
    Numpad5: "KP5",
    F12: "F12",
    Space: "Space",
    ShiftLeft: "LeftShift",
    ControlRight: "RightCtrl",
    ArrowUp: "Up",
    Escape: "Esc",
    Period: "Dot",
    BracketLeft: "LeftBrace",
  };
  for (const [code, name] of Object.entries(cases)) {
    assert.equal(phantomKey(code), name, code);
  }
  assert.equal(phantomKey("MetaLeft"), null);
  assert.equal(phantomKey("F13"), null);
  assert.equal(mouseKey(0), "MouseLeft");
  assert.equal(mouseKey(2), "MouseRight");
  assert.equal(mouseKey(9), null);
});

test("each tool places a control Phantom understands", () => {
  const at = { x: 0.5, y: 0.25 };
  const button = newNode("tap", at);
  assert.deepEqual(button, { id: "button-1", type: "tap", pos: at, key: "" });
  assert.equal(newNode("fire", at).key, "MouseLeft");
  assert.deepEqual(newNode("joystick", at).keys, {
    up: "W",
    down: "S",
    left: "A",
    right: "D",
  });
  const aim = newNode("aim", at);
  assert.equal(aim.type, "aim");
  assert.deepEqual(aim.anchor, at);
  assert.equal(aim.pos, undefined);
  // Ids stay unique.
  assert.equal(newNode("tap", at, [button]).id, "button-2");
});

test("moving keeps a control on the screen", () => {
  const moved = moveNode(newNode("tap", { x: 0.1, y: 0.1 }), { x: 1.4, y: -2 });
  assert.deepEqual(moved.pos, { x: 1, y: 0 });
  const aim = moveNode(newNode("aim", { x: 0.1, y: 0.1 }), {
    x: 0.33333,
    y: 0.5,
  });
  assert.deepEqual(nodePoint(aim), { x: 0.333, y: 0.5 });
  assert.equal(aim.pos, undefined);
});

test("problems name unbound, reserved and doubled keys", () => {
  const nodes = [
    { ...newNode("tap", { x: 0, y: 0 }), id: "jump", key: "Space" },
    { ...newNode("tap", { x: 0, y: 0 }), id: "crouch", key: "Space" },
    { ...newNode("tap", { x: 0, y: 0 }), id: "reload", key: "" },
    { ...newNode("tap", { x: 0, y: 0 }), id: "map", key: "F8" },
    newNode("joystick", { x: 0, y: 0 }),
  ];
  const texts = problems(nodes).map((p) => `${p.id} ${p.text}`);
  assert.deepEqual(texts, [
    "crouch Space is also used by jump",
    "reload has no key yet",
    "map F8 is a Phantom hotkey",
  ]);
  assert.deepEqual(problems(nodes.slice(4)), []);
});

test("the profile has unique slots, the screenshot's size, and keeps foreign controls", () => {
  const foreign = { id: "combo", type: "macro", key: "Q", steps: [] };
  const nodes = [
    newNode("joystick", { x: 0.2, y: 0.7 }),
    foreign,
    { ...newNode("tap", { x: 0.9, y: 0.8 }), key: "Space", slot: 7 },
  ];
  const profile = toProfile({
    name: "Game",
    width: 1280,
    height: 720,
    nodes,
    base: { global_sensitivity: 1.4, extra: true },
  });
  assert.equal(profile.version, 1);
  assert.deepEqual(profile.screen, { width: 1280, height: 720 });
  assert.equal(profile.global_sensitivity, 1.4);
  assert.equal(profile.extra, true);
  assert.deepEqual(
    profile.nodes.map((n) => n.slot),
    [0, undefined, 1]
  );
  assert.deepEqual(profile.nodes[1], foreign);
  assert.deepEqual(fromProfile(profile), profile.nodes);
  assert.deepEqual(fromProfile(null), []);
});
