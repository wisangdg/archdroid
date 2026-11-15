/**
 * The keymap editor's model: controls placed on a game screenshot, and the
 * Phantom profile they become (src-tauri/src/keymap.rs saves it).
 *
 * Positions are normalized (0..1) like Phantom's, so they stay on the same
 * spot of the game at any window size. Controls ArchDroid does not edit
 * (macros, wheels, layers from Phantom's own GUI) are kept as they are.
 */

/** What a click on the screenshot places. */
export const TOOLS = {
  tap: { label: "Button", hint: "A key presses this spot" },
  fire: { label: "Fire", hint: "Left mouse button presses this spot" },
  joystick: { label: "Joystick", hint: "WASD moves this stick" },
  aim: { label: "Aim", hint: "The mouse turns the camera here" },
};

/** Phantom's own hotkeys: capture, aim, pause, preview, quit. */
export const RESERVED_KEYS = ["F1", "F2", "F8", "F9", "F10"];

const CODE_NAMES = {
  Space: "Space",
  Enter: "Enter",
  NumpadEnter: "Enter",
  Escape: "Esc",
  Tab: "Tab",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  CapsLock: "CapsLock",
  ShiftLeft: "LeftShift",
  ShiftRight: "RightShift",
  ControlLeft: "LeftCtrl",
  ControlRight: "RightCtrl",
  AltLeft: "LeftAlt",
  AltRight: "RightAlt",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Minus: "Minus",
  Equal: "Equal",
  BracketLeft: "LeftBrace",
  BracketRight: "RightBrace",
  Semicolon: "Semicolon",
  Quote: "Apostrophe",
  Backquote: "Grave",
  Backslash: "Backslash",
  Comma: "Comma",
  Period: "Dot",
  Slash: "Slash",
};

/**
 * Phantom's name for a pressed key (`KeyboardEvent.code`, which ignores the
 * layout like Phantom's evdev codes do), or null for a key it cannot map.
 */
export function phantomKey(code) {
  if (typeof code !== "string") return null;
  let match = /^Key([A-Z])$/.exec(code);
  if (match) return match[1];
  match = /^Digit([0-9])$/.exec(code);
  if (match) return match[1];
  match = /^Numpad([0-9])$/.exec(code);
  if (match) return `KP${match[1]}`;
  if (/^F([1-9]|1[0-2])$/.test(code)) return code;
  return CODE_NAMES[code] ?? null;
}

const MOUSE_BUTTONS = [
  "MouseLeft",
  "MouseMiddle",
  "MouseRight",
  "MouseBack",
  "MouseForward",
];

/** Phantom's name for `MouseEvent.button`. */
export function mouseKey(button) {
  return MOUSE_BUTTONS[button] ?? null;
}

/** Short label for a key on the screenshot. */
export function keyLabel(key) {
  if (!key) return "?";
  const short = {
    MouseLeft: "LMB",
    MouseRight: "RMB",
    MouseMiddle: "MMB",
    LeftShift: "Shift",
    RightShift: "RShift",
    LeftCtrl: "Ctrl",
    RightCtrl: "RCtrl",
    LeftAlt: "Alt",
    RightAlt: "RAlt",
  };
  return short[key] ?? key;
}

const clamp = (value) => Math.min(1, Math.max(0, Number(value) || 0));
const round = (value) => Math.round(value * 1000) / 1000;
const point = ({ x, y }) => ({ x: round(clamp(x)), y: round(clamp(y)) });

/** Where a control sits on the screenshot, or null if it has no single spot. */
export function nodePoint(node) {
  return node.pos ?? node.anchor ?? null;
}

/** A copy of `node` moved to `at` (normalized). */
export function moveNode(node, at) {
  const field = node.anchor && !node.pos ? "anchor" : "pos";
  return { ...node, [field]: point(at) };
}

function nextId(prefix, nodes) {
  const taken = new Set(nodes.map((node) => node.id));
  for (let i = 1; ; i++) {
    if (!taken.has(`${prefix}-${i}`)) return `${prefix}-${i}`;
  }
}

/** A new control of `tool` at `at`. A Button starts unbound: the editor asks for its key. */
export function newNode(tool, at, nodes = []) {
  const spot = point(at);
  switch (tool) {
    case "fire":
      return {
        id: nextId("fire", nodes),
        type: "tap",
        pos: spot,
        key: "MouseLeft",
      };
    case "joystick":
      return {
        id: nextId("move", nodes),
        type: "joystick",
        pos: spot,
        radius: 0.07,
        keys: { up: "W", down: "S", left: "A", right: "D" },
      };
    case "aim":
      return {
        id: nextId("aim", nodes),
        type: "aim",
        anchor: spot,
        reach: 0.18,
        sensitivity: 1.0,
        activation_mode: "always_on",
        invert_y: false,
      };
    default:
      return { id: nextId("button", nodes), type: "tap", pos: spot, key: "" };
  }
}

/** The keys a control listens to. */
export function nodeKeys(node) {
  if (node.type === "joystick") return Object.values(node.keys ?? {});
  if (node.type === "aim") {
    return node.activation_mode === "always_on" ? [] : [node.activation_key];
  }
  return node.key !== undefined ? [node.key] : [];
}

/**
 * What stops the keymap from working, in words for the editor. An unbound
 * button or a key used twice is a mistake; Phantom's hotkeys would never
 * reach the game.
 */
export function problems(nodes) {
  const found = [];
  const owners = new Map();
  for (const node of nodes) {
    for (const key of nodeKeys(node)) {
      if (!key) {
        found.push({ id: node.id, text: "has no key yet" });
        continue;
      }
      if (RESERVED_KEYS.includes(key)) {
        found.push({ id: node.id, text: `${key} is a Phantom hotkey` });
      }
      if (owners.has(key) && owners.get(key) !== node.id) {
        found.push({
          id: node.id,
          text: `${key} is also used by ${owners.get(key)}`,
        });
      } else {
        owners.set(key, node.id);
      }
    }
  }
  return found;
}

const TOUCH_TYPES = new Set([
  "tap",
  "hold_tap",
  "toggle_tap",
  "joystick",
  "drag",
  "aim",
  "mouse_camera",
  "repeat_tap",
]);

/**
 * The Phantom profile for `nodes`. Slots are numbered again so they stay
 * unique however controls were added or removed; `screen` is the
 * screenshot's size, the size Android draws at.
 */
export function toProfile({ name, width, height, nodes, base = {} }) {
  let slot = 0;
  return {
    ...base,
    name: name || base.name || "Keymap",
    version: 1,
    screen: { width, height },
    global_sensitivity: base.global_sensitivity ?? 1.0,
    nodes: nodes.map((node) =>
      TOUCH_TYPES.has(node.type) || typeof node.slot === "number"
        ? { ...node, slot: slot++ }
        : node
    ),
  };
}

/** Controls of a saved profile, ready for the editor. */
export function fromProfile(profile) {
  return Array.isArray(profile?.nodes) ? profile.nodes : [];
}
