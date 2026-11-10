import { describe, it, expect, vi, afterEach } from "vitest";
import {
  render,
  screen,
  renderHook,
  act,
  fireEvent,
} from "@testing-library/react";
import ToastContainer from "../components/ui/ToastContainer";
import { useToasts, defaultDuration, MAX_TOASTS } from "../hooks/useToasts";

afterEach(() => vi.useRealTimers());

describe("toasts", () => {
  it("keeps the newest three and merges a repeated toast", () => {
    const { result } = renderHook(() => useToasts());
    act(() => {
      for (let i = 0; i < 6; i++)
        result.current.addToast({ title: "App Stopped", message: `app ${i}` });
    });
    expect(result.current.toasts.map((t) => t.message)).toEqual([
      "app 3",
      "app 4",
      "app 5",
    ]);
    expect(result.current.toasts.length).toBe(MAX_TOASTS);
    act(() => {
      result.current.addToast({ title: "App Stopped", message: "app 4" });
    });
    expect(result.current.toasts.map((t) => t.message)).toEqual([
      "app 3",
      "app 5",
      "app 4",
    ]);
  });

  it("gives errors and long messages more time", () => {
    expect(defaultDuration("info", "short")).toBe(4000);
    expect(defaultDuration("error", "short")).toBe(12000);
    expect(defaultDuration("warning", "x".repeat(200))).toBe(8000 + 120 * 50);
    const { result } = renderHook(() => useToasts());
    act(() => result.current.addToast({ message: "m", duration: 2500 }));
    expect(result.current.toasts[0].duration).toBe(2500);
  });

  it("has its live region before the first toast", () => {
    const { container } = render(
      <ToastContainer toasts={[]} onDismiss={() => {}} />
    );
    expect(container.querySelector("[aria-live='polite']")).toBeTruthy();
  });

  it("does not dismiss a toast while the pointer is on it", () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    render(
      <ToastContainer
        toasts={[
          {
            id: "t1",
            type: "error",
            title: "Launch Failed",
            message: "why",
            duration: 1000,
          },
        ]}
        onDismiss={onDismiss}
      />
    );
    const toast = screen.getByRole("status");
    fireEvent.mouseEnter(toast);
    act(() => vi.advanceTimersByTime(5000));
    expect(onDismiss).not.toHaveBeenCalled();
    fireEvent.mouseLeave(toast);
    act(() => vi.advanceTimersByTime(1000));
    expect(onDismiss).toHaveBeenCalledWith("t1");
    expect(screen.getByRole("button", { name: "Copy message" })).toBeTruthy();
  });
});
