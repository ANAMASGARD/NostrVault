# Verification and platform foundations

Each milestone is complete only after actual web, Linux, and Android builds and launch checks pass. `npm run verify:commit` runs all current automated gates sequentially and records commands/statuses in ignored `test-results/gate-all.json`. A failed or missing gate exits nonzero; it never authorizes a commit. Add feature checks as their milestones land. Hardware, real signer, usability, and release checks require separately recorded evidence.

## Toolchains

Use Python 3 (standard-library lint audit/tests), Node 22.23.1, npm 11.8.0, Rust 1.98.0 with rustfmt/Clippy, wasm32-unknown-unknown, aarch64-linux-android, and x86_64-linux-android targets. Frontend and Rust dependencies are locked; the Gradle distribution has a pinned SHA-256. Use `npm ci`, and `--locked` for Cargo gates. Install `wasm-bindgen-cli` 0.2.129 with `cargo install wasm-bindgen-cli --version 0.2.129 --locked`. Install `tauri-driver` 2.1.0 with `cargo install tauri-driver --version 2.1.0 --locked`. Install browser binaries with `npx --no-install playwright install chromium firefox`.

Fedora native prerequisites used during setup:

```sh
sudo dnf install webkit2gtk4.1-devel openssl-devel librsvg2-devel libxdo-devel xorg-x11-server-Xvfb patchelf
```

WebKitWebDriver must also be on PATH. Linux tests use an isolated temporary app-data directory under Xvfb and remove it afterward; no real vault is opened. DEB/RPM packaging is separate from the application launch test. Build concurrency defaults to two Cargo jobs to limit host memory use.

Android requires Java 17, an Android SDK, NDK 27.1.12297006, platform 37.0 and build tools 37.0.0 matching the generated project, and an authorized emulator. Set JAVA_HOME, ANDROID_HOME, and NDK_HOME to local installations; do not commit machine-specific paths. Install an API 36 Google APIs x86_64 emulator image for foundation tests. On this Fedora host, emulator 37.2.12 crashed in SwiftShader; Mesa under Xvfb booted successfully:

```sh
adb start-server
LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a "$ANDROID_HOME/emulator/emulator" -avd nostrvault-foundation -no-window -no-audio -no-boot-anim -no-snapshot -gpu host -feature -Vulkan -memory 2048 -cores 2
```

Set `ANDROID_SERIAL=emulator-5554` only after that disposable emulator is booted. No physical-device or signer result is inferred from that emulator.

## Checks

- Formatting: Prettier for owned frontend, scripts, tests, configs and new docs; rustfmt for native source. Existing user-authored plan and AGENTS.md are excluded from blanket reformatting.
- TypeScript: strict compiler and typed ESLint, React hooks and refresh rules, warnings fail lint.
- Unit: bounded request/reply validation, cancellation and stale-response handling, capability/CSP contracts, shared Rust validation/crypto vectors, and SQLite transaction/reopen behavior. Tests group meaningful cases into tables rather than repeating implementation details.
- Web: build the real `vault-wasm` module and bindings before type checking on a clean checkout. Chromium and Firefox execute core validation/crypto, IndexedDB rollback and replacement-Worker reopening, cancellation/retry, and missing/corrupt/aborted module failures. A native Rust helper exchanges fresh age ciphertext with both browsers; independently generated Go age fixtures are also decrypted. No networking SDK or TypeScript cryptographic fallback.
- Core: explicit fmt/Clippy/tests and native/WASM dependency-tree checks; no Tauri, SQLite, JNI or browser bindings in `vault-core`.
- Linux: locked Clippy/tests, production DEB/RPM build, isolated Xvfb/D-Bus session, real compiled application launch/Rust IPC and encrypted fixture proof, rejected unauthorized IPC, blocked inline script under production CSP.
- Android: independent headless JNI library compilation for ARM64 and x86_64, native release then debug compilation, Kotlin formatting, Android lint/audit checks, separate headless and UI emulator instrumentation. The headless invocation asserts no Activity exists and exercises success, malformed and oversized requests. ARM64 execution remains unverified; an ARM64 build is not runtime evidence. Building release first avoids a redundant debug rebuild. The empty JVM unit task is omitted; add a unit-test gate when actual JVM tests land. The gate fails if the emulator is absent or instrumentation fails. It reuses the native library just built by the Tauri CLI for direct Gradle lint/test tasks; this avoids invoking the CLI-only Rust task without its IPC server.
- Final: `git diff --check` and review intended paths before an authorized local commit.

The main command reruns the complete gate before every milestone commit. Individual commands support diagnosis and are not substitutes for it. A workflow file is a CI definition until its remote execution is observed. Milestone 01 proves minimal Worker/WASM execution; milestone 02 adds shared portable engine/storage/crypto/JNI contracts; milestone 06 remains the first real signer → relay → saved-message → restart → offline-reader checkpoint.

PR CI runs two platform jobs: web/Linux and Android. Branch pushes do not duplicate those jobs; pushes to `main` and manual dispatch still run verification. Emulator startup failures print their captured log. Existing runtime and failure-path tests remain; no extra tests are added for workflow plumbing.

CI sets one temporary `ANDROID_AVD_HOME` for AVD creation and emulator lookup and checks the generated configuration before starting the emulator. This avoids mismatched defaults between SDK tools and the emulator.

## Reviewed Android lint exceptions

Run `python3 scripts/verify-android-lint.py` after the Tauri Android debug build, with the documented Java/SDK/NDK environment. It first checks pinned Wry version and generated-source SHA-256 values, then runs strict lint with an empty exception configuration. That audit must fail with exactly the nine approved issue/path/message tuples plus the single documented TrimLambda hint. A changed version pair, source hash, application-owned finding, extra finding or removed finding requires re-review.

Then the same strict lint task runs with the nine approved scoped exceptions and must succeed with zero errors/warnings and only the known hint. Both reports/logs remain available under ignored build/test output. Android lint's file and regex selectors are alternatives, so the independent unsuppressed audit enforces their conjunction instead of pretending a path-plus-regex XML entry provides it. No severity setting changes between these runs. The audit's expected failure is accepted only after exact structured-report comparison, never from its exit code alone.

`python3 -m unittest discover -s tests/tooling -p 'test_*.py'` covers changed source/version, application-owned findings, unexpected/stale findings and reviewed-report boundaries. The full gate runs these tests and both lint passes. See [lint decision](android-lint-blockers.md) for rationale, risks and re-review conditions.

## CI runtime target

PR CI runs two platform jobs. `verify:web-linux` runs common formatting, types, lint and unit guardrails once before the web and Linux gates; the Android job runs its platform checks directly. `verify:commit` retains the complete local milestone gate. Rust dependency/driver caches are separate by job, and Android uses the pinned Gradle cache action without publishing build scans or dependency graphs. Cache hits never replace compiling changed code or executing tests.

The target is under seven minutes on a warmed hosted runner. Cold toolchains and cache misses can exceed that; report measured durations before claiming the target achieved. Failed Linux runs preserve the exact built binary for three days and print live process/listening-port diagnostics before cleanup. These are diagnostic artifacts, not release publication.

Gradle task-output caching is enabled, and the Android checks reuse the Gradle daemon within the job. Lint report validation and the emulator instrumentation remain required. Hosted run `37229099465` verified the Linux desktop-session correction in 3m48s. The requested five-minute total is a performance target, not a guarantee for cold caches, hosted setup or Android builds.

## Milestone 03 vault checks

The complete local gate now contains **20 top-level commands**. It retains every
foundation gate and adds independent native-process vault reopening and KDF
measurement. Browser tests use actual Workers, Web Locks, IndexedDB, and fresh
browser processes with persistent disposable profiles. Native tests exercise
SQLite transactions, ownership, migrations, account isolation and sealed ingress;
Android tests cover headless JNI, Activity recreation and force-stopped process
relaunch with retained app data. The Android vault sequence installs the test
APKs once because Gradle's connected runner uninstalls them after an invocation.

Production crypto tests use the fixed 64 MiB Argon2 profile and shared Rust code,
including cross-runtime encrypted-record exchange. KDF measurements perform one
warm-up and five measured derivations. Browser reports are Playwright attachments;
native and emulator reports are `test-results/m03-native-kdf.json` and
`test-results/m03-android-kdf.json`. Workspace size is 65536 KiB, distinct from
process RSS. Run the browser measurement test with `--workers=1` for isolated
measurement without other test workers. Runtime versions and observed samples
are recorded in the milestone handoff, not assumed from dependency declarations.

Privacy checks inspect logical IndexedDB, accessible browser backing files,
native SQLite and companion artifacts, and the Android fixture database. Public
fixture passwords and body markers provide positive controls; these are bounded
marker scans, not proof of forensic erasure. Fault injection aborts real storage
transactions to exercise quota/disk-full recovery; it does not fill the host disk.
No M02 proof store is imported. Sealed ingress is internal and synthetic only;
age confidentiality does not authenticate a submitter or validate a Nostr event.

## M04 candidate verification

`npm run verify:commit` retains 20 top-level commands and includes identity Rust,
main-page broker, browser lifecycle and Android foreground-result parsing tests.
It never installs a signer or calls third-party infrastructure. Existing M01–M03
regressions, CSP/ACL checks and exact Android lint exceptions are retained.

`NOSTRVAULT_SIGNER_CACHE=/path/to/prepared-cache npm run verify:signers` verifies
all frozen artifact hashes before running a real local nak acceptance subset.
It performs no downloads. The command currently exits nonzero because the full
Firefox/Android/negative-case acceptance matrix remains incomplete. Its structured
report is `test-results/signers.json`; raw signer protocol diagnostics are not saved.
For focused development only, append `-- --linux-subset`. A passing subset does not
make M04 complete. The native fixture uses only an isolated local relay and a public
disposable fixture identity, never a user's identity or public relay.

The explicit Android `RealSignerTest` is outside ordinary deterministic instrumentation.
It needs the frozen Amber APK and a separately prepared disposable signer account,
plus instrumentation argument realSigners=true. It is currently failing and must
not be counted as a pass. Replacement Chromium research ended early under the
maintainer's speed constraint; browser private decryption remains unsupported.
