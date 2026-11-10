// While the window is hidden or in the background nobody reads the status, so
// background polls are cut down: a hidden window skips every tick, an
// unfocused one polls at most once per UNFOCUSED_POLL_MS. Focusing the window
// checks again right away (see the focus listeners next to each poll).
export const UNFOCUSED_POLL_MS = 30000;

export function pollDue(
  lastRun,
  now,
  { hidden = document.hidden, focused = document.hasFocus() } = {}
) {
  if (hidden) return false;
  if (focused) return true;
  return now - lastRun >= UNFOCUSED_POLL_MS;
}
