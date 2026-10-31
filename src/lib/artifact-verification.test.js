import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../..");

test("release public key exists and contains expected key id", () => {
  const pubkeyPath = path.join(
    repoRoot,
    "packaging/release/archdroid-release.asc"
  );
  assert.ok(fs.existsSync(pubkeyPath), "archdroid-release.asc must exist");

  const content = fs.readFileSync(pubkeyPath, "utf8");
  assert.ok(
    content.includes("BEGIN PGP PUBLIC KEY BLOCK"),
    "must be ASCII armored PGP key"
  );

  // Verify key details using gpg
  const keyInfo = execFileSync("gpg", ["--show-keys", pubkeyPath], {
    encoding: "utf8",
  });
  assert.ok(
    keyInfo.includes("EB0290C2138FA472E18F020318F47067ACC5BBF9"),
    "must match release key fingerprint"
  );
  assert.ok(
    keyInfo.includes("ArchDroid Release Signing Key"),
    "must match identity"
  );
});

test("release generation and verification scripts exist and are executable", () => {
  const genScript = path.join(repoRoot, "scripts/release/generate-release.sh");
  const verifyScript = path.join(repoRoot, "scripts/release/verify-release.sh");

  assert.ok(fs.existsSync(genScript));
  assert.ok(fs.existsSync(verifyScript));

  const genMode = fs.statSync(genScript).mode;
  const verifyMode = fs.statSync(verifyScript).mode;

  assert.ok((genMode & 0o111) !== 0, "generate-release.sh must be executable");
  assert.ok((verifyMode & 0o111) !== 0, "verify-release.sh must be executable");
});

test("verify-release.sh validates staged artifacts when present", (t) => {
  const distDir = path.join(repoRoot, "dist-release");
  if (!fs.existsSync(path.join(distDir, "SHA256SUMS"))) {
    t.skip("dist-release not staged");
    return;
  }

  const verifyScript = path.join(repoRoot, "scripts/release/verify-release.sh");
  const output = execFileSync(verifyScript, [distDir], { encoding: "utf8" });
  assert.ok(output.includes("All artifacts in"));
  assert.ok(output.includes("successfully verified!"));
});

test("platform support claims in README strictly match test evidence", () => {
  const readme = fs.readFileSync(path.join(repoRoot, "README.md"), "utf8");

  // Tier 1 must be Arch Linux, verified live
  assert.ok(readme.includes("Tier 1 (Didukung Penuh & Teruji Live)"));
  assert.ok(readme.includes("PKGBUILD lokal (`makepkg -si`) / AppImage"));

  // No unverified claims
  assert.ok(
    !readme.includes("AUR / Source / AppImage"),
    "AUR package not yet published to AUR"
  );
  assert.ok(readme.includes("Tier 2 (Best-Effort — Belum Diuji Live)"));

  // Relative links only, no file:/// leaks
  assert.ok(
    !readme.includes("file:///"),
    "README must not leak local developer file:/// paths"
  );
});
