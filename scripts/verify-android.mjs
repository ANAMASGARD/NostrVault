import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";

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
  "--no-daemon",
  "--console=plain",
  "-PabiList=x86_64",
  "-ParchList=x86_64",
  "-PtargetList=x86_64",
];
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
  [
    ...gradle,
    "ktfmtCheck",
    ":app:testUniversalDebugUnitTest",
    "-x",
    ":app:rustBuildUniversalDebug",
  ],
  "src-tauri/gen/android",
);
run("python3", ["scripts/verify-android-lint.py"]);
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
// Rebuild the emulator variant after release packaging switches native assets.
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
const serial = process.env.ANDROID_SERIAL;
if (!serial || !/^emulator-\d+$/.test(serial))
  throw new Error(
    "Set ANDROID_SERIAL to a booted disposable emulator (for example emulator-5554). This gate does not install onto physical devices.",
  );
const boot = spawnSync(
  "adb",
  ["-s", serial, "shell", "getprop", "sys.boot_completed"],
  { encoding: "utf8" },
);
if (boot.status !== 0 || boot.stdout.trim() !== "1")
  throw new Error(
    `Emulator ${serial} is not booted. Build checks do not replace instrumentation.`,
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
