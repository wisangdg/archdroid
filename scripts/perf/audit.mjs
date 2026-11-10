// Typography and contrast audit of the rendered UI (Chromium, host fonts).
import { startServer, startDriver, hostApps, sleep } from "./lib.mjs";

const AUDIT = `
function parse(c) {
  const m = c.match(/rgba?\\(([^)]+)\\)/);
  if (!m) return null;
  const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
  return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
}
function over(top, bottom) {
  const a = top.a + bottom.a * (1 - top.a);
  if (a === 0) return { r: 0, g: 0, b: 0, a: 0 };
  return {
    r: (top.r * top.a + bottom.r * bottom.a * (1 - top.a)) / a,
    g: (top.g * top.a + bottom.g * bottom.a * (1 - top.a)) / a,
    b: (top.b * top.a + bottom.b * bottom.a * (1 - top.a)) / a,
    a,
  };
}
function lum(c) {
  const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4); };
  return 0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b);
}
function ratio(a, b) {
  const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
  return (x + 0.05) / (y + 0.05);
}
function background(el) {
  const chain = [];
  let gradient = false;
  for (let n = el; n; n = n.parentElement) {
    const s = getComputedStyle(n);
    if (s.backgroundImage && s.backgroundImage !== "none" && n !== document.body) gradient = true;
    const c = parse(s.backgroundColor);
    if (c && c.a > 0) chain.push(c);
    if (c && c.a === 1) break;
  }
  let bg = { r: 8, g: 11, b: 16, a: 1 };
  for (const c of chain.reverse()) bg = over(c, bg);
  return { bg, gradient };
}
const seen = new Map();
const sizes = {};
let total = 0;
const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
for (let node = walker.nextNode(); node; node = walker.nextNode()) {
  const text = node.textContent.trim();
  if (!text) continue;
  const el = node.parentElement;
  const rect = el.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) continue;
  const s = getComputedStyle(el);
  if (s.visibility === "hidden" || el.closest(".sr-only")) continue;
  const size = parseFloat(s.fontSize);
  const weight = parseInt(s.fontWeight, 10);
  total++;
  sizes[size] = (sizes[size] || 0) + 1;
  let fg = parse(s.color);
  const { bg, gradient } = background(el);
  // Opacity of disabled controls lowers the effective contrast too.
  let opacity = 1;
  for (let n = el; n; n = n.parentElement) opacity *= parseFloat(getComputedStyle(n).opacity);
  fg = over({ ...fg, a: fg.a * opacity }, bg);
  const r = ratio(fg, bg);
  const large = size >= 24 || (size >= 18.66 && weight >= 700);
  const need = large ? 3 : 4.5;
  if (r < need) {
    const key = [s.color, Math.round(bg.r), Math.round(bg.g), Math.round(bg.b), size, opacity < 1 ? "disabled" : ""].join("|");
    const cls = (el.className && String(el.className).split(" ")[0]) || el.tagName.toLowerCase();
    const prev = seen.get(key);
    if (prev) { prev.count++; if (!prev.classes.includes(cls) && prev.classes.length < 5) prev.classes.push(cls); }
    else seen.set(key, { ratio: +r.toFixed(2), need, size, color: s.color, bg: "rgb(" + [bg.r, bg.g, bg.b].map(Math.round).join(",") + ")", gradient, disabled: opacity < 1, classes: [cls], sample: text.slice(0, 40), count: 1 });
  }
}
return { total, sizes, fails: [...seen.values()].sort((a, b) => a.ratio - b.ratio) };
`;

const FONTS = `
const canvas = document.createElement("canvas").getContext("2d");
const width = (family) => { canvas.font = "40px " + family; return canvas.measureText("Applications mmmWWWiii 0123").width; };
const has = (name, generic) => width('"' + name + '", ' + generic) !== width(generic);
return {
  Outfit: has("Outfit", "monospace") && has("Outfit", "serif"),
  JetBrainsMono: has("JetBrains Mono", "serif") && has("JetBrains Mono", "sans-serif"),
  body: getComputedStyle(document.body).fontFamily,
};`;

const server = await startServer({ online: true, booting: false, apps: hostApps() });
const d = await startDriver();
const url = `http://127.0.0.1:${server.address().port}`;
const report = {};
try {
  await d.viewport(1100, 720);
  await d.navigate(url);
  await d.waitFor('return document.querySelectorAll(".app-row").length > 0');
  await sleep(500);
  console.log("fonts:", JSON.stringify(await d.exec(FONTS)));
  for (const tab of ["My Apps", "Keymapping", "Install Tools", "Settings"]) {
    await d.tab(tab);
    await sleep(300);
    report[tab] = await d.exec(AUDIT);
  }
  // Engine stopped: disabled controls and the offline notes.
  server.state = { ...server.state, online: false };
  await d.navigate(url);
  await d.waitFor('return document.querySelectorAll(".app-row").length > 0');
  await d.tab("Settings");
  await sleep(300);
  report["Settings (engine stopped)"] = await d.exec(AUDIT);
  await d.tab("My Apps");
  report["My Apps (engine stopped)"] = await d.exec(AUDIT);
} finally {
  await d.close();
  server.close();
}
const allSizes = {};
for (const [tab, r] of Object.entries(report)) {
  console.log(`\n== ${tab}: ${r.total} text runs, ${r.fails.length} failing colour pairs`);
  for (const [s, n] of Object.entries(r.sizes)) allSizes[s] = (allSizes[s] || 0) + n;
  for (const f of r.fails)
    console.log(
      `  ${f.ratio}:1 (need ${f.need}) ${f.size}px ${f.color} on ${f.bg}${f.gradient ? " +gradient" : ""}${f.disabled ? " [disabled]" : ""} x${f.count} .${f.classes.join(" .")} "${f.sample}"`
    );
}
console.log("\nfont sizes (px: text runs):", JSON.stringify(Object.fromEntries(Object.entries(allSizes).sort((a, b) => a[0] - b[0]))));
