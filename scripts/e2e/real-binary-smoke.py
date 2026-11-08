#!/usr/bin/env python3
"""Smoke test of the real release binary: WebKitGTK, the Rust backend, CSP.

`npm run test:e2e` drives the built frontend in Chromium with a fake IPC
layer. This drives the actual `archdroid` binary through WebKitWebDriver, so
it catches what that cannot: a binary that does not start, IPC that does not
answer, and a production CSP that is missing or trips during use. Same
approach as scripts/test-vm/live/csp_probe.py, but it fails instead of only
reporting.

Usage (needs a display, e.g. xvfb-run, and the driver started with
TAURI_WEBVIEW_AUTOMATION=true):

    TAURI_WEBVIEW_AUTOMATION=true WebKitWebDriver --port=4444 &
    scripts/e2e/real-binary-smoke.py src-tauri/target/release/archdroid

Without Waydroid (CI) the app shows its setup screen; the host checks on it
come from the backend, which proves IPC works end to end.
"""
import json
import sys
import time
import urllib.error
import urllib.request

WD = "http://127.0.0.1:4444"


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(WD + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            return json.load(r)["value"]
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{method} {path}: {e.read().decode()}") from None


def wait_driver(timeout=30):
    end = time.time() + timeout
    while time.time() < end:
        try:
            urllib.request.urlopen(WD + "/status", timeout=2)
            return
        except OSError:
            time.sleep(0.5)
    raise RuntimeError("WebKitWebDriver did not start")


def main(binary):
    failures = []

    def expect(ok, what):
        print(("  ok    " if ok else "  FAIL  ") + what)
        if not ok:
            failures.append(what)

    wait_driver()
    sid = call("POST", "/session", {"capabilities": {"alwaysMatch": {
        "webkitgtk:browserOptions": {"binary": binary, "args": []}}}})["sessionId"]
    s = f"/session/{sid}"

    def js(src, *args):
        return call("POST", s + "/execute/sync", {"script": src, "args": list(args)})

    def ajs(src):
        return call("POST", s + "/execute/async", {"script": src, "args": []})

    def wait_for(cond, timeout=30):
        end = time.time() + timeout
        while time.time() < end:
            if js("return !!(" + cond + ")"):
                return True
            time.sleep(0.5)
        return False

    try:
        expect(wait_for("document.querySelector('#root')?.children.length"),
               "the window renders the React root")
        js("""window.__cspv = [];
              document.addEventListener('securitypolicyviolation', e =>
                window.__cspv.push({dir: e.violatedDirective, uri: e.blockedURI}));""")

        # The status poll and the host checks are IPC round trips to Rust.
        expect(wait_for("!/Reading Waydroid status/.test(document.body.innerText)", 30),
               "waydroid_status answered over IPC")
        setup = js("return /Initialize Android Emulator/.test(document.body.innerText)")
        if setup:
            expect(wait_for("/Wayland|Binder|KVM/.test(document.body.innerText)", 30),
                   "check_host_diagnostics answered over IPC (setup screen)")
        else:
            for tab in ["Keymapping", "Install Tools", "Settings", "My Apps"]:
                clicked = js("""const t = arguments[0];
                  const b = [...document.querySelectorAll('button')]
                    .find(e => e.textContent.trim() === t);
                  if (!b) return false; b.click(); return true;""", tab)
                expect(clicked, f"tab {tab} opens")
                time.sleep(1)

        resources = js("return performance.getEntriesByType('resource').map(e => e.name)")
        remote = [r for r in resources if r.startswith(("http://", "https://"))
                  and "localhost" not in r]
        expect(not remote, f"no remote resources loaded ({remote})")
        expect(js("return window.__cspv") == [], "normal use trips no CSP violation")

        # The production policy must block each of these.
        evaluated = js("try { eval('1+1'); return 'ALLOWED' } catch (e) { return 'blocked' }")
        expect(evaluated == "blocked", "eval is blocked")
        inline = ajs("""const done = arguments[arguments.length - 1];
            window.__inl = 'not run';
            const s = document.createElement('script'); s.textContent = "window.__inl = 'RAN'";
            document.head.appendChild(s); setTimeout(() => done(window.__inl), 300);""")
        expect(inline == "not run", "inline script is blocked")
        fetched = ajs("""const done = arguments[arguments.length - 1];
            fetch('https://example.com/').then(() => done('ALLOWED'), () => done('blocked'));""")
        expect(fetched == "blocked", "remote fetch is blocked")
        script = ajs("""const done = arguments[arguments.length - 1];
            const s = document.createElement('script'); s.src = 'https://example.com/x.js';
            s.onload = () => done('ALLOWED'); s.onerror = () => done('blocked');
            document.head.appendChild(s);""")
        expect(script == "blocked", "remote script is blocked")
    finally:
        call("DELETE", s)

    if failures:
        print(f"{len(failures)} check(s) failed", file=sys.stderr)
        return 1
    print("real binary smoke passed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
