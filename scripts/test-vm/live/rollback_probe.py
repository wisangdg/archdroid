#!/usr/bin/env python3
"""After installing a package version with pacman -U (upgrade or downgrade):
report the version the app shows, the add-on badges in Install Tools (markers
written by another version must still read correctly), then Start Engine, the
app count, and Stop Engine. Usage: rollback_probe.py /usr/bin/archdroid OUT TAG"""
import base64, json, sys, time, urllib.request

WD = "http://127.0.0.1:4444"
BIN, OUT, TAG = sys.argv[1], sys.argv[2], sys.argv[3]


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


def click(label):
    if not js("""const el = [...document.querySelectorAll('button')]
                   .find(e => e.textContent.trim().includes(arguments[0]) && !e.disabled);
                 if (!el) return false; el.click(); return true;""", label):
        raise RuntimeError(f"no enabled button with {label!r}")
    time.sleep(1.5)


def until(pattern, timeout):
    end = time.time() + timeout
    while time.time() < end:
        if js("return new RegExp(arguments[0]).test(document.body.innerText)", pattern):
            return True
        time.sleep(1)
    return False


log = {"tag": TAG}
try:
    time.sleep(8)
    log["version_shown"] = js("return document.body.innerText.match(/ArchDroid v[0-9.]+/)?.[0]")
    click("Install Tools")
    time.sleep(2)
    body = text()
    log["badges"] = {
        name: (body.split(name, 1)[1][:80].split("\n")[1:3] if name in body else None)
        for name in ("Google Play Services (GApps)", "ARM Translation Layer (libhoudini)")
    }
    shot(f"95-{TAG}-tools")
    click("Start Engine")
    log["engine_running"] = until("Engine Running", 180)
    until("[1-9]\\d* applications? ready", 120)
    log["apps"] = js("return document.body.innerText.match(/\\d+ applications? ready to launch/)?.[0]")
    shot(f"96-{TAG}-running")
    click("Stop Engine")
    log["engine_stopped"] = until("Engine Stopped", 120)
finally:
    json.dump(log, open(f"{OUT}/rollback-{TAG}.json", "w"), indent=2)
    print(json.dumps(log, indent=2))
    call("DELETE", S)
