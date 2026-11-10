import { useCallback, useState } from "react";

/** Most toasts on screen; older ones leave when a new one arrives. */
export const MAX_TOASTS = 3;

/**
 * How long a toast stays when the caller gives no duration: an error has to be
 * read (and often copied), and a long message needs longer than a short one.
 */
export function defaultDuration(type, message = "") {
  const base = type === "error" ? 12000 : type === "warning" ? 8000 : 4000;
  const extra = Math.max(0, String(message).length - 80) * 50;
  return Math.min(base + extra, 30000);
}

/**
 * In-app toast queue. `duration` is in ms (0 = stays until dismissed); the
 * toast itself runs the timer so it can pause while the pointer is on it.
 * The same toast again restarts the one on screen instead of stacking.
 */
export function useToasts() {
  const [toasts, setToasts] = useState([]);

  const addToast = useCallback(
    ({ type = "info", title, message, duration }) => {
      const id =
        Date.now().toString() + Math.random().toString(36).substring(2, 6);
      const toast = {
        id,
        type,
        title,
        message,
        duration: duration ?? defaultDuration(type, message),
      };
      setToasts((prev) =>
        [
          ...prev.filter(
            (t) =>
              !(t.type === type && t.title === title && t.message === message)
          ),
          toast,
        ].slice(-MAX_TOASTS)
      );
    },
    []
  );

  const removeToast = useCallback((id) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return { toasts, addToast, removeToast };
}
