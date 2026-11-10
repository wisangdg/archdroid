import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";

const css = readFileSync(new URL("../index.css", import.meta.url), "utf8");

/** Declarations of every rule whose selector matches `pattern`. */
function rules(pattern) {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .filter(([, selector]) => pattern.test(selector))
    .map(([, selector, body]) => ({ selector: selector.trim(), body }));
}

test("a running engine does not animate", () => {
  // "Engine Running" lasts for hours; an endless animation there kept
  // WebKitGTK repainting (measured: a full core without GPU compositing).
  for (const { selector, body } of rules(/\.status-online|\.engine-pill/)) {
    assert.ok(
      !/animation\s*:/.test(body),
      `${selector} must not animate in the steady state`
    );
  }
  assert.ok(
    rules(/\.status-booting \.status-dot/).some(({ body }) =>
      /animation\s*:/.test(body)
    ),
    "the booting state, which ends by itself, keeps its animation"
  );
});

test("the top bar has no backdrop blur", () => {
  for (const { selector, body } of rules(/(^|[\s,])\.topbar\s*$/)) {
    assert.ok(!/backdrop-filter/.test(body), `${selector} must not blur`);
  }
});

test("the UI fonts ship with the app and are used through the tokens", () => {
  const faces = [...css.matchAll(/@font-face\s*\{([^}]*)\}/g)].map((m) => m[1]);
  for (const family of ["Outfit", "JetBrains Mono"]) {
    const own = faces.filter((face) => face.includes(`"${family}"`));
    assert.ok(own.length > 0, `${family} has an @font-face`);
    for (const face of own) {
      const file = /url\("\.\/(assets\/fonts\/[^"]+)"\)/.exec(face)?.[1];
      assert.ok(file, `${family} is loaded from src/assets/fonts`);
      assert.ok(existsSync(new URL(`../${file}`, import.meta.url)), file);
    }
  }
  for (const licence of ["Outfit-OFL.txt", "JetBrainsMono-OFL.txt"]) {
    assert.ok(
      existsSync(new URL(`../assets/fonts/${licence}`, import.meta.url)),
      `${licence} ships next to the font`
    );
  }
  const withoutFaces = css.replace(/@font-face\s*\{[^}]*\}/g, "");
  assert.ok(
    !/font-family:\s*"JetBrains Mono"/.test(withoutFaces),
    "monospace text uses var(--font-mono)"
  );
});
