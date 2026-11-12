# UI state machines

The frontend's long-lived states, where each one lives, and how a test or a
script can read it. Branch on the named state (`phase`, `outcome`, `kind`),
not on combinations of booleans: the combinations are where impossible states
and disagreeing labels came from.

| Machine | States | Source of truth | Read it from the DOM |
|---|---|---|---|
| Engine | `checking` `uninitialized` `stopped` `unreachable` `starting` `booting` `running` `stopping` `restarting` | `enginePhase()` in `src/lib/enginePhase.js`, fed by `useEngineStatus` (`phase`) | `.status-badge[data-engine-phase]` |
| Streaming task (terminal) | `running` → `succeeded` \| `failed` \| `cancelled` (`ended`: output stopped without a result) | `terminalOutcome()` + `TERMINAL_OUTCOMES` in `src/components/terminal/stepConfigs.js` | `.terminal-modal[data-task-outcome][data-task-type]` |
| Task stage (stepper) | the `[STEP] <id>` stages per task | `TASK_STEP_CONFIGS` / `detectStepProgress()` (same file) | stepper markup |
| APK install | idle → installing (file) → cancelling → idle | `useApkInstall` (`installingApk`, `cancellingInstall`) | `.install-progress` |
| Pending launch | none → waiting for the engine → launched \| dropped | `pendingLaunch` in `src/App.jsx` | toasts ("Starting Engine First", "Android Is Booting") |
| Touch mode | `unknown` `unsupported` `needs_access` `off` `on` `stopped` | `touchModeState()` in `src/hooks/useTouchMode.js` | badge in the Touch Mode panel |
| Setup | `missing` `uninitialized` `initializing` `ready` | `setupStage()` in `src/lib/setupStage.js` | setup title |

## Rules the machines enforce

- **One engine operation at a time.** `handleStart`/`handleStop`/`handleRestart`
  claim a slot; a second call while one runs is ignored. While an operation
  runs, its phase wins over the polled status (a starting session reports
  offline for seconds; that is "Starting Engine…", not "Engine Stopped").
- **An unreadable status is `unreachable`**, never shown as a stopped engine.
- **A launch request only lives while the engine is on its way up.** It opens
  once Android has booted and is dropped when the engine ends up `stopped` or
  `unreachable`. Launch during `stopping` is refused with a toast; during
  `restarting` it waits for the boot.
- **The backend task slot is shared** (`TaskGuard` in `src-tauri/src/stream.rs`):
  installers, Factory Reset, View Logs, APK install and uninstall. While it is
  taken (`taskSlotBusy` in `App.jsx`), the installer, Factory Reset, View
  Logs and display-profile buttons are disabled; anything else that reaches
  the backend then gets `busy` and says so. A task refused with `busy` closes the terminal and says why.
- **Cancel is never lost.** The terminal and the APK install both cancel by
  task ID; an APK Cancel pressed before the ID arrives is sent when it does,
  and stops the rest of a multi-file drop.

## Adding a state

Add it to the table that owns it (`ENGINE_PHASES`, `TERMINAL_OUTCOMES`, …)
with its label and style, then a case in the pure function and its test
(`src/lib/enginePhase.test.js`, `src/test/state-machines.test.jsx`). Views
read the table; they should not need a new ternary.
