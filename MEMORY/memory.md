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

Next: stop for maintainer authorization to create `build: establish multiplatform verification and product contracts`. Do not commit, push or start 02 under the current instruction.
