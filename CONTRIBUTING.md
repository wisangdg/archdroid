# Contributing to ArchDroid

Thanks for helping. ArchDroid asks for root on the user's machine, so changes are held to a few strict rules; please read them before opening a pull request.

## Build and test

```bash
npm ci
npm run tauri dev                                   # Vite + Tauri, live reload

npm run lint && npm run format:check                # frontend lint and formatting
npm test                                            # node unit tests + Vitest components
npm run test:e2e                                    # WebDriver smoke (needs chromium + chromedriver)

cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features dev-commands -- -D warnings
cargo test
cargo test --features dev-commands
```

CI runs all of these on every pull request; a PR is merged only when they pass.

Live tests against a real Waydroid, polkit, and the installers run in a disposable VM: see [`scripts/test-vm/`](scripts/test-vm/) and [`docs/P0-8.1-TEST-VM.md`](docs/P0-8.1-TEST-VM.md).

## Backend rules (`src-tauri/src/`)

1. **One module per area.** Waydroid is only started from `waydroid.rs`, adb only from `adb.rs`. Host programs are started through `hostenv::host_command`, never `Command::new` (a test enforces this; it keeps the AppImage's library paths out of host tools).
2. **Root only through `stream_privileged_script`.** Never call `sudo`/`pkexec` inside a script. Every root action needs a consent dialog in the UI that shows the command and the packages first.
3. **No unquoted input in a shell.** Build argv vectors; when a script must contain a value, pass it through `sh_quote`.
4. **Pure helpers get unit tests** (argv builders, parsers, coordinate maths). They must not need root or a running Waydroid.
5. **Production IPC only with its UI.** A command is registered in the default build only together with the UI that calls it and its test; everything else stays behind the `dev-commands` feature.
6. **Every child process has a deadline**, and state-changing commands hold the single-flight `TaskGuard`.
7. **No telemetry**, analytics, or network access the user did not ask for (see [PRIVACY.md](PRIVACY.md)).

## Frontend rules (`src/`)

- Branch on `CommandResult.code`, never on message text.
- Branch on the named UI state (`phase`, `outcome`, `kind`), not on combinations of booleans; [docs/STATE-MACHINES.md](docs/STATE-MACHINES.md) lists each machine, its states, and the `data-*` attribute that exposes it.
- Show success only after the backend confirmed it (`status === "success"` or `cmd-finished`).
- No HTML from strings (`dangerouslySetInnerHTML`, `innerHTML`) and no remote content; the CSP and `src/lib/webview-security.test.js` enforce this.
- Keyboard and screen-reader use must keep working (`src/test/accessibility.test.jsx`).

## Commits and documentation

- [Conventional Commits](https://www.conventionalcommits.org/) (`fix(ui): ...`, `feat(input): ...`, `docs: ...`), one logical change per commit.
- User-visible changes go into [CHANGELOG.md](CHANGELOG.md) under `[Unreleased]`.
- Claims in the README follow test evidence. Do not mark a distribution or feature as working without having run it.

## Reporting bugs and vulnerabilities

- Bugs: use the issue template and attach a Diagnostic Bundle (Settings → Diagnostic Bundle).
- Vulnerabilities: report privately as described in [SECURITY.md](SECURITY.md).
