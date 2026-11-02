#!/usr/bin/env python3
"""What the application log records for two failures, i.e. what a Diagnostic
Bundle would show: Install / Update Phantom rejected by its preflight (no Rust
or JDK on the host), and Install GApps while GitHub is unreachable (block it in
/etc/hosts and remove ~/.cache/archdroid/gapps_installed first). Reports the
[STEP]/[ERROR] lines, the "last output" block and the command error lines.
Usage: log_failure.py BIN OUT"""
import glob, json, os, sys, time, urllib.request

WD = "http://127.0.0.1:4444"
BIN, OUT = sys.argv[1], sys.argv[2]
LOG = glob.glob(os.path.expanduser("~/.local/share/io.github.wisangdg.archdroid/logs/*.log"))[0]


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(WD + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=180) as r:
        return json.load(r)["value"]


start = os.path.getsize(LOG)
sid = call("POST", "/session", {"capabilities": {"alwaysMatch": {
    "webkitgtk:browserOptions": {"binary": BIN, "args": []}}}})["sessionId"]
S = f"/session/{sid}"
js = lambda src, *a: call("POST", S + "/execute/sync", {"script": src, "args": list(a)})


def click(label):
    if not js("""const el = [...document.querySelectorAll('button')]
                   .find(e => e.textContent.trim().includes(arguments[0]) && !e.disabled);
                 if (!el) return false; el.click(); return true;""", label):
        raise RuntimeError(f"no enabled button with {label!r}")
    time.sleep(1.5)


log = {}
try:
    time.sleep(8)
    click("Keymapping")
    click("Install / Update Phantom")
    time.sleep(3)
    click("Install Tools")
    click("Install GApps")
    click("Proceed & Authorize Root")
    end = time.time() + 300
    t = ""
    while time.time() < end:
        t = js("return (document.querySelector('.terminal-modal') || {}).innerText || ''")
        if "Operation Failed" in t or "EXECUTION FAILED" in t or "[SUCCESS]" in t:
            break
        time.sleep(2)
    log["terminal_errors"] = [l for l in t.splitlines() if "[ERROR]" in l][:4]
    time.sleep(2)
    with open(LOG) as f:
        f.seek(start)
        new = f.read()
    log["log_task_lines"] = [l[:200] for l in new.splitlines()
                             if "[STEP]" in l or "[ERROR]" in l or "last output" in l or "failed:" in l or "command error" in l]
    tail_at = new.find("last output:")
    log["log_last_output_excerpt"] = new[tail_at:tail_at + 600].splitlines()[:8] if tail_at >= 0 else None
    log["status_poll_lines_logged"] = sum('["status"] ok' in l for l in new.splitlines())
finally:
    json.dump(log, open(f"{OUT}/log-failure.json", "w"), indent=2)
    print(json.dumps(log, indent=2))
    call("DELETE", S)
