// Does the stepper survive a long download (progress lines past the 2000-line cap)?
// Also: console default state, last-line visibility, context-menu focus.
import { startServer, startDriver, hostApps, sleep } from "./lib.mjs";

const server = await startServer({ online: true, booting: false, apps: hostApps() });
const d = await startDriver();
const url = `http://127.0.0.1:${server.address().port}`;
const phase = () => d.exec('return document.querySelector(".status-card-phase")?.textContent');
const emit = (lines) =>
  d.execAsync(
    `const [lines, done] = arguments; let i = 0;
     const ch = new MessageChannel();
     ch.port1.onmessage = () => {
       for (let k = 0; k < 20 && i < lines.length; k++, i++)
         window.__emitTauriEvent("cmd-output", { task_id: "task-probe", line: lines[i] });
       if (i < lines.length) ch.port2.postMessage(0);
       else requestAnimationFrame(() => requestAnimationFrame(done));
     };
     ch.port2.postMessage(0);`,
    [lines]
  );
try {
  await d.viewport(1100, 720);
  await d.navigate(url);
  await d.waitFor('return document.querySelectorAll(".app-row").length > 0');
  await d.tab("Install Tools");
  await d.exec(`[...document.querySelectorAll("button")].find(b => b.textContent.includes("Install ARM Support")).click();`);
  await d.waitFor('return !!document.querySelector(".confirm-modal-card")');
  await d.shot("confirm-root");
  console.log("confirm dialog focus:", await d.exec("return document.activeElement.textContent"));
  await d.exec(`document.querySelector(".confirm-modal-actions .btn-primary").click();`);
  await d.waitFor('return !!document.querySelector(".terminal-modal")');
  await sleep(300);
  console.log(
    "first stepper task, console expanded by default:",
    await d.exec('return !!document.querySelector(".terminal-console-body")'),
    "| toggle says:",
    await d.exec('return document.querySelector(".btn-console-toggle span").textContent')
  );
  const track = await d.exec(
    `const c = document.querySelector(".stepper-container"); const nodes = [...document.querySelectorAll(".stepper-node")];
     const cr = c.getBoundingClientRect();
     return { steps: nodes.length, visible: nodes.filter(n => n.getBoundingClientRect().right <= cr.right + 1).length, scrollWidth: c.scrollWidth, clientWidth: c.clientWidth };`
  );
  console.log("stepper:", JSON.stringify(track));

  await emit(["[STEP] deps", "resolving", "[STEP] repo", "cloning", "[STEP] env", "pip install", "[STEP] download"]);
  console.log("after [STEP] download:", await phase());
  const bar = (i) => ` ${Math.floor(i / 25)}%|███▏      | ${i}M/2.5G [00:10<01:00, 1.1MB/s]`;
  await emit(Array.from({ length: 1900 }, (_, i) => bar(i)));
  console.log("after 1900 progress lines:", await phase());
  await emit(Array.from({ length: 200 }, (_, i) => bar(1900 + i)));
  console.log("after 2100 progress lines:", await phase());
  await d.shot("stepper-after-flood");

  // Finish: is the last line visible?
  await d.exec(`window.__emitTauriEvent("cmd-finished", { task_id: "task-probe", success: true, code: null, message: null }); window.__finishTask(true);`);
  await sleep(500);
  console.log(
    "after cmd-finished:",
    JSON.stringify(
      await d.exec(
        `const el = document.querySelector(".terminal-console-body"); const last = el.lastElementChild.getBoundingClientRect(); const box = el.getBoundingClientRect();
         return { lastLine: el.lastElementChild.textContent.trim(), scrollTop: Math.round(el.scrollTop), maxScroll: el.scrollHeight - el.clientHeight, lastLineVisible: last.bottom <= box.bottom + 1 };`
      )
    )
  );
  await d.shot("terminal-finished");
  await d.exec(`[...document.querySelectorAll(".terminal-footer-actions button")].find(b => /Done|Close/.test(b.textContent)).click();`);
  await sleep(300);

  // Toasts: how long does an error stay, and how many stack up?
  await d.tab("My Apps");
  // Context menu: does a re-render move the keyboard focus back to the first item?
  await d.exec(`const row = document.querySelector(".app-row"); row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, clientX: 200, clientY: 300 }));`);
  await d.waitFor('return !!document.querySelector(".custom-context-menu")');
  await d.exec(`window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true })); window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));`);
  const focusBefore = await d.exec("return document.activeElement.textContent");
  await d.shot("context-menu");
  await sleep(11000); // one app-list poll re-renders MyAppsView
  const focusAfter = await d.exec("return document.activeElement.textContent");
  console.log(`context menu focus: "${focusBefore}" -> after one list poll: "${focusAfter}"`);
} finally {
  await d.close();
  server.close();
}
