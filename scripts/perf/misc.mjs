// Smaller behaviour checks against the production bundle (Chromium, mocked IPC).
import { startServer, startDriver, hostApps, sleep } from "./lib.mjs";

const server = await startServer({ online: true, booting: false, apps: hostApps() });
const d = await startDriver();
const url = `http://127.0.0.1:${server.address().port}`;
const buttons = (sel) =>
  d.exec(`return [...document.querySelectorAll(arguments[0])].map(b => b.textContent.trim() + (b.disabled ? " [disabled]" : ""))`, [sel]);
try {
  await d.viewport(1100, 720);

  // 1. What the UI sends when it opens with the engine stopped.
  server.state = { ...server.state, online: false };
  await d.navigate(url);
  await d.waitFor('return document.querySelectorAll(".app-row").length > 0');
  await sleep(2500);
  const counts = await d.exec(
    "const n = {}; for (const i of window.__invocations) if (!i.cmd.startsWith('plugin:')) n[i.cmd] = (n[i.cmd]||0)+1; return n;"
  );
  console.log("startup, engine stopped:", JSON.stringify(counts));

  // 2. Advanced property form with the engine stopped.
  await d.tab("Settings");
  console.log("engine stopped:", JSON.stringify(await buttons(".prop-form button, .btn-apply-res")));

  // 3. Display profile: Restart button before and after Apply.
  server.state = { ...server.state, online: true };
  await d.navigate(url);
  await d.waitFor('return document.querySelectorAll(".app-row").length > 0');
  await d.tab("Settings");
  await sleep(600);
  console.log("display, loaded:", JSON.stringify(await buttons(".res-actions-right button")));
  await d.exec(`[...document.querySelectorAll(".preset-card")].find(c => c.textContent.includes("Standard 720p")).click();`);
  await sleep(200);
  console.log("display, 720p selected (not applied):", JSON.stringify(await buttons(".res-actions-right button")));
  await d.exec(`document.querySelector(".btn-apply-res").click();`);
  await sleep(1200);
  console.log("display, after Apply:", JSON.stringify(await buttons(".res-actions-right button")), "|", await d.exec(`return document.querySelector(".res-apply-notice").innerText.replace(/\\n/g, " ")`));
  console.log("toasts:", JSON.stringify(await d.exec(`return [...document.querySelectorAll(".toast-item")].map(t => t.innerText.replace(/\\n/g, " | "))`)));

  // 4. Tab switch: is the My Apps state kept?
  await d.tab("My Apps");
  await d.exec(`
    const input = document.querySelector("#app-search");
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set.call(input, "o");
    input.dispatchEvent(new Event("input", { bubbles: true }));`);
  await sleep(200);
  await d.exec(`document.querySelectorAll(".app-row")[2].click();`);
  await sleep(200);
  const before = await d.exec(`return { query: document.querySelector("#app-search").value, selected: document.querySelector("#app-detail-title").textContent, rows: document.querySelectorAll(".app-row").length }`);
  await d.tab("Keymapping");
  await d.tab("My Apps");
  const after = await d.exec(`return { query: document.querySelector("#app-search").value, selected: document.querySelector("#app-detail-title").textContent, rows: document.querySelectorAll(".app-row").length }`);
  console.log("tab switch:", JSON.stringify(before), "->", JSON.stringify(after));

  // 5. Toast stack.
  for (let i = 0; i < 6; i++) {
    await d.exec(`[...document.querySelectorAll(".app-detail-actions button")].find(b => b.textContent.includes("Force stop")).click();`);
    await sleep(120);
  }
  await sleep(400);
  const toasts = await d.exec(`const c = document.querySelector(".toast-container").getBoundingClientRect(); return { count: document.querySelectorAll(".toast-item").length, top: Math.round(c.top), bottom: Math.round(c.bottom), right: Math.round(innerWidth - c.right), width: Math.round(c.width), userSelect: getComputedStyle(document.querySelector(".toast-message")).userSelect }`);
  console.log("toast stack:", JSON.stringify(toasts));
  await d.shot("toast-stack");
  const sel = await d.exec(`const pick = (s) => { const el = document.querySelector(s); return el ? getComputedStyle(el).userSelect : null; };
    return { appName: pick("#app-detail-title"), package: pick(".app-detail-package"), fact: pick(".app-fact dd"), toast: pick(".toast-message") }`);
  console.log("user-select:", JSON.stringify(sel));
  await sleep(4500);
  console.log("toasts left after 4.5 s:", await d.exec(`return document.querySelectorAll(".toast-item").length`));
} finally {
  await d.close();
  server.close();
}
