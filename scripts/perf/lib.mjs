// Shared harness for the UI probes: serves dist/ with a mocked Tauri bridge and
// drives headless Chromium through chromedriver (WebDriver + CDP passthrough).
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import { spawn } from "node:child_process";

const HERE = path.dirname(new URL(import.meta.url).pathname);
export const REPO = path.resolve(HERE, "../..");
export const DIST = path.join(REPO, "dist");
/** Screenshots go outside the repo; set PERF_OUT to choose where. */
export const OUT = process.env.PERF_OUT || path.join(os.tmpdir(), "archdroid-perf-shots");

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".png": "image/png",
  ".woff2": "font/woff2",
};

/** Real launcher entries of this host, so the list looks like the real app. */
export function hostApps() {
  const dir = path.join(process.env.HOME, ".local/share/applications");
  const apps = [];
  const names = fs.existsSync(dir) ? fs.readdirSync(dir).sort() : [];
  for (const name of names) {
    const m = name.match(/^waydroid\.(.+)\.desktop$/);
    if (!m) continue;
    const text = fs.readFileSync(path.join(dir, name), "utf8");
    const field = (key) =>
      text.split("\n").find((l) => l.startsWith(key))?.slice(key.length);
    const iconPath = field("Icon=");
    let icon = null;
    try {
      const bytes = fs.readFileSync(iconPath);
      icon = `data:image/png;base64,${bytes.toString("base64")}`;
    } catch {
      icon = null;
    }
    apps.push({ id: m[1], name: field("Name=") || m[1], package: m[1], icon });
  }
  if (apps.length === 0) {
    for (const [name, pkg] of [["Calculator", "com.android.calculator2"], ["Files", "com.android.documentsui"], ["Settings", "com.android.settings"], ["Plants Vs Zombies 2", "com.ea.game.pvz2_row"]])
      apps.push({ id: pkg, name, package: pkg, icon: null });
  }
  return apps;
}

/** `count` apps, cycling the host's real icons. */
export function manyApps(count) {
  const base = hostApps();
  const out = [];
  for (let i = 0; i < count; i++) {
    const src = base[i % base.length];
    out.push(
      i < base.length
        ? src
        : {
            id: `com.example.game${i}`,
            name: `${src.name} ${i}`,
            package: `com.example.game${i}`,
            icon: src.icon,
          }
    );
  }
  return out;
}

const DIAGNOSTICS = {
  package_manager: "pacman",
  items: [
    ["session", "Display Session", "ok", "Wayland (Hyprland)", "Waydroid needs a Wayland compositor."],
    ["kvm", "CPU Virtualization", "ok", "x86_64, SSE4.2", "The Android image needs SSE4.2."],
    ["binder", "Kernel Binder IPC", "ok", "binder_linux loaded", "Android IPC is available."],
    ["gpu", "GPU Driver", "warning", "NVIDIA (proprietary) + Intel", "Waydroid renders on the Intel GPU; NVIDIA-only hosts need software rendering.", "Set ro.hardware.gralloc=default and ro.hardware.egl=swiftshader"],
    ["service", "Waydroid Container Service", "ok", "active", "waydroid-container.service is running."],
    ["waydroid", "Waydroid", "ok", "1.6.3", "Installed from the distribution repository."],
    ["waydroid_image", "Android Image", "ok", "LineageOS 20", "Initialized."],
    ["network", "Network", "ok", "Connected", "Needed to download images and add-ons."],
    ["disk", "Free Disk Space", "ok", "135 GB free", "At least 3 GB is recommended."],
    ["elevation", "Root Elevation", "ok", "pkexec", "Root is requested per action."],
    ["adb", "ADB", "ok", "/usr/bin/adb", "Used for split APKs, DPI and Force stop."],
    ["python", "Python", "ok", "3.14", "waydroid_script needs Python 3."],
    ["lzip", "lzip", "error", "Missing", "Needed to unpack the GApps archive.", "sudo pacman -S lzip"],
    ["squashfs", "squashfs-tools", "ok", "4.7", "Needed by add-on installers."],
    ["git", "git", "ok", "2.51", "Needed to install Phantom."],
    ["terminal", "Terminal Emulator", "info", "ghostty", "Phantom runs in a terminal."],
    ["uinput", "/dev/uinput", "ok", "writable", "Used by touch mode."],
  ].map(([key, title, status, value, detail, remediation]) => ({
    key,
    title,
    status,
    value,
    detail,
    remediation: remediation || null,
  })),
};

export function bridge(state) {
  return `
window.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
  transformCallback(callback, once) {
    const id = Math.floor(Math.random() * 1e9);
    window["_" + id] = (...args) => { if (once) delete window["_" + id]; return callback(...args); };
    return id;
  },
  unregisterCallback(id) { delete window["_" + id]; },
  invoke: async (cmd, args) => window.__MOCK_INVOKE__(cmd, args),
  convertFileSrc: (p) => p,
};
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
window.__registeredEvents = {};
window.__invocations = [];
window.__mockState = ${JSON.stringify(state)};
window.__DIAG = ${JSON.stringify(DIAGNOSTICS)};
const PROPS = {
  "persist.waydroid.multi_windows": "true",
  "persist.waydroid.invert_colors": "false",
  "persist.waydroid.width": "1920",
  "persist.waydroid.height": "1080",
};
window.__MOCK_INVOKE__ = async function (cmd, args) {
  window.__invocations.push({ cmd, t: performance.now() });
  const s = window.__mockState;
  if (cmd === "plugin:event|listen") {
    (window.__registeredEvents[args.event] ||= []).push(args.handler);
    return args.handler;
  }
  if (cmd === "plugin:event|unlisten") return null;
  if (cmd === "waydroid_status")
    return s.online
      ? { status: "success", code: null, data: "Session:\\tRUNNING\\nContainer:\\tRUNNING\\nVendor type:\\tMAINLINE\\nIP address:\\t192.168.240.112\\n" }
      : s.initialized === false
        ? { status: "error", code: "not_initialized", message: "Waydroid is not initialized" }
        : { status: "success", code: null, data: "Session:\\tSTOPPED\\nVendor type:\\tMAINLINE\\n" };
  if (cmd === "waydroid_boot_state") return { status: "success", data: s.booting ? "booting" : "booted" };
  if (cmd === "check_host_diagnostics") return window.__DIAG;
  if (cmd === "get_waydroid_apps") return JSON.parse(JSON.stringify(s.apps));
  if (cmd === "check_gapps_status") return { status: "installed" };
  if (cmd === "check_houdini_status") return { status: "not_installed" };
  if (cmd === "check_multitouch_status") return { status: "partial" };
  if (cmd === "waydroid_prop_get") return s.online ? { status: "success", data: PROPS[args.key] ?? "" } : { status: "error", message: "session stopped" };
  if (cmd === "waydroid_prop_set") { if (!s.online) return { status: "error", code: "command_failed", message: "WayDroid session is stopped" }; PROPS[args.key] = args.value; return { status: "success" }; }
  if (cmd === "waydroid_density_set") { window.__dpi = String(args.dpi); return { status: "success" }; }
  if (cmd === "waydroid_density_get") return s.online ? { status: "success", data: window.__dpi || "280" } : { status: "error", message: "session stopped" };
  if (cmd === "touch_mode_status") return { supported: true, uinput_access: true, mice: ["Logitech G502 HERO"], enabled: false, running: false, error: null };
  if (cmd === "app_log_location") return { status: "success", data: "~/.local/share/io.github.wisangdg.archdroid/logs/archdroid.log" };
  if (cmd === "preview_package_install") return { packageManager: "pacman", packages: ["lzip", "squashfs-tools"], command: "pacman -S --needed lzip squashfs-tools" };
  if (["install_gapps", "install_houdini", "install_multitouch", "waydroid_wipe", "waydroid_log", "waydroid_init", "install_phantom"].includes(cmd)) {
    setTimeout(() => window.__emitTauriEvent("cmd-started", { task_id: "task-probe" }), 5);
    return new Promise((resolve) => { window.__finishTask = (ok) => resolve(ok ? { status: "success" } : { status: "error", code: "command_failed", message: "failed" }); });
  }
  return { status: "success" };
};
window.__emitTauriEvent = function (eventName, payload) {
  for (const id of window.__registeredEvents[eventName] || []) {
    const fn = window["_" + id];
    if (typeof fn === "function") fn({ event: eventName, payload });
  }
};
`;
}

export function startServer(state) {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      let reqPath = req.url.split("?")[0];
      if (reqPath === "/") reqPath = "/index.html";
      const filePath = path.join(DIST, reqPath);
      if (reqPath === "/index.html") {
        const html = fs
          .readFileSync(filePath, "utf8")
          .replace("<head>", `<head><script>${bridge(server.state)}</script>`);
        res.writeHead(200, { "Content-Type": MIME[".html"] });
        res.end(html);
        return;
      }
      if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
        res.writeHead(200, {
          "Content-Type": MIME[path.extname(filePath)] || "application/octet-stream",
        });
        fs.createReadStream(filePath).pipe(res);
      } else {
        res.writeHead(404);
        res.end();
      }
    });
    server.state = state;
    server.listen(0, "127.0.0.1", () => resolve(server));
  });
}

export async function startDriver() {
  const port = 9600 + Math.floor(Math.random() * 300);
  const proc = spawn("/usr/bin/chromedriver", [`--port=${port}`], {
    stdio: ["ignore", "ignore", "ignore"],
  });
  const base = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 80; i++) {
    try {
      const r = await fetch(`${base}/status`);
      if ((await r.json())?.value?.ready) break;
    } catch {
      /* not up yet */
    }
    await sleep(100);
  }
  const res = await fetch(`${base}/session`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      capabilities: {
        alwaysMatch: {
          browserName: "chrome",
          "goog:chromeOptions": {
            binary: "/usr/bin/chromium",
            args: [
              "--headless=new",
              "--no-sandbox",
              "--disable-gpu",
              "--disable-dev-shm-usage",
              "--window-size=1100,720",
              "--hide-scrollbars=false",
            ],
          },
        },
      },
    }),
  });
  const data = await res.json();
  const id = data.value.sessionId;
  const call = async (method, suffix, body) => {
    const r = await fetch(`${base}/session/${id}${suffix}`, {
      method,
      headers: { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const j = await r.json();
    if (j.value && j.value.error) throw new Error(`${suffix}: ${j.value.message}`);
    return j.value;
  };
  const d = {
    navigate: (url) => call("POST", "/url", { url }),
    exec: (script, args = []) => call("POST", "/execute/sync", { script, args }),
    execAsync: (script, args = []) => call("POST", "/execute/async", { script, args }),
    cdp: (cmd, params = {}) => call("POST", "/goog/cdp/execute", { cmd, params }),
    async viewport(width, height) {
      await d.cdp("Emulation.setDeviceMetricsOverride", {
        width,
        height,
        deviceScaleFactor: 1,
        mobile: false,
      });
    },
    async shot(name) {
      const b64 = await call("GET", "/screenshot");
      fs.mkdirSync(OUT, { recursive: true });
      const file = path.join(OUT, `${name}.png`);
      fs.writeFileSync(file, Buffer.from(b64, "base64"));
      return file;
    },
    async waitFor(script, timeout = 8000) {
      const start = Date.now();
      while (Date.now() - start < timeout) {
        try {
          if (await d.exec(script)) return true;
        } catch {
          /* retry */
        }
        await sleep(50);
      }
      throw new Error(`timeout: ${script}`);
    },
    async metrics() {
      const { metrics } = await d.cdp("Performance.getMetrics");
      return Object.fromEntries(metrics.map((m) => [m.name, m.value]));
    },
    async tab(label) {
      await d.exec(
        `[...document.querySelectorAll(".menu-item")].find(t => t.textContent.includes(arguments[0])).click();`,
        [label]
      );
      await sleep(250);
    },
    async close() {
      await call("DELETE", "");
      proc.kill();
    },
  };
  await call("POST", "/timeouts", { script: 120000 });
  await d.cdp("Performance.enable");
  return d;
}

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function diff(after, before, keys) {
  const out = {};
  for (const k of keys) out[k] = +(1000 * (after[k] - before[k])).toFixed(1);
  return out;
}
export const PERF_KEYS = ["TaskDuration", "ScriptDuration", "LayoutDuration", "RecalcStyleDuration"];
