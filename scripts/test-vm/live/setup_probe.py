#!/usr/bin/env python3
"""Tier 2 smoke on a host without Waydroid: the setup screen renders, CSP is
enforced, host diagnostics run, and the Initialize confirmation shows the
distro's package plan. Never confirms the modal (no root action)."""
import base64, json, sys, time, urllib.request

WD = "http://127.0.0.1:4444"
BIN, OUT = sys.argv[1], sys.argv[2]


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(WD + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.load(r)["value"]


sid = call("POST", "/session", {"capabilities": {"alwaysMatch": {
    "webkitgtk:browserOptions": {"binary": BIN, "args": []}}}})["sessionId"]
S = f"/session/{sid}"
js = lambda src, *a: call("POST", S + "/execute/sync", {"script": src, "args": list(a)})
ajs = lambda src: call("POST", S + "/execute/async", {"script": src, "args": []})
shot = lambda n: open(f"{OUT}/{n}.png", "wb").write(base64.b64decode(call("GET", S + "/screenshot")))

log = {}
try:
    time.sleep(8)
    js("""window.__cspv = []; document.addEventListener('securitypolicyviolation',
          e => window.__cspv.push(e.violatedDirective + ' ' + e.blockedURI));""")
    log["url"] = js("return location.href")
    log["screen_text"] = js("return document.body.innerText.split('\\n').filter(Boolean).slice(0, 12)")
    shot("50-setup")
    js("""const b = [...document.querySelectorAll('button')].find(b => /Re-check/.test(b.textContent)); if (b) b.click();""")
    time.sleep(6)
    log["diagnostics"] = js("""return [...document.querySelectorAll('[class*=diag] li, [class*=check-item], [class*=preflight] li')]
                               .map(e => e.innerText.replace(/\\s+/g, ' ')).slice(0, 20)""")
    js("""const b = [...document.querySelectorAll('button')].find(b => /Initialize|Install Waydroid/.test(b.textContent) && !b.disabled);
          window.__initClicked = !!b; if (b) b.click();""")
    time.sleep(4)
    log["init_clicked"] = js("return window.__initClicked")
    log["confirm_modal"] = js("return (document.querySelector('[role=dialog]') || {}).innerText?.replace(/\\s+/g, ' ')")
    shot("51-init-confirm")
    log["violations_during_use"] = js("return window.__cspv")
    js("window.__cspv = []")
    log["eval"] = js("try { eval('1'); return 'ALLOWED' } catch (e) { return 'blocked: ' + e.name }")
    log["remote_fetch"] = ajs("""const d = arguments[arguments.length - 1];
        fetch('https://example.com/').then(() => d('ALLOWED'), e => d('blocked: ' + e.name));""")
    time.sleep(1)
    log["violations_from_probes"] = js("return window.__cspv")
finally:
    json.dump(log, open(f"{OUT}/setup-probe.json", "w"), indent=2)
    print(json.dumps(log, indent=2))
    call("DELETE", S)
