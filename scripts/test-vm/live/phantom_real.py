#!/usr/bin/env python3
"""Install Phantom from the Keymapping tab, then Launch Keymapper with the real
phantom-gui and check that the process stays up. Needs Rust, a JDK, and an
Android SDK (d8 + android.jar) in the VM; run from a login shell."""
import base64, json, subprocess, sys, time, urllib.request

WD = "http://127.0.0.1:4444"
BIN, OUT = sys.argv[1], sys.argv[2]


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(WD + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=180) as r:
        return json.load(r)["value"]


sid = call("POST", "/session", {"capabilities": {"alwaysMatch": {
    "webkitgtk:browserOptions": {"binary": BIN, "args": []}}}})["sessionId"]
S = f"/session/{sid}"
js = lambda src, *a: call("POST", S + "/execute/sync", {"script": src, "args": list(a)})
shot = lambda n: open(f"{OUT}/{n}.png", "wb").write(base64.b64decode(call("GET", S + "/screenshot")))
toasts = lambda: js("return [...document.querySelectorAll('.toast, [role=alert], [role=status]')].map(t => t.innerText.replace(/\\s+/g, ' '))")


def click(text, scope="button, a, li"):
    if not js("""const el = [...document.querySelectorAll(arguments[1])]
                   .find(e => e.textContent.trim().includes(arguments[0]) && !e.disabled);
                 if (!el) return false; el.click(); return true;""", text, scope):
        raise RuntimeError(f"no enabled element with {text!r}")
    time.sleep(1)


log = {}
try:
    time.sleep(6)
    click("Keymapping")
    t0 = time.time()
    click("Install / Update Phantom", "button")
    end = time.time() + 1500
    while time.time() < end:
        done = js("""const b = [...document.querySelectorAll('.terminal-modal button')].map(b => b.textContent).join(' ');
                     return /Done|Close/.test(b) && !document.querySelector('.btn-cancel-top')""")
        if done:
            break
        time.sleep(5)
    log["install_seconds"] = round(time.time() - t0)
    log["install_phase"] = js("return (document.querySelector('.terminal-modal') || {}).innerText?.split('\\n').slice(0, 4)")
    log["install_tail"] = js("return [...document.querySelectorAll('.terminal-modal pre, .terminal-console-body *')].map(e => e.textContent).join('\\n').split('\\n').filter(Boolean).slice(-6)")
    shot("30-phantom-installed")
    click("Done", ".terminal-modal button") if js("return /Done/.test(document.body.innerText)") else click("Close", ".terminal-modal button")
    log["installed_files"] = subprocess.run("ls -la ~/.local/bin/phantom ~/.local/bin/phantom-gui; phantom-gui --version 2>&1 | head -1",
                                            shell=True, capture_output=True, text=True).stdout
    js("document.querySelector('[title=\"Launch Phantom Keymapper\"]').click()")
    time.sleep(6)
    log["launch_toasts"] = toasts()
    log["phantom_process"] = subprocess.run(["pgrep", "-a", "phantom-gui"], capture_output=True, text=True).stdout
    subprocess.run("import -window root ~/dnd/31-phantom-launched.png", shell=True)
finally:
    json.dump(log, open(f"{OUT}/phantom-real.json", "w"), indent=2)
    print(json.dumps(log, indent=2))
    call("DELETE", S)
