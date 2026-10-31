import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const projectRoot = new URL("../../", import.meta.url);
const read = (path) => readFileSync(new URL(path, projectRoot), "utf8");

test("host diagnostics renders the remediation command for a failing item", () => {
  const source = read("src/components/settings/HostDiagnostics.jsx");
  assert.match(source, /item\.remediation/, "remediation is never read");
  assert.match(source, /diag-remediation/, "remediation has no rendered block");
});

test("setup wizard runs the host preflight and renders remediation", () => {
  const source = read("src/components/setup/SetupWizard.jsx");
  assert.match(
    source,
    /invoke\(\s*"check_host_diagnostics"\s*\)/,
    "wizard does not call the preflight command"
  );
  assert.match(source, /item\.remediation/, "wizard never renders remediation");
  assert.match(source, /setup-preflight-fix/, "remediation has no rendered block");
});

test("setup wizard blocks initialization when a required dependency is missing", () => {
  const source = read("src/components/setup/SetupWizard.jsx");
  assert.match(source, /blocksInitialization/, "no blocking predicate");
  assert.match(source, /package_manager/, "package manager fallback is ignored");
  assert.match(source, /disabled=\{!canInitialize\}/, "init button is never gated");
});
