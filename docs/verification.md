# Verification and platform foundations

Milestone 01 is complete only after actual web, Linux, and Android builds and launch checks pass. `npm run verify:commit` runs all current automated gates sequentially and records commands/statuses in ignored `test-results/gate-all.json`. A failed or missing gate exits nonzero; it never authorizes a commit. Add feature checks as their milestones land. Hardware, real signer, usability, and release checks require separately recorded evidence.

## Toolchains

Use Python 3 (standard-library lint audit/tests), Node 22.23.1, npm 11.8.0, Rust 1.98.0 with rustfmt/Clippy, wasm32-unknown-unknown, aarch64-linux-android, and x86_64-linux-android targets. Frontend and Rust dependencies are locked; the Gradle distribution has a pinned SHA-256. Use `npm ci`, and `--locked` for Cargo gates. Install `tauri-driver` 2.1.0 with `cargo install tauri-driver --version 2.1.0 --locked`. Install browser binaries with `npx --no-install playwright install chromium firefox`.

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
- Unit: host-response validation and capability/CSP contract checks; Rust serialized runtime contract.
- Web: compile `tools/runtime-probe` to wasm32, bundle a distinct WASM asset and Worker, and execute the real Rust probe in Chromium and Firefox. Validate typed replies, keyboard activation, reload and narrow viewport. Missing, corrupt and aborted module responses must visibly fail and retry successfully. No native injection, external requests or TypeScript success fallback.
- Linux: locked Clippy/tests, production DEB/RPM build, real compiled application launch/Rust IPC, rejected unauthorized IPC, blocked inline script under production CSP.
- Android: native debug and release compilation, Kotlin formatting, Android lint/audit checks, JVM unit task (currently NO-SOURCE, not coverage), installed emulator instrumentation. The gate fails if the emulator is absent or instrumentation fails. It reuses the native library just built by the Tauri CLI for direct Gradle lint/test tasks; this avoids invoking the CLI-only Rust task without its IPC server.
- Final: `git diff --check` and review intended paths before an authorized local commit.

The main command reruns the complete gate before every milestone commit. Individual commands support diagnosis and are not substitutes for it. A workflow file is a CI definition until its remote execution is observed. Milestone 01 proves minimal Worker/WASM execution; milestone 02 adds shared portable engine/storage/crypto/JNI contracts; milestone 06 remains the first real signer → relay → saved-message → restart → offline-reader checkpoint.

PR CI runs two platform jobs: web/Linux and Android. Branch pushes do not duplicate those jobs; pushes to `main` and manual dispatch still run verification. Emulator startup failures print their captured log. Existing runtime and failure-path tests remain; no extra tests are added for workflow plumbing.

## Reviewed Android lint exceptions

Run `python3 scripts/verify-android-lint.py` after the Tauri Android debug build, with the documented Java/SDK/NDK environment. It first checks pinned Wry version and generated-source SHA-256 values, then runs strict lint with an empty exception configuration. That audit must fail with exactly the nine approved issue/path/message tuples plus the single documented TrimLambda hint. A changed version pair, source hash, application-owned finding, extra finding or removed finding requires re-review.

Then the same strict lint task runs with the nine approved scoped exceptions and must succeed with zero errors/warnings and only the known hint. Both reports/logs remain available under ignored build/test output. Android lint's file and regex selectors are alternatives, so the independent unsuppressed audit enforces their conjunction instead of pretending a path-plus-regex XML entry provides it. No severity setting changes between these runs. The audit's expected failure is accepted only after exact structured-report comparison, never from its exit code alone.

`python3 -m unittest discover -s tests/tooling -p 'test_*.py'` covers changed source/version, application-owned findings, unexpected/stale findings and reviewed-report boundaries. The full gate runs these tests and both lint passes. See [lint decision](android-lint-blockers.md) for rationale, risks and re-review conditions.
