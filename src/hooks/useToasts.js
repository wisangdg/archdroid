import { useCallback, useState } from "react";

/** In-app toast queue. Each toast removes itself after `duration` ms (0 = sticky). */
export function useToasts() {
  const [toasts, setToasts] = useState([]);

  const addToast = useCallback(
    ({ type = "info", title, message, duration = 4000 }) => {
      const id =
        Date.now().toString() + Math.random().toString(36).substring(2, 6);
      setToasts((prev) => [...prev, { id, type, title, message }]);

      if (duration > 0) {
        setTimeout(() => {
          setToasts((prev) => prev.filter((t) => t.id !== id));
        }, duration);
      }
    },
    []
  );

  const removeToast = useCallback((id) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  return { toasts, addToast, removeToast };
}
