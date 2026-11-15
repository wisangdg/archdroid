import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import KeymapEditor from "../components/keymap/KeymapEditor";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const APP = { name: "Shooter", package: "com.example.shooter" };
const SHOT = {
  data_url: "data:image/png;base64,AAAA",
  width: 1000,
  height: 500,
};

function setup(saved = null, replies = {}) {
  invoke.mockImplementation(async (cmd) => {
    if (cmd === "keymap_load") return saved;
    if (cmd === "keymap_screenshot") return SHOT;
    if (cmd === "keymap_save") return { status: "success", data: "/p.json" };
    if (cmd === "keymap_apply") return { status: "success" };
    return replies[cmd];
  });
  const addToast = vi.fn();
  const onClose = vi.fn();
  render(<KeymapEditor app={APP} addToast={addToast} onClose={onClose} />);
  return { addToast, onClose };
}

async function canvas() {
  const el = await screen.findByTestId("keymap-canvas");
  el.getBoundingClientRect = () => ({
    left: 0,
    top: 0,
    width: 1000,
    height: 500,
    right: 1000,
    bottom: 500,
  });
  return el;
}

const savedProfile = () =>
  invoke.mock.calls.find(([cmd]) => cmd === "keymap_save")?.[1];

describe("keymap editor", () => {
  beforeEach(() => invoke.mockReset());

  it("places a button where the game shows it and binds the next key", async () => {
    setup();
    const stage = await canvas();
    fireEvent.click(stage, { clientX: 250, clientY: 100 });
    // The new button asks for its key right away.
    expect(
      screen.getByRole("button", { name: /Key: not set/ }).textContent
    ).toBe("Press a key…");
    fireEvent.keyDown(window, { code: "KeyJ", key: "j" });
    const node = within(stage).getByRole("button", { name: "button-1: J" });
    expect(node.style.left).toBe("25%");
    expect(node.style.top).toBe("20%");

    fireEvent.click(screen.getByRole("radio", { name: /Joystick/ }));
    fireEvent.click(stage, { clientX: 150, clientY: 350 });
    fireEvent.click(screen.getByRole("button", { name: /Save & Apply/ }));

    await waitFor(() => expect(savedProfile()).toBeTruthy());
    const { package: pkg, profile } = savedProfile();
    expect(pkg).toBe("com.example.shooter");
    expect(profile.screen).toEqual({ width: 1000, height: 500 });
    expect(profile.name).toBe("Shooter");
    expect(profile.nodes).toEqual([
      {
        id: "button-1",
        type: "tap",
        pos: { x: 0.25, y: 0.2 },
        key: "J",
        slot: 0,
      },
      expect.objectContaining({
        type: "joystick",
        pos: { x: 0.15, y: 0.7 },
        keys: { up: "W", down: "S", left: "A", right: "D" },
        slot: 1,
      }),
    ]);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("keymap_apply", {
        package: "com.example.shooter",
      })
    );
  });

  it("does not save a button without a key", async () => {
    const { addToast } = setup();
    fireEvent.click(await canvas(), { clientX: 500, clientY: 250 });
    fireEvent.keyDown(window, { code: "Escape", key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: /^Save$/ }));
    expect(addToast).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Keymap Not Finished",
        message: "button-1 has no key yet.",
      })
    );
    expect(savedProfile()).toBeUndefined();
  });

  it("opens the saved keymap of the game and asks before discarding edits", async () => {
    const { onClose } = setup({
      name: "Shooter",
      version: 1,
      screen: { width: 1920, height: 1080 },
      nodes: [
        {
          id: "jump",
          type: "tap",
          slot: 0,
          pos: { x: 0.9, y: 0.8 },
          key: "Space",
        },
      ],
    });
    const stage = await canvas();
    const jump = await within(stage).findByRole("button", {
      name: "jump: Space",
    });
    fireEvent.pointerDown(jump);
    fireEvent.click(screen.getByRole("button", { name: /Remove/ }));
    fireEvent.click(
      screen.getByRole("button", { name: "Close keymap editor" })
    );
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(onClose).toHaveBeenCalled();
  });

  it("says what to do when the game screen cannot be captured", async () => {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === "keymap_screenshot")
        throw { message: "The Waydroid session is not running." };
      return null;
    });
    render(<KeymapEditor app={APP} addToast={vi.fn()} onClose={vi.fn()} />);
    expect(
      await screen.findByText(/Could not capture the game screen/)
    ).toBeTruthy();
    expect(
      screen.getByText(/open the game, then press Recapture/)
    ).toBeTruthy();
  });
});
