import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const srcDir = path.resolve(__dirname, "../../src-tauri/src");

// Inside the AppImage, a host program started with plain Command::new inherits
// the bundle's LD_LIBRARY_PATH/GI_TYPELIB_PATH; host waydroid then failed with
// "undefined symbol: g_once_init_leave_pointer" (0.1.4 on Arch). Production
// code must start host programs through hostenv::host_command.
test("production Rust code starts host programs only through host_command", () => {
  const offenders = [];
  for (const file of fs.readdirSync(srcDir).filter((f) => f.endsWith(".rs"))) {
    if (file === "hostenv.rs") continue;
    const source = fs.readFileSync(path.join(srcDir, file), "utf8");
    const production = source.split("#[cfg(test)]\nmod tests")[0];
    production.split("\n").forEach((line, i) => {
      if (/\bCommand::new\(/.test(line)) offenders.push(`${file}:${i + 1}`);
    });
  }
  assert.deepEqual(offenders, []);
});
