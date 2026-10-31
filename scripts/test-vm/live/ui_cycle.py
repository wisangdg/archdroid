#!/usr/bin/env python3
"""Live check of the unreleased terminal fixes: stepper after Cancel (Houdini)
and the wipe title after completion."""
import base64, glob, json, sys, time, urllib.request

WD = "http://127.0.0.1:4444"
BIN, OUT = sys.argv[1], sys.argv[2]


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(WD + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=180) as r:
            return json.load(r)["value"]
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{method} {path}: {e.read().decode()}")


sid = call("POST", "/session", {"capabilities": {"alwaysMatch": {
    "webkitgtk:browserOptions": {"binary": BIN, "args": []}}}})["sessionId"]
S = f"/session/{sid}"
js = lambda src, *a: call("POST", S + "/execute/sync", {"script": src, "args": list(a)})


def shot(name):
    open(f"{OUT}/{name}.png", "wb").write(base64.b64decode(call("GET", S + "/screenshot")))


def click(text, scope="button"):
    ok = js("""const el = [...document.querySelectorAll(arguments[1])]
                 .find(e => e.textContent.trim().includes(arguments[0]) && !e.disabled);
               if (!el) return false; el.click(); return true;""", text, scope)
    if not ok:
        raise RuntimeError(f"no enabled {scope} with {text!r}")
    time.sleep(1)


def until(cond, timeout):
    end = time.time() + timeout
    while time.time() < end:
        if js("return !!(" + cond + ")"):
            return True
        time.sleep(0.5)
    return False


STEPPER = """return {
  title: (document.querySelector('.terminal-modal h2, .terminal-modal h3') || {}).textContent,
  phase: [...document.querySelectorAll('.terminal-modal *')].map(e => e.childElementCount ? '' : e.textContent)
           .find(t => /Operation Cancelled|Completed|Failed|in Progress/i.test(t || '')),
  nodes: [...document.querySelectorAll('.stepper-node')].map(n =>
           n.className.replace('stepper-node ', '') + ':' + n.textContent.trim())}"""

log = {}
try:
    time.sleep(6)
    click("Install Tools", "button, a, li")
    click("Install ARM")
    click("Proceed & Authorize Root")
    # Cancel once the run has got past the first stage.
    reached = until("document.querySelector('.stepper-node-completed')", 240)
    log["houdini_reached_stage2"] = reached
    log["houdini_before_cancel"] = js(STEPPER)
    js("document.querySelector('.btn-cancel-top').click()")
    until("/Operation Cancelled/.test(document.body.innerText)", 30)
    time.sleep(1)
    log["houdini_after_cancel"] = js(STEPPER)
    time.sleep(1)
    log["workdirs_left"] = glob.glob("/var/tmp/archdroid.*")
    shot("20-houdini-cancelled")
    click("Close", ".terminal-modal button")

    click("Settings", "button, a, li")
    click("Factory Reset (Wipe All Data)")
    click("Wipe Everything")
    log["wipe_in_progress"] = js(STEPPER)
    until("/Done/.test([...document.querySelectorAll('.terminal-modal button')].map(b => b.textContent).join(' '))", 300)
    time.sleep(1)
    log["wipe_finished"] = js(STEPPER)
    shot("21-wipe-finished")
finally:
    json.dump(log, open(f"{OUT}/ui-cycle.json", "w"), indent=2)
    print(json.dumps(log, indent=2))
    call("DELETE", S)
