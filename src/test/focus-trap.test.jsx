import { describe, it, expect, vi } from "vitest";
import { render, fireEvent } from "@testing-library/react";
import { useFocusTrap } from "../lib/useFocusTrap";

function Dialog({ onClose }) {
  // Inline callback, like every caller: a new function on each render.
  const ref = useFocusTrap(true, onClose ? () => onClose() : undefined);
  return (
    <div ref={ref}>
      <input type="range" aria-label="Size" />
      <button className="btn btn-primary">Save</button>
    </div>
  );
}

describe("useFocusTrap", () => {
  it("keeps focus where the user put it when the dialog re-renders", () => {
    const { getByLabelText, rerender } = render(<Dialog onClose={vi.fn()} />);
    const slider = getByLabelText("Size");
    slider.focus();

    for (let i = 0; i < 5; i++) rerender(<Dialog onClose={vi.fn()} />);

    expect(document.activeElement).toBe(slider);
  });

  it("calls the latest close callback on Escape, and none when it is gone", () => {
    const first = vi.fn();
    const latest = vi.fn();
    const { rerender } = render(<Dialog onClose={first} />);
    rerender(<Dialog onClose={latest} />);

    fireEvent.keyDown(window, { key: "Escape" });
    expect(first).not.toHaveBeenCalled();
    expect(latest).toHaveBeenCalledTimes(1);

    rerender(<Dialog onClose={undefined} />);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(latest).toHaveBeenCalledTimes(1);
  });
});
