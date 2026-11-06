# Upstream report: `persist.waydroid.fake_touch` does not reach WebView

Draft issue and pull request for [waydroid/android_vendor_waydroid](https://github.com/waydroid/android_vendor_waydroid).
ArchDroid works around the problem with [touch mode](../../src-tauri/src/touchmode.rs). This draft fixes the cause for every Waydroid user, on any compositor.

**Status.** The patch applies cleanly to LineageOS `lineage-20.0`. Its two new methods compile and pass behavioural checks against stub `MotionEvent`/`InputDevice` classes. It has **not** been built into a Waydroid image or run on a device. Say so in the pull request.

Files in this directory:

| File | What it is |
|---|---|
| `0016-Fake-touch-inputs-for-select-apps.patch` | Drop-in replacement for `waydroid-patches/base-patches-33/frameworks/base/0016-Fake-touch-inputs-for-select-apps.patch` |
| `waydroid-fake-touch-finger.incremental.diff` | The same change as a diff on top of today's 0016, for review |

---

## Issue text

**Title:** `fake_touch` only changes the event source, so WebView still treats clicks as mouse input

**Summary.** With `persist.waydroid.fake_touch` set for a package, mouse clicks still do not work in touch-only web content inside that app. The patch rewrites the event source to `SOURCE_TOUCHSCREEN`, but the tool type stays `TOOL_TYPE_MOUSE` and the button state stays `BUTTON_PRIMARY`. Hover and `BUTTON_PRESS`/`BUTTON_RELEASE` events are also still delivered.

**Reproduce.**
1. Install Plants vs Zombies 2 (`com.ea.game.pvz2_row`). On first launch it shows EA's birth-date prompt, an HTML page in a WebView whose month/year picker reacts only to touch.
2. `waydroid prop set persist.waydroid.fake_touch com.ea.game.pvz2_row`, then restart the app.
3. Click the date field with a mouse. The field animates, but the picker never opens.
4. For comparison, `input touchscreen tap <x> <y>` or a real (or uinput) touchscreen on the host opens the picker at once.

Tested on LineageOS 20.0 (`20260927-GAPPS`), vendor `20261022-MAINLINE`, Waydroid 1.6.3, Hyprland 0.56.

**Cause.** Chromium's `EventForwarder.touchEventRequiresSpecialHandling()` (`ui/android/java/src/org/chromium/ui/base/EventForwarder.java`) sends a touch-path event to `onMouseEvent()` when `event.getToolType(0) == TOOL_TYPE_MOUSE` and `getButtonState() != 0`. The source is not consulted there, so the current patch has no effect on WebView. This also explains why `input mouse tap` works: it injects `buttonState == 0`, which Chromium treats as touch.

**Fix.** In `ViewRootImpl.WindowInputEventReceiver.onInputEvent`, for apps that match `fake_touch`:
- A primary-button `DOWN`/`MOVE`/`UP`/`CANCEL` is re-created as a finger event: `TOOL_TYPE_FINGER`, `buttonState = 0`, `SOURCE_TOUCHSCREEN`, and pressure 1 when it was 0. Raw coordinates are kept.
- Hover events and primary `BUTTON_PRESS`/`BUTTON_RELEASE` are dropped, because a finger has neither.
- Right and middle clicks and the scroll wheel are delivered unchanged.

`MotionEvent` has no tool-type setter, so the finger event is a copy. The copy is enqueued without a receiver, like other synthesized events, and the original mouse event is finished at once. That keeps `InputEventReceiver`'s sequence bookkeeping intact.

---

## Pull request text

**Title:** `base-patches-33: make fake_touch deliver real finger events`

Replaces the body of `0016-Fake-touch-inputs-for-select-apps.patch`. The property, its glob matching, and the log line are unchanged. Only how a matching app's mouse events are rewritten changes (see the issue for the cause).

Testing so far:
- The patch applies to LineageOS `lineage-20.0` `core/java/android/view/ViewRootImpl.java`.
- The new methods compile, and a stub-based check covers hover drop, the primary-button gesture, right click, and the wheel.
- **Not yet built into an image.** Help with a device test is welcome: with `fake_touch` set for PvZ2, the birth-date picker should open and scroll with the mouse.
