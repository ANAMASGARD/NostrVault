import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
const serial = process.env.ANDROID_SERIAL;
if (!serial || !/^emulator-\d+$/.test(serial) || !process.env.ANDROID_HOME)
  throw new Error("A disposable Android emulator is required");
const adb = join(process.env.ANDROID_HOME, "platform-tools", "adb");
function call(args) {
  const result = spawnSync(adb, ["-s", serial, ...args], {
    encoding: "utf8",
    timeout: 180000,
    maxBuffer: 4 * 1024 * 1024,
  });
  if (result.status !== 0)
    throw new Error(
      `adb ${args[0]} failed: ${result.stderr || result.stdout || result.error?.message}`,
    );
  return result.stdout;
}
// Gradle's connected runner uninstalls its APKs. Install once here so the M03
// create -> actual process exit -> reopen sequence retains the same app data.
call([
  "install",
  "-r",
  "src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk",
]);
call([
  "install",
  "-r",
  "src-tauri/gen/android/app/build/outputs/apk/androidTest/universal/debug/app-universal-debug-androidTest.apk",
]);
try {
  call(["shell", "pm", "clear", "com.nostrvault.app"]);
  mkdirSync("test-results", { recursive: true });
  for (const name of [
    "HeadlessVaultTest",
    "VaultCreateTest",
    "VaultReopenTest",
  ]) {
    call(["shell", "am", "force-stop", "com.nostrvault.app"]);
    const output = call([
      "shell",
      "am",
      "instrument",
      "-w",
      "-r",
      "-e",
      "class",
      `com.nostrvault.app.${name}`,
      "com.nostrvault.app.test/androidx.test.runner.AndroidJUnitRunner",
    ]);
    writeFileSync(`test-results/m03-android-${name}.txt`, output);
    if (
      !/OK \(1 test\)/.test(output) ||
      /FAILURES|INSTRUMENTATION_FAILED/.test(output)
    )
      throw new Error(`Android ${name} failed: ${output}`);
    console.log(`PASS Android ${name}`);
    if (name === "HeadlessVaultTest") {
      const benchmark = JSON.parse(
        call([
          "exec-out",
          "run-as",
          "com.nostrvault.app",
          "cat",
          "files/m03-kdf.json",
        ]),
      );
      if (
        benchmark.milliseconds.length !== 5 ||
        benchmark.milliseconds.some((n) => !Number.isFinite(n) || n <= 0)
      )
        throw new Error("Android KDF probe failed");
      writeFileSync(
        "test-results/m03-android-kdf.json",
        JSON.stringify(benchmark, null, 2),
      );
      console.log(JSON.stringify(benchmark));
    }
  }
  const database = spawnSync(
    adb,
    [
      "-s",
      serial,
      "exec-out",
      "run-as",
      "com.nostrvault.app",
      "cat",
      "vault/vault.sqlite",
    ],
    { maxBuffer: 8 * 1024 * 1024 },
  );
  if (database.status !== 0 || !database.stdout.length)
    throw new Error("Android vault inspection unavailable");
  const markers = [
    "android fixture password",
    "changed android password",
    "lossAcknowledged",
  ];
  const containsMarker = (bytes) =>
    markers.some((marker) => bytes.includes(Buffer.from(marker)));
  const positive = Buffer.from(
    call([
      "exec-out",
      "run-as",
      "com.nostrvault.app",
      "cat",
      "files/m03-plaintext-control",
    ]),
  );
  if (!containsMarker(positive))
    throw new Error("Android file positive control failed");
  if (containsMarker(database.stdout))
    throw new Error("Plaintext fixture marker in Android database");
  console.log(
    "PASS Android production SQLite marker scan; no physical ARM64 claim",
  );
} finally {
  call(["uninstall", "com.nostrvault.app.test"]);
  call(["uninstall", "com.nostrvault.app"]);
}
