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

test("release AppImages come from the Ubuntu 22.04 workflow and pass the glibc floor", () => {
  const gen = fs.readFileSync(
    path.join(repoRoot, "scripts/release/generate-release.sh"),
    "utf8"
  );
  assert.match(gen, /check-glibc\.sh" "\$APPIMAGE_PKG" 2\.35/);

  const check = path.join(repoRoot, "scripts/release/check-glibc.sh");
  assert.ok(
    (fs.statSync(check).mode & 0o111) !== 0,
    "check-glibc.sh must be executable"
  );

  const workflow = fs.readFileSync(
    path.join(repoRoot, ".github/workflows/appimage.yml"),
    "utf8"
  );
  assert.match(workflow, /runs-on: ubuntu-22\.04/);
  assert.match(workflow, /check-glibc\.sh [^\n]*2\.35/);
  // Without this the Ubuntu 22.04 AppImage shows a blank window on Arch.
  assert.match(workflow, /repack-appimage\.sh/);
  // Third-party actions are pinned to a commit, like ci.yml.
  for (const uses of workflow.match(/uses: \S+/g)) {
    assert.match(uses, /@[0-9a-f]{40}$/, `${uses} is not pinned to a commit`);
  }
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
  assert.ok(readme.includes("Tier 1 (Fully Supported & Live-Tested)"));
  assert.ok(
    readme.includes("`.pkg.tar.zst` (release), local PKGBUILD, AppImage")
  );

  // No unverified claims
  assert.ok(
    !readme.includes("AUR / Source / AppImage"),
    "AUR package not yet published to AUR"
  );
  // Tier 2 stays best-effort; the only live-tested Tier 2 claim is the .deb
  // on Ubuntu 24.04 (docs/P0-8.2-MATRIX-RESULTS.md), and the untested
  // distros must still be named as untested.
  assert.ok(readme.includes("Tier 2 (Best-Effort)"));
  assert.ok(readme.includes("live-tested in an Ubuntu 24.04 LTS VM"));
  assert.ok(readme.includes("live-tested in a Fedora 44 VM"));
  assert.ok(readme.includes("The RHEL family is not live-tested."));
  // openSUSE: the package and setup screen were tested, Waydroid was not.
  const opensuse = readme
    .split("\n")
    .find((line) => line.startsWith("| **openSUSE"));
  assert.ok(opensuse.includes("no official Waydroid package"));
  assert.ok(opensuse.includes("Waydroid itself was not run"));
  assert.ok(
    !/init via polkit|engine start/.test(opensuse),
    "openSUSE row must not claim the Waydroid flow was tested"
  );

  // Package names and file names as the bundler produces them: the
  // .deb/.rpm package is `arch-droid`, files start with `ArchDroid`.
  assert.ok(readme.includes("sudo apt remove arch-droid"));
  assert.ok(readme.includes("sudo dnf remove arch-droid"));
  assert.ok(!/(apt|dnf) remove archdroid\b/.test(readme));
  assert.ok(readme.includes("./ArchDroid_*_amd64.deb"));
  assert.ok(readme.includes("./ArchDroid-*.x86_64.rpm"));

  // Relative links only, no file:/// leaks
  assert.ok(
    !readme.includes("file:///"),
    "README must not leak local developer file:/// paths"
  );
});
