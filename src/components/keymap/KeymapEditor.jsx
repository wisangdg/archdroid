import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  X,
  RotateCw,
  Save,
  Play,
  Trash2,
  Circle,
  Crosshair,
  MousePointerClick,
  Move,
  Loader2,
} from "lucide-react";
import { useFocusTrap } from "../../lib/useFocusTrap";
import {
  TOOLS,
  RESERVED_KEYS,
  phantomKey,
  mouseKey,
  keyLabel,
  newNode,
  moveNode,
  nodePoint,
  problems,
  toProfile,
  fromProfile,
} from "../../lib/keymapProfile";

const TOOL_ICONS = {
  tap: Circle,
  fire: MousePointerClick,
  joystick: Move,
  aim: Crosshair,
};

const DIRECTIONS = [
  ["up", "Up"],
  ["left", "Left"],
  ["down", "Down"],
  ["right", "Right"],
];

/**
 * LDPlayer-style keymapping: the game's current screen is the canvas. Pick a
 * tool, click where the game shows that button, press the key for it, drag
 * to adjust. Save writes a Phantom profile for this game; Apply loads it into
 * the running Phantom.
 */
export default function KeymapEditor({ app, onClose, addToast }) {
  // Escape goes through the same unsaved-changes check as the close button.
  const closeRef = useRef(onClose);
  const addToastRef = useRef(addToast);
  addToastRef.current = addToast;
  const modalRef = useFocusTrap(true, () => closeRef.current());
  const [confirmDiscard, setConfirmDiscard] = useState(false);
  const [shot, setShot] = useState(null); // { data_url, width, height }
  const [shotError, setShotError] = useState(null);
  // A screenshot is a data URL of several MB. Built once per capture: inside
  // the render it was copied again on every pointer move of a drag.
  const canvasStyle = useMemo(
    () =>
      shot && {
        aspectRatio: `${shot.width} / ${shot.height}`,
        "--keymap-ratio": shot.width / shot.height,
        backgroundImage: `url(${shot.data_url})`,
      },
    [shot]
  );
  const [nodes, setNodes] = useState([]);
  const [base, setBase] = useState({});
  const [tool, setToolState] = useState("tap");
  const [selectedId, setSelectedIdState] = useState(null);
  // Which key the next key press binds: { id, field } (field: "key",
  // "activation_key" or a joystick direction).
  const [listening, setListeningState] = useState(null);
  // Input handlers read these refs, not the last render: a click right after
  // picking a tool, or a key right after placing a button, must not see the
  // previous state.
  const toolRef = useRef(tool);
  const selectedRef = useRef(selectedId);
  const listeningRef = useRef(listening);
  const nodesRef = useRef(nodes);
  nodesRef.current = nodes;
  const setTool = (value) => {
    toolRef.current = value;
    setToolState(value);
  };
  const setSelectedId = (value) => {
    selectedRef.current = value;
    setSelectedIdState(value);
  };
  const setListening = (value) => {
    listeningRef.current = value;
    setListeningState(value);
  };
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const canvasRef = useRef(null);
  const dragRef = useRef(null);

  // A capture is three adb calls; clicks while one runs would queue more.
  const [capturing, setCapturing] = useState(false);
  const takeScreenshot = useCallback(async () => {
    setShotError(null);
    setCapturing(true);
    try {
      setShot(await invoke("keymap_screenshot", { package: app.package }));
    } catch (err) {
      setShotError(err?.message || String(err));
    } finally {
      setCapturing(false);
    }
  }, [app.package]);

  useEffect(() => {
    let live = true;
    invoke("keymap_load", { package: app.package })
      .then((profile) => {
        if (!live || !profile) return;
        setNodes(fromProfile(profile));
        setBase(profile);
      })
      .catch((err) =>
        addToast?.({
          type: "warning",
          title: "Saved Keymap Not Read",
          message: err?.message || String(err),
        })
      );
    takeScreenshot();
    return () => {
      live = false;
    };
  }, [app.package, takeScreenshot, addToast]);

  const selected = nodes.find((node) => node.id === selectedId) ?? null;
  const issues = problems(nodes);

  const update = (id, change) => {
    setNodes((prev) =>
      prev.map((node) => (node.id === id ? change(node) : node))
    );
    setDirty(true);
  };

  const bind = (target, key) => {
    if (!key) return;
    update(target.id, (node) => {
      if (target.field in (node.keys ?? {})) {
        return { ...node, keys: { ...node.keys, [target.field]: key } };
      }
      return { ...node, [target.field]: key };
    });
    setListening(null);
  };

  // One listener for the editor's lifetime. While a key is being bound, the
  // next key press is the binding: it must not close the dialog (Escape
  // cancels instead) or move focus (Tab). Otherwise Delete removes the
  // selected control.
  useEffect(() => {
    const onKey = (e) => {
      const target = listeningRef.current;
      if (target) {
        e.preventDefault();
        e.stopImmediatePropagation();
        if (e.key === "Escape") {
          setListening(null);
          return;
        }
        const key = phantomKey(e.code);
        if (!key) {
          addToastRef.current?.({
            type: "warning",
            title: "Key Not Supported",
            message: `Phantom cannot use ${e.code || e.key}. Pick another key.`,
            duration: 3000,
          });
          return;
        }
        bind(target, key);
        return;
      }
      if (!selectedRef.current) return;
      if (e.target.closest?.("input, select, textarea")) return;
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        removeSelected();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
    // Everything it calls goes through refs and state setters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const removeSelected = () => {
    const id = selectedRef.current;
    setNodes((prev) => prev.filter((node) => node.id !== id));
    setSelectedId(null);
    setListening(null);
    setDirty(true);
  };

  /** Normalized position of a pointer event on the screenshot. */
  const toPoint = (e) => {
    const rect = canvasRef.current.getBoundingClientRect();
    return {
      x: (e.clientX - rect.left) / rect.width,
      y: (e.clientY - rect.top) / rect.height,
    };
  };

  const handleCanvasClick = (e) => {
    if (!shot || e.target !== e.currentTarget) return;
    const node = newNode(toolRef.current, toPoint(e), nodesRef.current);
    nodesRef.current = [...nodesRef.current, node];
    setNodes((prev) => [...prev, node]);
    setSelectedId(node.id);
    setDirty(true);
    // A new button asks for its key right away, like LDPlayer.
    setListening(
      node.type === "tap" && !node.key ? { id: node.id, field: "key" } : null
    );
  };

  const startDrag = (e, node) => {
    e.stopPropagation();
    setSelectedId(node.id);
    dragRef.current = { id: node.id, moved: false };
    e.currentTarget.setPointerCapture?.(e.pointerId);
  };

  const onDrag = (e) => {
    const drag = dragRef.current;
    if (!drag) return;
    drag.moved = true;
    update(drag.id, (node) => moveNode(node, toPoint(e)));
  };

  const endDrag = () => {
    dragRef.current = null;
  };

  const save = async ({ apply = false } = {}) => {
    if (!shot) return;
    if (issues.length > 0) {
      setSelectedId(issues[0].id);
      addToast?.({
        type: "warning",
        title: "Keymap Not Finished",
        message: `${issues[0].id} ${issues[0].text}.`,
      });
      return;
    }
    setSaving(true);
    try {
      const profile = toProfile({
        name: app.name,
        width: shot.width,
        height: shot.height,
        nodes,
        base,
      });
      const res = await invoke("keymap_save", {
        package: app.package,
        profile,
      });
      if (res?.status !== "success") {
        addToast?.({
          type: "error",
          title: "Keymap Not Saved",
          message: res?.message || "The keymap could not be saved.",
        });
        return;
      }
      setDirty(false);
      if (!apply) {
        addToast?.({
          type: "success",
          title: "Keymap Saved",
          message: `Saved for ${app.name}: ${res.data}`,
        });
        return;
      }
      const applied = await invoke("keymap_apply", { package: app.package });
      addToast?.(
        applied?.status === "success"
          ? {
              type: "success",
              title: "Keymap Active",
              message: `${app.name}'s keymap is loaded. Press F8 in the game to capture keyboard and mouse, and F2 when you are done: Phantom holds the keyboard while it runs.`,
            }
          : {
              type: "warning",
              title: "Saved, Not Applied",
              message: applied?.message || "Phantom did not load the keymap.",
            }
      );
    } catch (err) {
      addToast?.({
        type: "error",
        title: "Keymap Not Saved",
        message: err?.message || String(err),
      });
    } finally {
      setSaving(false);
    }
  };

  const close = () => {
    if (dirty) setConfirmDiscard(true);
    else onClose();
  };
  closeRef.current = close;

  const keyButton = (field, value, label) => {
    const active = listening?.id === selected.id && listening.field === field;
    return (
      <button
        type="button"
        className={`btn btn-outline btn-sm keymap-key-btn ${active ? "listening" : ""}`}
        onClick={() => setListening(active ? null : { id: selected.id, field })}
        aria-label={`${label}: ${value || "not set"}`}
      >
        {active ? "Press a key…" : value || "Set key"}
      </button>
    );
  };

  return (
    <div className="terminal-overlay">
      <div
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="keymap-editor-title"
        className="glass-panel keymap-editor"
      >
        <header className="keymap-editor-head">
          <div>
            <h3 id="keymap-editor-title">Keymap: {app.name}</h3>
            <p className="keymap-editor-sub">
              Open the game first. Pick a tool, click where the game shows the
              button, then press its key. Drag to adjust.
            </p>
          </div>
          <div className="keymap-editor-actions">
            <button
              type="button"
              className="btn btn-outline btn-sm"
              onClick={takeScreenshot}
              disabled={capturing}
              title="Capture the game screen again"
            >
              <RotateCw size={14} aria-hidden="true" />
              <span>{capturing ? "Capturing…" : "Recapture"}</span>
            </button>
            <button
              type="button"
              className="btn btn-outline btn-sm"
              onClick={() => save()}
              disabled={!shot || saving}
            >
              <Save size={14} aria-hidden="true" />
              <span>Save</span>
            </button>
            <button
              type="button"
              className="btn btn-primary btn-sm"
              onClick={() => save({ apply: true })}
              disabled={!shot || saving}
            >
              {saving ? (
                <Loader2 size={14} className="spinner" aria-hidden="true" />
              ) : (
                <Play size={14} aria-hidden="true" />
              )}
              <span>Save &amp; Apply</span>
            </button>
            <button
              type="button"
              className="btn-icon-close"
              onClick={close}
              aria-label="Close keymap editor"
            >
              <X size={18} aria-hidden="true" />
            </button>
          </div>
        </header>

        {confirmDiscard && (
          <div className="keymap-discard" role="alert">
            <span>This keymap has unsaved changes.</span>
            <button
              type="button"
              className="btn btn-outline btn-sm"
              onClick={() => setConfirmDiscard(false)}
            >
              Keep Editing
            </button>
            <button
              type="button"
              className="btn btn-outline-danger btn-sm"
              onClick={onClose}
            >
              Discard
            </button>
          </div>
        )}

        <div className="keymap-editor-body">
          <div
            className="keymap-tools"
            role="radiogroup"
            aria-label="Control to place"
          >
            {Object.entries(TOOLS).map(([id, spec]) => {
              const Icon = TOOL_ICONS[id];
              return (
                <button
                  key={id}
                  type="button"
                  role="radio"
                  aria-checked={tool === id}
                  className={`keymap-tool ${tool === id ? "active" : ""}`}
                  onClick={() => setTool(id)}
                  title={spec.hint}
                >
                  <Icon size={18} aria-hidden="true" />
                  <span>{spec.label}</span>
                </button>
              );
            })}
          </div>

          <div className="keymap-stage">
            {shot ? (
              <div
                ref={canvasRef}
                className="keymap-canvas"
                style={canvasStyle}
                onClick={handleCanvasClick}
                onPointerMove={onDrag}
                onPointerUp={endDrag}
                data-testid="keymap-canvas"
              >
                {nodes.map((node) => {
                  const at = nodePoint(node);
                  if (!at) return null;
                  const size =
                    node.type === "joystick"
                      ? `${(node.radius ?? 0.07) * 200}%`
                      : undefined;
                  const label =
                    node.type === "joystick"
                      ? Object.values(node.keys ?? {})
                          .map(keyLabel)
                          .join("")
                      : node.type === "aim"
                        ? "Aim"
                        : keyLabel(node.key);
                  const flagged = issues.some((p) => p.id === node.id);
                  return (
                    <button
                      key={node.id}
                      type="button"
                      className={`keymap-node keymap-node-${node.type} ${
                        node.id === selectedId ? "selected" : ""
                      } ${flagged ? "flagged" : ""}`}
                      style={{
                        left: `${at.x * 100}%`,
                        top: `${at.y * 100}%`,
                        width: size,
                      }}
                      onPointerDown={(e) => startDrag(e, node)}
                      onClick={(e) => e.stopPropagation()}
                      aria-label={`${node.id}: ${label}`}
                      aria-pressed={node.id === selectedId}
                    >
                      <span>{label}</span>
                    </button>
                  );
                })}
              </div>
            ) : (
              <div className="keymap-stage-empty" role="status">
                {shotError ? (
                  <>
                    <p>Could not capture the game screen: {shotError}</p>
                    <p>
                      Start the engine and open the game, then press Recapture.
                    </p>
                  </>
                ) : (
                  <p>
                    <Loader2 size={16} className="spinner" aria-hidden="true" />{" "}
                    Capturing the game screen…
                  </p>
                )}
              </div>
            )}
          </div>

          <aside className="keymap-inspector" aria-label="Selected control">
            {selected ? (
              <>
                <h4>{selected.id}</h4>
                {selected.type === "tap" && (
                  <div className="keymap-field">
                    <span>Key</span>
                    {keyButton("key", selected.key, "Key")}
                    <div className="keymap-mouse-row">
                      {[0, 2].map((button) => (
                        <button
                          key={button}
                          type="button"
                          className="btn btn-ghost btn-sm"
                          onClick={() =>
                            bind(
                              { id: selected.id, field: "key" },
                              mouseKey(button)
                            )
                          }
                        >
                          {button === 0 ? "Left click" : "Right click"}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
                {selected.type === "joystick" && (
                  <>
                    {DIRECTIONS.map(([field, label]) => (
                      <div className="keymap-field" key={field}>
                        <span>{label}</span>
                        {keyButton(field, selected.keys?.[field], label)}
                      </div>
                    ))}
                    <label className="keymap-field">
                      <span>Size</span>
                      <input
                        type="range"
                        min="0.03"
                        max="0.2"
                        step="0.005"
                        value={selected.radius ?? 0.07}
                        onChange={(e) =>
                          update(selected.id, (node) => ({
                            ...node,
                            radius: Number(e.target.value),
                          }))
                        }
                      />
                    </label>
                  </>
                )}
                {selected.type === "aim" && (
                  <>
                    <label className="keymap-field">
                      <span>Active</span>
                      <select
                        value={selected.activation_mode}
                        onChange={(e) =>
                          update(selected.id, (node) => {
                            const always = e.target.value === "always_on";
                            const next = {
                              ...node,
                              activation_mode: e.target.value,
                            };
                            if (always) delete next.activation_key;
                            else
                              next.activation_key =
                                node.activation_key || "MouseRight";
                            return next;
                          })
                        }
                      >
                        <option value="always_on">
                          Always (while captured)
                        </option>
                        <option value="while_held">While a key is held</option>
                        <option value="toggle">Toggled by a key</option>
                      </select>
                    </label>
                    {selected.activation_mode !== "always_on" && (
                      <div className="keymap-field">
                        <span>Key</span>
                        {keyButton(
                          "activation_key",
                          selected.activation_key,
                          "Aim key"
                        )}
                      </div>
                    )}
                    <label className="keymap-field">
                      <span>Speed</span>
                      <input
                        type="range"
                        min="0.2"
                        max="3"
                        step="0.05"
                        value={selected.sensitivity ?? 1}
                        onChange={(e) =>
                          update(selected.id, (node) => ({
                            ...node,
                            sensitivity: Number(e.target.value),
                          }))
                        }
                      />
                    </label>
                  </>
                )}
                {!["tap", "joystick", "aim"].includes(selected.type) && (
                  <p className="keymap-hint">
                    A {selected.type} control from Phantom&apos;s editor: drag
                    it here, change it in Phantom.
                  </p>
                )}
                <button
                  type="button"
                  className="btn btn-outline-danger btn-sm"
                  onClick={removeSelected}
                >
                  <Trash2 size={14} aria-hidden="true" />
                  <span>Remove</span>
                </button>
              </>
            ) : (
              <p className="keymap-hint">
                {nodes.length === 0
                  ? "Click on the game screen to place the first control."
                  : "Select a control to change its key."}
              </p>
            )}
            {issues.length > 0 && (
              <ul className="keymap-issues" aria-label="Problems">
                {issues.map((p) => (
                  <li key={`${p.id}-${p.text}`}>
                    <button type="button" onClick={() => setSelectedId(p.id)}>
                      {p.id} {p.text}
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <p className="keymap-hint keymap-hotkeys">
              In the game: F8 captures keyboard and mouse, F1 switches mouse
              aim, F9 pauses. {RESERVED_KEYS.join(", ")} stay Phantom&apos;s.
            </p>
          </aside>
        </div>
      </div>
    </div>
  );
}
