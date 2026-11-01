#!/usr/bin/env python3
"""Fresh host without Waydroid: Initialize from the setup screen (the app
installs waydroid with the distro package manager, then runs waydroid init),
then Start Engine. Records the package plan, the result, and the app count."""
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
text = lambda: js("return document.body.innerText")


def click(label, scope="button"):
    if not js("""const el = [...document.querySelectorAll(arguments[1])]
                   .find(e => e.textContent.trim().includes(arguments[0]) && !e.disabled);
                 if (!el) return false; el.click(); return true;""", label, scope):
        raise RuntimeError(f"no enabled {scope} with {label!r}")
    time.sleep(1.5)


def until(pattern, timeout):
    end = time.time() + timeout
    while time.time() < end:
        if js("return new RegExp(arguments[0]).test(document.body.innerText)", pattern):
            return True
        time.sleep(3)
    return False


log = {}
try:
    time.sleep(8)
    click("Initialize Android Emulator")
    log["confirm_modal"] = js("return (document.querySelector('[role=dialog]') || {}).innerText?.replace(/\\s+/g, ' ')")
    shot("60-init-confirm")
    click("Initialize & Authorize Root")
    t0 = time.time()
    finished = until("All Stages Completed|Finished successfully|Operation Failed|EXECUTION FAILED|Operation cancelled", 1800)
    log["init_seconds"] = round(time.time() - t0)
    log["init_result"] = js("return (document.querySelector('.terminal-modal') || {}).innerText?.split('\\n').slice(0, 3)")
    log["init_tail"] = js("return [...document.querySelectorAll('.terminal-console-body *')].map(e => e.textContent).filter(Boolean).slice(-8)")
    shot("61-init-done")
    if js("return /Done|Close/.test([...document.querySelectorAll('.terminal-modal button')].map(b => b.textContent).join(' '))"):
        js("[...document.querySelectorAll('.terminal-modal button')].find(b => /Done|Close/.test(b.textContent)).click()")
    time.sleep(5)
    log["status_after_init"] = subprocess.run(["waydroid", "status"], capture_output=True, text=True).stdout.splitlines()[:2]
    click("Start Engine")
    log["engine_running"] = until("Engine Running", 240)
    time.sleep(20)
    log["subtitle"] = js("return document.body.innerText.match(/\\d+ applications? ready to launch/)?.[0]")
    shot("62-engine-running")
finally:
    json.dump(log, open(f"{OUT}/tier2-init.json", "w"), indent=2)
    print(json.dumps(log, indent=2))
    call("DELETE", S)
