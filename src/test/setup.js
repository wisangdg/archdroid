import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

// jsdom never gives the document focus, which would put every poll on the
// background schedule (src/lib/pollGate.js). The app window has focus.
document.hasFocus = () => true;
