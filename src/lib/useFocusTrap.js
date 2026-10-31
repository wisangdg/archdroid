import { useEffect, useRef } from "react";

const FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(", ");

/**
 * Traps keyboard focus within container, manages initial focus and restores
 * focus to the trigger element when the container closes or unmounts.
 */
export function useFocusTrap(isOpen, onClose) {
  const containerRef = useRef(null);
  const previousActiveElement = useRef(null);

  useEffect(() => {
    if (!isOpen) return;

    previousActiveElement.current = document.activeElement;

    const el = containerRef.current;
    if (!el) return;

    // Set initial focus: prefer autofocus/primary action button, otherwise first focusable
    const focusables = Array.from(el.querySelectorAll(FOCUSABLE_SELECTOR));
    const initialTarget =
      el.querySelector("[autofocus], [data-autofocus]") ||
      el.querySelector("button.btn-primary, button.btn-danger") ||
      focusables[0];
    if (initialTarget && typeof initialTarget.focus === "function") {
      initialTarget.focus();
    }

    const handleKeyDown = (e) => {
      if (e.key === "Escape") {
        if (onClose) {
          e.preventDefault();
          onClose();
        }
        return;
      }

      if (e.key === "Tab") {
        const items = Array.from(el.querySelectorAll(FOCUSABLE_SELECTOR));
        if (items.length === 0) {
          e.preventDefault();
          return;
        }

        const first = items[0];
        const last = items[items.length - 1];

        if (e.shiftKey) {
          if (
            document.activeElement === first ||
            !el.contains(document.activeElement)
          ) {
            e.preventDefault();
            last.focus();
          }
        } else {
          if (
            document.activeElement === last ||
            !el.contains(document.activeElement)
          ) {
            e.preventDefault();
            first.focus();
          }
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      if (
        previousActiveElement.current &&
        typeof previousActiveElement.current.focus === "function"
      ) {
        previousActiveElement.current.focus();
      }
    };
  }, [isOpen, onClose]);

  return containerRef;
}
