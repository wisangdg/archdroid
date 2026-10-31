# Dependency & supply-chain notes

## CI gates
- `npm audit --omit=dev` (production dependencies must be clean).
- `cargo audit` (pinned `cargo-audit` 0.22.2) on `src-tauri/Cargo.lock`.
- All third-party GitHub Actions are pinned to commit SHAs; the workflow token is `contents: read`.

## Accepted risk
`npm audit` (dev dependencies, after Vite 8 / Vitest 5 upgrade) reports
**6 moderate** findings, all `minimatch` DoS (GHSA-g3hv-75wg-m249) reached via
`eslint` and `eslint-plugin-react`. They only run on developer/CI machines on
repository-controlled globs, never ship in the app, and the only offered "fix"
is a downgrade to eslint@4. Re-check when ESLint publishes a patched release.

## Tauri capabilities
`capabilities/default.json` grants only `core:event:allow-listen`,
`core:event:allow-unlisten` and `dialog:allow-open`; `core:default` is not used.

## Desktop icons
`Icon=` targets from `waydroid.*.desktop` files are read only if absolute,
a regular file, <= 512 KiB and PNG or SVG by content (MIME derived from the
bytes). XPM and anything else yield no icon.

## ADB auto-authorization
ArchDroid appends the host's ADB public key to the Waydroid container's
`adb_keys` so `adb` connects without a prompt. Every ADB command targets the
container's own serial (`<container-ip>:5555`, via `-s`), never the default
device, so other attached phones/emulators are untouched. Regression tests in
`src-tauri/src/adb.rs` cover serial targeting, key appending without
duplication, and rejection of multi-line key material.
