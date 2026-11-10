#!/usr/bin/env python3
"""Drive the installed ArchDroid (real binary, WebKitGTK) through WebKitWebDriver
and measure what the UI experiences. Usage: vmprobe.py PHASE [PHASE...]"""
import base64, json, os, subprocess, sys, time, urllib.request

WD = "http://127.0.0.1:4444"
OUT = os.path.expanduser("~/out/review")
os.makedirs(OUT, exist_ok=True)


def call(method, path, body=None, timeout=400):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(WD + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.load(r)["value"]


sid = call("POST", "/session", {"capabilities": {"alwaysMatch": {
    "webkitgtk:browserOptions": {"binary": "/usr/bin/archdroid", "args": []}}}})["sessionId"]
S = f"/session/{sid}"
call("POST", S + "/timeouts", {"script": 380000})
js = lambda src, *a: call("POST", S + "/execute/sync", {"script": src, "args": list(a)})
ajs = lambda src, *a: call("POST", S + "/execute/async", {"script": src, "args": list(a)})


def shot(name):
    open(f"{OUT}/{name}.png", "wb").write(base64.b64decode(call("GET", S + "/screenshot")))


def sh(cmd):
    return subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout.strip()


def click(label, exact=False):
    ok = js("""const el = [...document.querySelectorAll('button')]
                 .find(e => (arguments[1] ? e.textContent.trim() === arguments[0]
                                          : e.textContent.trim().includes(arguments[0])) && !e.disabled);
               if (!el) return false; el.click(); return true;""", label, exact)
    if not ok:
        raise RuntimeError(f"no enabled button with {label!r}")
    time.sleep(1.2)


def until(pattern, timeout):
    end = time.time() + timeout
    while time.time() < end:
        if js("return new RegExp(arguments[0]).test(document.body.innerText)", pattern):
            return True
        time.sleep(1)
    return False


TIMED = """
const [cmd, args, done] = arguments;
const t0 = performance.now();
window.__TAURI_INTERNALS__.invoke(cmd, args).then(
  (r) => done({ ms: Math.round(performance.now() - t0), status: r && r.status, code: r && r.code,
                bytes: JSON.stringify(r === undefined ? null : r).length,
                data: typeof (r && r.data) === "string" ? r.data.slice(0, 60) : undefined }),
  (e) => done({ ms: Math.round(performance.now() - t0), rejected: String(e && e.message || e).slice(0, 120) }));
"""


def timed(cmd, args=None, repeat=3):
    runs = [ajs(TIMED, cmd, args or {}) for _ in range(repeat)]
    ms = sorted(r["ms"] for r in runs)
    last = runs[-1]
    extra = {k: v for k, v in last.items() if k != "ms"}
    print(f"  {cmd:28s} {str(args or ''):48s} ms={ms} {extra}", flush=True)
    return runs


def cpu_pids():
    """pid -> (name, own jiffies, waited-children jiffies) for the app's processes."""
    out = {}
    for pid in sh("pgrep -f 'archdroid|WebKit'").split():
        try:
            stat = open(f"/proc/{pid}/stat").read()
            name = stat[stat.index("(") + 1:stat.rindex(")")]
            f = stat[stat.rindex(")") + 2:].split()
            out[pid] = (name, int(f[11]) + int(f[12]), int(f[13]) + int(f[14]))
        except OSError:
            pass
    return out


def cpu_window(label, seconds):
    hz = os.sysconf("SC_CLK_TCK")
    a = cpu_pids()
    time.sleep(seconds)
    b = cpu_pids()
    parts = []
    total = 0.0
    for pid, (name, own, kids) in b.items():
        if pid not in a or name in ("pgrep", "python3", "sh"):
            continue
        d_own = (own - a[pid][1]) / hz
        d_kids = (kids - a[pid][2]) / hz
        total += d_own + d_kids
        if d_own or d_kids:
            parts.append(f"{name}={100 * d_own / seconds:.1f}%" + (f" (+children {100 * d_kids / seconds:.1f}%)" if d_kids else ""))
    print(f"  {label:58s} total={100 * total / seconds:5.1f}% of one core over {seconds}s | {', '.join(parts)}", flush=True)


def phase_stopped():
    print("== IPC timings, engine stopped")
    for cmd, args in [("waydroid_status", None), ("get_waydroid_apps", None), ("check_gapps_status", None),
                      ("check_houdini_status", None), ("check_multitouch_status", None),
                      ("touch_mode_status", None), ("check_host_diagnostics", None),
                      ("waydroid_prop_get", {"key": "persist.waydroid.width"}),
                      ("waydroid_density_get", None), ("waydroid_boot_state", None),
                      ("app_log_location", None), ("app_log_read", None)]:
        timed(cmd, args)
    print("== startup: commands the UI sends when it opens (wrapped after load, so re-run by reload)")
    js("""window.__ipc = []; const real = window.__TAURI_INTERNALS__.invoke;
          sessionStorage.setItem('probe', '1');""")


def phase_idle_stopped():
    print("== idle CPU, engine stopped, My Apps")
    cpu_window("engine stopped, window visible", 60)


def phase_launch_stopped():
    print("== Launch an app while the engine is stopped")
    print("  before:", sh("waydroid status | head -2 | tr '\\n' ' '"))
    js("""window.__launch = { t0: performance.now(), done: null };
          window.__TAURI_INTERNALS__.invoke('waydroid_launch_app', { package: 'com.android.settings' }).then(
            (r) => { window.__launch.done = { ms: Math.round(performance.now() - window.__launch.t0), r }; },
            (e) => { window.__launch.done = { ms: Math.round(performance.now() - window.__launch.t0), e: String(e) }; });""")
    start = time.time()
    seen_busy = None
    while time.time() - start < 170:
        time.sleep(10)
        done = js("return window.__launch.done")
        bar = js("return document.querySelector('.status-text')?.textContent")
        status = sh("waydroid status | head -2 | tr '\\n' ' '")
        procs = sh("pgrep -af 'waydroid app launch|waydroid session' | grep -v pgrep | cut -c1-70 | tr '\\n' ';'")
        if seen_busy is None and not done:
            seen_busy = ajs(TIMED, "waydroid_prop_set", {"key": "persist.waydroid.multi_windows", "value": "true"})
            print("  another exclusive command meanwhile:", seen_busy, flush=True)
        print(f"  t+{int(time.time() - start):3d}s top bar={bar!r} waydroid: {status} | procs: {procs} | invoke: {done}", flush=True)
        if done:
            break
    time.sleep(8)
    print("  after:", sh("waydroid status | head -3 | tr '\\n' ' '"), "| top bar:", js("return document.querySelector('.status-text')?.textContent"))
    print("  toasts:", js("return [...document.querySelectorAll('.toast-item')].map(t => t.innerText.replace(/\\n/g, ' | ').slice(0, 200))"))
    print("  lxc:", sh("sudo -n lxc-info -P /var/lib/waydroid/lxc -n waydroid 2>&1 | head -3 | tr '\\n' ' '"))
    shot("launch-stopped-after")



def phase_launch_broken():
    print("== Launch again in the half-dead state (container up, session manager killed)")
    print("  status:", sh("waydroid status | head -2 | tr '\\n' ' '"), "| top bar:", js("return document.querySelector('.status-text')?.textContent"))
    timed("waydroid_launch_app", {"package": "com.android.settings"}, repeat=1)
    print("  resumed activity:", sh("adb -s 192.168.240.112:5555 shell dumpsys activity activities 2>/dev/null | grep -m2 -i 'ResumedActivity' | cut -c1-120"))
    click("Stop Engine")
    print("  Stop Engine -> stopped:", until("Engine Stopped", 90), "|", sh("waydroid status | head -1"))



def phase_launch_broken2():
    print("== Same state made by hand: session started by `waydroid app launch`, then that process killed")
    subprocess.Popen("nohup waydroid app launch com.android.settings >/tmp/launch.log 2>&1 &", shell=True)
    for _ in range(60):
        time.sleep(3)
        if sh("adb -s 192.168.240.112:5555 shell getprop sys.boot_completed 2>/dev/null") == "1":
            break
        sh("adb connect 192.168.240.112:5555 >/dev/null 2>&1")
    time.sleep(15)
    top = "adb -s 192.168.240.112:5555 shell dumpsys activity activities 2>/dev/null | grep -m1 topResumedActivity | cut -c1-110"
    print("  booted; resumed:", sh(top))
    sh("pkill -9 -f 'waydroid app launch'")
    time.sleep(8)
    print("  killed; status:", sh("waydroid status | head -2 | tr '\\n' ' '"), "| top bar:", js("return document.querySelector('.status-text')?.textContent"))
    timed("waydroid_launch_app", {"package": "com.android.calculator2"}, repeat=1)
    time.sleep(6)
    print("  resumed after launching Calculator:", sh(top))
    print("  cli says:", sh("timeout 20 waydroid app launch com.android.calculator2 2>&1 | tr '\\n' ' '"))
    click("Stop Engine")
    print("  Stop Engine -> stopped:", until("Engine Stopped", 90), "|", sh("waydroid status | head -1"))



def phase_pulse_variants():
    print("== what makes the pulsing dot expensive (engine running, My Apps, 40 s each)")
    click("My Apps")
    time.sleep(3)
    print("  user-select (WebKitGTK):", js("""const pick = (s) => { const el = document.querySelector(s); return el ? getComputedStyle(el).webkitUserSelect || getComputedStyle(el).userSelect : null; };
        return { appName: pick('#app-detail-title'), package: pick('.app-detail-package'), search: pick('#app-search') }"""))
    variants = [
        ("as shipped", None),
        ("top bar without backdrop-filter", ".topbar{backdrop-filter:none !important;-webkit-backdrop-filter:none !important}"),
        ("no backdrop-filter anywhere", "*{backdrop-filter:none !important;-webkit-backdrop-filter:none !important}"),
        ("pulse animates opacity only (no box-shadow, no scale)", "@keyframes probePulse{0%,100%{opacity:1}50%{opacity:.45}} .status-online .status-dot{animation:probePulse 2s infinite !important;box-shadow:none !important}"),
        ("pulse off", ".status-dot{animation:none !important}"),
    ]
    for label, css in variants:
        js("document.getElementById('probe-css')?.remove()")
        if css:
            js("const s = document.createElement('style'); s.id = 'probe-css'; s.textContent = arguments[0]; document.head.appendChild(s);", css)
        time.sleep(3)
        cpu_window(label, 40)
    js("document.getElementById('probe-css')?.remove()")


def phase_start():
    print("== Start Engine from the UI")
    t0 = time.time()
    click("Start Engine")
    print("  Engine Running|Booting shown:", until("Android Booting|Engine Running", 180), f"after {time.time() - t0:.0f}s")
    print("  booted:", until("Engine Running", 240), f"after {time.time() - t0:.0f}s")
    time.sleep(10)


def phase_running():
    print("== IPC timings, engine running and booted")
    for cmd, args in [("waydroid_status", None), ("get_waydroid_apps", None), ("waydroid_boot_state", None),
                      ("waydroid_prop_get", {"key": "persist.waydroid.width"}),
                      ("waydroid_density_get", None), ("check_gapps_status", None),
                      ("check_houdini_status", None), ("check_multitouch_status", None),
                      ("check_host_diagnostics", None)]:
        timed(cmd, args)
    print("  Settings load as the UI does it (4 x prop get + density, one after another):")
    seq = ajs("""const done = arguments[arguments.length - 1]; const inv = window.__TAURI_INTERNALS__.invoke; (async () => {
        const t0 = performance.now();
        for (const key of ['persist.waydroid.multi_windows', 'persist.waydroid.invert_colors', 'persist.waydroid.width', 'persist.waydroid.height'])
          await inv('waydroid_prop_get', { key });
        await inv('waydroid_density_get');
        const seq = Math.round(performance.now() - t0);
        const t1 = performance.now();
        await Promise.all([...['persist.waydroid.multi_windows', 'persist.waydroid.invert_colors', 'persist.waydroid.width', 'persist.waydroid.height']
          .map((key) => inv('waydroid_prop_get', { key })), inv('waydroid_density_get')]);
        done({ sequential_ms: seq, parallel_ms: Math.round(performance.now() - t1) });
      })();""")
    print("   ", seq)


def phase_idle_running():
    print("== idle CPU, engine running, My Apps (status dot pulses)")
    click("My Apps")
    time.sleep(3)
    cpu_window("engine running, as shipped", 60)
    js("""const s = document.createElement('style'); s.id = 'probe-nopulse';
          s.textContent = '.status-dot{animation:none !important}'; document.head.appendChild(s);""")
    time.sleep(2)
    cpu_window("engine running, pulse animation off", 60)
    js("document.getElementById('probe-nopulse').remove()")


SCROLL = """
const done = arguments[arguments.length - 1];
const el = document.querySelector('.content-scrollable');
el.scrollTop = 0;
const max = el.scrollHeight - el.clientHeight;
let frames = 0; const t0 = performance.now();
const step = () => {
  frames++;
  el.scrollTop = Math.min(max, el.scrollTop + 24);
  if (el.scrollTop >= max - 1) return done({ px: max, frames, ms: Math.round(performance.now() - t0), fps: +(1000 * frames / (performance.now() - t0)).toFixed(1) });
  requestAnimationFrame(step);
};
requestAnimationFrame(step);
"""


def phase_scroll():
    print("== scrolling Settings top to bottom, 24 px per frame")
    click("Settings")
    time.sleep(4)
    for label, css in [("as shipped", None),
                       ("backdrop-filter off", "*{backdrop-filter:none !important;-webkit-backdrop-filter:none !important}"),
                       ("as shipped (again)", None)]:
        if css:
            js("const s = document.createElement('style'); s.id = 'probe-css'; s.textContent = arguments[0]; document.head.appendChild(s);", css)
        else:
            js("document.getElementById('probe-css')?.remove()")
        time.sleep(1)
        a = cpu_pids()
        r = ajs(SCROLL)
        b = cpu_pids()
        hz = os.sysconf("SC_CLK_TCK")
        cpu = sum((b[p][1] - a[p][1]) / hz for p in b if p in a and b[p][0] not in ("pgrep", "python3", "sh"))
        print(f"  {label:24s} {r} cpu={cpu:.2f}s", flush=True)
    click("My Apps")


def phase_logs():
    print("== View Logs:", sh("wc -l < /var/lib/waydroid/waydroid.log"), "lines in waydroid.log")
    click("Settings")
    time.sleep(2)
    t0 = time.time()
    js("""window.__t0 = performance.now(); window.__lastChange = 0; window.__n = 0;
          new MutationObserver(() => { window.__lastChange = performance.now(); }).observe(document.body, { childList: true, subtree: true, characterData: true });
          [...document.querySelectorAll('button')].find(b => b.textContent.includes('View Logs')).click();""")
    end = time.time() + 90
    while time.time() < end:
        time.sleep(1)
        st = js("return { n: document.querySelectorAll('.terminal-log-line').length, idle: performance.now() - window.__lastChange, total: window.__lastChange - window.__t0, finished: !!document.querySelector('.terminal-footer-actions .btn-primary') }")
        if st["finished"] and st["idle"] > 1500:
            break
    print(f"  lines rendered={st['n']} time from click to last DOM change={st['total'] / 1000:.2f}s (wall {time.time() - t0:.1f}s)")
    st = js("""const el = document.querySelector('.terminal-console-body'); const last = el.lastElementChild.getBoundingClientRect(); const box = el.getBoundingClientRect();
               return { lastLine: el.lastElementChild.textContent.trim(), lastLineVisible: last.bottom <= box.bottom + 1, scrollTop: Math.round(el.scrollTop), maxScroll: el.scrollHeight - el.clientHeight };""")
    print("  after finish:", st)
    shot("view-logs")
    click("Done")
    click("My Apps")


def phase_shots():
    print("== screenshots (WebKitGTK)")
    for tab in ["My Apps", "Keymapping", "Install Tools", "Settings"]:
        click(tab)
        time.sleep(2.5)
        shot("tab-" + tab.replace(" ", ""))
    for i, y in enumerate([700, 1400, 2100, 2800, 3500]):
        js("document.querySelector('.content-scrollable').scrollTop = arguments[0]", y)
        time.sleep(1)
        shot(f"settings-scroll-{i + 1}")
    click("Factory Reset")
    time.sleep(1.5)
    shot("confirm-wipe")
    print("  wipe dialog focus:", js("return document.activeElement.textContent"))
    click("Cancel", True)
    click("Install Tools")
    try:
        click("Install GApps")
        time.sleep(2)
        shot("confirm-gapps")
        click("Cancel", True)
    except RuntimeError as e:
        print("  ", e)
    click("My Apps")
    js("""const row = document.querySelectorAll('.app-row')[2]; const r = row.getBoundingClientRect();
          row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: r.left + 120, clientY: r.top + 20 }));""")
    time.sleep(1)
    shot("context-menu")
    js("window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))")


def phase_stop():
    print("== Stop Engine")
    try:
        click("Stop Engine")
        print("  stopped:", until("Engine Stopped", 90))
    except RuntimeError as e:
        print("  ", e, "|", sh("waydroid status | head -1"))


try:
    time.sleep(8)
    print("window:", js("return [innerWidth, innerHeight, devicePixelRatio, document.visibilityState]"),
          "| top bar height:", js("return Math.round(document.querySelector('.topbar').getBoundingClientRect().height)"))
    for name in sys.argv[1:]:
        globals()["phase_" + name]()
finally:
    try:
        call("DELETE", S)
    except Exception:
        pass
