#!/usr/bin/env python3
"""Launch Keymapper against stub phantom-gui binaries: one that dies at once,
one that keeps running. Checks the app's reporting, not Phantom itself."""
import json, os, sys, time, urllib.request
WD = "http://127.0.0.1:4444"; BIN = sys.argv[1]
def call(m, p, b=None):
    r = urllib.request.Request(WD + p, data=json.dumps(b).encode() if b is not None else None, method=m, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(r, timeout=120) as x: return json.load(x)["value"]
stub = os.path.expanduser("~/.local/bin/phantom-gui"); os.makedirs(os.path.dirname(stub), exist_ok=True)
out = {}
for name, body in [("dies", "#!/bin/sh\necho 'cannot open display' >&2\nexit 1\n"), ("runs", "#!/bin/sh\nexec sleep 30\n")]:
    open(stub, "w").write(body); os.chmod(stub, 0o755)
    sid = call("POST", "/session", {"capabilities": {"alwaysMatch": {"webkitgtk:browserOptions": {"binary": BIN, "args": []}}}})["sessionId"]
    js = lambda s: call("POST", f"/session/{sid}/execute/sync", {"script": s, "args": []})
    time.sleep(5)
    js("[...document.querySelectorAll('button, a, li')].find(e => e.textContent.trim() === 'Keymapping').click()"); time.sleep(1)
    js("document.querySelector('[title=\"Launch Phantom Keymapper\"]').click()"); time.sleep(4)
    out[name] = js("return [...document.querySelectorAll('.toast, [role=alert], [role=status]')].map(t => t.innerText.replace(/\\s+/g, ' '))")
    call("DELETE", f"/session/{sid}")
os.remove(stub)
print(json.dumps(out, indent=2))
