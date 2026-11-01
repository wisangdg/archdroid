#!/usr/bin/env python3
"""After Waydroid is initialized: Start Engine from the header, wait for the
app list, drop an APK (dnd.sh, XDND), then Stop Engine."""
import base64, json, os, subprocess, sys, time, urllib.request

WD = "http://127.0.0.1:4444"
BIN, OUT, APK = sys.argv[1], sys.argv[2], sys.argv[3]
HERE = os.path.dirname(os.path.abspath(__file__))


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
subtitle = lambda: js("return document.body.innerText.match(/\\d+ applications? ready to launch/)?.[0]")


def until(pattern, timeout):
    end = time.time() + timeout
    while time.time() < end:
        if js("return new RegExp(arguments[0]).test(document.body.innerText)", pattern):
            return True
        time.sleep(1)
    return False


def click(label):
    js("[...document.querySelectorAll('button')].find(b => b.textContent.includes(arguments[0]) && !b.disabled).click()", label)


log = {}
try:
    time.sleep(8)
    click("Start Engine")
    t0 = time.time()
    log["engine_running"] = until("Engine Running", 180)
    log["start_seconds"] = round(time.time() - t0)
    until("[1-9]\\d* applications? ready", 120)
    log["apps_after_start"] = subtitle()
    shot("70-engine-running")
    subprocess.run([os.path.join(HERE, "dnd.sh"), APK, "71-apk-drop"], capture_output=True)
    log["drop_toast"] = until("APK Installed|Install Failed|Install Error", 120) and js(
        "return document.body.innerText.match(/APK Installed|Install Failed[^\\n]*|Install Error[^\\n]*/)?.[0]")
    time.sleep(3)
    log["apps_after_drop"] = subtitle()
    log["user_apps"] = js("return document.body.innerText.match(/User Apps & Games\\s*\\d+/)?.[0]")
    shot("72-after-drop")
    click("Stop Engine")
    log["engine_stopped"] = until("Engine Stopped", 120)
    log["status_after_stop"] = subprocess.run(["waydroid", "status"], capture_output=True, text=True).stdout.splitlines()[:2]
finally:
    json.dump(log, open(f"{OUT}/tier2-engine.json", "w"), indent=2)
    print(json.dumps(log, indent=2))
    call("DELETE", S)
