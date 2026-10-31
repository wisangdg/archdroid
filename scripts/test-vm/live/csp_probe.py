#!/usr/bin/env python3
"""Drive the real ArchDroid release binary through WebKitWebDriver and check
that the production CSP is active and never trips during normal use."""
import base64, json, sys, time, urllib.request

WD = "http://127.0.0.1:4444"
BIN = sys.argv[1]
OUT = sys.argv[2]


def call(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(WD + path, data=data, method=method,
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            return json.load(r)["value"]
    except urllib.error.HTTPError as e:
        raise RuntimeError(f"{method} {path}: {e.read().decode()}")


sid = call("POST", "/session", {"capabilities": {"alwaysMatch": {
    "webkitgtk:browserOptions": {"binary": BIN, "args": []}}}})["sessionId"]
S = f"/session/{sid}"


def js(src, *args):
    return call("POST", S + "/execute/sync", {"script": src, "args": list(args)})


def ajs(src):
    return call("POST", S + "/execute/async", {"script": src, "args": []})


def shot(name):
    png = call("GET", S + "/screenshot")
    open(f"{OUT}/{name}.png", "wb").write(base64.b64decode(png))


def click_text(text, selector="button, a, [role=button], [role=tab], li"):
    ok = js("""const [t, sel] = arguments;
      const el = [...document.querySelectorAll(sel)].find(e => e.textContent.trim().includes(t));
      if (!el) return false; el.click(); return true;""", text, selector)
    if not ok:
        raise RuntimeError(f"no clickable element with text {text!r}")
    time.sleep(1.5)


def wait_for(cond, timeout=30):
    end = time.time() + timeout
    while time.time() < end:
        if js("return !!(" + cond + ")"):
            return True
        time.sleep(0.5)
    return False


report = {}
try:
    wait_for("document.querySelector('#root') && document.querySelector('#root').children.length")
    time.sleep(3)
    report["url"] = js("return location.href")
    report["csp_meta"] = js("""const m = document.querySelector('meta[http-equiv="Content-Security-Policy"]');
                              return m ? m.content : null""")
    # Listen for violations from here on; startup is covered by the resource check.
    js("""window.__cspv = [];
          document.addEventListener('securitypolicyviolation', e =>
            window.__cspv.push({dir: e.violatedDirective, uri: e.blockedURI, src: e.sourceFile}));""")
    report["resources"] = js("return performance.getEntriesByType('resource').map(e => e.name)")
    report["scripts"] = js("return [...document.scripts].map(s => s.src || '(inline ' + s.textContent.length + ' chars)')")
    report["stylesheets_ok"] = js("return [...document.styleSheets].map(s => { try { return [s.href, s.cssRules.length] } catch (e) { return [s.href, String(e)] } })")
    shot("01-startup")

    # Normal use: every tab, Host Diagnostics rescan, diagnostic bundle (IPC + long call).
    for tab in ["Keymapping", "Install Tools", "Settings", "My Apps"]:
        click_text(tab)
        shot("02-" + tab.replace(" ", "-").lower())
    click_text("Settings")
    for t in ["Re-check"]:
        try:
            click_text(t, "button")
            break
        except RuntimeError:
            pass
    click_text("Create Bundle", "button")
    wait_for("[...document.querySelectorAll('[role=status]')].some(e => /Saved to|Could not/.test(e.textContent))", 60)
    report["bundle_status"] = js("return [...document.querySelectorAll('[role=status]')].map(e => e.textContent)")
    shot("03-settings-bundle")
    report["violations_during_use"] = js("return window.__cspv")
    report["resources_after_use"] = js("return performance.getEntriesByType('resource').map(e => e.name)")

    # Negative probes: each must be blocked by the production policy.
    js("window.__cspv = []")
    try:
        report["eval"] = js("try { eval('1+1'); return 'ALLOWED' } catch (e) { return 'blocked: ' + e.name }")
    except RuntimeError as e:
        report["eval"] = "blocked (driver error): " + str(e)[:200]
    report["inline_script"] = ajs("""const done = arguments[arguments.length - 1];
        window.__inl = 'not run';
        const s = document.createElement('script'); s.textContent = "window.__inl = 'RAN'";
        document.head.appendChild(s); setTimeout(() => done(window.__inl), 300);""")
    report["remote_fetch"] = ajs("""const done = arguments[arguments.length - 1];
        fetch('https://example.com/').then(() => done('ALLOWED'), e => done('blocked: ' + e.name));""")
    report["remote_img"] = ajs("""const done = arguments[arguments.length - 1];
        const i = new Image(); i.onload = () => done('ALLOWED'); i.onerror = () => done('blocked');
        i.src = 'https://example.com/x.png';""")
    report["remote_script"] = ajs("""const done = arguments[arguments.length - 1];
        const s = document.createElement('script'); s.src = 'https://example.com/x.js';
        s.onload = () => done('ALLOWED'); s.onerror = () => done('blocked'); document.head.appendChild(s);""")
    time.sleep(1)
    report["violations_from_probes"] = js("return window.__cspv")
finally:
    json.dump(report, open(f"{OUT}/csp-report.json", "w"), indent=2)
    print(json.dumps(report, indent=2))
    call("DELETE", S)
