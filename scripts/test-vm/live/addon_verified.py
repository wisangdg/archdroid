#!/usr/bin/env python3
"""Install GApps and then ARM Translation from Install Tools and record the
verified-download step: the stepper stage, the "verified (sha256 ...)" log
line, the final result, and the archives left in the private cache (none:
the work dir is removed after the root script)."""
import base64, json, sys, time, urllib.request

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


def click(label):
    if not js("""const el = [...document.querySelectorAll('button')]
                   .find(e => e.textContent.trim().includes(arguments[0]) && !e.disabled);
                 if (!el) return false; el.click(); return true;""", label):
        raise RuntimeError(f"no enabled button with {label!r}")
    time.sleep(1.5)


def terminal_text():
    return js("return (document.querySelector('.terminal-modal') || {}).innerText || ''")


def run_addon(button, done_pattern, name, log):
    click("Install Tools")
    click(button)
    click("Proceed & Authorize Root")
    t0 = time.time()
    seen_download_stage = False
    while time.time() - t0 < 1800:
        text = terminal_text()
        if "Verified Download" in text and js(
                "return !!document.querySelector('.stepper-node-active')?.innerText.includes('Verified Download')"):
            seen_download_stage = True
        if done_pattern in text or "[ERROR]" in text or "Operation Failed" in text:
            break
        time.sleep(2)
    text = terminal_text()
    log[name] = {
        "seconds": round(time.time() - t0),
        "download_stage_active_seen": seen_download_stage,
        "verified_lines": [l for l in text.splitlines() if "verified (sha256" in l or "SHA-256" in l],
        "success": done_pattern in text,
        "errors": [l for l in text.splitlines() if "[ERROR]" in l][:5],
    }
    shot(f"90-{name}")
    js("[...document.querySelectorAll('.terminal-modal button')].find(b => /Done|Close/.test(b.textContent))?.click()")
    time.sleep(3)


log = {}
try:
    time.sleep(8)
    run_addon("Install GApps", "GApps installation complete", "gapps", log)
    run_addon("Install ARM Support", "ARM Translation installation complete", "houdini", log)
    click("Install Tools")
    log["addon_badges"] = js("return [...document.querySelectorAll('.tool-card, [class*=card]')].map(c => c.innerText.split('\\n').slice(0, 2).join(' | ')).filter(t => /GApps|houdini|Translation/i.test(t)).slice(0, 4)")
    shot("91-install-tools-after")
finally:
    json.dump(log, open(f"{OUT}/addon-verified.json", "w"), indent=2)
    print(json.dumps(log, indent=2))
    call("DELETE", S)
