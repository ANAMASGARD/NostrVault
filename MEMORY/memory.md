# Implementation status

## 2026-10-04 — milestone-01-foundations — Milestone 01 gates passed

Status: implemented and verified locally; exactly 85 reviewed milestone paths staged for maintainer commit authorization. No milestone commit, push, merge, release publication or public deployment. HEAD remains the initial commit `4fdfb4eef5fb63e7977167b8d59246a2761c9e2b`. Milestone 02 is unstarted.

Implemented:

- Preserved the existing plan and AGENTS instructions; recorded approved scope, archive-derived transcript trust, private recipient-inbox/AUTH recovery policy, onboarding and platform contracts.
- Replaced starter greeting with a development screen and real Rust runtime IPC on Linux/Android. Capabilities permit only runtime_info; unused opener removed; production CSP enforced.
- Added dependency locks, pinned toolchains, strict TypeScript/ESLint/format checks, unit/browser/native tests, build gates and pinned-action CI. Web/native asset outputs are separate.
- Added a dependency-free Rust probe in `tools/runtime-probe`, compiled WASM asset, real Worker invocation and validated request/reply bridge to the UI. Missing, corrupt and unloadable WASM fails explicitly; retries use a fresh Worker. No TypeScript success fallback or shared protocol/crypto/storage engine.
- Initialized Android, restricted platform backup behavior, added Kotlin formatting, lint and real instrumentation. Nine maintainer-approved lint exceptions preserve strict thresholds and are tied to issue/path/message, exact version pairs, Wry 0.57.0 and generated-file hashes. No Wry patch or dependency upgrades to silence notices.

Decisions and evidence:

- The maintainer explicitly assigned minimal Worker/WASM smoke coverage to 01; shared protocol, crypto, storage, archive interoperability and headless JNI stay in 02. Milestone 06 remains real signer → approved relay → captured/decrypted message → restart → offline reading without network/signer.
- Android lint cannot combine path and regex selectors with AND. The gate first runs unsuppressed strict lint and requires exactly the nine approved diagnostics plus one documented TrimLambda hint, then runs scoped lint and requires zero errors/warnings plus that hint. Unexpected or stale findings fail; an expected failure exit alone is never accepted.
- Removed only the two hash-verified generated Wry files and ran `cargo clean --manifest-path src-tauri/Cargo.toml --target x86_64-linux-android -p wry`. The locked x86_64 Android debug build regenerated identical hashes. No generated-source edit or shared Cargo registry patch.
- Initial Android lint failed with 38 errors, reduced to nine after app-owned fixes. The subsequent reviewed exceptions resolved that gate. Earlier browser tests proved UI only; current tests prove actual Rust/WASM execution.

Final aggregate verification: with documented JAVA_HOME, ANDROID_HOME, NDK_HOME, `ANDROID_SERIAL=emulator-5554` and `CARGO_BUILD_JOBS=2`, `npm run verify:commit` exited 0. All 17 steps passed. Machine-readable command/timing results are in ignored `test-results/gate-all.json`; full log is `test-results/milestone-01-final.log`.

| Command/check                                                                                       | Final result |
| --------------------------------------------------------------------------------------------------- | ------------ |
| `npm run format:check`                                                                              | Passed       |
| `npm run typecheck`                                                                                 | Passed       |
| `npm run lint`                                                                                      | Passed       |
| `npm run test:unit`                                                                                 | Passed       |
| `python3 -m unittest discover -s tests/tooling -p test_*.py`                                        | Passed       |
| `cargo fmt --manifest-path tools/runtime-probe/Cargo.toml -- --check`                               | Passed       |
| `cargo clippy --locked --manifest-path tools/runtime-probe/Cargo.toml --all-targets -- -D warnings` | Passed       |
| `cargo test --locked --manifest-path tools/runtime-probe/Cargo.toml`                                | Passed       |
| `npm run build:web`                                                                                 | Passed       |
| `npm run test:web`                                                                                  | Passed       |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`                                         | Passed       |
| `cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`           | Passed       |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml`                                          | Passed       |
| `npm run tauri -- build --bundles deb,rpm -- --locked`                                              | Passed       |
| `npm run test:linux`                                                                                | Passed       |
| `node scripts/verify-android.mjs`                                                                   | Passed       |
| `git diff --check`                                                                                  | Passed       |

Coverage: 22 TypeScript tests; seven Python lint-policy tests; ten Playwright tests across Chromium and Firefox; one Rust probe test plus one native Rust contract test; one Android instrumentation test with zero failures/errors/skips. Android JVM unit task reported NO-SOURCE and is not counted as coverage.

The Android gate built x86_64 debug, checked Kotlin formatting, ran the JVM task, passed the exact unsuppressed audit and scoped lint, built unsigned ARM64 release, rebuilt x86_64 debug and passed installed-emulator instrumentation. The emulator test calls real Rust, verifies ACL denial and recreates the Activity. Linux release DEB/RPM packaging and real WebKit IPC/ACL/CSP checks passed. All build artifacts remain ignored.

Independent verification also passed: `npm run verify:web`, `python3 scripts/verify-android-lint.py`, and `go run github.com/rhysd/actionlint/cmd/actionlint@v1.7.7 .github/workflows/verify.yml`. Formatting/whitespace checks are repeated after evidence-only documentation updates.

Actual environment: Fedora 44 x86_64; Node 22.23.1/npm 11.8.0; Rust 1.98.0; Chromium 153.0.8010.12/Firefox 155.0 via Playwright 1.63.0; WebKitGTK 2.54.0/Tauri 2.12.1; Java 17.0.20.1, SDK 37.0, NDK 27.1.12297006, Gradle 9.6.1; emulator 37.2.12/API36 Android16/WebView 133.0.6943.137. SwiftShader crashed before app installation; Mesa/Xvfb worked. Gradle's initial timed-out download was recovered using the exact official checksum-verified ZIP; TLS/checksums were never bypassed.

Review: 85 candidate paths examined; index initially empty. No local config filenames, large build outputs or private-key/credential pattern matches found in the intended changes (limited scan, not exhaustive security audit). The only new binary inputs are launcher PNGs and the Gradle wrapper JAR. Generated Kotlin, compiled WASM, native libraries, APKs, local.properties, caches and test reports are ignored. Native capabilities and CSP remain narrow. Existing user-authored AGENTS and plan content is preserved and included as intended milestone documentation. No unrelated changes were removed.

Not verified: physical devices, real signers, hosted CI/second Linux distribution, all backup/vault/chat/archive/recovery functionality from milestones 02–15. No account, production credential or public relay was used. These are foundation results, not product support or a security certification.

Cleanup: stopped the task-owned emulator and Gradle 9.6.1 daemon; adb confirms no connected devices. Unrelated processes were preserved.

At the original checkpoint, wait for maintainer authorization to create `build: establish multiplatform verification and product contracts`; that wait ended when the user explicitly authorized staging, signed commits, push, and PR creation below. Do not merge or start 02.

## 2026-10-04 — hosted CI remediation (Milestone 01 follow-up)

The authorized milestone commit `2205369a29df44e038731459eed4c1858b2c2954` is signed off, pushed on `milestone-01-foundations`, and included in open PR #1. The user authorized additional signed commits, push, and PR work. Do not merge or start Milestone 02.

Implemented follow-up fixes:

- `.github/workflows/verify.yml` uses explicit `$ANDROID_HOME/cmdline-tools/latest/bin` paths for `sdkmanager` and `avdmanager`, and `$ANDROID_HOME/platform-tools/adb` for device operations. This fixes the two initial hosted Android runs, which failed before compilation because `sdkmanager` was not on PATH.
- `scripts/test-linux.mjs` retries removal of the isolated Tauri profile on transient `ENOTEMPTY` during WebKit/Tauri teardown.
- `docs/compatibility.md` and this status record distinguish the initial hosted failure, local emulator availability failure, and final successful local run.

Verification on the exact follow-up candidate: `npm run verify:linux` passed; actionlint and `git diff --check` passed. The first full rerun stopped at Android instrumentation because `emulator-5554` had exited; no pass was claimed. Restarted the documented Mesa/Xvfb emulator and reran `npm run verify:commit`: all 17 steps passed, including Android lint audit/scoped lint, unsigned ARM64 release build, installed emulator instrumentation (1 test, zero failures/errors/skips), and Linux package/runtime checks. `test-results/gate-all.json` records `complete: true`. The two previous hosted run IDs were `37222489213` and `37222498966`; new hosted results for the correction are pending push.

Next: stage only `.github/workflows/verify.yml`, `scripts/test-linux.mjs`, `docs/compatibility.md`, and `MEMORY/memory.md`; review the exact staged patch; create a DCO-signed remediation commit; push; monitor PR #1 checks and update its description with observed hosted results. Do not merge or advance milestone.

## 2026-10-04 — Android hosted startup investigation

Follow-up commit `7619aa6` was signed off and pushed. Hosted SDK installation now succeeds. Android job `111499136287` then failed with exit 124 waiting 180 seconds for `emulator-5554`; shutdown reported connection refused. The Android gate was skipped, so this is setup failure rather than a failed app test. Startup output was redirected to an unreported file, preventing identification of the emulator's exit reason from the existing logs.

User requested minimal, maintainable tests. The four PR entries are two platform jobs duplicated by push and PR triggers. Removed feature-branch push triggering (retained PR/main/manual runs), and added printing of the emulator startup log on failure. No new tests or weakened assertions. Next: run local verification, push this diagnostic correction, inspect the hosted emulator exit evidence, then fix the actual cause and verify remotely.

Diagnostic candidate verification: `npm run verify:commit` passed all 17 steps, including one API 36 emulator instrumentation test. Actionlint passed. The hosted emulator exit reason remains unknown until its startup log is observed.

Hosted diagnostic run `37224527470` revealed Android emulator 37.2.12 exiting with `Unknown AVD name [nostrvault-ci]`: AVD creation and emulator lookup used different default directories. Startup now exports one `ANDROID_AVD_HOME` under `RUNNER_TEMP`, creates it, and checks the generated `.ini` before launching. Local manual probe confirmed the creation tool writes that file and `emulator -list-avds` resolves `nostrvault-ci`; the disposable probe directory was removed.

The previous hosted Linux job also failed: Rust checks and packages passed, but WebKit WebDriver session creation timed out after 30 seconds. The same smoke test passed in an isolated Ubuntu 24.04 container using WebKit 2.52.6, with baseline rendering and with software compositing. It also passed locally with an unavailable D-Bus address. These probes do not establish the hosted failure's exact cause. Added scoped WebKit startup and kernel diagnostics to the existing CI job; no new tests, relaxed assertions, sandbox bypass, or timeout increase.

Shared-AVD-directory candidate: actionlint and the complete `npm run verify:commit` passed (17 steps, one emulator instrumentation test). AVD lookup correction remains to be observed in hosted CI; Linux diagnostics remain investigative.

Hosted run `37225242471` successfully booted the Android emulator with the explicit shared AVD directory and entered `verify:android`, confirming the startup correction. Simplified the Android gate to build ARM64 release first and x86_64 debug once before lint/instrumentation. Removed the empty JVM task (`NO-SOURCE`); no actual test case or assertion was removed. The simplified candidate passed the complete local `npm run verify:commit` (17 steps). Hosted Linux still timed out creating its WebDriver session; the kernel diagnostics show Firefox namespace messages, but its browser tests passed, and there is no recorded WebKit denial. The Ubuntu 24.04/WebKit 2.52.6 container probe passed. Testing a CI-only WebKit software-compositing setting for Xvfb; this is an environment hypothesis, not a confirmed hosted fix. Removed unsupported WebKit debug channels and replaced broad kernel output with focused process diagnostics. No sandbox, CSP, IPC assertion, or test timeout is relaxed.

Hosted Android run `37225242471` subsequently completed debug/release compilation but stopped at the script's boot probe. The SDK-path adb successfully stopped that same emulator afterward, while the probe invoked bare `adb` and discarded spawn/stderr details. Aligned the probe with `$ANDROID_HOME/platform-tools/adb` and now preserve launch errors and unsuccessful probe diagnostics. A PATH-isolation local probe reproduces bare-adb ENOENT while SDK-path adb returns boot-completed `1`; the hosted failure's exact bare-adb error was not recorded, so the next run must confirm this correction.

Final candidate verification: `WEBKIT_DISABLE_COMPOSITING_MODE=1 npm run verify:commit` with documented Java/SDK/NDK/emulator prerequisites passed all 17 steps, including both Android builds, lint and one instrumentation test. Actionlint, Prettier and `git diff --check` passed. Hosted confirmation of the SDK-path probe and Linux headless setting remains pending.

## 2026-10-05 — Linux startup boundary and CI runtime work

Hosted run `37226490311`: Android passed its complete platform gate; web/Linux failed before WebDriver session creation. The compositing hypothesis failed and is removed. The shell's diagnostic `rg` was unavailable and masked the original timeout exit with 127; diagnostic collection now happens in the test before teardown using ordinary process/port tools, preserving the original exception.

Implemented candidate: one combined web/Linux gate runs common frontend guardrails once; CI Android runs its platform gate without repeating the same common frontend work. Full `verify:commit` remains unchanged in scope. Added full-SHA-pinned Rust and Gradle caching, with separate Rust job keys and read-only Gradle cache for forks. Failed Linux CI retains its exact release binary for three days for controlled reproduction. No runtime/security assertion, lint requirement or test case was removed. Research sources and evidence limitations are in `docs/research-ci-linux.md`.

Target: measured warm CI under seven minutes. Cold runs and misses remain potentially slower; no timing or hosted Linux success is claimed yet. Local final candidate `npm run verify:commit` passed all 17 checks; actionlint and Prettier passed. Ubuntu-native production build reproduction remains in progress. A failure-only, one-variable `NO_AT_BRIDGE=1` experiment reruns the existing Linux smoke; even if it passes, the original failed gate remains failed. Its local probe passed. This is diagnostic evidence collection, not a claimed accessibility fix or additional test case.

Hosted diagnostic run `37227965680`: both baseline and NO_AT_BRIDGE variants failed before session creation. The app was alive but had no inspector TCP listener and no WebKit child processes. Rust cache saved successfully (~998 MB). Downloaded the exact hosted release binary into a disposable directory; it passed on Ubuntu 24.04/WebKit 2.52.6, as did the independently compiled and DEB/RPM-packaged Ubuntu production binary. A fresh local session bus reproduced the exact AT-SPI warning while the smoke passed, so that warning is not sufficient evidence of the cause.

Next candidate launches the Linux smoke with `xvfb-run -a dbus-run-session -- node ...`, giving its desktop services a new session bus and the correct Xvfb DISPLAY instead of inheriting the runner session. Removed the unsuccessful bridge diagnostic. The exact hosted binary passes with this launch too. Full final local `npm run verify:commit` passed all 17 checks, including the isolated Linux launch, production packaging, Android lint and instrumentation. Actionlint and formatting passed. Hosted confirmation is pending; the precise internal service stall is not yet established.
