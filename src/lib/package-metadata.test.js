import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "../..");

test("tauri.conf.json bundle metadata is complete and accurately declared", () => {
  const confPath = path.join(repoRoot, "src-tauri/tauri.conf.json");
  const conf = JSON.parse(fs.readFileSync(confPath, "utf8"));

  assert.equal(conf.mainBinaryName, "archdroid");
  assert.equal(conf.bundle.active, true);
  assert.equal(conf.bundle.license, "MIT");
  assert.ok(conf.bundle.publisher.includes("Wisang Drillian Geni"));
  assert.ok(conf.bundle.homepage.startsWith("https://github.com/"));
  assert.ok(conf.bundle.copyright.includes("2026"));

  // Linux package runtime dependencies
  // Ubuntu 24.04 has no waydroid package (it comes from repo.waydro.id), so
  // a hard dependency made the .deb uninstallable; the setup screen handles a
  // host without Waydroid instead.
  assert.ok(Array.isArray(conf.bundle.linux?.deb?.depends));
  assert.ok(!conf.bundle.linux.deb.depends.includes("waydroid"));
  assert.ok(Array.isArray(conf.bundle.linux?.deb?.recommends));
  assert.ok(conf.bundle.linux.deb.recommends.includes("waydroid"));
  assert.ok(conf.bundle.linux.deb.recommends.includes("squashfs-tools"));
  assert.ok(conf.bundle.linux.deb.recommends.includes("lzip"));
  assert.ok(conf.bundle.linux.deb.recommends.includes("git"));

  assert.ok(Array.isArray(conf.bundle.linux?.rpm?.depends));
  assert.ok(conf.bundle.linux.rpm.depends.includes("waydroid"));
});

test("package.json and LICENSE file declare matching licensing and repo metadata", () => {
  const pkgPath = path.join(repoRoot, "package.json");
  const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));

  assert.equal(pkg.license, "MIT");
  assert.ok(pkg.author.includes("Wisang Drillian Geni"));
  assert.equal(pkg.homepage, "https://github.com/wisangdg/archdroid");
  assert.equal(
    pkg.repository?.url,
    "https://github.com/wisangdg/archdroid.git"
  );

  const licensePath = path.join(repoRoot, "LICENSE");
  assert.ok(fs.existsSync(licensePath));
  const licenseText = fs.readFileSync(licensePath, "utf8");
  assert.ok(licenseText.includes("MIT License"));
  assert.ok(licenseText.includes("Wisang Drillian Geni"));
});

test("third-party brand names are audited and metadata points to legitimate repository", () => {
  const pkg = JSON.parse(
    fs.readFileSync(path.join(repoRoot, "package.json"), "utf8")
  );
  const conf = JSON.parse(
    fs.readFileSync(path.join(repoRoot, "src-tauri/tauri.conf.json"), "utf8")
  );
  const indexHtml = fs.readFileSync(path.join(repoRoot, "index.html"), "utf8");
  const pkgbuild = fs.readFileSync(
    path.join(repoRoot, "packaging/arch/PKGBUILD"),
    "utf8"
  );
  const readme = fs.readFileSync(path.join(repoRoot, "README.md"), "utf8");

  // Legit owner identifier and repo URLs
  assert.equal(conf.identifier, "io.github.wisangdg.archdroid");
  assert.equal(conf.bundle.homepage, "https://github.com/wisangdg/archdroid");
  assert.ok(pkgbuild.includes('url="https://github.com/wisangdg/archdroid"'));

  // No commercial emulator branding in package metadata and window title
  assert.ok(!pkg.description.toLowerCase().includes("ldplayer"));
  assert.ok(!pkg.description.toLowerCase().includes("nox"));
  assert.ok(!conf.bundle.longDescription.toLowerCase().includes("ldplayer"));
  assert.ok(!conf.bundle.shortDescription.toLowerCase().includes("ldplayer"));
  assert.ok(!indexHtml.toLowerCase().includes("ldplayer"));
  assert.ok(!indexHtml.toLowerCase().includes("nox"));
  assert.ok(!pkgbuild.toLowerCase().includes("ldplayer-style"));

  // README contains explicit trademark disclaimer
  assert.ok(readme.includes("Trademark Disclaimer"));
});
