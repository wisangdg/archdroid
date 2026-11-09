//! Touch mode: a left click over a Waydroid window becomes a finger touch.
//!
//! Why this exists (every claim was checked on a running Waydroid, not assumed):
//!
//! - Waydroid forwards the host mouse to Android as a mouse (`wayland_pointer`,
//!   tool type `MOUSE`). Pages inside an Android WebView that only listen for
//!   touch, such as EA's birth-date picker in Plants vs Zombies 2, ignore it.
//! - `persist.waydroid.fake_touch` does not help: the framework patch behind it
//!   (`0016-Fake-touch-inputs-for-select-apps.patch`) only rewrites the event
//!   *source* to touchscreen. The tool type stays `MOUSE` and the button state
//!   stays set, and Chromium's `EventForwarder` sends exactly those events down
//!   its mouse path (see `docs/upstream/waydroid-fake-touch.md`).
//! - A touch from a Linux touchscreen reaches Android through Waydroid's
//!   `wl_touch` path as a real finger, and the same picker opens at once.
//!
//! So touch mode grabs the physical mice, relays them through a virtual mouse,
//! and turns a left-button press over a Waydroid window into a touch on a
//! virtual touchscreen. While the finger is down the cursor does not move:
//! Waydroid would otherwise send hover events, and Android drops the touch
//! gesture as soon as a hover from the mouse arrives.
//!
//! Cursor and window positions come from Hyprland's IPC socket. Other
//! compositors are reported as unsupported instead of silently doing nothing.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, Device, EventType, InputEvent, KeyCode, PropType,
    RelativeAxisCode, UinputAbsSetup,
};
use serde::{Deserialize, Serialize};

/// Names of the devices touch mode creates. Anything whose name starts with
/// [`DEVICE_PREFIX`] is never grabbed, so the bridge cannot feed itself.
pub const DEVICE_PREFIX: &str = "ArchDroid";
const MOUSE_NAME: &str = "ArchDroid Virtual Mouse";
const TOUCH_NAME: &str = "ArchDroid Touch";

/// Touchscreen coordinate range. The compositor maps the whole range onto the
/// output, so only the ratio matters.
pub const TOUCH_MAX: i32 = 32767;

/// Installed by the packages; the in-app setup writes the same rule to `/etc`.
pub const UDEV_RULE_FILE: &str = "70-archdroid-touch.rules";
pub const PACKAGED_RULE_DIR: &str = "/usr/lib/udev/rules.d";
pub const LOCAL_RULE_DIR: &str = "/etc/udev/rules.d";

/// `uaccess` gives the logged-in seat user an ACL on the node, the same way
/// Steam's `60-steam-input.rules` does. Keyboards are left out on purpose: touch
/// mode never needs to read keystrokes. The rule must sort before
/// `73-seat-late.rules`, which turns the tag into an ACL.
pub const UDEV_RULE: &str = "\
# ArchDroid touch mode: lets the logged-in desktop user turn mouse clicks over
# Waydroid windows into touches. Keyboards are deliberately not included.
KERNEL==\"uinput\", SUBSYSTEM==\"misc\", TAG+=\"uaccess\", OPTIONS+=\"static_node=uinput\"
SUBSYSTEM==\"input\", KERNEL==\"event*\", ENV{ID_INPUT_MOUSE}==\"1\", ENV{ID_INPUT_KEYBOARD}!=\"1\", TAG+=\"uaccess\"
";

// ---------------------------------------------------------------------------
// Pure geometry and parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn contains(&self, (x, y): (f64, f64)) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    /// Keeps a dragged finger inside the window it started in.
    pub fn clamp(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (
            x.clamp(self.x, self.x + self.w - 1.0),
            y.clamp(self.y, self.y + self.h - 1.0),
        )
    }
}

/// What a press would land on: the window under the cursor and the output it
/// is on, both in the compositor's logical (scaled) coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub cursor: (f64, f64),
    pub window: Rect,
    pub monitor: Rect,
}

/// Waydroid names its windows `waydroid.<package>` in multi-window mode and
/// `Waydroid` for the full Android UI.
pub fn is_waydroid_class(class: &str) -> bool {
    class == "Waydroid" || class.starts_with("waydroid.")
}

/// Logical point to touchscreen units on `monitor`, or `None` off the output.
pub fn touch_abs(point: (f64, f64), monitor: &Rect) -> Option<(i32, i32)> {
    if !monitor.contains(point) || monitor.w <= 0.0 || monitor.h <= 0.0 {
        return None;
    }
    let scale = |offset: f64, size: f64| ((offset / size) * f64::from(TOUCH_MAX)).round() as i32;
    Some((
        scale(point.0 - monitor.x, monitor.w).clamp(0, TOUCH_MAX),
        scale(point.1 - monitor.y, monitor.h).clamp(0, TOUCH_MAX),
    ))
}

/// `j/cursorpos` answers `{"x": 905, "y": 940}` in logical coordinates.
pub fn parse_cursor(json: &str) -> Option<(f64, f64)> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    Some((value.get("x")?.as_f64()?, value.get("y")?.as_f64()?))
}

/// The active window as `(class, rect)`; `None` when nothing is focused, which
/// Hyprland answers with `{}`.
pub fn parse_active_window(json: &str) -> Option<(String, Rect)> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let class = value.get("class")?.as_str()?.to_string();
    let at = value.get("at")?.as_array()?;
    let size = value.get("size")?.as_array()?;
    Some((
        class,
        Rect {
            x: at.first()?.as_f64()?,
            y: at.get(1)?.as_f64()?,
            w: size.first()?.as_f64()?,
            h: size.get(1)?.as_f64()?,
        },
    ))
}

/// Outputs in logical coordinates. `width`/`height` are physical pixels, so
/// they are divided by the scale, and swapped for 90/270 degree transforms.
pub fn parse_monitors(json: &str) -> Vec<Rect> {
    let Ok(serde_json::Value::Array(monitors)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    monitors
        .iter()
        .filter_map(|monitor| {
            let number = |key: &str| monitor.get(key)?.as_f64();
            let scale = number("scale").filter(|scale| *scale > 0.0).unwrap_or(1.0);
            let (mut w, mut h) = (number("width")? / scale, number("height")? / scale);
            if number("transform").is_some_and(|t| (t as i64) % 2 == 1) {
                std::mem::swap(&mut w, &mut h);
            }
            Some(Rect {
                x: number("x")?,
                y: number("y")?,
                w,
                h,
            })
        })
        .collect()
}

/// Combines the three IPC answers into a [`Target`], or `None` when the press
/// should stay a normal mouse click.
pub fn resolve_target(
    cursor: Option<(f64, f64)>,
    active: Option<(String, Rect)>,
    monitors: &[Rect],
) -> Option<Target> {
    let cursor = cursor?;
    let (class, window) = active?;
    if !is_waydroid_class(&class) || !window.contains(cursor) {
        return None;
    }
    let monitor = *monitors.iter().find(|monitor| monitor.contains(cursor))?;
    Some(Target {
        cursor,
        window,
        monitor,
    })
}

// ---------------------------------------------------------------------------
// Click-to-touch state machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Relay these events to the virtual mouse (a SYN is appended on emit).
    Forward(Vec<InputEvent>),
    TouchDown(i32, i32),
    TouchMove(i32, i32),
    TouchUp,
    /// Put the cursor where the finger was lifted, in logical coordinates.
    WarpCursor(f64, f64),
}

/// Answers "what is under the cursor right now". Real code asks Hyprland;
/// tests use a fixed answer.
pub trait Probe {
    fn target(&mut self) -> Option<Target>;
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    pos: (f64, f64),
    window: Rect,
    monitor: Rect,
    moved: bool,
}

#[derive(Debug, Default)]
pub struct Bridge {
    drag: Option<Drag>,
}

impl Bridge {
    pub fn touching(&self) -> bool {
        self.drag.is_some()
    }

    /// Handles one evdev frame (the events up to a `SYN_REPORT`, which is not
    /// included) from any physical mouse.
    pub fn on_frame(&mut self, frame: &[InputEvent], probe: &mut dyn Probe) -> Vec<Action> {
        let mut forward = Vec::new();
        let mut actions = Vec::new();
        let mut moved_this_frame = false;

        for event in frame {
            let (kind, code, value) = (event.event_type(), event.code(), event.value());
            if kind == EventType::SYNCHRONIZATION {
                continue;
            }
            if kind == EventType::KEY && code == KeyCode::BTN_LEFT.code() {
                match (value, self.drag) {
                    (1, None) => match probe.target() {
                        Some(target) => match touch_abs(target.cursor, &target.monitor) {
                            Some((x, y)) => {
                                self.drag = Some(Drag {
                                    pos: target.cursor,
                                    window: target.window,
                                    monitor: target.monitor,
                                    moved: false,
                                });
                                actions.push(Action::TouchDown(x, y));
                            }
                            None => forward.push(*event),
                        },
                        None => forward.push(*event),
                    },
                    (0, Some(drag)) => {
                        actions.push(Action::TouchUp);
                        if drag.moved {
                            actions.push(Action::WarpCursor(drag.pos.0, drag.pos.1));
                        }
                        self.drag = None;
                    }
                    // Autorepeat or a second press while the finger is down.
                    (_, Some(_)) => {}
                    _ => forward.push(*event),
                }
                continue;
            }
            if kind == EventType::RELATIVE {
                if let Some(drag) = self.drag.as_mut() {
                    let delta = f64::from(value);
                    if code == RelativeAxisCode::REL_X.0 {
                        drag.pos = drag.window.clamp((drag.pos.0 + delta, drag.pos.1));
                        drag.moved = true;
                        moved_this_frame = true;
                        continue;
                    }
                    if code == RelativeAxisCode::REL_Y.0 {
                        drag.pos = drag.window.clamp((drag.pos.0, drag.pos.1 + delta));
                        drag.moved = true;
                        moved_this_frame = true;
                        continue;
                    }
                }
            }
            forward.push(*event);
        }

        if moved_this_frame {
            if let Some((x, y)) = self
                .drag
                .and_then(|drag| touch_abs(drag.pos, &drag.monitor))
            {
                actions.push(Action::TouchMove(x, y));
            }
        }
        if !forward.is_empty() {
            actions.insert(0, Action::Forward(forward));
        }
        actions
    }
}

// ---------------------------------------------------------------------------
// Hyprland IPC
// ---------------------------------------------------------------------------

fn hyprland_socket() -> Option<PathBuf> {
    let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let path = Path::new(&runtime)
        .join("hypr")
        .join(signature)
        .join(".socket.sock");
    path.exists().then_some(path)
}

fn hyprland_request(socket: &Path, command: &str) -> io::Result<String> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.write_all(command.as_bytes())?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    Ok(reply)
}

struct Hyprland {
    socket: PathBuf,
}

impl Probe for Hyprland {
    fn target(&mut self) -> Option<Target> {
        let ask = |command| hyprland_request(&self.socket, command).ok();
        resolve_target(
            ask("j/cursorpos").as_deref().and_then(parse_cursor),
            ask("j/activewindow")
                .as_deref()
                .and_then(parse_active_window),
            &ask("j/monitors")
                .as_deref()
                .map(parse_monitors)
                .unwrap_or_default(),
        )
    }
}

impl Hyprland {
    /// Hyprland 0.5x with a Lua config takes `hl.dsp.cursor.move`; older
    /// releases take `movecursor x y`. Try the new form first.
    fn warp_cursor(&self, x: f64, y: f64) {
        let (x, y) = (x.round() as i64, y.round() as i64);
        let lua = format!("dispatch hl.dsp.cursor.move({{ x = {x}, y = {y} }})");
        let reply = hyprland_request(&self.socket, &lua).unwrap_or_default();
        if !reply.trim_start().starts_with("ok") {
            let _ = hyprland_request(&self.socket, &format!("dispatch movecursor {x} {y}"));
        }
    }
}

// ---------------------------------------------------------------------------
// Devices
// ---------------------------------------------------------------------------

fn is_mouse(device: &Device) -> bool {
    let name = device.name().unwrap_or_default();
    if name.starts_with(DEVICE_PREFIX) {
        return false;
    }
    let has_motion = device.supported_relative_axes().is_some_and(|axes| {
        axes.contains(RelativeAxisCode::REL_X) && axes.contains(RelativeAxisCode::REL_Y)
    });
    let has_left = device
        .supported_keys()
        .is_some_and(|keys| keys.contains(KeyCode::BTN_LEFT));
    // Touchpads and tablets are absolute devices; leave them alone.
    let absolute = device.supported_absolute_axes().is_some();
    has_motion && has_left && !absolute
}

/// Every readable mouse, keyed by its device node.
fn readable_mice() -> Vec<(PathBuf, Device)> {
    let Ok(entries) = std::fs::read_dir("/dev/input") else {
        return Vec::new();
    };
    let mut mice: Vec<(PathBuf, Device)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("event"))
        })
        .filter_map(|path| Device::open(&path).ok().map(|device| (path, device)))
        .filter(|(_, device)| is_mouse(device))
        .collect();
    mice.sort_by(|a, b| a.0.cmp(&b.0));
    mice
}

/// Union of what the grabbed mice can send, so extra buttons and wheels keep
/// working through the relay.
fn relay_capabilities(
    mice: &[(PathBuf, Device)],
) -> (AttributeSet<KeyCode>, AttributeSet<RelativeAxisCode>) {
    let mut keys = AttributeSet::<KeyCode>::new();
    let mut axes = AttributeSet::<RelativeAxisCode>::new();
    for key in [KeyCode::BTN_LEFT, KeyCode::BTN_RIGHT, KeyCode::BTN_MIDDLE] {
        keys.insert(key);
    }
    for axis in [
        RelativeAxisCode::REL_X,
        RelativeAxisCode::REL_Y,
        RelativeAxisCode::REL_WHEEL,
        RelativeAxisCode::REL_HWHEEL,
        RelativeAxisCode::REL_WHEEL_HI_RES,
        RelativeAxisCode::REL_HWHEEL_HI_RES,
    ] {
        axes.insert(axis);
    }
    for (_, device) in mice {
        if let Some(supported) = device.supported_keys() {
            for key in supported.iter() {
                keys.insert(key);
            }
        }
        if let Some(supported) = device.supported_relative_axes() {
            for axis in supported.iter() {
                axes.insert(axis);
            }
        }
    }
    (keys, axes)
}

fn build_relay(
    keys: &AttributeSet<KeyCode>,
    axes: &AttributeSet<RelativeAxisCode>,
) -> io::Result<VirtualDevice> {
    VirtualDevice::builder()?
        .name(MOUSE_NAME)
        .with_keys(keys)?
        .with_relative_axes(axes)?
        .build()
}

fn build_touchscreen() -> io::Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    keys.insert(KeyCode::BTN_TOUCH);
    keys.insert(KeyCode::BTN_TOOL_FINGER);
    let mut props = AttributeSet::<PropType>::new();
    props.insert(PropType::DIRECT);
    // Resolution in units per millimetre keeps libinput from guessing a size.
    let axis = |code, max| UinputAbsSetup::new(code, AbsInfo::new(0, 0, max, 0, 0, 100));
    VirtualDevice::builder()?
        .name(TOUCH_NAME)
        .with_keys(&keys)?
        .with_properties(&props)?
        .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_X, TOUCH_MAX))?
        .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_Y, TOUCH_MAX))?
        .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_MT_SLOT, 0))?
        .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_MT_TRACKING_ID, 65535))?
        .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_MT_POSITION_X, TOUCH_MAX))?
        .with_absolute_axis(&axis(AbsoluteAxisCode::ABS_MT_POSITION_Y, TOUCH_MAX))?
        .build()
}

/// The evdev frames for one touch transition, in kernel MT protocol B order.
pub fn touch_events(action: &Action, tracking_id: i32) -> Vec<InputEvent> {
    let abs = |code: AbsoluteAxisCode, value| InputEvent::new(EventType::ABSOLUTE.0, code.0, value);
    let key = |code: KeyCode, value| InputEvent::new(EventType::KEY.0, code.code(), value);
    match *action {
        Action::TouchDown(x, y) => vec![
            abs(AbsoluteAxisCode::ABS_MT_SLOT, 0),
            abs(AbsoluteAxisCode::ABS_MT_TRACKING_ID, tracking_id),
            abs(AbsoluteAxisCode::ABS_MT_POSITION_X, x),
            abs(AbsoluteAxisCode::ABS_MT_POSITION_Y, y),
            key(KeyCode::BTN_TOUCH, 1),
            key(KeyCode::BTN_TOOL_FINGER, 1),
            abs(AbsoluteAxisCode::ABS_X, x),
            abs(AbsoluteAxisCode::ABS_Y, y),
        ],
        Action::TouchMove(x, y) => vec![
            abs(AbsoluteAxisCode::ABS_MT_SLOT, 0),
            abs(AbsoluteAxisCode::ABS_MT_POSITION_X, x),
            abs(AbsoluteAxisCode::ABS_MT_POSITION_Y, y),
            abs(AbsoluteAxisCode::ABS_X, x),
            abs(AbsoluteAxisCode::ABS_Y, y),
        ],
        Action::TouchUp => vec![
            abs(AbsoluteAxisCode::ABS_MT_SLOT, 0),
            abs(AbsoluteAxisCode::ABS_MT_TRACKING_ID, -1),
            key(KeyCode::BTN_TOUCH, 0),
            key(KeyCode::BTN_TOOL_FINGER, 0),
        ],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Access checks and status
// ---------------------------------------------------------------------------

fn uinput_writable() -> bool {
    std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/uinput")
        .is_ok()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TouchModeStatus {
    /// The user asked for touch mode (persisted across launches).
    pub enabled: bool,
    /// The bridge thread is running and holds the mice.
    pub running: bool,
    /// The compositor is one touch mode can ask for cursor positions.
    pub supported: bool,
    pub uinput_access: bool,
    pub mice: Vec<String>,
    /// The packaged or locally installed udev rule is present.
    pub rule_installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn rule_installed() -> bool {
    [PACKAGED_RULE_DIR, LOCAL_RULE_DIR]
        .iter()
        .any(|dir| Path::new(dir).join(UDEV_RULE_FILE).exists())
}

/// Root-side script for installs without the packaged rule (AppImage, source).
/// The payload is base64 like `capabilities::install_script`, so nothing in
/// the rule can escape its quoting.
pub fn install_rule_script() -> String {
    use base64::{engine::general_purpose, Engine as _};

    let encoded = general_purpose::STANDARD.encode(UDEV_RULE.as_bytes());
    let target = Path::new(LOCAL_RULE_DIR).join(UDEV_RULE_FILE);
    let target = target.to_string_lossy();
    format!(
        "set -euo pipefail\n\
         mkdir -p '{LOCAL_RULE_DIR}'\n\
         printf '%s' '{encoded}' | base64 -d > '{target}.tmp'\n\
         chmod 644 '{target}.tmp'\n\
         chown root:root '{target}.tmp'\n\
         mv '{target}.tmp' '{target}'\n\
         udevadm control --reload-rules\n\
         udevadm trigger --action=change --subsystem-match=misc --sysname-match=uinput\n\
         udevadm trigger --action=change --subsystem-match=input\n\
         udevadm settle --timeout=10 || true\n\
         echo '[SUCCESS] Touch mode access rule installed.'\n"
    )
}

// ---------------------------------------------------------------------------
// Persisted preference
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Deserialize)]
struct Preference {
    enabled: bool,
}

fn preference_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(std::env::temp_dir)
        .join("io.github.archdroid")
        .join("touch-mode.json")
}

fn load_enabled(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Preference>(&text).ok())
        .is_some_and(|preference| preference.enabled)
}

fn save_enabled(path: &Path, enabled: bool) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(&Preference { enabled })?)?;
    std::fs::rename(tmp, path)
}

// ---------------------------------------------------------------------------
// The bridge thread
// ---------------------------------------------------------------------------

struct Running {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn set_error(error: Option<String>) {
    if let Ok(mut slot) = LAST_ERROR.lock() {
        *slot = error;
    }
}

fn is_running() -> bool {
    RUNNING
        .lock()
        .map(|slot| {
            slot.as_ref()
                .is_some_and(|running| !running.handle.is_finished())
        })
        .unwrap_or(false)
}

pub fn status() -> TouchModeStatus {
    TouchModeStatus {
        enabled: load_enabled(&preference_path()),
        running: is_running(),
        supported: hyprland_socket().is_some(),
        uinput_access: uinput_writable(),
        mice: readable_mice()
            .iter()
            .map(|(_, device)| device.name().unwrap_or("mouse").to_string())
            .collect(),
        rule_installed: rule_installed(),
        error: LAST_ERROR.lock().ok().and_then(|slot| slot.clone()),
    }
}

/// Turns touch mode on or off and remembers the choice. Errors explain what
/// is missing; the preference is only stored once the bridge actually runs.
pub fn set_enabled(enabled: bool) -> Result<TouchModeStatus, String> {
    if enabled {
        start()?;
    } else {
        stop();
    }
    save_enabled(&preference_path(), enabled)
        .map_err(|error| format!("Touch mode changed, but the setting was not saved: {error}"))?;
    Ok(status())
}

/// Called once at app start: brings touch mode back if it was left on.
pub fn restore() {
    if load_enabled(&preference_path()) {
        if let Err(error) = start() {
            log::warn!("touch mode was enabled but could not start: {error}");
        }
    }
}

fn start() -> Result<(), String> {
    let mut slot = RUNNING
        .lock()
        .map_err(|_| "touch mode state is poisoned".to_string())?;
    if slot
        .as_ref()
        .is_some_and(|running| !running.handle.is_finished())
    {
        return Ok(());
    }
    let socket = hyprland_socket().ok_or(
        "Touch mode needs Hyprland for now: other compositors do not tell apps where the cursor is.",
    )?;
    if !uinput_writable() {
        return Err("No write access to /dev/uinput. Use \"Enable access\" first, then log out and back in if it still fails.".into());
    }
    let mice = readable_mice();
    if mice.is_empty() {
        return Err("No mouse is readable by your user. Use \"Enable access\" first; touchpads are not supported yet.".into());
    }
    // Build both virtual devices before grabbing anything: if either fails,
    // the physical mouse must keep working.
    let (keys, axes) = relay_capabilities(&mice);
    let relay = build_relay(&keys, &axes)
        .map_err(|error| format!("Could not create the virtual mouse: {error}"))?;
    let touch = build_touchscreen()
        .map_err(|error| format!("Could not create the virtual touchscreen: {error}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    set_error(None);
    let handle = thread::Builder::new()
        .name("touch-mode".into())
        .spawn(move || {
            if let Err(error) = run(mice, relay, touch, Hyprland { socket }, &flag, &keys, &axes) {
                log::error!("touch mode stopped: {error}");
                set_error(Some(error.to_string()));
            }
        })
        .map_err(|error| format!("Could not start touch mode: {error}"))?;
    *slot = Some(Running { stop, handle });
    Ok(())
}

fn stop() {
    let running = RUNNING.lock().ok().and_then(|mut slot| slot.take());
    if let Some(running) = running {
        running.stop.store(true, Ordering::SeqCst);
        let _ = running.handle.join();
    }
}

/// Whether a newly plugged mouse fits the relay that already exists.
fn fits_relay(
    device: &Device,
    keys: &AttributeSet<KeyCode>,
    axes: &AttributeSet<RelativeAxisCode>,
) -> bool {
    device
        .supported_keys()
        .is_none_or(|supported| supported.iter().all(|key| keys.contains(key)))
        && device
            .supported_relative_axes()
            .is_none_or(|supported| supported.iter().all(|axis| axes.contains(axis)))
}

fn grab(path: &Path, mut device: Device) -> Option<Device> {
    if let Err(error) = device.grab() {
        log::warn!("touch mode could not grab {}: {error}", path.display());
        return None;
    }
    if let Err(error) = device.set_nonblocking(true) {
        log::warn!("touch mode could not poll {}: {error}", path.display());
        let _ = device.ungrab();
        return None;
    }
    Some(device)
}

fn run(
    mice: Vec<(PathBuf, Device)>,
    mut relay: VirtualDevice,
    mut touch: VirtualDevice,
    hyprland: Hyprland,
    stop: &AtomicBool,
    keys: &AttributeSet<KeyCode>,
    axes: &AttributeSet<RelativeAxisCode>,
) -> io::Result<()> {
    let mut grabbed: HashMap<PathBuf, (Device, Vec<InputEvent>)> = mice
        .into_iter()
        .filter_map(|(path, device)| grab(&path, device).map(|device| (path, (device, Vec::new()))))
        .collect();
    if grabbed.is_empty() {
        return Err(io::Error::other("no mouse could be grabbed"));
    }

    let mut bridge = Bridge::default();
    let mut probe = hyprland;
    let mut tracking_id = 0;
    let mut last_scan = Instant::now();

    while !stop.load(Ordering::SeqCst) {
        let paths: Vec<PathBuf> = grabbed.keys().cloned().collect();
        let mut fds: Vec<libc::pollfd> = paths
            .iter()
            .map(|path| libc::pollfd {
                fd: grabbed[path].0.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();
        // SAFETY: `fds` is a valid, exclusively borrowed array of pollfd for
        // the duration of the call, and its length is passed alongside it.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 100) };
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }

        let mut gone = Vec::new();
        for (index, path) in paths.iter().enumerate() {
            let revents = fds[index].revents;
            if revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
                gone.push(path.clone());
                continue;
            }
            if revents & libc::POLLIN == 0 {
                continue;
            }
            let Some((device, pending)) = grabbed.get_mut(path) else {
                continue;
            };
            let events: Vec<InputEvent> = match device.fetch_events() {
                Ok(events) => events.collect(),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                Err(_) => {
                    gone.push(path.clone());
                    continue;
                }
            };
            for event in events {
                if event.event_type() == EventType::SYNCHRONIZATION {
                    let frame = std::mem::take(pending);
                    for action in bridge.on_frame(&frame, &mut probe) {
                        apply(&action, &mut relay, &mut touch, &probe, &mut tracking_id)?;
                    }
                } else {
                    pending.push(event);
                }
            }
        }
        for path in gone {
            grabbed.remove(&path);
        }

        // Pick up mice plugged in after start, as long as the relay can carry
        // everything they send.
        if !bridge.touching() && last_scan.elapsed() > Duration::from_secs(2) {
            last_scan = Instant::now();
            for (path, device) in readable_mice() {
                if !grabbed.contains_key(&path) && fits_relay(&device, keys, axes) {
                    if let Some(device) = grab(&path, device) {
                        grabbed.insert(path, (device, Vec::new()));
                    }
                }
            }
        }
    }

    if bridge.touching() {
        apply(
            &Action::TouchUp,
            &mut relay,
            &mut touch,
            &probe,
            &mut tracking_id,
        )?;
    }
    // Dropping the devices releases the grabs; the physical mice go straight
    // back to the compositor.
    Ok(())
}

fn apply(
    action: &Action,
    relay: &mut VirtualDevice,
    touch: &mut VirtualDevice,
    hyprland: &Hyprland,
    tracking_id: &mut i32,
) -> io::Result<()> {
    match action {
        Action::Forward(events) => relay.emit(events),
        Action::WarpCursor(x, y) => {
            hyprland.warp_cursor(*x, *y);
            Ok(())
        }
        Action::TouchDown(..) => {
            *tracking_id = (*tracking_id + 1) % 65535;
            touch.emit(&touch_events(action, *tracking_id))
        }
        Action::TouchMove(..) | Action::TouchUp => touch.emit(&touch_events(action, *tracking_id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = Rect {
        x: 0.0,
        y: 192.0,
        w: 1536.0,
        h: 960.0,
    };
    const WINDOW: Rect = Rect {
        x: 17.0,
        y: 230.0,
        w: 1502.0,
        h: 910.0,
    };

    struct Fixed(Option<Target>);
    impl Probe for Fixed {
        fn target(&mut self) -> Option<Target> {
            self.0
        }
    }

    fn over_waydroid(cursor: (f64, f64)) -> Fixed {
        Fixed(Some(Target {
            cursor,
            window: WINDOW,
            monitor: MONITOR,
        }))
    }

    fn key(code: KeyCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::KEY.0, code.code(), value)
    }
    fn rel(code: RelativeAxisCode, value: i32) -> InputEvent {
        InputEvent::new(EventType::RELATIVE.0, code.0, value)
    }

    #[test]
    fn waydroid_windows_are_recognised_by_class() {
        assert!(is_waydroid_class("waydroid.com.ea.game.pvz2_row"));
        assert!(is_waydroid_class("Waydroid"));
        assert!(!is_waydroid_class("archdroid"));
        assert!(!is_waydroid_class("org.mozilla.firefox"));
        assert!(!is_waydroid_class("waydroidish"));
    }

    #[test]
    fn touch_abs_maps_the_monitor_onto_the_full_range() {
        assert_eq!(touch_abs((0.0, 192.0), &MONITOR), Some((0, 0)));
        assert_eq!(touch_abs((768.0, 672.0), &MONITOR), Some((16384, 16384)));
        assert_eq!(touch_abs((-1.0, 500.0), &MONITOR), None);
        assert_eq!(touch_abs((10.0, 100.0), &MONITOR), None, "above the output");
        let empty = Rect { w: 0.0, ..MONITOR };
        assert_eq!(touch_abs((0.0, 192.0), &empty), None);
    }

    #[test]
    fn hyprland_replies_parse() {
        assert_eq!(
            parse_cursor("\n{\n    \"x\": 905,\n    \"y\": 940\n}\n"),
            Some((905.0, 940.0))
        );
        assert_eq!(parse_cursor("garbage"), None);

        let active =
            r#"{"class": "waydroid.com.ea.game.pvz2_row", "at": [17, 230], "size": [1502, 910]}"#;
        assert_eq!(
            parse_active_window(active),
            Some(("waydroid.com.ea.game.pvz2_row".to_string(), WINDOW))
        );
        assert_eq!(parse_active_window("{}"), None, "nothing focused");

        let monitors = r#"[{"x": 0, "y": 192, "width": 1920, "height": 1200, "scale": 1.25, "transform": 0},
                          {"x": 1536, "y": 0, "width": 1080, "height": 1920, "scale": 1.0, "transform": 1}]"#;
        assert_eq!(
            parse_monitors(monitors),
            vec![
                MONITOR,
                Rect {
                    x: 1536.0,
                    y: 0.0,
                    w: 1920.0,
                    h: 1080.0
                }
            ]
        );
        assert!(parse_monitors("{}").is_empty());
    }

    #[test]
    fn only_a_focused_waydroid_window_under_the_cursor_is_a_target() {
        let monitors = [MONITOR];
        let waydroid = Some(("waydroid.x".to_string(), WINDOW));
        assert!(resolve_target(Some((500.0, 500.0)), waydroid.clone(), &monitors).is_some());
        assert!(
            resolve_target(Some((5.0, 500.0)), waydroid.clone(), &monitors).is_none(),
            "outside the window"
        );
        assert!(resolve_target(None, waydroid.clone(), &monitors).is_none());
        assert!(resolve_target(
            Some((500.0, 500.0)),
            Some(("kitty".into(), WINDOW)),
            &monitors
        )
        .is_none());
        assert!(
            resolve_target(Some((500.0, 500.0)), waydroid, &[]).is_none(),
            "no output"
        );
    }

    #[test]
    fn a_click_over_waydroid_becomes_a_tap_and_never_reaches_the_mouse() {
        let mut bridge = Bridge::default();
        let mut probe = over_waydroid((768.0, 672.0));
        assert_eq!(
            bridge.on_frame(&[key(KeyCode::BTN_LEFT, 1)], &mut probe),
            vec![Action::TouchDown(16384, 16384)]
        );
        assert!(bridge.touching());
        assert_eq!(
            bridge.on_frame(&[key(KeyCode::BTN_LEFT, 0)], &mut probe),
            vec![Action::TouchUp],
            "no warp when the finger did not move"
        );
        assert!(!bridge.touching());
    }

    #[test]
    fn a_drag_moves_the_finger_and_freezes_the_cursor() {
        let mut bridge = Bridge::default();
        let mut probe = over_waydroid((768.0, 672.0));
        bridge.on_frame(&[key(KeyCode::BTN_LEFT, 1)], &mut probe);

        let actions = bridge.on_frame(
            &[
                rel(RelativeAxisCode::REL_X, 10),
                rel(RelativeAxisCode::REL_Y, -20),
            ],
            &mut probe,
        );
        assert_eq!(
            actions.len(),
            1,
            "motion is not forwarded while touching: {actions:?}"
        );
        assert!(matches!(actions[0], Action::TouchMove(..)));

        let release = bridge.on_frame(&[key(KeyCode::BTN_LEFT, 0)], &mut probe);
        assert_eq!(
            release,
            vec![Action::TouchUp, Action::WarpCursor(778.0, 652.0)]
        );
    }

    #[test]
    fn a_drag_stays_inside_the_window_it_started_in() {
        let mut bridge = Bridge::default();
        let mut probe = over_waydroid((20.0, 240.0));
        bridge.on_frame(&[key(KeyCode::BTN_LEFT, 1)], &mut probe);
        bridge.on_frame(
            &[
                rel(RelativeAxisCode::REL_X, -500),
                rel(RelativeAxisCode::REL_Y, -500),
            ],
            &mut probe,
        );
        let release = bridge.on_frame(&[key(KeyCode::BTN_LEFT, 0)], &mut probe);
        assert_eq!(release[1], Action::WarpCursor(WINDOW.x, WINDOW.y));
    }

    #[test]
    fn clicks_elsewhere_and_other_buttons_pass_through_untouched() {
        let mut bridge = Bridge::default();
        let mut elsewhere = Fixed(None);
        let press = [key(KeyCode::BTN_LEFT, 1)];
        assert_eq!(
            bridge.on_frame(&press, &mut elsewhere),
            vec![Action::Forward(press.to_vec())]
        );
        assert!(!bridge.touching());

        let mut probe = over_waydroid((768.0, 672.0));
        let other = [
            key(KeyCode::BTN_RIGHT, 1),
            rel(RelativeAxisCode::REL_WHEEL, 1),
        ];
        assert_eq!(
            bridge.on_frame(&other, &mut probe),
            vec![Action::Forward(other.to_vec())]
        );
    }

    #[test]
    fn right_click_during_a_touch_is_still_relayed() {
        let mut bridge = Bridge::default();
        let mut probe = over_waydroid((768.0, 672.0));
        bridge.on_frame(&[key(KeyCode::BTN_LEFT, 1)], &mut probe);
        let actions = bridge.on_frame(
            &[key(KeyCode::BTN_RIGHT, 1), key(KeyCode::BTN_LEFT, 2)],
            &mut probe,
        );
        assert_eq!(
            actions,
            vec![Action::Forward(vec![key(KeyCode::BTN_RIGHT, 1)])]
        );
        assert!(bridge.touching(), "autorepeat does not lift the finger");
    }

    #[test]
    fn touch_frames_follow_the_multitouch_protocol() {
        let down = touch_events(&Action::TouchDown(10, 20), 7);
        let codes: Vec<u16> = down.iter().map(|event| event.code()).collect();
        assert_eq!(codes[0], AbsoluteAxisCode::ABS_MT_SLOT.0);
        assert_eq!(
            (down[1].code(), down[1].value()),
            (AbsoluteAxisCode::ABS_MT_TRACKING_ID.0, 7)
        );
        assert!(down
            .iter()
            .any(|e| e.code() == KeyCode::BTN_TOUCH.code() && e.value() == 1));

        let up = touch_events(&Action::TouchUp, 7);
        assert!(up
            .iter()
            .any(|e| e.code() == AbsoluteAxisCode::ABS_MT_TRACKING_ID.0 && e.value() == -1));
        assert!(up
            .iter()
            .any(|e| e.code() == KeyCode::BTN_TOUCH.code() && e.value() == 0));
        assert!(touch_events(&Action::WarpCursor(1.0, 1.0), 7).is_empty());
    }

    #[test]
    fn the_preference_round_trips_and_defaults_to_off() {
        let dir = std::env::temp_dir().join(format!("archdroid-touch-{}", std::process::id()));
        let path = dir.join("touch-mode.json");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!load_enabled(&path), "missing file means off");
        save_enabled(&path, true).unwrap();
        assert!(load_enabled(&path));
        save_enabled(&path, false).unwrap();
        assert!(!load_enabled(&path));
        std::fs::write(&path, "not json").unwrap();
        assert!(!load_enabled(&path), "a corrupt file means off");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_rule_grants_uinput_and_mice_but_never_keyboards() {
        assert!(UDEV_RULE.contains("KERNEL==\"uinput\""));
        assert!(UDEV_RULE.contains("ENV{ID_INPUT_MOUSE}==\"1\""));
        assert!(UDEV_RULE.contains("ENV{ID_INPUT_KEYBOARD}!=\"1\""));
        assert!(!UDEV_RULE.contains("ID_INPUT_KEYBOARD}==\"1\""));
        assert!(
            UDEV_RULE_FILE < "73-seat-late.rules",
            "uaccess is applied by 73-seat-late"
        );
    }

    #[test]
    fn the_packaged_rule_file_matches_the_built_in_rule() {
        let packaged = include_str!("../../packaging/udev/70-archdroid-touch.rules");
        assert_eq!(packaged, UDEV_RULE);
    }

    #[test]
    fn install_rule_script_is_valid_bash_and_carries_the_rule() {
        use base64::Engine as _;
        let script = install_rule_script();
        assert!(script.starts_with("set -euo pipefail\n"));
        let encoded = script
            .split_once("printf '%s' '")
            .and_then(|(_, rest)| rest.split_once('\''))
            .map(|(payload, _)| payload)
            .expect("quoted payload");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), UDEV_RULE);

        let output = std::process::Command::new("bash")
            .args(["-n", "-c", &script])
            .output()
            .expect("failed to run bash");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Drives the real bridge for a while so a person or script can click through
/// it: `ARCHDROID_TOUCH_LIVE_SECS=10 cargo test live_bridge -- --ignored`.
/// It grabs the real mice, so it never runs in the normal test suite.
#[cfg(test)]
mod live {
    #[test]
    #[ignore = "grabs the real mice; run by hand inside a Hyprland session"]
    fn live_bridge() {
        let secs = std::env::var("ARCHDROID_TOUCH_LIVE_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(10);
        super::start().expect("touch mode should start");
        std::thread::sleep(std::time::Duration::from_secs(secs));
        assert!(
            super::is_running(),
            "bridge died: {:?}",
            super::status().error
        );
        super::stop();
        assert!(!super::is_running());
    }

    /// Everything except reading the physical mouse: real Hyprland probe, real
    /// virtual touchscreen and relay, fed with a synthetic press. Focus a
    /// Waydroid window and put the cursor on the spot first; it refuses to
    /// click anything else.
    #[test]
    #[ignore = "creates virtual input devices; run by hand inside a Hyprland session"]
    fn live_synthetic_click() {
        use super::*;
        let drag_px: i32 = std::env::var("ARCHDROID_TOUCH_DRAG_PX")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let socket = hyprland_socket().expect("Hyprland IPC socket");
        let mut probe = Hyprland { socket };
        assert!(
            probe.target().is_some(),
            "the cursor is not over a focused Waydroid window; refusing to click"
        );
        let (keys, axes) = relay_capabilities(&[]);
        let mut relay = build_relay(&keys, &axes).unwrap();
        let mut touch = build_touchscreen().unwrap();
        // Waydroid creates its touch pipe only after the seat gains touch.
        std::thread::sleep(Duration::from_secs(5));

        let key = |value| InputEvent::new(EventType::KEY.0, KeyCode::BTN_LEFT.code(), value);
        let mut bridge = Bridge::default();
        let mut tracking_id = 0;
        let mut feed = |frame: &[InputEvent], bridge: &mut Bridge, probe: &mut Hyprland| {
            for action in bridge.on_frame(frame, probe) {
                eprintln!("action {action:?}");
                apply(&action, &mut relay, &mut touch, probe, &mut tracking_id).unwrap();
            }
        };
        feed(&[key(1)], &mut bridge, &mut probe);
        assert!(bridge.touching(), "the press did not become a touch");
        for _ in 0..(drag_px.abs() / 8) {
            std::thread::sleep(Duration::from_millis(10));
            let step = InputEvent::new(
                EventType::RELATIVE.0,
                RelativeAxisCode::REL_Y.0,
                8 * drag_px.signum(),
            );
            feed(&[step], &mut bridge, &mut probe);
        }
        std::thread::sleep(Duration::from_millis(80));
        feed(&[key(0)], &mut bridge, &mut probe);
        assert!(!bridge.touching());
        std::thread::sleep(Duration::from_secs(1));
    }
}
