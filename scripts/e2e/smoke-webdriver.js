#!/usr/bin/env node
/**
 * End-to-End WebDriver Smoke Test for ArchDroid
 *
 * Runs headless Chromium via W3C WebDriver (ChromeDriver) against the built
 * frontend (`dist/`), testing critical UI workflows and matrices:
 *   1. Startup & Initialized app loading (online status badge, app grid, header)
 *   2. Tab navigation (My Apps, Keymapping, Install Tools, Settings)
 *   3. Setup wizard preflight blocking on uninitialized & failed diagnostics
 *   4. Drag-and-drop non-APK rejection toast
 *   5. Keymapper missing terminal emulator error handling (Matrix #18)
 *   6. Streaming task initiation, cancel button, and task-id routing
 *   7. Factory reset wipe data confirmation modal
 */

import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { spawn, execSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, "../..");
const DIST_DIR = path.resolve(REPO_ROOT, "dist");

// MIME types for static server
const MIME_TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "application/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".woff2": "font/woff2",
};

// Find ChromeDriver executable
function findExecutable(name) {
  try {
    return execSync(`which ${name}`, { encoding: "utf8" }).trim();
  } catch {
    return null;
  }
}

const CHROMEDRIVER_BIN =
  findExecutable("chromedriver") || "/usr/bin/chromedriver";
const CHROMIUM_BIN =
  findExecutable("chromium") ||
  findExecutable("google-chrome") ||
  "/usr/bin/chromium";

if (!fs.existsSync(CHROMEDRIVER_BIN)) {
  console.warn(
    `[E2E] ChromeDriver not found at ${CHROMEDRIVER_BIN}. Skipping E2E test.`
  );
  process.exit(0);
}

// Ensure dist/ exists
if (!fs.existsSync(path.join(DIST_DIR, "index.html"))) {
  console.log("[E2E] Building frontend before testing...");
  execSync("npm run build", { cwd: REPO_ROOT, stdio: "inherit" });
}

// Injected bridge script simulating Tauri IPC and window internals
const TAURI_MOCK_BRIDGE = `
window.__TAURI_INTERNALS__ = {
  metadata: {
    currentWindow: { label: "main" },
    currentWebview: { label: "main" }
  },
  transformCallback: function(callback, once) {
    const id = Math.floor(Math.random() * 10000000);
    window["_" + id] = function(...args) {
      if (once) delete window["_" + id];
      return callback(...args);
    };
    return id;
  },
  unregisterCallback: function(id) {
    delete window["_" + id];
  },
  invoke: async function(cmd, args, options) {
    return window.__MOCK_INVOKE__(cmd, args, options);
  },
  convertFileSrc: function(filePath) {
    return filePath;
  }
};

window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
  unregisterListener: function() {}
};

window.__registeredEvents = {};
window.__invocations = [];

window.__mockState = {
  waydroidStatus: {
    status: "success",
    code: null,
    data: "Session: RUNNING\\nContainer: RUNNING\\n"
  },
  apps: [
    { name: "Settings", package: "com.android.settings", icon: null },
    { name: "Gallery", package: "com.android.gallery3d", icon: null }
  ],
  diagnostics: {
    items: [
      { key: "binder", title: "Kernel Binder IPC (/dev/binder)", status: "ok", value: "Available" },
      { key: "wayland", title: "Wayland Compositor Session", status: "ok", value: "WAYLAND_DISPLAY=wayland-1" },
      { key: "network", title: "Network Connection", status: "ok", value: "Connected" }
    ],
    package_manager: "pacman"
  },
  gappsStatus: "not_installed",
  houdiniStatus: "installed",
  multitouchStatus: "installed",
  keymapperError: null
};

// Check query param for setup wizard simulation
const urlParams = new URLSearchParams(window.location.search);
if (urlParams.get("state") === "not_initialized") {
  window.__mockState.waydroidStatus = {
    status: "error",
    code: "not_initialized",
    message: "Waydroid is not initialized"
  };
  window.__mockState.diagnostics = {
    items: [
      { key: "binder", title: "Kernel Binder IPC (/dev/binder)", status: "ok", value: "Available" },
      { key: "network", title: "Network Connection", status: "error", value: "No Route", detail: "DNS unreachable", remediation: "Check network config." }
    ],
    package_manager: "pacman"
  };
}

window.__MOCK_INVOKE__ = async function(cmd, args) {
  window.__invocations.push({ cmd, args });

  if (cmd === "plugin:event|listen") {
    const ev = args.event;
    if (!window.__registeredEvents[ev]) window.__registeredEvents[ev] = [];
    window.__registeredEvents[ev].push(args.handler);
    return args.handler;
  }
  if (cmd === "plugin:event|unlisten") {
    return null;
  }

  if (cmd === "waydroid_status") {
    return window.__mockState.waydroidStatus;
  }
  if (cmd === "check_host_diagnostics") {
    return window.__mockState.diagnostics;
  }
  if (cmd === "get_waydroid_apps") {
    return window.__mockState.apps;
  }
  if (cmd === "check_gapps_status") {
    return { status: window.__mockState.gappsStatus };
  }
  if (cmd === "check_houdini_status") {
    return { status: window.__mockState.houdiniStatus };
  }
  if (cmd === "check_multitouch_status") {
    return { status: window.__mockState.multitouchStatus };
  }
  if (cmd === "waydroid_prop_get") {
    return { status: "success", data: "false" };
  }
  if (cmd === "waydroid_prop_set") {
    return { status: "success", data: "true" };
  }
  if (cmd === "preview_package_install") {
    return {
      packageManager: "pacman",
      packages: ["lxc", "curl"],
      command: "pacman -S --needed lxc curl"
    };
  }
  if (cmd === "run_phantom") {
    if (window.__mockState.keymapperError) {
      return window.__mockState.keymapperError;
    }
    return { status: "success" };
  }
  if (cmd === "install_gapps" || cmd === "install_houdini" || cmd === "install_multitouch" || cmd === "waydroid_wipe") {
    setTimeout(() => {
      window.__emitTauriEvent("cmd-started", { task_id: "task-e2e-live" });
    }, 10);
    return { status: "success" };
  }
  if (cmd === "cancel_task") {
    setTimeout(() => {
      window.__emitTauriEvent("cmd-finished", {
        task_id: "task-e2e-live",
        success: false,
        code: "cancelled",
        message: "Operation was cancelled by user."
      });
    }, 10);
    return { status: "success" };
  }
  return { status: "success" };
};

window.__emitTauriEvent = function(eventName, payload) {
  const handlers = window.__registeredEvents[eventName] || [];
  for (const handlerId of handlers) {
    const fn = window["_" + handlerId];
    if (typeof fn === "function") {
      fn({ event: eventName, payload });
    }
  }
};

window.__triggerDragDrop = function(paths) {
  window.__emitTauriEvent("tauri://drag-drop", {
    paths: paths,
    position: { x: 100, y: 100 }
  });
};
`;

// Start local HTTP server serving dist/
function startServer() {
  return new Promise((resolve) => {
    const server = http.createServer((req, res) => {
      let reqPath = req.url.split("?")[0];
      if (reqPath === "/") reqPath = "/index.html";

      const filePath = path.join(DIST_DIR, reqPath);
      const ext = path.extname(filePath);

      if (reqPath === "/index.html" && fs.existsSync(filePath)) {
        let html = fs.readFileSync(filePath, "utf8");
        // Inject bridge before any scripts
        html = html.replace(
          "<head>",
          `<head><script>${TAURI_MOCK_BRIDGE}</script>`
        );
        res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
        res.end(html);
        return;
      }

      if (fs.existsSync(filePath) && fs.statSync(filePath).isFile()) {
        const mime = MIME_TYPES[ext] || "application/octet-stream";
        res.writeHead(200, { "Content-Type": mime });
        fs.createReadStream(filePath).pipe(res);
      } else {
        res.writeHead(404);
        res.end("Not Found");
      }
    });

    server.listen(0, "127.0.0.1", () => {
      const port = server.address().port;
      resolve({ server, port });
    });
  });
}

// Start ChromeDriver
async function startChromeDriver(port) {
  const proc = spawn(
    CHROMEDRIVER_BIN,
    [`--port=${port}`, "--allowed-ips=127.0.0.1"],
    {
      stdio: ["ignore", "pipe", "pipe"],
    }
  );

  let stderrOutput = "";
  proc.stderr.on("data", (chunk) => {
    stderrOutput += chunk.toString();
  });

  const start = Date.now();
  while (Date.now() - start < 6000) {
    if (proc.exitCode !== null) {
      throw new Error(
        `ChromeDriver process exited prematurely with code ${proc.exitCode}: ${stderrOutput}`
      );
    }
    try {
      const res = await fetch(`http://127.0.0.1:${port}/status`);
      const data = await res.json();
      if (data?.value?.ready) {
        return proc;
      }
    } catch {
      // not yet listening, retry
    }
    await new Promise((r) => setTimeout(r, 100));
  }

  proc.kill();
  throw new Error(
    `Timed out waiting for ChromeDriver on port ${port}. Stderr: ${stderrOutput}`
  );
}

// Helper WebDriver client
class WebDriver {
  constructor(baseUrl) {
    this.baseUrl = baseUrl;
    this.sessionId = null;
  }

  async createSession() {
    const chromeOptions = {
      args: [
        "--headless=new",
        "--no-sandbox",
        "--disable-gpu",
        "--disable-dev-shm-usage",
        "--window-size=1280,800",
      ],
    };

    if (CHROMIUM_BIN && fs.existsSync(CHROMIUM_BIN)) {
      chromeOptions.binary = CHROMIUM_BIN;
    }

    const res = await fetch(`${this.baseUrl}/session`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        capabilities: {
          alwaysMatch: {
            browserName: "chrome",
            "goog:chromeOptions": chromeOptions,
          },
        },
      }),
    });
    const data = await res.json();
    if (!data.value || !data.value.sessionId) {
      const errMessage = data?.value?.message || JSON.stringify(data);
      if (
        errMessage.includes("cannot find Chrome binary") ||
        errMessage.includes("cannot find chrome binary") ||
        errMessage.includes("Chrome failed to start")
      ) {
        console.warn(
          `[E2E] Compatible Chromium browser not launchable on this runner: ${errMessage}. Skipping E2E test.`
        );
        return false;
      }
      throw new Error(`Failed to create WebDriver session: ${errMessage}`);
    }
    this.sessionId = data.value.sessionId;
    return true;
  }

  async navigate(url) {
    await fetch(`${this.baseUrl}/session/${this.sessionId}/url`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ url }),
    });
  }

  async execute(script, args = []) {
    const res = await fetch(
      `${this.baseUrl}/session/${this.sessionId}/execute/sync`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ script, args }),
      }
    );
    const data = await res.json();
    if (data.value && data.value.error) {
      throw new Error(`Execute error: ${data.value.message || data.value.error}`);
    }
    return data.value;
  }

  async waitFor(predicateScript, timeoutMs = 4000, intervalMs = 100) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      try {
        const ok = await this.execute(predicateScript);
        if (ok) return true;
      } catch {
        // continue polling
      }
      await new Promise((r) => setTimeout(r, intervalMs));
    }
    throw new Error(`Timeout waiting for condition: ${predicateScript}`);
  }

  async closeSession() {
    if (this.sessionId) {
      await fetch(`${this.baseUrl}/session/${this.sessionId}`, {
        method: "DELETE",
      }).catch(() => {});
      this.sessionId = null;
    }
  }
}

async function runSmokeTests() {
  console.log("[E2E] Starting WebDriver smoke tests...");
  const { server, port: webPort } = await startServer();
  const driverPort = 9520 + Math.floor(Math.random() * 200);

  let cdProc = null;
  let driver = null;

  try {
    cdProc = await startChromeDriver(driverPort);
    driver = new WebDriver(`http://127.0.0.1:${driverPort}`);
    const sessionOk = await driver.createSession();
    if (!sessionOk) {
      return;
    }

    const appUrl = `http://127.0.0.1:${webPort}`;
    console.log(`[E2E] Web server: ${appUrl}, ChromeDriver: port ${driverPort}`);

    // --- Scenario 1: Initial startup & online state ---
    console.log("[E2E] Scenario 1: Testing startup & online status...");
    await driver.navigate(appUrl);
    await driver.waitFor(
      'return document.querySelector(".brand-title")?.textContent === "ArchDroid";'
    );
    await driver.waitFor(
      'return document.querySelector(".status-badge")?.classList?.contains("status-online") || document.querySelector(".status-pill-mini")?.textContent?.includes("Online");'
    );
    await driver.waitFor(
      'return document.querySelectorAll(".app-card").length >= 2;'
    );
    console.log("  ✔ Startup, brand header, and app grid rendered successfully.");

    // --- Scenario 2: Tab Navigation ---
    console.log("[E2E] Scenario 2: Testing tab navigation...");

    // Click Keymapping tab
    await driver.execute(`
      const tabs = Array.from(document.querySelectorAll(".menu-item"));
      const keymapTab = tabs.find(t => t.textContent.includes("Keymapping"));
      if (keymapTab) keymapTab.click();
    `);
    await driver.waitFor(
      'return !!document.querySelector(".keymap-hero-content h2")?.textContent?.includes("Phantom Keymapper");'
    );
    console.log("  ✔ Navigated to Keymapping tab.");

    // Click Install Tools tab
    await driver.execute(`
      const tabs = Array.from(document.querySelectorAll(".menu-item"));
      const toolsTab = tabs.find(t => t.textContent.includes("Install Tools"));
      if (toolsTab) toolsTab.click();
    `);
    await driver.waitFor(
      'return !!document.querySelector(".tools-view-container") && ' +
      '!!document.querySelector(".tool-card h3")?.textContent?.includes("Google Play Services");'
    );
    console.log("  ✔ Navigated to Install Tools tab.");

    // Click Settings tab
    await driver.execute(`
      const tabs = Array.from(document.querySelectorAll(".menu-item"));
      const settingsTab = tabs.find(t => t.textContent.includes("Settings"));
      if (settingsTab) settingsTab.click();
    `);
    await driver.waitFor(
      'return !!document.querySelector(".danger-card-header h3")?.textContent?.includes("Danger Zone: Factory Reset");'
    );
    console.log("  ✔ Navigated to Settings tab.");

    // Return to My Apps
    await driver.execute(`
      const tabs = Array.from(document.querySelectorAll(".menu-item"));
      const appsTab = tabs.find(t => t.textContent.includes("My Apps"));
      if (appsTab) appsTab.click();
    `);
    await driver.waitFor(
      'return document.querySelectorAll(".app-card").length >= 2;'
    );
    console.log("  ✔ Returned to My Apps tab.");

    // --- Scenario 3: Drag & Drop APK Validation ---
    console.log("[E2E] Scenario 3: Testing drag & drop invalid file rejection...");
    await driver.execute(`
      window.__triggerDragDrop(["/home/user/malicious_document.pdf"]);
    `);
    await driver.waitFor(
      'return Array.from(document.querySelectorAll(".toast-title")).some(t => t.textContent === "Unsupported File");'
    );
    console.log("  ✔ Rejected non-APK file with 'Unsupported File' warning toast.");

    // --- Scenario 4: Matrix #18 - Keymapper missing terminal emulator ---
    console.log("[E2E] Scenario 4: Testing missing terminal emulator error toast (Matrix #18)...");
    await driver.execute(`
      window.__mockState.keymapperError = {
        status: "error",
        code: "not_found",
        message: "Keymapper Error: cannot find 'terminal emulator' on PATH (checked: ghostty, alacritty, kitty, foot, gnome-terminal, ptyxis, xfce4-terminal, konsole). No supported terminal emulator was found."
      };
      const tabs = Array.from(document.querySelectorAll(".menu-item"));
      const keymapTab = tabs.find(t => t.textContent.includes("Keymapping"));
      if (keymapTab) keymapTab.click();
    `);
    await driver.waitFor(
      'return !!document.querySelector(".keymap-actions button");'
    );
    await driver.execute(`
      const launchBtn = Array.from(document.querySelectorAll(".keymap-actions button"))
        .find(b => b.textContent.includes("Launch Keymapper"));
      if (launchBtn) launchBtn.click();
    `);
    await driver.waitFor(
      'return Array.from(document.querySelectorAll(".toast-title")).some(t => t.textContent === "Keymapper Error");'
    );
    console.log("  ✔ Missing terminal emulator error handled and displayed via error toast.");

    // --- Scenario 5: Streaming task cancel & task_id routing ---
    console.log("[E2E] Scenario 5: Testing streaming task cancel & task_id routing...");
    await driver.execute(`
      const tabs = Array.from(document.querySelectorAll(".menu-item"));
      const toolsTab = tabs.find(t => t.textContent.includes("Install Tools"));
      if (toolsTab) toolsTab.click();
    `);
    await driver.waitFor(
      'return !!document.querySelector(".tool-card-footer button");'
    );
    // Click Install GApps
    await driver.execute(`
      const gappsBtn = Array.from(document.querySelectorAll(".tool-card-footer button"))
        .find(b => b.textContent.includes("Install GApps"));
      if (gappsBtn) gappsBtn.click();
    `);
    // Wait for confirm modal
    await driver.waitFor(
      'return !!document.querySelector(".confirm-modal-title")?.textContent?.includes("Install Google Play Services");'
    );
    // Click Proceed & Authorize Root
    await driver.execute(`
      const confirmBtn = Array.from(document.querySelectorAll(".confirm-modal-actions button"))
        .find(b => b.textContent.includes("Proceed & Authorize Root"));
      if (confirmBtn) confirmBtn.click();
    `);
    // Terminal modal should be open and cancel button visible
    await driver.waitFor(
      'return !!document.querySelector(".terminal-modal");'
    );
    await driver.waitFor(
      'return !!document.querySelector(".btn-cancel-top");'
    );
    // Click Cancel
    await driver.execute(`
      const cancelBtn = document.querySelector(".btn-cancel-top");
      if (cancelBtn) cancelBtn.click();
    `);
    // Verify cancel text appears and cancel_task was invoked with active task_id
    await driver.waitFor(
      'return Array.from(document.querySelectorAll(".terminal-status-text"))' +
      '.some(b => b.textContent.includes("Operation cancelled"));'
    );
    const cancelInvocations = await driver.execute(`
      return window.__invocations.filter(i => i.cmd === "cancel_task");
    `);
    if (cancelInvocations.length === 0 || cancelInvocations[0].args?.taskId !== "task-e2e-live") {
      throw new Error(`cancel_task not invoked with expected task ID: ${JSON.stringify(cancelInvocations)}`);
    }
    console.log("  ✔ Streaming task initiated, cancel clicked, and cancel_task routed correctly with task ID.");

    // Close terminal modal
    await driver.execute(`
      const closeBtn = document.querySelector(".btn-icon-close") || Array.from(document.querySelectorAll("button")).find(b => b.textContent.includes("Close Terminal"));
      if (closeBtn) closeBtn.click();
    `);

    // --- Scenario 6: Factory Reset / Wipe Data Modal ---
    console.log("[E2E] Scenario 6: Testing wipe data confirmation modal...");
    await driver.execute(`
      const tabs = Array.from(document.querySelectorAll(".menu-item"));
      const settingsTab = tabs.find(t => t.textContent.includes("Settings"));
      if (settingsTab) settingsTab.click();
    `);
    await driver.waitFor(
      'return !!document.querySelector(".btn-wipe");'
    );
    await driver.execute(`
      document.querySelector(".btn-wipe")?.click();
    `);
    await driver.waitFor(
      'return !!document.querySelector(".confirm-modal-title")?.textContent?.includes("Factory Reset Waydroid?");'
    );
    // Click Cancel on confirm dialog
    await driver.execute(`
      const cancelBtn = Array.from(document.querySelectorAll(".confirm-modal-actions button"))
        .find(b => b.textContent.includes("Cancel"));
      if (cancelBtn) cancelBtn.click();
    `);
    await driver.waitFor(
      'return !document.querySelector(".confirm-modal-overlay");'
    );
    console.log("  ✔ Wipe data modal opened with danger warning and safely cancelled.");

    // --- Scenario 7: Setup Wizard Preflight Blocking ---
    console.log("[E2E] Scenario 7: Testing Setup Wizard preflight blocking...");
    await driver.navigate(`${appUrl}/?state=not_initialized`);
    await driver.waitFor(
      'return !!document.querySelector(".setup-wizard-container");'
    );
    await driver.waitFor(
      'return !!document.querySelector(".btn-primary[disabled]");'
    );
    await driver.waitFor(
      'return !!document.querySelector(".setup-preflight-blocked");'
    );
    console.log("  ✔ Setup Wizard preflight blocked initialization when diagnostic failed.");

    console.log("\n[E2E] All 7 WebDriver smoke test scenarios passed successfully!");
  } finally {
    if (driver) {
      await driver.closeSession();
    }
    if (cdProc) {
      cdProc.kill();
    }
    server.close();
  }
}

runSmokeTests().catch((err) => {
  console.error("\n[E2E] Test suite failed:", err);
  process.exit(1);
});
