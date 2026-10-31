import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../..');

test('tauri.conf.json bundle metadata is complete and accurately declared', () => {
  const confPath = path.join(repoRoot, 'src-tauri/tauri.conf.json');
  const conf = JSON.parse(fs.readFileSync(confPath, 'utf8'));

  assert.equal(conf.mainBinaryName, 'archdroid');
  assert.equal(conf.bundle.active, true);
  assert.equal(conf.bundle.license, 'MIT');
  assert.ok(conf.bundle.publisher.includes('Wisang Drillian Geni'));
  assert.ok(conf.bundle.homepage.startsWith('https://github.com/'));
  assert.ok(conf.bundle.copyright.includes('2026'));

  // Linux package runtime dependencies
  assert.ok(Array.isArray(conf.bundle.linux?.deb?.depends));
  assert.ok(conf.bundle.linux.deb.depends.includes('waydroid'));
  assert.ok(Array.isArray(conf.bundle.linux?.deb?.recommends));
  assert.ok(conf.bundle.linux.deb.recommends.includes('squashfs-tools'));
  assert.ok(conf.bundle.linux.deb.recommends.includes('lzip'));
  assert.ok(conf.bundle.linux.deb.recommends.includes('git'));

  assert.ok(Array.isArray(conf.bundle.linux?.rpm?.depends));
  assert.ok(conf.bundle.linux.rpm.depends.includes('waydroid'));
});

test('package.json and LICENSE file declare matching licensing and repo metadata', () => {
  const pkgPath = path.join(repoRoot, 'package.json');
  const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));

  assert.equal(pkg.license, 'MIT');
  assert.ok(pkg.author.includes('Wisang Drillian Geni'));
  assert.ok(pkg.homepage.startsWith('https://github.com/'));
  assert.ok(pkg.repository?.url.includes('archdroid'));

  const licensePath = path.join(repoRoot, 'LICENSE');
  assert.ok(fs.existsSync(licensePath));
  const licenseText = fs.readFileSync(licensePath, 'utf8');
  assert.ok(licenseText.includes('MIT License'));
  assert.ok(licenseText.includes('Wisang Drillian Geni'));
});
