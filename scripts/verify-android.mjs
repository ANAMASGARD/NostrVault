import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";

for (const variable of ["JAVA_HOME", "ANDROID_HOME", "NDK_HOME"]) {
  if (!process.env[variable])
    throw new Error(
      `Set ${variable} to the installed Android prerequisite. See docs/verification.md.`,
    );
}
if (!existsSync("src-tauri/gen/android/gradlew"))
  throw new Error(
    "Initialize the Android project with npm run tauri -- android init --ci.",
  );
function run(command, args, cwd = process.cwd()) {
  console.log(`Android gate: ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, {
    cwd,
    env: process.env,
    stdio: "inherit",
  });
  if (result.status !== 0) process.exit(result.status || 1);
}
const gradle = [
  "--daemon",
  "--console=plain",
  "-PabiList=x86_64",
  "-ParchList=x86_64",
  "-PtargetList=x86_64",
];
// Package release first, then leave the debug native assets ready for the emulator.
run("npm", [
  "run",
  "tauri",
  "--",
  "android",
  "build",
  "--ci",
  "--target",
  "aarch64",
  "--apk",
  "--",
  "--locked",
]);
run("npm", [
  "run",
  "tauri",
  "--",
  "android",
  "build",
  "--ci",
  "--debug",
  "--target",
  "x86_64",
  "--apk",
  "--",
  "--locked",
]);
// Tauri's CLI has just compiled the native library. Direct Gradle checks reuse
// that artifact because Tauri's Rust build task needs the CLI's live IPC server.
run(
  "./gradlew",
  [...gradle, "ktfmtCheck", "-x", ":app:rustBuildUniversalDebug"],
  "src-tauri/gen/android",
);
run("python3", ["scripts/verify-android-lint.py"]);
const serial = process.env.ANDROID_SERIAL;
if (!serial || !/^emulator-\d+$/.test(serial))
  throw new Error(
    "Set ANDROID_SERIAL to a booted disposable emulator (for example emulator-5554). This gate does not install onto physical devices.",
  );
const boot = spawnSync(
  join(process.env.ANDROID_HOME, "platform-tools", "adb"),
  ["-s", serial, "shell", "getprop", "sys.boot_completed"],
  { encoding: "utf8" },
);
if (boot.error) throw boot.error;
if (boot.status !== 0 || boot.stdout.trim() !== "1")
  throw new Error(
    `Emulator ${serial} is not booted (adb exit ${boot.status}, response ${JSON.stringify(boot.stdout.trim())}, diagnostic ${JSON.stringify(boot.stderr.trim())}). Build checks do not replace instrumentation.`,
  );
run(
  "./gradlew",
  [
    ...gradle,
    ":app:connectedUniversalDebugAndroidTest",
    "-x",
    ":app:rustBuildUniversalDebug",
  ],
  "src-tauri/gen/android",
);
