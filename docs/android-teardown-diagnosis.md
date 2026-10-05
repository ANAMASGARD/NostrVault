# Android teardown diagnosis — 2026-10-05

Status: the native failure is causally localized, but there is no accepted
production correction. M04 and PR #4 remain incomplete. PR #5 is untouched.

## Findings

The same API-36 environment reproduced the destroyed-mutex fatal on:

| Candidate                                      | Operation                                                    | Result                                                                                |
| ---------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| M03 `ca309ad8de76feaaedd7d6a35c28e3969c0060f3` | Existing vault create/lock/recreate/close                    | Original runner reported success despite FORTIFY; shared strict collector rejected it |
| M04 `f62af935f46914c56dbe08eb767ea750cf3d5f4d` | Existing vault create/lock/recreate/close                    | Strict collector rejected FORTIFY after test completion                               |
| M04                                            | Launch/close without creating a vault or requesting a signer | Same fatal                                                                            |
| Isolated M03 with bare Tauri builder           | Launch/close, no registered custom command handlers          | Same fatal, including an instrumentation process-crash result                         |
| Bare builder using `App::run_return`           | One launch/close                                             | Passed, no attributable fatal                                                         |
| The same `run_return` prototype                | Close and relaunch in the same process                       | Failed: `Native Activity has no WebView`                                              |

This is not evidence that signer logic caused the failure. M03's original
runner checked `OK (1 test)` but did not inspect the post-test crash buffer.

Matching native images and the NDK LLDB debugger identified the fatal threads as
`hwuiTask0` and `hwuiTask1`. The failing stack is:

```text
libc: HandleUsingDestroyedMutex(pthread_mutex_t*, char const*)
libc: pthread_mutex_lock
libc: pthread_cond_wait
Android libc++ condition-variable implementation (unsymbolicated frame)
libhwui: android::uirenderer::CommonPool::CommonPool() worker thread
libc: __pthread_start / __start_thread
```

The mutex address `0x7ecd94a02c38` is inside the mapped `.bss` region of
`/system/lib64/libhwui.so`, not the vault or signer runtime. A separate exit
breakpoint captured:

```text
nostrvault_lib::run
tauri::Builder::run -> App::run -> tauri-runtime-wry Runtime::run
tao Android EventLoop::run, android/mod.rs:205
std::process::exit(0) -> libc exit
```

The final Activity destruction triggers native event-loop termination and
process-wide exit while Android's CommonPool workers still use their global
mutex. The bare-builder reproduction and isolated `run_return` experiment
support process teardown as the causal boundary. The exact static destructor
instruction was not captured; do not claim it was symbolicated.

## Accepted production correction — 2026-10-05

NostrVault now keeps the Tauri/Tao event loop alive on Android instead of exiting
the process when the last Activity closes:

- `RunEvent::ExitRequested`: call `api.prevent_exit()` (including the
  `code: None` path emitted after Activity destruction).
- Track whether the main WebView is attached; clear the flag and drop stale window
  handles on Activity teardown.
- On `RunEvent::Resumed` and mobile `WindowEvent::Resumed` / `Focused(true)`,
  recreate the configured `main` window when detached.
- On main-window destruction, call `Runtime::secure_lock()` so an unlocked vault
  session is not retained in a process that stays resident.

Implementation: [`src-tauri/src/android_lifecycle.rs`](src-tauri/src/android_lifecycle.rs)
and [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs). An optional
`android-lifecycle-minimal` feature builds a bare reproducer for
`scripts/verify-android-lifecycle-minimal.mjs`.

Verified locally on API-36 x86_64 emulator with the strict collector (no
attributable FORTIFY/crash-buffer fatals):

| Check                                                                                   | Result                           |
| --------------------------------------------------------------------------------------- | -------------------------------- |
| `LifecycleDiagnosisTest`                                                                | Pass                             |
| `LifecycleFrameworkTest#closeRecreate`                                                  | Pass                             |
| `LifecycleFrameworkTest#freshProcessReopen`                                             | Pass                             |
| `VaultCreateTest`                                                                       | Pass                             |
| `VaultReopenTest`                                                                       | Pass                             |
| `VaultCreateTest` × 10 + `VaultReopenTest` × 10 (`verify-android-lifecycle-repeat.mjs`) | Pass (2026-10-05, API-36 x86_64) |

Same-process **second** `ActivityScenario.launch` after a full Activity finish
still does not reliably rebind a WebView (Wry activity-id behavior; see
[tauri#15671](https://github.com/tauri-apps/tauri/issues/15671)). Configuration
`recreate()` and fresh instrumentation runs are covered; a dedicated
same-process relaunch instrumentation case remains future work. Do not use
`App::run_return` as a substitute.

Commands actually run:

```sh
npm run tauri -- android build --ci --debug --target x86_64 --apk -- --locked
node scripts/verify-android-lifecycle-repeat.mjs   # NOSTRVAULT_ANDROID_LIFECYCLE_REPEAT=10
```

M04 completion, hosted CI green, merge, and release are still not claimed.

## Why no quick production patch was accepted

The installed Tauri API documents that `App::run` exits the process and
`App::run_return` returns instead. Returning avoids the fatal in the minimal
close case, but fails same-process Activity relaunch. Wry's process lifecycle
observer initializes the native event loop only once. Therefore simply replacing
`run` with `run_return` would exchange a crash for a broken application lifecycle.

Both experiments were confined to an isolated checkout. NostrVault's production
runtime, dependencies, generated Wry code, lint exceptions, CSP, vault ownership,
and cryptography were not changed. No maintained fixed dependency version has
been verified in this task; an upgrade is not proposed as if it were established.

The production correction above satisfies close, configuration recreation, and
fresh-process reopen under the strict collector, including ten create and ten
reopen repetitions. Same-process Activity relaunch after a full finish remains
the outstanding lifecycle gap relative to [tauri#15671](https://github.com/tauri-apps/tauri/issues/15671).
Do not suppress native fatals, disable hardware rendering, use a test-only exit
bypass, or add delays.

## Collector and evidence

Both Android verification scripts use `scripts/android-instrumentation.mjs`.
Each class retains instrumentation output, continuous bounded threadtime logcat,
crash buffer, Activity/process snapshots, accessible DropBox/tombstone evidence,
and artifact/runtime versions. Collection occurs after failures/timeouts; a
diagnostic write failure does not replace the primary instrumentation error.
Unrelated-process fatals are excluded using test/process PID evidence. Failed
required collection fails the gate. Cleanup attempts both fixture uninstalls and
preserves the primary error.

Logs are sanitized before persistence. Known fixture passwords, configured
redaction values, privacy/credential markers, secret encodings and ordinary
provider/application console output are removed. This collector is for disposable
emulator fixtures only. It is not a general collector for user profiles.

Ignored local evidence lives under `test-results/android/`:

- `baseline-m03/VaultCreateTest/`: strict M03 comparison.
- `VaultCreateTest/` and `LifecycleDiagnosisTest/`: M04 failures.
- `LifecycleNativeDebugger/`: matching symbolicated exit/fatal stacks.
- `framework-minimal/`: bare-builder failure and reproduction source.
- `framework-return/`: rejected `run_return` experiment and relaunch failure.

No DropBox tombstone was available before instrumentation stopped the process.
The native debugger therefore captured the failure before abort. Debugger
attachment used root on the disposable emulator; ordinary regression checks do
not require root. A debugger assertion occurred during one conditional-breakpoint
experiment; its exit stack was subsequently cross-checked against the matching
library and a separate successful fatal breakpoint. It is not counted as a test.

Environment: Fedora 44 x86_64; Java 17.0.20.1; SDK platform/build tools 37.0;
Android emulator 37.2.12/API 36, Android 16 fingerprint
`google/sdk_gphone64_x86_64/emu64xa:16/BE2A.250530.026.F3/13894323:userdebug/dev-keys`;
WebView 133.0.6943.137; NDK 27.1.12297006; Gradle 9.6.1;
Tauri 2.12.1, Wry 0.57.0, Tao 0.37.1. Both candidate builds used the same
environment, AVD and signed test APK configuration. SHA-256 values and ELF notes
are recorded in each `versions.txt`; the app library has no emitted GNU build ID,
so exact file hashes were used for matching.

## Reproduction

Use documented Java/SDK/NDK environment variables and a disposable API-36 emulator.
Build the x86_64 debug and test APKs, then execute:

```sh
node scripts/verify-android-vault.mjs
```

With the lifecycle correction above, `node scripts/verify-android-vault.mjs`
should pass `VaultCreateTest` and `VaultReopenTest` when the strict collector
reports no attributable fatals. Re-run after rebuilding the x86_64 debug APK.
For the minimal reproducer, build the same locked environment in an isolated
checkout with only this Rust application entry point:

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("minimal Tauri reproduction failed");
}
```

Install its app/test APKs and run `LifecycleDiagnosisTest` through the shared
collector. Its normal mode launches, waits for the bundled document title, and
closes the Activity. Optional `-e nativeDebugger true` waits for creation of the
fixture's `files/native-close` signal before close, allowing a native debugger
to attach. This condition-based diagnostic rendezvous times out after 60 seconds;
it is not a production sleep or a passing-test workaround.

## Verification boundary

Diagnostic unit tests cover app-fatal attribution after successful JUnit output,
unrelated-process rejection, process-start attribution, secret-redaction positive
controls, and preservation of primary failures/timeouts with finally evidence.
The existing tooling gate runs these tests.

Focused formatting, TypeScript, ESLint, Rust fmt/clippy/tests, and Android strict
lint checks are recorded in repository memory. No full `verify:commit`, complete
signer acceptance, hosted correction, twenty-repeat stability result, M04
completion, commit, push or PR change is claimed.
