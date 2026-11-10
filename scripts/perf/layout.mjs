// Layout probe: top-bar rows, overflow and screenshots at several window sizes.
import { startServer, startDriver, hostApps, sleep } from "./lib.mjs";

const SIZES = [
  [1920, 1080],
  [1366, 768],
  [1280, 720],
  [1100, 720], // default window
  [960, 1080], // half of a 1080p screen (tiled)
  [860, 580], // minimum window
  [640, 1080], // a third of a 1080p screen (tiled)
  [480, 900],
];
const TABS = ["My Apps", "Keymapping", "Install Tools", "Settings"];

const MEASURE = `
const vw = window.innerWidth, vh = window.innerHeight;
const top = document.querySelector(".topbar").getBoundingClientRect();
const overflowing = [...document.querySelectorAll("body *")].filter((el) => {
  const r = el.getBoundingClientRect();
  if (r.width === 0 || r.height === 0) return false;
  const style = getComputedStyle(el);
  if (style.position === "fixed" && el.className.includes?.("toast")) return false;
  return r.right > vw + 1 || r.left < -1;
}).map((el) => el.className || el.tagName).slice(0, 6);
const scroller = document.querySelector(".content-scrollable");
return {
  vw, vh,
  topbarH: Math.round(top.height),
  topbarShare: +(100 * top.height / vh).toFixed(1),
  hScroll: document.documentElement.scrollWidth > vw,
  overflowing,
  contentH: Math.round(scroller.getBoundingClientRect().height),
  scrollH: scroller.scrollHeight,
};`;

const server = await startServer({ online: true, booting: false, apps: hostApps() });
const d = await startDriver();
const url = `http://127.0.0.1:${server.address().port}`;
const rows = [];
try {
  for (const online of [true, false]) {
    server.state = { ...server.state, online };
    for (const [w, h] of SIZES) {
      await d.viewport(w, h);
      await d.navigate(url);
      await d.waitFor('return document.querySelectorAll(".app-row").length > 0');
      await sleep(400);
      for (const tab of TABS) {
        await d.tab(tab);
        const m = await d.exec(MEASURE);
        rows.push({ online, tab, ...m });
        if (
          (w === 1100 || w === 860 || w === 640 || w === 1920) &&
          (online || tab === "My Apps")
        ) {
          await d.shot(`${w}x${h}-${online ? "on" : "off"}-${tab.replace(/ /g, "")}`);
        }
      }
    }
  }
} finally {
  await d.close();
  server.close();
}
for (const r of rows) {
  console.log(
    `${r.online ? "on " : "off"} ${String(r.vw).padStart(4)}x${String(r.vh).padEnd(4)} ${r.tab.padEnd(13)} topbar=${String(r.topbarH).padStart(3)}px (${r.topbarShare}% of height) hScroll=${r.hScroll} content=${r.contentH}/${r.scrollH} ${r.overflowing.length ? "overflow: " + r.overflowing.join(" | ") : ""}`
  );
}
