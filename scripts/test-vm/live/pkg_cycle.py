#!/usr/bin/env python3
"""Uninstall (UI) and reinstall (native drag-and-drop) F-Droid while the
Waydroid container starts frozen; record the app count and card each time."""
import base64, json, os, subprocess, sys, time, urllib.request

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


def state():
    return js("""return {
      subtitle: (document.querySelector('header p, .header-subtitle') || {}).textContent || document.body.innerText.match(/\\d+ applications? ready to launch/)?.[0],
      fdroid: !!document.querySelector('[aria-label="Uninstall F-Droid"]'),
      toasts: [...document.querySelectorAll('.toast, [role=alert], [role=status]')].map(t => t.innerText.replace(/\\s+/g, ' ')).slice(-3)}""")


def container():
    return subprocess.run(["waydroid", "status"], capture_output=True, text=True).stdout.split("\n")[1]


def wait_toast(pattern, timeout=60):
    end = time.time() + timeout
    while time.time() < end:
        if js("return new RegExp(arguments[0]).test(document.body.innerText)", pattern):
            return round(timeout - (end - time.time()), 1)
        time.sleep(0.25)
    return None


log = {}
try:
    time.sleep(8)
    log["start"] = state()
    subprocess.run(["sudo", "waydroid", "container", "freeze"], check=True)
    time.sleep(1)
    log["before_uninstall_container"] = container()

    js("document.querySelector('[aria-label=\"Uninstall F-Droid\"]').click()")
    time.sleep(1)
    js("""[...document.querySelectorAll('[role=dialog] button, .modal button')]
          .find(b => b.textContent.trim() === 'Uninstall App').click()""")
    log["uninstall_seconds"] = wait_toast("App Removed|Uninstall Failed")
    time.sleep(0.5)
    log["after_uninstall"] = state()
    log["after_uninstall_container"] = container()
    shot("10-after-uninstall")

    subprocess.run(["sudo", "waydroid", "container", "freeze"], capture_output=True)
    time.sleep(1)
    log["before_install_container"] = container()
    subprocess.run([os.path.join(os.path.dirname(os.path.abspath(__file__)), "dnd.sh"), "/home/tester/test-data/F-Droid.apk", "11-drop"], capture_output=True)
    log["install_seconds"] = wait_toast("APK Installed|Install Failed|Install Error")
    time.sleep(0.5)
    log["after_install"] = state()
    log["after_install_container"] = container()
    shot("12-after-install")
finally:
    json.dump(log, open(f"{OUT}/pkg-cycle.json", "w"), indent=2)
    print(json.dumps(log, indent=2))
    call("DELETE", S)
