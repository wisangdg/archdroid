// Render-cost probe (Chromium): idle polling, search typing, terminal streaming.
import { startServer, startDriver, manyApps, sleep, diff, PERF_KEYS } from "./lib.mjs";

const server = await startServer({ online: true, booting: false, apps: manyApps(15) });
const d = await startDriver();
const url = `http://127.0.0.1:${server.address().port}`;

async function load(count, online = true) {
  server.state = { online, booting: false, apps: manyApps(count) };
  const before = Date.now();
  await d.navigate(url);
  await d.waitFor(`return document.querySelectorAll(".app-row").length >= ${Math.min(count, 1)}`);
  await sleep(1500);
  return Date.now() - before;
}

async function measure(label, action, settleMs) {
  const before = await d.metrics();
  const t0 = Date.now();
  const extra = await action();
  if (settleMs) await sleep(settleMs);
  const after = await d.metrics();
  const delta = diff(after, before, PERF_KEYS);
  console.log(
    `${label.padEnd(62)} wall=${String(Date.now() - t0).padStart(6)}ms task=${String(delta.TaskDuration).padStart(8)}ms script=${String(delta.ScriptDuration).padStart(8)}ms layout=${String(delta.LayoutDuration).padStart(7)}ms style=${String(delta.RecalcStyleDuration).padStart(7)}ms ${extra ? JSON.stringify(extra) : ""}`
  );
  return delta;
}

try {
  await d.viewport(1100, 720);

  // 1. Idle for 30 s on My Apps: one status poll per 5 s, one list poll per 10 s.
  for (const count of [15, 100, 300]) {
    await load(count);
    const payload = await d.exec("return JSON.stringify(window.__mockState.apps).length");
    await measure(`idle 30 s, engine running, ${count} apps (list payload ${(payload / 1024).toFixed(0)} KB)`, async () => {
      await sleep(30000);
      return await d.exec(
        "const n = {}; for (const i of window.__invocations.filter(i => i.t > performance.now() - 30000)) n[i.cmd] = (n[i.cmd]||0)+1; return n;"
      );
    });
  }
  await load(15, false);
  await measure("idle 30 s, engine stopped, 15 apps", async () => {
    await sleep(30000);
    return await d.exec(
      "const n = {}; for (const i of window.__invocations.filter(i => i.t > performance.now() - 30000)) n[i.cmd] = (n[i.cmd]||0)+1; return n;"
    );
  });

  // 2. Typing in the search box: 10 keystrokes.
  for (const count of [15, 300]) {
    await load(count);
    await measure(`type 10 characters in search, ${count} apps`, async () => {
      await d.execAsync(`
        const done = arguments[arguments.length - 1];
        const input = document.querySelector("#app-search");
        const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
        const text = "calculator";
        let i = 0;
        const step = () => {
          if (i === text.length) return requestAnimationFrame(() => requestAnimationFrame(done));
          setter.call(input, text.slice(0, ++i));
          input.dispatchEvent(new Event("input", { bubbles: true }));
          requestAnimationFrame(() => requestAnimationFrame(step));
        };
        step();`);
    });
  }

  // 3. Terminal: lines arrive one per task, the way Tauri delivers events.
  const stream = async (count, perTask) =>
    d.execAsync(
      `
      const [count, perTask, done] = arguments;
      const t0 = performance.now();
      let sent = 0, longest = 0, last = performance.now();
      const channel = new MessageChannel();
      const line = (i) => (i % 3 === 0
        ? " " + (i % 100) + "%|████▏     | " + i + "M/1.2G [00:10<01:00, 12.5MB/s]"
        : "(" + i + ") [12:00:00] Reading package lists and unpacking overlay file number " + i);
      channel.port1.onmessage = () => {
        const now = performance.now();
        longest = Math.max(longest, now - last);
        last = now;
        for (let k = 0; k < perTask && sent < count; k++, sent++)
          window.__emitTauriEvent("cmd-output", { task_id: "task-probe", line: line(sent) });
        if (sent < count) return channel.port2.postMessage(0);
        const sentAt = performance.now();
        requestAnimationFrame(() => requestAnimationFrame(() => done({
          emitMs: Math.round(sentAt - t0),
          renderedMs: Math.round(performance.now() - t0),
          longestGapMs: Math.round(longest),
          domLines: document.querySelectorAll(".terminal-log-line").length,
        })));
      };
      channel.port2.postMessage(0);`,
      [count, perTask]
    );

  // 3a. View Logs: no stepper, console always visible. tail -n 1000.
  await load(15);
  await d.tab("Settings");
  await d.exec(`[...document.querySelectorAll("button")].find(b => b.textContent.includes("View Logs")).click();`);
  await d.waitFor('return !!document.querySelector(".terminal-modal")');
  await sleep(200);
  for (const count of [1000, 1000, 3000]) {
    await measure(`View Logs: ${count} more lines, console visible`, () => stream(count, 1));
  }

  // 3b. Installer with a stepper: console collapsed by default.
  await load(15);
  await d.tab("Install Tools");
  await d.exec(`[...document.querySelectorAll("button")].find(b => b.textContent.includes("Install ARM Support")).click();`);
  await d.waitFor('return !!document.querySelector(".confirm-modal-card")');
  await d.exec(`document.querySelector(".confirm-modal-actions .btn-primary").click();`);
  await d.waitFor('return !!document.querySelector(".terminal-modal")');
  await sleep(200);
  await measure("installer (stepper): 1000 lines, console collapsed", () => stream(1000, 1));
  await measure("installer (stepper): 3000 more lines, console collapsed", () => stream(3000, 1));
  await d.shot("terminal-stepper-flood");
  await d.exec(`document.querySelector(".btn-console-toggle").click();`);
  await sleep(300);
  await measure("installer (stepper): 1000 more lines, console open (2000 kept)", () => stream(1000, 1));
  await d.shot("terminal-console-flood");
  const scroll = await d.exec(
    `const el = document.querySelector(".terminal-console-body"); return { scrollTop: Math.round(el.scrollTop), max: el.scrollHeight - el.clientHeight };`
  );
  console.log("after the last line: console scrollTop/max =", JSON.stringify(scroll));
} finally {
  await d.close();
  server.close();
}
